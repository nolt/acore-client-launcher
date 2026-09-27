//! The tools behind the gear: clear cache and open the game folder
//! ("Check files" is a full update, see `updater`).

use std::process::Command;

use launcher_core::clear_cache;

use crate::{MainWindow, Notice, config};

/// The cache folders as the player sees them, e.g. "Cache/".
pub fn cache_dirs_label() -> String {
    config::CACHE_DIRS
        .iter()
        .map(|d| format!("{d}/"))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn clear(ui: &MainWindow) {
    ui.set_notice(match clear_cache(&config::game_dir(), config::CACHE_DIRS) {
        Ok(()) => Notice::CacheCleared,
        Err(e) => {
            eprintln!("clear cache: {e}");
            Notice::CacheFailed
        }
    });
}

pub fn open_game_dir() {
    let dir = config::game_dir();
    let opener = if cfg!(windows) { "explorer" } else { "xdg-open" };
    if let Err(e) = Command::new(opener).arg(&dir).spawn() {
        eprintln!("{opener} {}: {e}", dir.display());
    }
}
