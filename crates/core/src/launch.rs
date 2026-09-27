//! Starts the game program from the game folder. On Linux a Windows `.exe`
//! runs through Wine: by default `wine` with the prefix from the environment
//! (`WINEPREFIX`, else `~/.wine`), overridable per machine in
//! `launcher.local.json`.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;

/// Per-machine Wine settings in the game folder, written by the player. Never
/// part of a manifest, so updates never touch it.
pub const LOCAL_CONFIG: &str = "launcher.local.json";

/// How a Windows game starts on Linux. Every field of `launcher.local.json`
/// is optional; unknown fields are ignored.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WineConfig {
    /// `wine`, `wine64` or a full path.
    pub wine_command: String,
    /// Prefix to run in; `None` keeps the one from the environment. A leading
    /// `~/` means the home folder.
    pub wine_prefix: Option<String>,
}

impl Default for WineConfig {
    fn default() -> Self {
        WineConfig {
            wine_command: "wine".to_owned(),
            wine_prefix: None,
        }
    }
}

impl WineConfig {
    /// Missing file: defaults. A broken file also falls back to the defaults
    /// (and says so on stderr) rather than blocking PLAY.
    pub fn load(game_dir: &Path) -> Self {
        let path = game_dir.join(LOCAL_CONFIG);
        let Ok(json) = std::fs::read(&path) else {
            return WineConfig::default();
        };
        serde_json::from_slice(&json).unwrap_or_else(|e| {
            eprintln!("{}: {e}; using the defaults", path.display());
            WineConfig::default()
        })
    }

    fn prefix(&self) -> Option<PathBuf> {
        let prefix = self.wine_prefix.as_deref().filter(|p| !p.is_empty())?;
        match prefix.strip_prefix("~/") {
            Some(rest) => dirs::home_dir().map(|home| home.join(rest)),
            None => Some(PathBuf::from(prefix)),
        }
    }
}

#[derive(Debug)]
pub enum LaunchError {
    /// The program is not in the game folder.
    Missing(PathBuf),
    /// A Windows program on Linux, and `wine` is not installed.
    NoWine,
    Spawn(io::Error),
    /// The game quit straight away, e.g. a broken Wine setup.
    Exited(ExitStatus),
}

impl fmt::Display for LaunchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LaunchError::Missing(path) => write!(f, "{} not found", path.display()),
            LaunchError::NoWine => write!(f, "wine is not installed"),
            LaunchError::Spawn(e) => write!(f, "could not start the game: {e}"),
            LaunchError::Exited(status) => write!(f, "the game quit right after starting ({status})"),
        }
    }
}

impl std::error::Error for LaunchError {}

/// Starts `program` (a path relative to `game_dir`) and returns without
/// waiting for it.
pub fn launch(game_dir: &Path, program: &str) -> Result<Child, LaunchError> {
    let path = game_dir.join(program);
    if !path.is_file() {
        return Err(LaunchError::Missing(path));
    }
    let wine = needs_wine(program).then(|| WineConfig::load(game_dir));
    command(game_dir, &path, wine.as_ref())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| match e.kind() {
            io::ErrorKind::NotFound if wine.is_some() => LaunchError::NoWine,
            _ => LaunchError::Spawn(e),
        })
}

/// Watches the game for `grace`: quitting within it counts as a failed start.
/// The game keeps running after this returns.
pub fn check_started(child: &mut Child, grace: Duration) -> Result<(), LaunchError> {
    let deadline = Instant::now() + grace;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Err(LaunchError::Exited(status)),
            Ok(None) if Instant::now() >= deadline => return Ok(()),
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(e) => return Err(LaunchError::Spawn(e)),
        }
    }
}

fn needs_wine(program: &str) -> bool {
    !cfg!(windows) && program.to_ascii_lowercase().ends_with(".exe")
}

fn command(game_dir: &Path, program: &Path, wine: Option<&WineConfig>) -> Command {
    let mut cmd = match wine {
        Some(config) => {
            let mut cmd = Command::new(&config.wine_command);
            cmd.arg(program).env("WINEDEBUG", "-all");
            if let Some(prefix) = config.prefix() {
                cmd.env("WINEPREFIX", prefix);
            }
            cmd
        }
        None => Command::new(program),
    };
    cmd.current_dir(game_dir);
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_program_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            launch(dir.path(), "Game.exe"),
            Err(LaunchError::Missing(_))
        ));
    }

    #[cfg(not(windows))]
    #[test]
    fn a_game_that_quits_at_once_is_a_failed_start() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        for (name, body) in [("quits.sh", "exit 0"), ("runs.sh", "sleep 5")] {
            let path = dir.path().join(name);
            std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let grace = Duration::from_millis(500);

        let mut quits = launch(dir.path(), "quits.sh").unwrap();
        assert!(matches!(
            check_started(&mut quits, grace),
            Err(LaunchError::Exited(_))
        ));

        let mut runs = launch(dir.path(), "runs.sh").unwrap();
        assert!(check_started(&mut runs, grace).is_ok());
        runs.kill().unwrap();
        runs.wait().unwrap();
    }

    #[cfg(not(windows))]
    #[test]
    fn a_windows_program_runs_through_wine() {
        assert!(needs_wine("Wow.exe") && needs_wine("bin/GAME.EXE"));
        assert!(!needs_wine("game.sh"));

        let dir = Path::new("/games/ac");
        let prefix_of = |cmd: &Command| {
            cmd.get_envs()
                .find(|(k, _)| *k == "WINEPREFIX")
                .and_then(|(_, v)| v.map(PathBuf::from))
        };

        // Default: plain wine, the prefix comes from the environment.
        let cmd = command(dir, &dir.join("Wow.exe"), Some(&WineConfig::default()));
        assert_eq!(cmd.get_program(), "wine");
        assert_eq!(cmd.get_args().collect::<Vec<_>>(), ["/games/ac/Wow.exe"]);
        assert_eq!(cmd.get_current_dir(), Some(dir));
        assert_eq!(prefix_of(&cmd), None);

        let config = WineConfig {
            wine_command: "wine64".into(),
            wine_prefix: Some("/home/a/.wine-ac".into()),
        };
        let cmd = command(dir, &dir.join("Wow.exe"), Some(&config));
        assert_eq!(cmd.get_program(), "wine64");
        assert_eq!(prefix_of(&cmd), Some(PathBuf::from("/home/a/.wine-ac")));
    }

    #[test]
    fn reads_the_local_config() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(WineConfig::load(dir.path()), WineConfig::default());

        std::fs::write(
            dir.path().join(LOCAL_CONFIG),
            r#"{ "mode": "wine", "winePrefix": "~/.winemu" }"#,
        )
        .unwrap();
        let config = WineConfig::load(dir.path());
        assert_eq!(config.wine_command, "wine");
        assert_eq!(config.prefix(), dirs::home_dir().map(|h| h.join(".winemu")));

        std::fs::write(dir.path().join(LOCAL_CONFIG), "{ broken").unwrap();
        assert_eq!(WineConfig::load(dir.path()), WineConfig::default());
    }
}
