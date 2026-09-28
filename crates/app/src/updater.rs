//! Runs the core updater in the background and mirrors its progress in the window.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use launcher_core::{Error, Pause, Progress, UpdateOptions, Url, format, update};
use slint::ComponentHandle;
use tokio::runtime::Runtime;

use crate::{Failure, GameStartError, MainWindow, Notice, Phase, config, self_update};

#[derive(Clone)]
pub struct Updater {
    runtime: Arc<Runtime>,
    running: Arc<AtomicBool>,
    /// Pause of the running update; a fresh one for every start.
    pause: Arc<Mutex<Pause>>,
    /// The launcher itself is checked once per start, before the first update.
    launcher_checked: Arc<AtomicBool>,
}

impl Updater {
    pub fn new() -> std::io::Result<Self> {
        Ok(Updater {
            runtime: Arc::new(Runtime::new()?),
            running: Arc::new(AtomicBool::new(false)),
            pause: Arc::default(),
            launcher_checked: Arc::new(AtomicBool::new(!self_update::wanted())),
        })
    }

    /// Starts an update unless one is already running. `full_check` hashes
    /// every file instead of trusting the saved state.
    pub fn start(&self, ui: &MainWindow, full_check: bool) {
        if self.running.swap(true, Ordering::SeqCst) {
            return;
        }
        ui.set_phase(Phase::Starting);
        ui.set_launch_error(GameStartError::None);
        ui.set_notice(Notice::None);
        ui.set_progress(0.0);
        ui.set_transfer_info("".into());
        let pause = Pause::default();
        *self.pause.lock().unwrap() = pause.clone();

        let weak = ui.as_weak();
        let running = self.running.clone();
        let launcher_checked = self.launcher_checked.clone();
        self.runtime.spawn(async move {
            if !launcher_checked.swap(true, Ordering::SeqCst) && self_update::run(&weak).await {
                return;
            }
            let result = match Url::parse(&config::manifest_url()) {
                Ok(manifest_url) => {
                    let options = UpdateOptions {
                        manifest_url,
                        game_dir: config::game_dir(),
                        full_check,
                        concurrency: config::DOWNLOAD_CONCURRENCY,
                        pause: Some(pause),
                    };
                    let progress_ui = weak.clone();
                    update(&options, move |p| {
                        let _ = progress_ui.upgrade_in_event_loop(move |ui| show_progress(&ui, p));
                    })
                    .await
                }
                Err(e) => Err(Error::Manifest(format!("manifest URL: {e}"))),
            };
            running.store(false, Ordering::SeqCst);
            let _ = weak.upgrade_in_event_loop(move |ui| {
                ui.set_transfer_info("".into());
                match result {
                    Ok(summary) => {
                        ui.set_progress(1.0);
                        ui.set_game_version(summary.manifest_version.into());
                        ui.set_phase(if summary.files_downloaded > 0 {
                            Phase::Updated
                        } else {
                            Phase::Ready
                        });
                    }
                    // Progress stays where it stopped; the next start resumes.
                    Err(Error::Paused) => ui.set_phase(Phase::Paused),
                    Err(e) => {
                        eprintln!("update failed: {e}");
                        ui.set_failure(failure_of(&e));
                        ui.set_phase(Phase::Failed);
                    }
                }
            });
        });
    }

    /// Stops the downloads of the running update; `start` resumes them.
    pub fn pause(&self) {
        self.pause.lock().unwrap().request();
    }
}

fn show_progress(ui: &MainWindow, progress: Progress) {
    match progress {
        Progress::FetchingManifest => ui.set_phase(Phase::Fetching),
        Progress::Checking { done, total } => {
            ui.set_phase(Phase::Checking);
            ui.set_done(done as i32);
            ui.set_total(total as i32);
            ui.set_progress(if total == 0 {
                1.0
            } else {
                done as f32 / total as f32
            });
        }
        Progress::Downloading {
            file,
            files_total,
            bytes_done,
            bytes_total,
            current,
            bytes_per_sec,
            eta,
        } => {
            ui.set_phase(Phase::Downloading);
            ui.set_done(file as i32);
            ui.set_total(files_total as i32);
            ui.set_bytes_done(format::bytes(bytes_done).into());
            ui.set_bytes_total(format::bytes(bytes_total).into());
            // The file name only, the full path does not fit.
            ui.set_current_file(current.rsplit('/').next().unwrap_or_default().into());
            ui.set_progress(if bytes_total == 0 {
                1.0
            } else {
                bytes_done as f32 / bytes_total as f32
            });
            let info = match (bytes_per_sec > 0.0, eta) {
                (true, Some(eta)) => format!("{}  ·  {}", format::rate(bytes_per_sec), format::eta(eta)),
                (true, None) => format::rate(bytes_per_sec),
                _ => String::new(),
            };
            ui.set_transfer_info(info.into());
        }
    }
}

fn failure_of(error: &Error) -> Failure {
    match error {
        Error::Http(_) => Failure::Network,
        Error::HttpStatus { .. } | Error::HashMismatch { .. } | Error::SizeMismatch { .. } => Failure::Server,
        Error::Io { .. } => Failure::Disk,
        Error::Manifest(_) => Failure::Manifest,
        Error::Paused => unreachable!("a pause is not a failure"),
    }
}
