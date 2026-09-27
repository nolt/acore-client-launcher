# acore-client-launcher

Cross-platform (Windows + Linux) launcher and patch updater for a WoW 3.3.5a client
connecting to an [AzerothCore](https://www.azerothcore.org/) server.

**Status: early work — UI mockup only.**

## Goals

- Single self-contained binary per platform — nothing to install, UI and artwork embedded.
- Background art, custom title bar, progress bar with speed and time left, Play button;
  re-skinnable per server via a branding layer.
- Game-agnostic: downloads exactly what the manifest on the patch host lists, never deletes
  files, ignores anything not in the manifest.
- First run: asks where to install, moves itself there and creates a shortcut.
- Tools: clear cache folders (on request only), full file check, open the game folder.

## Stack

Rust + [Slint](https://slint.dev/) (UI compiled into the binary), `reqwest`/`tokio`, `serde`,
`sha2`, `self-replace`. Windows build cross-compiled from Linux
(`x86_64-pc-windows-gnu`) in Docker.
