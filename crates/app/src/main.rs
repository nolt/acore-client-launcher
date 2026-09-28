// No console window on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod game;
mod mock;
mod self_update;
mod setup;
mod tools;
mod updater;

use slint::LogicalPosition;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::winit_030::WinitWindowAccessor;

use crate::updater::Updater;

slint::include_modules!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ui = MainWindow::new()?;
    ui.set_version(env!("CARGO_PKG_VERSION").into());
    ui.set_program(config::GAME_PROGRAM.into());
    ui.set_cache_dirs(tools::cache_dirs_label().into());
    ui.set_install_dir(config::game_dir().to_string_lossy().as_ref().into());

    match std::env::var("MOCK") {
        Ok(state) => mock::apply(&ui, &state),
        Err(_) => {
            let updater = Updater::new()?;
            if setup::needed() {
                setup::show(&ui, updater.clone());
            } else {
                setup::finish_move();
                updater.start(&ui, false);
            }

            let (weak, u) = (ui.as_weak(), updater.clone());
            ui.on_retry(move || {
                if let Some(ui) = weak.upgrade() {
                    u.start(&ui, false);
                }
            });
            let weak = ui.as_weak();
            ui.on_play(move || {
                if let Some(ui) = weak.upgrade() {
                    game::play(&ui);
                }
            });
            let weak = ui.as_weak();
            ui.on_clear_cache(move || {
                if let Some(ui) = weak.upgrade() {
                    tools::clear(&ui);
                }
            });
            ui.on_open_game_dir(tools::open_game_dir);
            let weak = ui.as_weak();
            ui.on_verify_files(move || {
                if let Some(ui) = weak.upgrade() {
                    updater.start(&ui, true);
                }
            });
        }
    }

    let weak = ui.as_weak();
    ui.on_start_drag(move |x, y| {
        if let Some(ui) = weak.upgrade() {
            ui.window().with_winit_window(|w| {
                let _ = w.drag_window();
            });
            // The compositor takes over the move and, on Wayland, never sends
            // the button release, so Slint keeps the title bar grabbed and
            // swallows every later click. Release it ourselves once this
            // event is done.
            let weak = weak.clone();
            slint::Timer::single_shot(std::time::Duration::ZERO, move || {
                if let Some(ui) = weak.upgrade() {
                    ui.window().dispatch_event(WindowEvent::PointerReleased {
                        position: LogicalPosition::new(x, y),
                        button: PointerEventButton::Left,
                    });
                }
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
