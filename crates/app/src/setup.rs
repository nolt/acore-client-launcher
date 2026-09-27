//! First run: asks where to install, moves the launcher there and creates the
//! shortcuts. A launcher started from its game folder skips all of this.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

use launcher_core::{Error, LocalState, Shortcut, create_shortcuts, install, remove_original};
use slint::ComponentHandle;

use crate::updater::Updater;
use crate::{Brand, InstallError, MainWindow, Overlay, config};

/// Menu icon on Linux.
const ICON_PNG: &[u8] = include_bytes!("../ui/assets/icon.png");

/// Passed to the installed copy: the launcher the player started, to delete.
const INSTALLED_FROM: &str = "--installed-from";

/// The game folder has no launcher state yet. `LAUNCHER_GAME_DIR` (testing)
/// counts as installed.
pub fn needed() -> bool {
    std::env::var_os("LAUNCHER_GAME_DIR").is_none() && !LocalState::exists(&config::game_dir())
}

/// Shows the install dialog; the update starts once the launcher runs from
/// the game folder.
pub fn show(ui: &MainWindow, updater: Updater) {
    let app_name = ui.global::<Brand>().get_app_name();
    let suggested = suggested_dir(&app_name).to_string_lossy().into_owned();
    ui.set_install_dir_example(suggested.as_str().into());
    set_dir(ui, &suggested);
    ui.set_overlay(Overlay::Install);

    let weak = ui.as_weak();
    ui.on_install_dir_edited(move |text| {
        if let Some(ui) = weak.upgrade() {
            set_dir(&ui, &text);
        }
    });

    let weak = ui.as_weak();
    ui.on_browse_install_dir(move || {
        let Some(ui) = weak.upgrade() else { return };
        let current = PathBuf::from(ui.get_install_dir().as_str());
        let mut dialog = rfd::FileDialog::new();
        if let Some(existing) = current.ancestors().find(|d| d.is_dir()) {
            dialog = dialog.set_directory(existing);
        }
        if let Some(dir) = dialog.pick_folder() {
            set_dir(&ui, &dir.to_string_lossy());
        }
    });

    let weak = ui.as_weak();
    ui.on_install(move || {
        if let Some(ui) = weak.upgrade() {
            run_install(&ui, &updater);
        }
    });
}

/// After a move: deletes the launcher the player started the install from,
/// once it has exited.
pub fn finish_move() {
    let args = std::env::args_os().skip(1);
    let Some(original) = args.skip_while(|a| a != INSTALLED_FROM).nth(1) else {
        return;
    };
    std::thread::spawn(move || {
        let Ok(exe) = std::env::current_exe() else { return };
        if let Err(e) = remove_original(Path::new(&original), &exe) {
            eprintln!("could not delete the original launcher: {e}");
        }
    });
}

fn run_install(ui: &MainWindow, updater: &Updater) {
    let dir = PathBuf::from(ui.get_install_dir().as_str());
    let result = std::env::current_exe()
        .map_err(|e| Error::Io {
            path: PathBuf::new(),
            source: e,
        })
        .and_then(|exe| Ok((install(&exe, &dir)?, exe)));
    let (installed, exe) = match result {
        Ok(paths) => paths,
        Err(e) => {
            eprintln!("install failed: {e}");
            ui.set_install_error(install_error_of(&e));
            return;
        }
    };

    if ui.get_create_shortcut() {
        let name = ui.global::<Brand>().get_app_name();
        let shortcut = Shortcut {
            name: &name,
            exe: &installed,
            icon_png: ICON_PNG,
        };
        // The game works without shortcuts; not worth stopping the install.
        if let Err(e) = create_shortcuts(&shortcut) {
            eprintln!("could not create shortcuts: {e}");
        }
    }

    if fs::canonicalize(&installed).ok() == fs::canonicalize(&exe).ok() {
        // Already in the game folder: nothing to move.
        ui.set_overlay(Overlay::None);
        updater.start(ui, false);
        return;
    }
    let started = Command::new(&installed)
        .arg(INSTALLED_FROM)
        .arg(&exe)
        .current_dir(&dir)
        .spawn();
    match started {
        Ok(_) => {
            let _ = slint::quit_event_loop();
        }
        Err(e) => {
            eprintln!("could not start {}: {e}", installed.display());
            ui.set_install_error(InstallError::Failed);
        }
    }
}

fn set_dir(ui: &MainWindow, text: &str) {
    ui.set_install_dir(text.into());
    ui.set_install_error(if text.is_empty() || Path::new(text).is_absolute() {
        InstallError::None
    } else {
        InstallError::NotAbsolute
    });
}

/// `~/Games/<name>` on Linux, `C:\Games\<name>` on Windows.
fn suggested_dir(app_name: &str) -> PathBuf {
    let games = if cfg!(windows) {
        let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into());
        PathBuf::from(format!("{drive}\\Games"))
    } else {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/"))
            .join("Games")
    };
    games.join(app_name)
}

fn install_error_of(error: &Error) -> InstallError {
    match error {
        Error::Io { source, .. }
            if matches!(
                source.kind(),
                ErrorKind::PermissionDenied | ErrorKind::ReadOnlyFilesystem
            ) =>
        {
            InstallError::NoPermission
        }
        _ => InstallError::Failed,
    }
}
