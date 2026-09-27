//! First run: the launcher copies itself into the game folder and starts again
//! from there, so the game folder is always the folder the launcher sits in.

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use crate::error::{Error, Result};
use crate::state::LocalState;

/// Written and deleted again to prove the folder is writable.
const PROBE_FILE: &str = ".launcher-write-test";

/// Creates `dir` if needed and checks the launcher can write to it.
pub fn check_writable(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    let probe = dir.join(PROBE_FILE);
    fs::write(&probe, b"").map_err(|e| Error::io(&probe, e))?;
    fs::remove_file(&probe).map_err(|e| Error::io(&probe, e))
}

/// Puts the launcher `exe` into `dir` and marks `dir` as installed. Returns the
/// launcher to start from now on - `exe` itself when it already sits in `dir`
/// (a launcher dropped into the game folder by hand is never moved).
pub fn install(exe: &Path, dir: &Path) -> Result<PathBuf> {
    check_writable(dir)?;

    let file_name = exe
        .file_name()
        .ok_or_else(|| Error::io(exe, std::io::ErrorKind::InvalidInput.into()))?;
    let target = dir.join(file_name);
    if !same_file(exe, &target) {
        fs::copy(exe, &target).map_err(|e| Error::io(&target, e))?;
    }

    if !LocalState::exists(dir) {
        LocalState::default().save(dir)?;
    }
    Ok(target)
}

/// Deletes the launcher the player started the install from (e.g. in
/// Downloads). Windows keeps a running program locked, so this waits until the
/// original has exited. Only a file the size of `installed` is deleted.
pub fn remove_original(original: &Path, installed: &Path) -> Result<()> {
    if same_file(original, installed) {
        return Ok(());
    }
    let size = |p: &Path| fs::metadata(p).map(|m| m.len());
    match (size(original), size(installed)) {
        (Ok(a), Ok(b)) if a == b => {}
        _ => return Ok(()),
    }

    let mut attempts = 20;
    loop {
        match fs::remove_file(original) {
            Ok(()) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => {
                attempts -= 1;
                if attempts == 0 {
                    return Err(Error::io(original, e));
                }
                thread::sleep(Duration::from_millis(500));
            }
        }
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}
