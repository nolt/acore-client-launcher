use std::collections::BTreeMap;
use std::fs::{self, Metadata};
use std::path::Path;
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Launcher bookkeeping in the game folder. Its presence also marks the folder
/// as installed.
pub const STATE_FILE: &str = ".launcher-state.json";

/// What the launcher last verified, so the next start can skip hashing files
/// whose size and modification time did not change.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalState {
    #[serde(default)]
    pub manifest_version: String,
    #[serde(default)]
    pub files: BTreeMap<String, FileRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRecord {
    pub size: u64,
    /// Modification time, nanoseconds since the Unix epoch.
    pub modified: u64,
    pub hash: String,
}

impl FileRecord {
    pub fn new(meta: &Metadata, hash: &str) -> Self {
        FileRecord {
            size: meta.len(),
            modified: modified_nanos(meta),
            hash: hash.to_owned(),
        }
    }

    /// The file on disk is still the one that was hashed.
    pub fn matches(&self, meta: &Metadata) -> bool {
        self.size == meta.len() && self.modified == modified_nanos(meta)
    }
}

fn modified_nanos(meta: &Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos() as u64)
}

impl LocalState {
    pub fn exists(game_dir: &Path) -> bool {
        game_dir.join(STATE_FILE).is_file()
    }

    /// Missing or unreadable state just means "hash everything".
    pub fn load(game_dir: &Path) -> Self {
        fs::read(game_dir.join(STATE_FILE))
            .ok()
            .and_then(|json| serde_json::from_slice(&json).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, game_dir: &Path) -> Result<()> {
        let path = game_dir.join(STATE_FILE);
        let tmp = game_dir.join(format!("{STATE_FILE}.tmp"));
        let json = serde_json::to_vec_pretty(self).expect("state serialises");
        fs::write(&tmp, json).map_err(|e| Error::io(&tmp, e))?;
        fs::rename(&tmp, &path).map_err(|e| Error::io(&path, e))
    }
}
