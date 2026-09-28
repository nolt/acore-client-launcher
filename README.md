# acore-client-launcher

Launcher and patch updater for a game client, for Linux and Windows. Built for a WoW 3.3.5a
client connecting to an [AzerothCore](https://www.azerothcore.org/) server, but the launcher
itself knows nothing about WoW: it downloads what a manifest on your file host lists and starts
one program.

One self-contained binary per platform: the UI, fonts and artwork are compiled in, nothing to
install. Written in Rust with [Slint](https://slint.dev/).

- [What the player gets](#what-the-player-gets)
- [Rules the launcher follows](#rules-the-launcher-follows)
- [Setting it up for your server](#setting-it-up-for-your-server)
- [Publishing game files and launcher updates](#publishing-game-files-and-launcher-updates)
- [File formats](#file-formats)
- [Files the launcher creates](#files-the-launcher-creates)
- [Platform notes](#platform-notes)
- [Development](#development)
- [Known issues](#known-issues)
- [License](#license)

## What the player gets

**First start.** The player downloads the launcher and starts it, e.g. from `Downloads`. The
launcher asks where to install the game: it suggests `~/Games/<name>` on Linux and
`C:\Games\<name>` on Windows. The player can type another path or pick a folder with
**Browse…**. On **Install** the launcher:

1. creates the folder and checks it can write there. A folder without write permission, such as
   `/opt` or `C:\Program Files`, gets a message asking for another one. The launcher never asks
   for admin rights;
2. copies itself into the folder and starts the copy;
3. creates shortcuts, if the box is ticked: the application menu and desktop on Linux, Start Menu
   and desktop on Windows;
4. deletes the original, e.g. from `Downloads`, once it has closed.

If the player picks the folder the launcher already sits in, nothing is moved. A folder that
already holds the game is fine: only missing or outdated files are downloaded.

**Every start.**

1. **Launcher update.** If a newer launcher is published, it downloads it, replaces itself and
   restarts.
2. **Game files.** It fetches the manifest, checks the game files and downloads what is missing
   or changed, one file at a time. The status line shows the file count, bytes, speed and time
   left. Pause stops the download and Resume continues where it stopped; the same button offers
   Retry when an update fails. The manifest and every file are retried a few times when the
   connection drops or a response arrives cut short.
3. **PLAY.** Once the files are up to date, PLAY starts the game. The launcher waits 3 seconds
   and closes if the game is still running. If the game quits within that time, the launcher
   stays open and says so.

**Tools** (gear icon, top left):

| Tool | What it does |
|---|---|
| **Clear cache** | Deletes the cache folders from the config (`Cache/` by default), after asking. The game rebuilds them. |
| **Check files** | Computes the SHA-256 of every game file instead of trusting size and date, then downloads damaged files again. |
| **Open game folder** | Opens the folder in the file manager. |
| **About** | Launcher version and the Slint attribution. |

The UI is in English and Polish. The language follows the system language.

## Rules the launcher follows

1. **It only knows the manifest.** No game-specific logic. A `realmlist.wtf` or a translation
   patch is just another file in the manifest.
2. **It never deletes game files.** It only adds missing files and replaces outdated ones. The
   only exception is Clear cache, and only after the player confirms.
3. **Files not in the manifest do not exist for it.** The player's own addons, patches,
   screenshots and settings are never touched.
4. **No server status, no pings.** It downloads and starts the game, nothing else.
5. **Downloads never corrupt a file.** Each file downloads to `<file>.part` and resumes after an
   interruption. The launcher checks size and SHA-256 and only then replaces the real file. A bad
   download leaves the old file as it was.

## Setting it up for your server

Two files decide how the launcher behaves and how it looks. Both are compiled in, so rebuild
after changing them.

### Behaviour: `crates/app/src/config.rs`

| Constant | Default | Meaning |
|---|---|---|
| `MANIFEST_URL` | `https://patch.example.org/version.json` | Full URL of the manifest on your file host. **Set this.** |
| `LAUNCHER_INFO` | `launcher.json` | Self-update file, relative to the manifest URL. If it is missing on the host, there is no self-update. |
| `GAME_PROGRAM` | `Wow.exe` | What PLAY starts, relative to the game folder. On Linux a `.exe` runs through Wine. |
| `CACHE_DIRS` | `["Cache"]` | Folders Clear cache deletes. An empty list hides the button. |
| `DOWNLOAD_CONCURRENCY` | `1` | Files downloaded at the same time. One by one keeps many players from saturating the host. |

### Looks: `crates/app/ui/brand.slint` and `crates/app/ui/assets/`

- `brand.slint` holds the server name and subtitle in the title bar, the plain name used for
  shortcuts and the install folder (`app-name`), the window size, all colours and the fonts.
  `title-mode` picks text, `assets/logo.png` or nothing, for when the artwork already has the
  title.
- `assets/background.jpg` is the background art.
- `assets/icon.png` is the window and Linux menu icon. `assets/icon.ico` is the Windows exe icon
  (sizes 16–256).
- `assets/fonts/` holds the fonts: Cinzel for titles and Inter for text. Both use the SIL Open
  Font License.

## Publishing game files and launcher updates

### Build

```bash
scripts/build.sh
```

This builds everything in Docker (`docker/Dockerfile.build`, based on `rust:1-bookworm`) and
puts the results in `out/dist/`. It prints one line per stage and lists the files at the end; the
full compiler output goes to `out/build.log`, and the last lines of it are shown if a stage fails.
The first run takes several minutes: it builds the image and compiles all dependencies for both
platforms. Later runs reuse both and only rebuild what changed.

| File | For |
|---|---|
| `acore-launcher` | Players on Linux (x86-64, glibc 2.36 or newer) |
| `acore-launcher.exe` | Players on Windows 10/11 (x86-64) |
| `acore-launcher-cli`, `acore-launcher-cli.exe` | You: generates `version.json` and `launcher.json` |

### Game files

Keep a local folder with the game exactly as players should have it, then generate the manifest:

```bash
out/dist/acore-launcher-cli manifest /path/to/game-files
# /path/to/game-files/version.json: version 2026.09.28, 3412 files, 17.1 GB
```

Upload the folder together with `version.json` to the host so that `MANIFEST_URL` points at
`version.json`. Options:

- `--version V` sets the version shown to players as "Updated to V". The default is today's date.
- `--base-url URL` is the folder the files are served from, relative to the manifest. By default
  they sit next to it.
- `--output FILE` writes the manifest somewhere else.

Hidden files and folders (names starting with `.`) are left out, because web servers usually
refuse to serve them. To publish an update, change the files, generate the manifest again and
upload both. Upload the files first and the manifest last, so players never see a manifest that
points at files that are not there yet.

### Launcher updates

1. Raise `version` in the root `Cargo.toml` (e.g. `0.1.0` → `0.2.0`) and run
   `scripts/build.sh`.
2. Put the new binaries in a folder on the host next to `version.json`, e.g. `launcher/`, and
   generate `launcher.json` next to `version.json`:

   ```bash
   acore-launcher-cli launcher 0.2.0 \
       --linux host/launcher/acore-launcher \
       --windows host/launcher/acore-launcher.exe \
       --output host/launcher.json
   ```

   Paths in `launcher.json` are stored relative to it (`launcher/acore-launcher`).
3. Upload the binaries first, then `launcher.json`.

At the next start every launcher older than the published version updates itself. Keep the
launcher binaries out of the game-files folder, otherwise they end up in the manifest.

## File formats

### `version.json` (game manifest)

```json
{
  "version": "2026.09.28",
  "generatedAtUtc": "2026-09-28T12:00:00Z",
  "baseUrl": "",
  "files": [
    { "path": "Data/plPL/patch-plPL-4.MPQ", "hash": "<sha-256, lowercase hex>", "size": 163053568 }
  ]
}
```

- `path` is relative to the game folder and uses `/`. The launcher rejects manifests with `..`,
  absolute paths, `\`, `:` or paths that differ only in letter case.
- `hash` alone decides whether a file is downloaded. `size` is a quick first check.
- `baseUrl` is where the files live relative to the manifest. Empty means next to it.

### `launcher.json` (self-update)

```json
{
  "version": "0.2.0",
  "files": {
    "linux-x64":   { "path": "launcher/acore-launcher",     "hash": "<sha-256>", "size": 16479920 },
    "windows-x64": { "path": "launcher/acore-launcher.exe", "hash": "<sha-256>", "size": 13176320 }
  }
}
```

A launcher updates itself only when `version` is higher than its own, compared number by number
(`0.10.0` > `0.9.3`), and when its platform has an entry.

## Files the launcher creates

| Where | What |
|---|---|
| `<game>/.launcher-state.json` | Size, date and hash of every checked file, so the next start does not hash unchanged files again. It also marks the folder as installed. |
| `<game>/<file>.part` | Unfinished download, resumed next time. |
| `<game>/.launcher-update` | New launcher during a self-update. It is removed after the swap. |
| `<game>/launcher.local.json` | Not created by the launcher: optional Wine settings the player writes on Linux (see Platform notes). Never part of a manifest. |
| Linux: `~/.local/share/applications/<name>-launcher.desktop`, `~/.local/share/icons/hicolor/256x256/apps/<name>-launcher.png`, desktop copy | Shortcuts. |
| Windows: `Start Menu\Programs\<name>.lnk`, `Desktop\<name>.lnk` | Shortcuts. |

## Platform notes

**Linux.**

- **Game start.** A Windows game (`GAME_PROGRAM` ending in `.exe`) starts through `wine` from
  `PATH`, in the Wine prefix from the environment: `WINEPREFIX` if set, otherwise `~/.wine`.
  Without Wine, PLAY shows a message.
- **Own Wine prefix or Wine build.** For a prefix of its own, or another Wine (`wine64`, a full
  path), put `launcher.local.json` in the game folder. Both fields are optional, `~/` means the
  home folder, and the file also works from a desktop shortcut, where no `WINEPREFIX` is set:

  ```json
  { "winePrefix": "~/.wine-azerothcore", "wineCommand": "wine" }
  ```
- **Rendering.** The window uses OpenGL (FemtoVG). Without OpenGL Slint falls back to its software
  renderer, which draws the gradients without rounded corners.
- **Browse….** The folder dialog goes through the XDG desktop portal
  (`xdg-desktop-portal`), which every common desktop has.
- **Desktop shortcut.** It is made executable and, on GNOME, marked trusted with `gio`.

**Windows.** Needs Windows 10 or 11, x86-64. The exe has an `asInvoker` manifest, so Windows does
not ask for admin rights. It is not code-signed, so SmartScreen shows "Unknown publisher" on the
first start ("More info" → "Run anyway").

## Development

Everything runs in the build image. Nothing needs to be installed on the host except Docker:

```bash
docker build -t acore-launcher-build -f docker/Dockerfile.build docker
run() { docker run --rm -v "$PWD":/src -w /src -e CARGO_HOME=/src/target/cargo-home acore-launcher-build "$@"; }

run cargo test --workspace
run cargo clippy --workspace --all-targets
run cargo fmt --all
```

### Workspace

| Crate | Contents |
|---|---|
| `crates/core` (`launcher-core`) | Everything except the window: manifest, file check, resumable downloads, manifest generator, install, shortcuts, game start, cache, self-update. |
| `crates/cli` (`acore-launcher-cli`) | `manifest`, `launcher` and `update` (an update without the window, useful on a server). |
| `crates/app` (`acore-launcher`) | The Slint window: `ui/*.slint`, and in `src/` the glue between the window and `core`. |

### Environment variables for testing

| Variable | Effect |
|---|---|
| `LAUNCHER_MANIFEST_URL` | Overrides `MANIFEST_URL`, e.g. a local `python3 -m http.server`. |
| `LAUNCHER_GAME_DIR` | Uses this game folder instead of the launcher's own, and skips the first-run install. |
| `MOCK=<state>` | Shows a fixed screen with no network: `downloading`, `paused`, `ready`, `no-wine`, `error`, `install`, `install-error`, `tools`, `clear-cache`, `about`. |
| `SLINT_BACKEND=winit-software` | Forces the software renderer. |

### Scripts

- `scripts/build.sh` makes the release binaries in `out/dist/`.
- `scripts/mockup-shots.sh` saves a screenshot of every `MOCK` state to `docs/mockups/`.
  `UI_LANG=pl_PL scripts/mockup-shots.sh` takes them in Polish.
- `scripts/wine-test.sh` installs the Windows exe from `Downloads` under Wine in a throw-away
  container. It saves screenshots and the resulting files to `out/wine-test/`.

### Translations

The texts live only in `.slint` files, wrapped in `@tr(...)`. Translations are gettext `.po` files
in `crates/app/lang/<language>/LC_MESSAGES/acore-launcher.po`, compiled into the binary.

- **Add a language.** Copy `crates/app/lang/acore-launcher.pot` to
  `lang/<code>/LC_MESSAGES/acore-launcher.po`, translate it and rebuild.
- **Refresh the template after changing texts:**

  ```bash
  run cargo install slint-tr-extractor
  run target/cargo-home/bin/slint-tr-extractor crates/app/ui/*.slint -o crates/app/lang/acore-launcher.pot
  ```

  Then merge it into the `.po` files, e.g. with `msgmerge`.

## Known issues

- **Wine on a virtual display.** Slint 1.18 crashes (divide by zero) when the monitor reports a
  0 Hz refresh rate. This happens under Wine on Xvfb, not on a real monitor. `scripts/wine-test.sh`
  uses an Xorg dummy display at 60 Hz for this reason.
- **Wine 8.** It lacks `bcryptprimitives.dll`, which Rust programs need, so the exe does not
  start under Wine 8. Wine 9 and newer are fine. Real Windows 10/11 always has it.

## License

- **Launcher.** MIT, see [LICENSE](LICENSE).
- **Slint.** Used under its Royalty-free license, whose condition is met by the About dialog
  (`AboutSlint`).
- **Fonts.** Cinzel and Inter, SIL Open Font License, texts in `crates/app/ui/assets/fonts/`.
- **Artwork.** The background is a placeholder. Replace it with art you have the rights to.
