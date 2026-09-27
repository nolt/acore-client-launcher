//! Per-server settings baked into the launcher at build time.

use std::path::PathBuf;

/// Full URL of the manifest on the patch host. Placeholder - set the real
/// host before building for a server.
const MANIFEST_URL: &str = "https://patch.example.org/version.json";

/// Program the PLAY button starts, relative to the game folder. On Linux a
/// Windows `.exe` runs through Wine.
pub const GAME_PROGRAM: &str = "Wow.exe";

/// Files downloaded at the same time. One by one: parallel connections from
/// every player would saturate the patch host.
pub const DOWNLOAD_CONCURRENCY: usize = 1;

/// `LAUNCHER_MANIFEST_URL` overrides the built-in URL (testing against a
/// local patch host).
pub fn manifest_url() -> String {
    std::env::var("LAUNCHER_MANIFEST_URL").unwrap_or_else(|_| MANIFEST_URL.to_owned())
}

/// The launcher lives in the game folder, so manifest paths resolve next to
/// it. `LAUNCHER_GAME_DIR` overrides that (running from the build folder).
pub fn game_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("LAUNCHER_GAME_DIR") {
        return PathBuf::from(dir);
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}
