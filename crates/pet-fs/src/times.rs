// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::{path::Path, time::UNIX_EPOCH};

pub struct MTimeCTime {
    pub mtime: u128,
    pub ctime: u128,
}

pub fn get_mtime_ctime(path: &Path) -> Option<MTimeCTime> {
    let mut mtime = None;
    let mut ctime = None;

    if let Ok(metadata) = path.metadata() {
        if let Ok(created) = metadata.created() {
            ctime = created
                .duration_since(UNIX_EPOCH)
                .ok()
                .map(|d| d.as_millis());
        }
        if let Ok(modified) = metadata.modified() {
            mtime = modified
                .duration_since(UNIX_EPOCH)
                .ok()
                .map(|d| d.as_millis());
        }
    }
    if let (Some(mtime), Some(ctime)) = (mtime, ctime) {
        return Some(MTimeCTime { mtime, ctime });
    }
    None
}
