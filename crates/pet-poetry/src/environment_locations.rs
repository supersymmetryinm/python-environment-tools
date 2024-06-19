// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use log::{error, trace};
use std::{collections::HashMap, path::PathBuf};

pub fn get_environments_for_folders(
    executable: &PathBuf,
    project_dirs: Vec<PathBuf>,
) -> HashMap<PathBuf, Vec<PathBuf>> {
    let mut envs = HashMap::new();
    for project_dir in project_dirs {
        if let Some(env) = get_environments(executable, &project_dir) {
            envs.insert(project_dir, env);
        }
    }
    envs
}

fn get_environments(executable: &PathBuf, project_dir: &PathBuf) -> Option<Vec<PathBuf>> {
    let result = std::process::Command::new(executable)
        .arg("env")
        .arg("list")
        .arg("--full-path")
        .current_dir(project_dir)
        .output();
    trace!("Executing Poetry: {:?} env list --full-path", executable);
    match result {
        Ok(output) => {
            if output.status.success() {
                let output = String::from_utf8_lossy(&output.stdout).to_string();
                Some(
                    output
                        .lines()
                        .map(|line|
                        // Remove the '(Activated)` suffix from the line
                        line.trim_end_matches(" (Activated)").trim())
                        .filter(|line| !line.is_empty())
                        .map(|line|
                        // Remove the '(Activated)` suffix from the line
                        PathBuf::from(line.trim_end_matches(" (Activated)").trim()))
                        .collect::<Vec<PathBuf>>(),
                )
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                trace!(
                    "Failed to get Poetry Envs using exe {:?} ({:?}) {}",
                    executable,
                    output.status.code().unwrap_or_default(),
                    stderr
                );
                None
            }
        }
        Err(err) => {
            error!("Failed to execute Poetry env list {:?}", err);
            None
        }
    }
}
