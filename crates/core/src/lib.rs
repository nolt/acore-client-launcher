//! Launcher core: no GUI, usable from the CLI and from tests.
//!
//! The launcher is game-agnostic. It downloads exactly what the manifest on the
//! patch host lists, never deletes game files and ignores anything the manifest
//! does not mention.

mod download;
mod error;
pub mod format;
mod generate;
mod hash;
mod manifest;
mod progress;
mod state;
mod update;

pub use error::{Error, Result};
pub use generate::{GenerateOptions, Generated, generate, today_version};
pub use manifest::{Manifest, ManifestFile};
pub use progress::Progress;
pub use reqwest::Url;
pub use state::{LocalState, STATE_FILE};
pub use update::{UpdateOptions, UpdateSummary, update};
