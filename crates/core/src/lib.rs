//! Launcher core: no GUI, usable from the CLI and from tests.
//!
//! The launcher is game-agnostic. It downloads exactly what the manifest on the
//! patch host lists, never deletes game files and ignores anything the manifest
//! does not mention.

mod cache;
mod download;
mod error;
pub mod format;
mod generate;
mod hash;
mod install;
mod launch;
mod manifest;
mod progress;
mod shortcut;
mod state;
mod update;

pub use cache::clear_cache;
pub use error::{Error, Result};
pub use generate::{GenerateOptions, Generated, generate, today_version};
pub use install::{check_writable, install, remove_original};
pub use launch::{LaunchError, check_started, launch};
pub use manifest::{Manifest, ManifestFile};
pub use progress::Progress;
pub use reqwest::Url;
pub use shortcut::{Shortcut, create_shortcuts};
pub use state::{LocalState, STATE_FILE};
pub use update::{UpdateOptions, UpdateSummary, update};
