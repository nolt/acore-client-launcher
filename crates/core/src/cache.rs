//! "Clear cache": deletes folders the game rebuilds by itself. Only ever on the
//! player's request - the launcher never deletes game files on its own.

use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use crate::error::{Error, Result};
use crate::manifest::check_relative_path;

/// Deletes each folder in `dirs` (paths relative to `game_dir`, `/`-separated).
/// Missing folders are fine.
pub fn clear_cache(game_dir: &Path, dirs: &[&str]) -> Result<()> {
    for dir in dirs {
        check_relative_path(dir)?;
        let mut path = game_dir.to_path_buf();
        path.extend(dir.split('/'));
        match fs::remove_dir_all(&path) {
            Err(e) if e.kind() != ErrorKind::NotFound => return Err(Error::io(&path, e)),
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deletes_only_the_listed_folders() {
        let game = tempfile::tempdir().unwrap();
        fs::create_dir_all(game.path().join("Cache/WDB/enUS")).unwrap();
        fs::write(game.path().join("Cache/WDB/enUS/creaturecache.wdb"), b"x").unwrap();
        fs::create_dir_all(game.path().join("Data")).unwrap();

        clear_cache(game.path(), &["Cache", "Logs"]).unwrap();

        assert!(!game.path().join("Cache").exists());
        assert!(game.path().join("Data").exists());
    }

    #[test]
    fn refuses_paths_outside_the_game_folder() {
        let game = tempfile::tempdir().unwrap();
        assert!(clear_cache(game.path(), &["../other"]).is_err());
        assert!(clear_cache(game.path(), &[""]).is_err());
    }
}
