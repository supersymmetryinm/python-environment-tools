// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use log::{info, trace, warn};
use pet_cache::Cache;
use pet_conda::CondaLocator;
use pet_core::os_environment::{Environment, EnvironmentApi};
use pet_core::python_environment::PythonEnvironmentCategory;
use pet_core::reporter::Reporter;
use pet_core::{Configuration, Locator};
use pet_env_var_path::get_search_paths_from_env_variables;
use pet_fs::times::get_mtime_ctime;
use pet_global_virtualenvs::list_global_virtual_envs_paths;
use pet_python_utils::env::PythonEnv;
use pet_python_utils::executable::{
    find_executable, find_executables, should_search_for_environments_in_path,
};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use std::{sync::Arc, thread};

use crate::locators::identify_python_environment_using_locators;

pub struct Summary {
    pub validation_time: Duration,
    pub search_time: Duration,
}

pub fn find_and_report_envs(
    reporter: &dyn Reporter,
    configuration: Configuration,
    locators: &Arc<Vec<Arc<dyn Locator>>>,
    conda_locator: Arc<dyn CondaLocator>,
    cache: Option<Arc<Cache>>,
) -> Summary {
    let start = std::time::Instant::now();
    let mut summary = Summary {
        validation_time: Duration::from_secs(0),
        search_time: Duration::from_secs(0),
    };
    // If we have a cache, then first validate the items in the cache.
    // Only after we have validated the cache, should we start looking for environments.
    if let Some(cache) = cache {
        report_validated_environments(reporter, cache, locators);
        summary.validation_time = start.elapsed();
    }

    // return;
    info!("Started Refreshing Environments");
    let start = std::time::Instant::now();

    // From settings
    let environment_paths = configuration.environment_paths.unwrap_or_default();
    let search_paths = configuration.search_paths.unwrap_or_default();
    let conda_executable = configuration.conda_executable;
    thread::scope(|s| {
        // 1. Find using known global locators.
        s.spawn(|| {
            // Find in all the finders
            thread::scope(|s| {
                for locator in locators.iter() {
                    let locator = locator.clone();
                    s.spawn(move || locator.find(reporter));
                }
            });

            // By now all conda envs have been found
            // Get the conda info in a separate thread.
            // & see if we can find more environments by spawning conda.
            // But we will not wait for this to complete.
            thread::spawn(move || {
                conda_locator.find_with_conda_executable(conda_executable);
                Some(())
            });
        });
        // Step 2.1: Search in some global locations for virtual envs.
        // Step 2.2: And also find in the current PATH variable
        s.spawn(|| {
            let environment = EnvironmentApi::new();
            let search_paths: Vec<PathBuf> = [
                get_search_paths_from_env_variables(&environment),
                list_global_virtual_envs_paths(
                    environment.get_env_var("WORKON_HOME".into()),
                    environment.get_user_home(),
                ),
                environment_paths,
            ]
            .concat();

            trace!(
                "Searching for environments in global folders: {:?}",
                search_paths
            );

            find_python_environments(search_paths, reporter, locators, false)
        });
        // Step 3: Find in workspace folders too.
        // This can be merged with step 2 as well, as we're only look for environments
        // in some folders.
        // However we want step 2 to happen faster, as that list of generally much smaller.
        // This list of folders generally map to workspace folders
        // & users can have a lot of workspace folders and can have a large number fo files/directories
        // that could the discovery.
        s.spawn(|| {
            if search_paths.is_empty() {
                return;
            }
            trace!(
                "Searching for environments in custom folders: {:?}",
                search_paths
            );
            find_python_environments_in_workspace_folders_recursive(
                search_paths,
                reporter,
                locators,
                0,
                1,
            );
        });
    });
    summary.search_time = start.elapsed();

    summary
}

fn report_validated_environments(
    reporter: &dyn Reporter,
    cache: Arc<Cache>,
    locators: &Arc<Vec<Arc<dyn Locator>>>,
) {
    // Index the locators
    let mut locator_map = HashMap::<PythonEnvironmentCategory, Arc<dyn Locator>>::new();
    for locator in locators.iter() {
        for category in locator.supported_categories().clone().into_iter() {
            locator_map.insert(category, locator.clone());
        }
    }
    let start = std::time::Instant::now();
    let duration = start.elapsed();
    println!("Time to get all environments: {:?}", duration);
    thread::scope(|s| {
        let environments = cache.get_all_environments();
        if environments.is_empty() {
            return;
        }
        info!("Validating cached environments");
        for env in environments {
            if let Some(executable) = &env.executable {
                let python_env =
                    PythonEnv::new(executable.clone(), env.prefix.clone(), env.version.clone());
                let category = env.category;
                let locators = locator_map.clone();
                let env = env.clone();
                s.spawn(move || {
                    let category = category;
                    // Find the locator associated with the environment.
                    // If we find the locator, then we can use it to validate the environment.
                    if let Some(locator) = locators.get(&category) {
                        // Check if the mtime/ctime for any of the exes have changed, if yes,
                        // Then no point validating this.

                        if let Some(times) = &env.times {
                            for (executable, cached_times) in times.iter() {
                                if let Some(new_times) = get_mtime_ctime(executable) {
                                    if cached_times.mtime != new_times.mtime
                                        || cached_times.ctime != new_times.ctime
                                    {
                                        trace!("Skipping validation for env {:?} due to change in mtime/time of its exe {:?}", executable, env.executable);
                                        return;
                                    }
                                }
                            }
                        }

                        // NOTE: Given the cache has been verified its safe to report the env as is.
                        // However its possible some code has changed in the new release,
                        // Or the manager has changed or the like,
                        // Hence safer to just get the environment info again.

                        if let Some(env) = locator.from(&python_env) {
                            // If the same locator can return the environment, then its valid.

                            // If this env has a manager, then report that
                            if let Some(manager) = env.manager.clone() {
                                reporter.report_manager(&manager);
                            }

                            reporter.report_environment(&env);

                            // TODO: populate from cache.
                        }
                    }
                });
            }
        }
    });
    let duration = start.elapsed();
    println!("Time to validte all environments: {:?}", duration);
}

fn find_python_environments_in_workspace_folders_recursive(
    paths: Vec<PathBuf>,
    reporter: &dyn Reporter,
    locators: &Arc<Vec<Arc<dyn Locator>>>,
    depth: u32,
    max_depth: u32,
) {
    thread::scope(|s| {
        // Find in cwd
        let paths1 = paths.clone();
        s.spawn(|| {
            find_python_environments(paths1, reporter, locators, true);

            if depth >= max_depth {
                return;
            }

            let bin = if cfg!(windows) { "Scripts" } else { "bin" };
            // If the folder has a bin or scripts, then ignore it, its most likely an env.
            // I.e. no point looking for python environments in a Python environment.
            let paths = paths
                .into_iter()
                .filter(|p| !p.join(bin).exists())
                .collect::<Vec<PathBuf>>();

            for path in paths {
                if let Ok(reader) = fs::read_dir(&path) {
                    let reader = reader
                        .filter_map(Result::ok)
                        .filter(|d| d.file_type().is_ok_and(|f| f.is_dir()))
                        .map(|p| p.path())
                        .filter(should_search_for_environments_in_path);

                    // Take a batch of 20 items at a time.
                    let reader = reader.fold(vec![], |f, a| {
                        let mut f = f;
                        if f.is_empty() {
                            f.push(vec![a]);
                            return f;
                        }
                        let last_item = f.last_mut().unwrap();
                        if last_item.is_empty() || last_item.len() < 20 {
                            last_item.push(a);
                            return f;
                        }
                        f.push(vec![a]);
                        f
                    });

                    for entry in reader {
                        find_python_environments_in_workspace_folders_recursive(
                            entry,
                            reporter,
                            locators,
                            depth + 1,
                            max_depth,
                        );
                    }
                }
            }
        });
    });
}

fn find_python_environments(
    paths: Vec<PathBuf>,
    reporter: &dyn Reporter,
    locators: &Arc<Vec<Arc<dyn Locator>>>,
    is_workspace_folder: bool,
) {
    if paths.is_empty() {
        return;
    }
    thread::scope(|s| {
        let chunks = if is_workspace_folder { paths.len() } else { 1 };
        for item in paths.chunks(chunks) {
            let lst = item.to_vec().clone();
            let locators = locators.clone();
            s.spawn(move || {
                find_python_environments_in_paths_with_locators(
                    lst,
                    &locators,
                    reporter,
                    is_workspace_folder,
                );
            });
        }
    });
}

fn find_python_environments_in_paths_with_locators(
    paths: Vec<PathBuf>,
    locators: &Arc<Vec<Arc<dyn Locator>>>,
    reporter: &dyn Reporter,
    is_workspace_folder: bool,
) {
    let executables = if is_workspace_folder {
        // If we're in a workspace folder, then we only need to look for bin/python or bin/python.exe
        // As workspace folders generally have either virtual env or conda env or the like.
        // They will not have environments that will ONLY have a file like `bin/python3`.
        // I.e. bin/python will almost always exist.
        paths
            .iter()
            // Paths like /Library/Frameworks/Python.framework/Versions/3.10/bin can end up in the current PATH variable.
            // Hence do not just look for files in a bin directory of the path.
            .flat_map(|p| find_executable(p))
            .filter_map(Option::Some)
            .collect::<Vec<PathBuf>>()
    } else {
        paths
            .iter()
            // Paths like /Library/Frameworks/Python.framework/Versions/3.10/bin can end up in the current PATH variable.
            // Hence do not just look for files in a bin directory of the path.
            .flat_map(find_executables)
            .filter(|p| {
                // Exclude python2 on macOS
                if std::env::consts::OS == "macos" {
                    return p.to_str().unwrap_or_default() != "/usr/bin/python2";
                }
                true
            })
            .collect::<Vec<PathBuf>>()
    };

    identify_python_executables_using_locators(executables, locators, reporter);
}

fn identify_python_executables_using_locators(
    executables: Vec<PathBuf>,
    locators: &Arc<Vec<Arc<dyn Locator>>>,
    reporter: &dyn Reporter,
) {
    for exe in executables.into_iter() {
        let executable = exe.clone();
        let env = PythonEnv::new(exe.to_owned(), None, None);
        if let Some(env) = identify_python_environment_using_locators(&env, locators) {
            reporter.report_environment(&env);
            continue;
        } else {
            warn!("Unknown Python Env {:?}", executable);
        }
    }
}
