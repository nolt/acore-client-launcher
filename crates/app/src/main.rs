// No console window on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use slint::winit_030::WinitWindowAccessor;

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = MainWindow::new()?;

    ui.set_version(env!("CARGO_PKG_VERSION").into());

    // UI mockup: no core yet, the screen state comes from MOCK.
    apply_mock_state(&ui, &std::env::var("MOCK").unwrap_or_default());

    let weak = ui.as_weak();
    ui.on_start_drag(move || {
        if let Some(ui) = weak.upgrade() {
            ui.window().with_winit_window(|w| {
                let _ = w.drag_window();
            });
        }
    });

    let weak = ui.as_weak();
    ui.on_minimise(move || {
        if let Some(ui) = weak.upgrade() {
            ui.window().set_minimized(true);
        }
    });

    ui.on_close_window(|| {
        let _ = slint::quit_event_loop();
    });

    ui.run()
}

fn apply_mock_state(ui: &MainWindow, state: &str) {
    ui.set_install_dir("/home/gracz/Games/AzerothCore".into());
    ui.set_cache_dirs("Cache/".into());

    match state {
        "ready" => ready(ui),
        "error" => {
            ui.set_status_text("Could not update. Check your connection and try again.".into());
            ui.set_can_retry(true);
        }
        "tools" => {
            ready(ui);
            ui.set_overlay(Overlay::Tools);
        }
        "clear-cache" => {
            ready(ui);
            ui.set_overlay(Overlay::ConfirmClearCache);
        }
        "about" => {
            ready(ui);
            ui.set_overlay(Overlay::About);
        }
        "install" => {
            ui.set_overlay(Overlay::Install);
        }
        "install-error" => {
            ui.set_install_dir("C:\\Program Files\\AzerothCore".into());
            ui.set_install_dir_error("No permission to write to this folder. Choose another one.".into());
            ui.set_overlay(Overlay::Install);
        }
        _ => {
            ui.set_status_text("Downloading 2/3  (148.2 MB / 311.0 MB)  ·  patch-plPL-4.MPQ".into());
            ui.set_transfer_info("4.2 MB/s  ·  ~40 s".into());
            ui.set_progress(0.48);
        }
    }
}

fn ready(ui: &MainWindow) {
    ui.set_status_text("Up to date — ready to play.".into());
    ui.set_progress(1.0);
    ui.set_can_play(true);
}
