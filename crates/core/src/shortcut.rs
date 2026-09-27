//! Desktop and menu shortcuts to the installed launcher: `.lnk` files on
//! Windows, `.desktop` entries on Linux.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};

pub struct Shortcut<'a> {
    /// Shown under the icon and in the menu, e.g. "AzerothCore".
    pub name: &'a str,
    /// The installed launcher.
    pub exe: &'a Path,
    /// Menu icon on Linux. Windows takes the icon from the exe.
    pub icon_png: &'a [u8],
}

/// Creates the menu shortcut and, when the user has a desktop folder, a
/// desktop one. Returns the files written.
pub fn create_shortcuts(shortcut: &Shortcut) -> Result<Vec<PathBuf>> {
    platform::create(shortcut)
}

/// `name` without characters that are invalid in file names.
fn file_name(name: &str) -> String {
    let clean: String = name
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') && !c.is_control())
        .collect();
    match clean.trim() {
        "" => "Launcher".to_owned(),
        s => s.to_owned(),
    }
}

fn not_found(what: &str) -> Error {
    Error::io(what, std::io::ErrorKind::NotFound.into())
}

#[cfg(windows)]
mod platform {
    use super::*;

    pub fn create(shortcut: &Shortcut) -> Result<Vec<PathBuf>> {
        let lnk = format!("{}.lnk", file_name(shortcut.name));
        let start_menu = dirs::data_dir()
            .ok_or_else(|| not_found("Start Menu"))?
            .join(r"Microsoft\Windows\Start Menu\Programs");

        let mut created = Vec::new();
        for dir in std::iter::once(start_menu).chain(dirs::desktop_dir().filter(|d| d.is_dir())) {
            let path = dir.join(&lnk);
            fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
            let mut link = mslnk::ShellLink::new(shortcut.exe).map_err(|e| lnk_error(shortcut.exe, e))?;
            link.set_working_dir(shortcut.exe.parent().map(|d| d.to_string_lossy().into_owned()));
            link.create_lnk(&path).map_err(|e| lnk_error(&path, e))?;
            created.push(path);
        }
        Ok(created)
    }

    fn lnk_error(path: &Path, e: mslnk::MSLinkError) -> Error {
        Error::io(path, std::io::Error::other(e.to_string()))
    }
}

#[cfg(not(windows))]
mod platform {
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Command, Stdio};

    use super::*;

    pub fn create(shortcut: &Shortcut) -> Result<Vec<PathBuf>> {
        let id = desktop_id(shortcut.name);
        let data = dirs::data_dir().ok_or_else(|| not_found("~/.local/share"))?;

        let icon = data.join("icons/hicolor/256x256/apps").join(format!("{id}.png"));
        write(&icon, shortcut.icon_png)?;

        let entry = desktop_entry(shortcut.name, shortcut.exe, &id);
        let file = format!("{id}.desktop");
        let menu = data.join("applications").join(&file);
        write(&menu, entry.as_bytes())?;
        let mut created = vec![menu];

        if let Some(desktop) = dirs::desktop_dir().filter(|d| d.is_dir()) {
            let path = desktop.join(&file);
            write(&path, entry.as_bytes())?;
            // Desktops only launch executable entries; GNOME also wants them
            // marked as trusted. Best effort - `gio` may be missing.
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(0o755));
            let _ = Command::new("gio")
                .args(["set", "-t", "string"])
                .arg(&path)
                .args(["metadata::trusted", "true"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            created.push(path);
        }
        Ok(created)
    }

    fn write(path: &Path, contents: &[u8]) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
        }
        fs::write(path, contents).map_err(|e| Error::io(path, e))
    }

    /// Lower-case ASCII name for the `.desktop` file and the icon.
    pub(super) fn desktop_id(name: &str) -> String {
        let mut id = String::new();
        for c in name.chars() {
            if c.is_ascii_alphanumeric() {
                id.push(c.to_ascii_lowercase());
            } else if !id.is_empty() && !id.ends_with('-') {
                id.push('-');
            }
        }
        let id = id.trim_end_matches('-');
        format!("{}-launcher", if id.is_empty() { "game" } else { id })
    }

    pub(super) fn desktop_entry(name: &str, exe: &Path, id: &str) -> String {
        let dir = exe.parent().unwrap_or(Path::new("/"));
        format!(
            "[Desktop Entry]\nType=Application\nName={}\nExec={}\nPath={}\nIcon={id}\nTerminal=false\nCategories=Game;\n",
            escape_value(&file_name(name)),
            escape_value(&quote_exec(&exe.to_string_lossy())),
            escape_value(&dir.to_string_lossy()),
        )
    }

    /// Exec argument quoting from the Desktop Entry spec.
    fn quote_exec(arg: &str) -> String {
        let mut quoted = String::from("\"");
        for c in arg.chars() {
            match c {
                '"' | '`' | '$' | '\\' => {
                    quoted.push('\\');
                    quoted.push(c);
                }
                '%' => quoted.push_str("%%"),
                _ => quoted.push(c),
            }
        }
        quoted.push('"');
        quoted
    }

    /// String value escapes from the Desktop Entry spec.
    fn escape_value(value: &str) -> String {
        value
            .replace('\\', "\\\\")
            .replace('\n', "\\n")
            .replace('\t', "\\t")
            .replace('\r', "\\r")
    }
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::platform::{desktop_entry, desktop_id};
    use super::*;

    #[test]
    fn desktop_id_is_plain_ascii() {
        assert_eq!(desktop_id("AzerothCore"), "azerothcore-launcher");
        assert_eq!(desktop_id("My  Server: 3.3.5"), "my-server-3-3-5-launcher");
        assert_eq!(desktop_id("***"), "game-launcher");
    }

    #[test]
    fn desktop_entry_quotes_the_path() {
        let entry = desktop_entry(
            "AzerothCore",
            Path::new("/home/a b/Games/50% \"x\"/launcher"),
            "ac-launcher",
        );
        assert!(
            entry.contains("Exec=\"/home/a b/Games/50%% \\\\\"x\\\\\"/launcher\"\n"),
            "{entry}"
        );
        assert!(entry.contains("Path=/home/a b/Games/50% \"x\"\n"), "{entry}");
        assert!(entry.contains("Name=AzerothCore\n"));
        assert!(entry.contains("Icon=ac-launcher\n"));
    }

    #[test]
    fn file_name_drops_invalid_characters() {
        assert_eq!(file_name("Azeroth: Core?"), "Azeroth Core");
        assert_eq!(file_name(" / "), "Launcher");
    }
}
