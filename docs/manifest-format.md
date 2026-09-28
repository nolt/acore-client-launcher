# Manifest format

*[Wersja polska](manifest-format.pl.md)*

Reference for the files the launcher reads from your host and keeps in the player's game folder.
You normally generate the host files with `acore-launcher-cli`; see
[Releasing updates](releasing-updates.md).

## `version.json` — game manifest

```json
{
  "version": "2026.09.28",
  "generatedAtUtc": "2026-09-28T12:00:00Z",
  "baseUrl": "",
  "files": [
    { "path": "Data/plPL/patch-plPL-4.MPQ", "hash": "<sha-256, lowercase hex>", "size": 163053568 },
    { "path": "Wow.exe", "hash": "<sha-256>", "size": 7704216 }
  ]
}
```

| Field | Required | Meaning |
|---|---|---|
| `version` | yes | Shown to players as "Updated to *version*". Any text; it does not decide anything. |
| `generatedAtUtc` | no | When the manifest was generated. Informational. |
| `baseUrl` | no | Where the files are served from, relative to the manifest (or a full URL). Empty = next to `version.json`. |
| `files[].path` | yes | Path relative to the game folder, with `/`. Spaces and `#` are fine; they are encoded in the URL. |
| `files[].hash` | yes | SHA-256 of the file, 64 hex digits. **The hash alone decides whether a file is downloaded.** |
| `files[].size` | yes | Size in bytes — a quick first check before hashing. |

The download URL of a file is the manifest's folder + `baseUrl` + `path`:
`https://example.org/patch/version.json` + `Data/plPL/patch-plPL-4.MPQ` →
`https://example.org/patch/Data/plPL/patch-plPL-4.MPQ`.

**Rejected manifests.** To keep downloads inside the game folder, the launcher refuses the whole
manifest ("The update server sent an invalid file list") if a path:

- is absolute, contains `..`, `.`, an empty segment (`a//b`), `\`, `:` or a NUL character;
- differs from another path only in letter case (`Data/a.txt` and `data/A.txt` are the same file
  on Windows and macOS);
- is reserved for the launcher: `.launcher-state.json`, `launcher.local.json` or anything ending in
  `.part` (any letter case);

or if a hash is not 64 hex digits.

## `launcher.json` — self-update

```json
{
  "version": "0.2.0",
  "files": {
    "linux-x64":   { "path": "launcher/acore-launcher",     "hash": "<sha-256>", "size": 16479920 },
    "windows-x64": { "path": "launcher/acore-launcher.exe", "hash": "<sha-256>", "size": 13176320 }
  }
}
```

- Lies next to `version.json` (the name is `LAUNCHER_INFO` in `config.rs`).
- `path` is relative to `launcher.json`.
- A launcher updates itself only when `version` is **higher** than its own, compared number by
  number (`0.10.0` > `0.9.3`; anything that is not plain numbers never counts as newer), and when
  its platform has an entry.
- Missing on the host (404) = no self-update, and the launcher goes on to the game files.

## `launcher.local.json` — player's Wine settings (Linux)

Optional, written by the player, in the game folder. The launcher never creates or downloads it,
and a manifest may not list it.

```json
{ "winePrefix": "~/.wine-azerothcore", "wineCommand": "wine" }
```

| Field | Default | Meaning |
|---|---|---|
| `winePrefix` | the environment's `WINEPREFIX`, else `~/.wine` | Wine prefix the game runs in. `~/` means the home folder. |
| `wineCommand` | `wine` | Wine binary: `wine`, `wine64` or a full path. |

Other fields are ignored. A broken file does not block PLAY: the launcher falls back to the
defaults.

## Files in the game folder

| File | What it is |
|---|---|
| `.launcher-state.json` | Size, modification time and hash of every checked file, so the next start does not hash unchanged files again. It also marks the folder as installed — without it the launcher shows the install dialog. Safe to delete: the next start hashes everything once. |
| `<file>.part` | An unfinished download. The next attempt continues it; it becomes the real file only after its size and hash match. |
| `.launcher-update` | A new launcher during a self-update; removed after the swap. |
| `launcher.local.json` | The player's Wine settings, see above. |
| `acore-launcher`, `acore-launcher.exe` | The launcher itself — it lives in the game folder. |

Shortcuts, outside the game folder:

| System | Files |
|---|---|
| Linux | `~/.local/share/applications/<name>-launcher.desktop`, `~/.local/share/icons/hicolor/256x256/apps/<name>-launcher.png`, and a copy of the `.desktop` file on the desktop |
| Windows | `Start Menu\Programs\<app-name>.lnk`, `Desktop\<app-name>.lnk` |
