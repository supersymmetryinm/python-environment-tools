// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use hash::compute_hash;
use log::warn;
use pet_core::python_environment::{get_environment_key, MTimeCTime, PythonEnvironment};
use pet_fs::times::get_mtime_ctime;
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

mod hash;

pub struct Cache {
    cache_is_dirty: AtomicBool,
    environments: Arc<Mutex<Vec<PythonEnvironment>>>,
    pub cache_dir: PathBuf,
}

impl Cache {
    pub fn new(cache_dir: PathBuf) -> Self {
        if let Err(e) = fs::create_dir_all(&cache_dir) {
            warn!("Failed to create cache directory: {:?}", e);
        }

        Self {
            cache_is_dirty: AtomicBool::new(true),
            environments: Arc::new(Mutex::new(Vec::new())),
            cache_dir,
        }
    }

    pub fn store(&self, environment: PythonEnvironment) {
        self.environments.lock().unwrap().push(environment.clone());
        if let Some(key) = get_environment_key(&environment) {
            let hash = compute_hash(key);
            let mut cache_file = self.cache_dir.join(hash);
            cache_file.set_extension("json");
            let mut environment = environment.clone();
            update_mtimes_ctimes(&mut environment);
            match serde_json::to_string_pretty(&environment) {
                Ok(json) => {
                    if let Err(e) = fs::write(cache_file, json) {
                        warn!("Failed to write environment to cache: {:?}", e);
                    } else {
                        self.cache_is_dirty.store(true, Ordering::Relaxed);
                    }
                }
                Err(e) => {
                    warn!("Failed to serialize environment for caching: {:?}", e);
                }
            }
        }
    }

    pub fn get_all_environments(&self) -> Vec<PythonEnvironment> {
        if !self.cache_is_dirty.load(Ordering::Relaxed) {
            return self.environments.lock().unwrap().clone();
        }
        let mut environments = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.cache_dir) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if let Some(extension) = path.extension() {
                    if extension.to_ascii_lowercase() == "json" {
                        if let Ok(contents) = fs::read_to_string(&path) {
                            if let Ok(environment) = serde_json::from_str(&contents) {
                                environments.push(environment);
                            } else {
                                warn!(
                                    "Failed to deserialize environment from cache file: {:?}",
                                    path
                                );
                            }
                        } else {
                            warn!("Failed to read cache file: {:?}", path);
                        }
                    }
                }
            }
        } else {
            warn!("Failed to read cache directory: {:?}", self.cache_dir);
        }
        let mut envs = self.environments.lock().unwrap();
        envs.clear();
        envs.extend(environments.iter().cloned());
        self.cache_is_dirty.store(false, Ordering::Relaxed);
        environments
    }
}

fn update_mtimes_ctimes(env: &mut PythonEnvironment) {
    if let Some(symlinks) = &env.symlinks {
        let mut times = HashMap::new();
        for executable in symlinks.iter() {
            if let Some(mtime_ctime) = get_mtime_ctime(executable) {
                times.insert(
                    executable.clone(),
                    MTimeCTime {
                        mtime: mtime_ctime.mtime,
                        ctime: mtime_ctime.ctime,
                    },
                );
            }
        }
        if !times.is_empty() {
            env.times = Some(times);
        }
    }
}
