use std::collections::HashSet;
use std::path::{Path, PathBuf};

use reqwest::Url;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::launch::LOCAL_CONFIG;
use crate::state::STATE_FILE;

/// `version.json` on the patch host: every file the game folder should have.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    /// Informational, shown in the UI.
    pub version: String,
    #[serde(default)]
    pub generated_at_utc: String,
    /// Where the files live relative to the manifest; empty = next to it.
    #[serde(default)]
    pub base_url: String,
    /// Sorted by path, so an unchanged release gives an identical manifest.
    pub files: Vec<ManifestFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestFile {
    /// Relative to the game folder, `/` separators.
    pub path: String,
    /// Lowercase hex SHA-256; the only thing that decides whether to download.
    pub hash: String,
    /// Bytes; a cheap pre-check before hashing.
    pub size: u64,
}

impl Manifest {
    pub fn parse(json: &[u8]) -> Result<Self> {
        let mut manifest: Manifest =
            serde_json::from_slice(json).map_err(|e| Error::Manifest(e.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Rejects anything that could write outside the game folder, and
    /// normalises hashes to lowercase.
    fn validate(&mut self) -> Result<()> {
        // Windows and macOS filesystems ignore case, so "a.txt" and "A.txt"
        // would be the same file there.
        let mut seen = HashSet::new();
        for file in &mut self.files {
            check_relative_path(&file.path)?;
            if !seen.insert(file.path.to_lowercase()) {
                return Err(Error::Manifest(format!("duplicate path {}", file.path)));
            }
            if file.hash.len() != 64 || !file.hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(Error::Manifest(format!("bad SHA-256 for {}", file.path)));
            }
            file.hash.make_ascii_lowercase();
        }
        Ok(())
    }

    /// Download URL of `file`: folder of the manifest + `baseUrl` + path.
    pub fn file_url(&self, manifest_url: &Url, file: &ManifestFile) -> Result<Url> {
        let bad = |e: &dyn std::fmt::Display| Error::Manifest(format!("bad URL: {e}"));
        let mut url = manifest_url.join(".").map_err(|e| bad(&e))?;
        if !self.base_url.is_empty() {
            let base = if self.base_url.ends_with('/') {
                self.base_url.clone()
            } else {
                format!("{}/", self.base_url)
            };
            url = url.join(&base).map_err(|e| bad(&e))?;
        }
        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| bad(&"URL cannot hold a path"))?;
            segments.pop_if_empty();
            // push() percent-encodes each segment (spaces, '#', '?', ...).
            segments.extend(file.path.split('/'));
        }
        Ok(url)
    }
}

impl ManifestFile {
    /// Where this file lives on disk.
    pub fn local_path(&self, game_dir: &Path) -> PathBuf {
        let mut path = game_dir.to_path_buf();
        path.extend(self.path.split('/'));
        path
    }
}

/// A manifest path must stay inside the game folder on every OS.
fn check_relative_path(path: &str) -> Result<()> {
    let invalid = |why: &str| Err(Error::Manifest(format!("path {path:?} {why}")));
    if path.is_empty() {
        return invalid("is empty");
    }
    // '\' is a separator on Windows, ':' a drive letter or an NTFS stream.
    if path.contains('\\') || path.contains(':') || path.contains('\0') {
        return invalid("contains a forbidden character");
    }
    if path.split('/').any(|s| s.is_empty() || s == "." || s == "..") {
        return invalid("is not a plain relative path");
    }
    // Case-insensitive: on Windows and macOS "A.PART" is the same file as "a.part".
    let lower = path.to_ascii_lowercase();
    if lower == STATE_FILE || lower == LOCAL_CONFIG || lower.ends_with(".part") {
        return invalid("is reserved for the launcher");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    fn manifest(paths: &[&str]) -> Manifest {
        Manifest {
            version: "1".into(),
            generated_at_utc: String::new(),
            base_url: String::new(),
            files: paths
                .iter()
                .map(|p| ManifestFile {
                    path: p.to_string(),
                    hash: HASH.into(),
                    size: 0,
                })
                .collect(),
        }
    }

    #[test]
    fn rejects_paths_leaving_the_game_folder() {
        for bad in [
            "../x",
            "a/../../x",
            "/etc/passwd",
            "C:/x",
            "a\\b",
            "a//b",
            "./a",
            STATE_FILE,
            "launcher.local.json",
            "a.part",
            "Data/A.PART",
        ] {
            assert!(manifest(&[bad]).validate().is_err(), "{bad} should be rejected");
        }
        assert!(
            manifest(&["Data/patch-4.MPQ", "Wow.exe", "a b/c#d.txt"])
                .validate()
                .is_ok()
        );
    }

    #[test]
    fn rejects_case_insensitive_duplicates() {
        assert!(manifest(&["Data/a.txt", "data/A.txt"]).validate().is_err());
    }

    #[test]
    fn builds_file_urls() {
        let url = Url::parse("https://host/patch/version.json").unwrap();
        let mut m = manifest(&["Data/pl PL/patch #1.MPQ"]);
        assert_eq!(
            m.file_url(&url, &m.files[0]).unwrap().as_str(),
            "https://host/patch/Data/pl%20PL/patch%20%231.MPQ"
        );
        m.base_url = "files".into();
        assert_eq!(
            m.file_url(&url, &m.files[0]).unwrap().as_str(),
            "https://host/patch/files/Data/pl%20PL/patch%20%231.MPQ"
        );
    }
}
