// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use env_variables::EnvVariables;
use environment_locations::get_environments_for_folders;
use log::error;
use manager::PoetryManager;
use pet_core::{
    os_environment::Environment,
    python_environment::{PythonEnvironment, PythonEnvironmentBuilder, PythonEnvironmentCategory},
    reporter::Reporter,
    Configuration, Locator, LocatorResult,
};
use pet_python_utils::{env::PythonEnv, executable::find_executables, version};
use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

mod env_variables;
mod environment_locations;
mod manager;

pub struct Poetry {
    pub project_dirs: Arc<Mutex<Vec<PathBuf>>>,
    pub env_vars: EnvVariables,
    pub poetry_executable: Arc<Mutex<Option<PathBuf>>>,
    searched: AtomicBool,
    environments: Arc<Mutex<Vec<PythonEnvironment>>>,
    manager: Arc<Mutex<Option<PoetryManager>>>,
}

impl Poetry {
    pub fn from(environment: &dyn Environment) -> impl Locator {
        Poetry {
            searched: AtomicBool::new(false),
            project_dirs: Arc::new(Mutex::new(vec![])),
            env_vars: EnvVariables::from(environment),
            poetry_executable: Arc::new(Mutex::new(Some(PathBuf::from(
                "/Users/donjayamanne/.local/bin/poetry",
            )))),
            environments: Arc::new(Mutex::new(vec![])),
            manager: Arc::new(Mutex::new(None)),
        }
    }
    fn find_with_cache(&self) -> Option<LocatorResult> {
        if let Ok(environments) = self.environments.lock() {
            if !environments.is_empty() {
                if let Ok(manager) = self.manager.lock() {
                    if let Some(manager) = manager.as_ref() {
                        return Some(LocatorResult {
                            managers: vec![manager.to_manager()],
                            environments: environments.clone(),
                        });
                    }
                }
            }
            if self.searched.load(Ordering::Relaxed) {
                return None;
            }
        }
        // First find the manager
        let manager = manager::PoetryManager::find(self.poetry_executable.lock().unwrap().clone());
        if let Some(manager) = manager {
            let mut mgr = self.manager.lock().unwrap();
            mgr.replace(manager.clone());
            drop(mgr);

            let project_dirs = self.project_dirs.lock().unwrap().clone();
            let result = get_environments_for_folders(&manager.executable, project_dirs);
            match self.environments.lock() {
                Ok(mut environments) => {
                    environments.clear();
                    for (project_dir, envs) in result {
                        for env in envs {
                            if let Some(env) =
                                create_poetry_env(&env, project_dir.clone(), manager.clone())
                            {
                                environments.push(env);
                            }
                        }
                    }
                    self.searched.store(true, Ordering::Relaxed);
                    Some(LocatorResult {
                        managers: vec![manager.to_manager()],
                        environments: environments.clone(),
                    })
                }
                Err(err) => {
                    error!("Failed to cache to Poetry environments: {:?}", err);
                    None
                }
            }
        } else {
            self.searched.store(true, Ordering::Relaxed);
            None
        }
    }
}

impl Locator for Poetry {
    fn configure(&self, config: &Configuration) {
        error!("Configuring Poetry locator");
        if let Some(search_paths) = &config.search_paths {
            if !search_paths.is_empty() {
                self.project_dirs.lock().unwrap().clear();
                self.project_dirs
                    .lock()
                    .unwrap()
                    .extend(search_paths.clone());
            }
        }
        if let Some(exe) = &config.poetry_executable {
            self.poetry_executable.lock().unwrap().replace(exe.clone());
        }
    }

    fn supported_categories(&self) -> Vec<PythonEnvironmentCategory> {
        vec![PythonEnvironmentCategory::Poetry]
    }

    fn from(&self, env: &PythonEnv) -> Option<PythonEnvironment> {
        if let Some(result) = self.find_with_cache() {
            for found_env in result.environments {
                if let Some(symlinks) = &found_env.symlinks {
                    if symlinks.contains(&env.executable) {
                        return Some(found_env.clone());
                    }
                }
            }
        }
        None
    }

    fn find(&self, reporter: &dyn Reporter) {
        if let Some(result) = self.find_with_cache() {
            for found_env in result.environments {
                if let Some(manager) = &found_env.manager {
                    reporter.report_manager(manager);
                }
                reporter.report_environment(&found_env);
            }
        }
    }
}

fn create_poetry_env(
    prefix: &PathBuf,
    project_dir: PathBuf,
    manager: PoetryManager,
) -> Option<PythonEnvironment> {
    if !prefix.exists() {
        return None;
    }
    let executables = find_executables(prefix);
    if executables.is_empty() {
        return None;
    }
    let version = version::from_creator_for_virtual_env(prefix);
    Some(
        PythonEnvironmentBuilder::new(PythonEnvironmentCategory::Poetry)
            .executable(Some(executables[0].clone()))
            .prefix(Some(prefix.clone()))
            .version(version)
            .manager(Some(manager.to_manager()))
            .project(Some(project_dir.clone()))
            .symlinks(Some(executables))
            .build(),
    )
}
