//! PLAY: starts the game and closes the launcher once the game is running.

use std::time::Duration;

use launcher_core::{LaunchError, check_started, launch};
use slint::ComponentHandle;

use crate::{GameStartError, MainWindow, config};

/// A game still running after this counts as started.
const START_GRACE: Duration = Duration::from_secs(3);

pub fn play(ui: &MainWindow) {
    ui.set_launch_error(GameStartError::None);
    let mut child = match launch(&config::game_dir(), config::GAME_PROGRAM) {
        Ok(child) => child,
        Err(e) => return show_error(ui, &e),
    };
    ui.set_game_starting(true);

    let weak = ui.as_weak();
    std::thread::spawn(move || {
        let result = check_started(&mut child, START_GRACE);
        let _ = weak.upgrade_in_event_loop(move |ui| match result {
            Ok(()) => {
                let _ = slint::quit_event_loop();
            }
            Err(e) => {
                ui.set_game_starting(false);
                show_error(&ui, &e);
            }
        });
    });
}

fn show_error(ui: &MainWindow, error: &LaunchError) {
    eprintln!("{error}");
    ui.set_launch_error(match error {
        LaunchError::Missing(_) => GameStartError::Missing,
        LaunchError::NoWine => GameStartError::NoWine,
        LaunchError::Spawn(_) => GameStartError::Failed,
        LaunchError::Exited(_) => GameStartError::Exited,
    });
}
