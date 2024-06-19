// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

pub fn compute_hash(value: PathBuf) -> String {
    let mut hasher = DefaultHasher::new();
    value.as_os_str().as_bytes().hash(&mut hasher);
    let hash = hasher.finish();

    format!("{:x}", hash)
}
