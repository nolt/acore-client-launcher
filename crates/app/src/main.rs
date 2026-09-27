// No console window on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod mock;
mod updater;

use slint::winit_030::WinitWindowAccessor;

use crate::updater::Updater;

slint::include_modules!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = MainWindow::new()?;
    ui.set_version(env!("CARGO_PKG_VERSION").into());

    match std::env::var("MOCK") {
        Ok(state) => mock::apply(&ui, &state),
        Err(_) => {
            let updater = Updater::new()?;
            updater.start(&ui, false);

            let (weak, u) = (ui.as_weak(), updater.clone());
            ui.on_retry(move || {
                if let Some(ui) = weak.upgrade() {
                    u.start(&ui, false);
                }
            });
            let weak = ui.as_weak();
            ui.on_verify_files(move || {
                if let Some(ui) = weak.upgrade() {
                    updater.start(&ui, true);
                }
            });
        }
    }

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

    ui.run()?;
    Ok(())
}
