//! Launcher self-update. `launcher.json` next to the game manifest names the
//! current launcher release and its binary per platform:
//!
//! ```json
//! { "version": "0.2.0",
//!   "files": { "windows-x64": { "path": "acore-launcher.exe", "hash": "…", "size": 123 },
//!              "linux-x64":   { "path": "acore-launcher",     "hash": "…", "size": 123 } } }
//! ```
//!
//! `path` is relative to `launcher.json`. A missing `launcher.json` or a
//! platform without an entry means no self-update.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use reqwest::header::CACHE_CONTROL;
use reqwest::{StatusCode, Url};
use serde::{Deserialize, Serialize};

use crate::download::download_with_retry;
use crate::error::{Error, Result};
use crate::manifest::{ManifestFile, check_relative_path};
use crate::progress::Progress;
use crate::update::{Shared, http_client};

/// The new launcher is downloaded here, inside the game folder, then swapped in.
const DOWNLOAD_NAME: &str = ".launcher-update";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LauncherInfo {
    pub version: String,
    /// Keyed by platform, see [`PLATFORM`].
    pub files: BTreeMap<String, ManifestFile>,
}

/// Key of this build in [`LauncherInfo::files`]; `None` on platforms without
/// self-update.
pub const PLATFORM: Option<&str> = if cfg!(all(windows, target_arch = "x86_64")) {
    Some("windows-x64")
} else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
    Some("linux-x64")
} else {
    None
};

/// A newer launcher to download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LauncherRelease {
    pub version: String,
    pub url: Url,
    pub file: ManifestFile,
}

/// Fetches `launcher.json` and returns the release for this platform when it
/// is newer than `current`.
pub async fn check_launcher_update(info_url: &Url, current: &str) -> Result<Option<LauncherRelease>> {
    let Some(platform) = PLATFORM else {
        return Ok(None);
    };
    let client = http_client()?;
    let response = client
        .get(info_url.clone())
        .header(CACHE_CONTROL, "no-cache")
        .send()
        .await?;
    match response.status() {
        StatusCode::NOT_FOUND => return Ok(None),
        status if !status.is_success() => {
            return Err(Error::HttpStatus {
                url: info_url.to_string(),
                status: status.as_u16(),
            });
        }
        _ => {}
    }
    let info: LauncherInfo = serde_json::from_slice(&response.bytes().await?)
        .map_err(|e| Error::Manifest(format!("launcher.json: {e}")))?;
    release_for(info, info_url, platform, current)
}

fn release_for(
    mut info: LauncherInfo,
    info_url: &Url,
    platform: &str,
    current: &str,
) -> Result<Option<LauncherRelease>> {
    if !is_newer(&info.version, current) {
        return Ok(None);
    }
    let Some(file) = info.files.remove(platform) else {
        return Ok(None);
    };
    check_relative_path(&file.path)?;
    let url = info_url
        .join(&file.path)
        .map_err(|e| Error::Manifest(format!("launcher path {:?}: {e}", file.path)))?;
    Ok(Some(LauncherRelease {
        version: info.version,
        url,
        file,
    }))
}

/// Downloads `release` into `dir` (resuming, hash-checked) and returns the file.
pub async fn download_launcher(
    release: &LauncherRelease,
    dir: &Path,
    report: impl Fn(Progress) + Send + Sync + 'static,
) -> Result<PathBuf> {
    let client = http_client()?;
    let file = ManifestFile {
        path: DOWNLOAD_NAME.to_owned(),
        ..release.file.clone()
    };
    let shared = Shared::new(1, file.size, Arc::new(report));
    shared.start_file(&file.path);
    download_with_retry(&client, &release.url, dir, &file, &shared).await?;
    shared.finish_file(&file.path);
    Ok(file.local_path(dir))
}

/// Puts the downloaded launcher in place of the running one; it takes effect
/// on the next start.
pub fn replace_running_launcher(new: &Path) -> Result<()> {
    self_replace::self_replace(new).map_err(|e| Error::io(new, e))?;
    std::fs::remove_file(new).map_err(|e| Error::io(new, e))
}

/// `a` is a higher dotted version than `b` ("0.10.0" > "0.9.3"). Anything
/// that is not plain numbers never counts as newer.
fn is_newer(a: &str, b: &str) -> bool {
    let parse = |v: &str| {
        v.trim_start_matches('v')
            .split('.')
            .map(str::parse::<u64>)
            .collect::<std::result::Result<Vec<_>, _>>()
            .ok()
    };
    match (parse(a), parse(b)) {
        (Some(a), Some(b)) => a > b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    fn info(version: &str, path: &str) -> LauncherInfo {
        let file = ManifestFile {
            path: path.to_owned(),
            hash: HASH.to_owned(),
            size: 0,
        };
        LauncherInfo {
            version: version.to_owned(),
            files: [("linux-x64".to_owned(), file)].into(),
        }
    }

    #[test]
    fn compares_versions_as_numbers() {
        assert!(is_newer("0.10.0", "0.9.3"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(is_newer("0.2.0.1", "0.2.0"));
        assert!(!is_newer("0.2.0", "0.2.0"));
        assert!(!is_newer("0.1.9", "0.2.0"));
        assert!(!is_newer("2.0-beta", "0.1.0"));
    }

    #[test]
    fn picks_a_newer_release_for_this_platform() {
        let url = Url::parse("https://patch.example.org/wow/launcher.json").unwrap();

        let release = release_for(info("0.2.0", "bin/acore-launcher"), &url, "linux-x64", "0.1.0")
            .unwrap()
            .unwrap();
        assert_eq!(
            release.url.as_str(),
            "https://patch.example.org/wow/bin/acore-launcher"
        );

        assert_eq!(
            release_for(info("0.1.0", "a"), &url, "linux-x64", "0.1.0").unwrap(),
            None
        );
        assert_eq!(
            release_for(info("0.2.0", "a"), &url, "windows-x64", "0.1.0").unwrap(),
            None
        );
        assert!(release_for(info("0.2.0", "../a"), &url, "linux-x64", "0.1.0").is_err());
    }
}
