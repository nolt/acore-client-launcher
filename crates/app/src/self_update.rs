//! Replaces the launcher with a newer release from the patch host and starts
//! it again. Any failure just leaves the current launcher running.

use std::process::Command;

use launcher_core::{
    Progress, Url, check_launcher_update, download_launcher, format, replace_running_launcher,
};
use slint::Weak;

use crate::{MainWindow, Phase, config};

/// Passed to the restarted launcher so it does not check again (a
/// `launcher.json` announcing a version the binary does not report would
/// otherwise restart it forever).
pub const UPDATED: &str = "--updated";

/// Whether this start should look for a newer launcher.
pub fn wanted() -> bool {
    !std::env::args_os().any(|a| a == UPDATED)
}

/// Returns true when a new launcher has been started and this one is closing.
pub async fn run(weak: &Weak<MainWindow>) -> bool {
    match try_run(weak).await {
        Ok(restarting) => restarting,
        Err(e) => {
            eprintln!("launcher self-update: {e}");
            false
        }
    }
}

async fn try_run(weak: &Weak<MainWindow>) -> Result<bool, Box<dyn std::error::Error>> {
    let info_url = Url::parse(&config::manifest_url())?.join(config::LAUNCHER_INFO)?;
    let Some(release) = check_launcher_update(&info_url, env!("CARGO_PKG_VERSION")).await? else {
        return Ok(false);
    };
    // Before the swap: on Linux the path of a replaced running binary reads
    // as "... (deleted)".
    let exe = std::env::current_exe()?;

    let _ = weak.upgrade_in_event_loop(|ui| ui.set_phase(Phase::LauncherUpdate));
    let progress_ui = weak.clone();
    let new = download_launcher(&release, &config::game_dir(), move |p| {
        if let Progress::Downloading {
            bytes_done,
            bytes_total,
            bytes_per_sec,
            ..
        } = p
        {
            let _ = progress_ui.upgrade_in_event_loop(move |ui| {
                ui.set_progress(if bytes_total == 0 {
                    1.0
                } else {
                    bytes_done as f32 / bytes_total as f32
                });
                if bytes_per_sec > 0.0 {
                    ui.set_transfer_info(format::rate(bytes_per_sec).into());
                }
            });
        }
    })
    .await?;
    replace_running_launcher(&new)?;

    Command::new(&exe)
        .arg(UPDATED)
        .current_dir(config::game_dir())
        .spawn()?;
    let _ = slint::invoke_from_event_loop(|| {
        let _ = slint::quit_event_loop();
    });
    Ok(true)
}
