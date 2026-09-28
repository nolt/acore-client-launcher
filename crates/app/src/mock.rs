//! Fixed screen states for screenshots (`MOCK=<state>`), no network involved.

use crate::{Failure, GameStartError, InstallError, MainWindow, Overlay, Phase};

pub fn apply(ui: &MainWindow, state: &str) {
    ui.set_install_dir("/home/gracz/Games/AzerothCore".into());
    ui.set_install_dir_example("/home/gracz/Games/AzerothCore".into());
    ui.set_cache_dirs("Cache/".into());
    ui.set_program("Wow.exe".into());
    ui.set_phase(Phase::Ready);
    ui.set_progress(1.0);

    match state {
        "ready" => {}
        "no-wine" => ui.set_launch_error(GameStartError::NoWine),
        "error" => {
            ui.set_phase(Phase::Failed);
            ui.set_failure(Failure::Network);
            ui.set_progress(0.0);
        }
        "tools" => ui.set_overlay(Overlay::Tools),
        "clear-cache" => ui.set_overlay(Overlay::ConfirmClearCache),
        "about" => ui.set_overlay(Overlay::About),
        "paused" => {
            ui.set_phase(Phase::Paused);
            ui.set_bytes_done("148.2 MB".into());
            ui.set_bytes_total("311.0 MB".into());
            ui.set_progress(0.48);
        }
        "install" | "install-error" => {
            ui.set_phase(Phase::Starting);
            ui.set_progress(0.0);
            if state == "install-error" {
                ui.set_install_dir("C:\\Program Files\\AzerothCore".into());
                ui.set_install_error(InstallError::NoPermission);
            }
            ui.set_overlay(Overlay::Install);
        }
        _ => {
            ui.set_phase(Phase::Downloading);
            ui.set_done(2);
            ui.set_total(3);
            ui.set_bytes_done("148.2 MB".into());
            ui.set_bytes_total("311.0 MB".into());
            ui.set_current_file("patch-plPL-4.MPQ".into());
            ui.set_transfer_info("4.2 MB/s  ·  ~40 s".into());
            ui.set_progress(0.48);
        }
    }
}
