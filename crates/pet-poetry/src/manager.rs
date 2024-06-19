// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use pet_core::manager::{EnvManager, EnvManagerType};
use std::path::PathBuf;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct PoetryManager {
    pub executable: PathBuf,
}

impl PoetryManager {
    pub fn find(executable: Option<PathBuf>) -> Option<Self> {
        if let Some(executable) = executable {
            if executable.is_file() {
                return Some(PoetryManager { executable });
            }
        }
        None
    }
    pub fn to_manager(&self) -> EnvManager {
        EnvManager {
            executable: self.executable.clone(),
            version: None,
            tool: EnvManagerType::Poetry,
        }
    }
}
