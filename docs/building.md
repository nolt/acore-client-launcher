# Building the launcher

*[Wersja polska](building.pl.md)*

How to build the launcher, where the files land and how to try it against a local web server
before publishing. For what to change before a build see [Branding](branding.md); for putting
the result on your host see [Releasing updates](releasing-updates.md).

## Prerequisites

- **Docker.** Everything builds inside `docker/Dockerfile.build` (based on `rust:1-bookworm`, with
  MinGW for the Windows exe). Nothing else is needed on the build machine — no Rust toolchain.
- About **6 GB** of free disk space for the image and the compiled dependencies in `target/`.

The Linux binary links against glibc 2.36 (Debian 12), so it runs on that and every newer
distribution.

## `scripts/build.sh`

```bash
scripts/build.sh
```

It prints one line per stage, a dot every 15 seconds while a stage runs, and the files at the end:

```
Building acore-launcher. The first build takes several minutes, later ones are quick.
[1/4] Preparing the build environment . done (23 s)
[2/4] Downloading dependencies  done (13 s)
[3/4] Compiling for Linux ................ done (4 min 13 s)
[4/4] Compiling for Windows ........... done (2 min 52 s)

Done in 7 min 41 s. Files in /path/to/acore-client-launcher/out/dist:
  acore-launcher              16M  launcher for Linux players
  acore-launcher.exe          13M  launcher for Windows players
  acore-launcher-cli         2.3M  manifest and launcher.json tool (Linux)
  acore-launcher-cli.exe     2.2M  manifest and launcher.json tool (Windows)
Full log: /path/to/acore-client-launcher/out/build.log
```

- The **first** build takes several minutes: it builds the image and compiles every dependency
  for both platforms. Later builds reuse both and only recompile what changed, usually in a minute
  or two.
- The full compiler output goes to `out/build.log`. If a stage fails, the script shows its last
  30 lines and stops.

## What goes to players

| File in `out/dist/` | For |
|---|---|
| `acore-launcher` | Players on Linux (x86-64) |
| `acore-launcher.exe` | Players on Windows 10/11 (x86-64) |
| `acore-launcher-cli`, `acore-launcher-cli.exe` | You: generates `version.json` and `launcher.json`, runs an update from the command line |

Players get only the launcher for their system — nothing else. Licenses are built into it
(About → Licenses). The launcher installs itself on the first start, see the
[Player guide](player-guide.md).

The name of the binaries is set in `crates/app/Cargo.toml`; see [Branding](branding.md#binary-name).

## Test it locally before publishing

You can try the whole cycle — install, download, PLAY — against a throw-away web server, without
touching the real host and without rebuilding for a test URL.

```bash
# 1. A folder with the client as players should have it; generate its manifest.
out/dist/acore-launcher-cli manifest ~/test-host

# 2. Serve it over HTTP (Python's built-in server is enough for a test).
cd ~/test-host && python3 -m http.server 8000

# 3. In another terminal, start the launcher with the test URL. It must point at version.json.
LAUNCHER_MANIFEST_URL=http://localhost:8000/version.json out/dist/acore-launcher
```

The launcher asks where to install, moves itself there and downloads the files. Things to know:

- Python's server does not support resuming (`Range`). After **Pause → Resume** the file that was
  in progress starts from zero; Apache and nginx continue where it stopped.
- The shortcuts the launcher creates do not know about `LAUNCHER_MANIFEST_URL`; started from a
  shortcut it uses the built-in URL. Start it from the terminal while testing.
- With `LAUNCHER_GAME_DIR=/some/folder` it skips the install dialog and uses that folder as the
  game folder.

### Environment variables for testing

| Variable | Effect |
|---|---|
| `LAUNCHER_MANIFEST_URL` | Overrides `MANIFEST_URL` from `config.rs`. Full URL of `version.json`. |
| `LAUNCHER_GAME_DIR` | Uses this game folder instead of the launcher's own and skips the first-run install. |
| `MOCK=<state>` | Shows a fixed screen with no network: `downloading`, `paused`, `ready`, `no-wine`, `error`, `install`, `install-error`, `tools`, `clear-cache`, `about`, `licenses`. |
| `SLINT_BACKEND=winit-software` | Forces the software renderer (for machines without OpenGL). |
| `LANG=pl_PL.UTF-8` | UI language on Linux (the launcher follows the system language). |

### Updating from the command line

`acore-launcher-cli update` does what the launcher does, without the window, and prints the exact
error when something fails — the quickest way to debug a host:

```bash
out/dist/acore-launcher-cli update http://localhost:8000/version.json ~/test-game
out/dist/acore-launcher-cli update http://localhost:8000/version.json ~/test-game --full-check
```

## Development

Everything runs in the build image:

```bash
docker build -t acore-launcher-build -f docker/Dockerfile.build docker
run() { docker run --rm -v "$PWD":/src -w /src -e CARGO_HOME=/src/target/cargo-home acore-launcher-build "$@"; }

run cargo test --workspace
run cargo clippy --workspace --all-targets
run cargo fmt --all
```

### Scripts

| Script | What it does |
|---|---|
| `scripts/build.sh` | Release binaries for Linux and Windows in `out/dist/`. |
| `scripts/mockup-shots.sh` | Screenshots of every `MOCK` state: PNGs in `docs/mockups/`, and the JPGs the docs use in `docs/assets/`. `UI_LANG=pl_PL scripts/mockup-shots.sh` takes the Polish ones (`*.pl.jpg`). Run both after changing the look. |
| `scripts/licenses.sh` | Regenerates `THIRD-PARTY-LICENSES.md` with `cargo-about`. Run it after adding or updating dependencies — the file is compiled into the launcher. |
| `scripts/wine-test.sh` | Installs the Windows exe from `Downloads` under Wine 10 in a throw-away container; screenshots and the resulting files go to `out/wine-test/`. |

### Translations

See [Branding → Language](branding.md#language).
