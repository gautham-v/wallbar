//! Everything wallbar knows about wallpapers, shared by the menu bar app and
//! the CLI (they are the same binary; see `crates/app`).
//!
//! - [`painting`]: filenames, `paintings.json`, and the painting object the
//!   CLI prints.
//! - [`folder`]: listing the wallpaper folder in folder order.
//! - [`pool`]: which images are in play, and next / previous / shuffle over
//!   them. Pure functions over indices, so the ordering rules are tested
//!   without a desktop.
//! - [`state`]: history and the match-appearance switch on disk.
//! - [`wallbar`]: the commands themselves, over a [`Desktop`] and a
//!   [`Random`] that tests replace.
//! - [`gesture`]: turning a stream of scroll deltas into single steps.
//! - [`macos`]: the real desktop — `NSWorkspace` on every `NSScreen`.
//!
//! The contract all of this implements is `docs/cli.md`.

pub mod folder;
pub mod gesture;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod painting;
pub mod pool;
pub mod state;
pub mod wallbar;

pub use painting::{Details, Mood, Painting};
pub use pool::Appearance;
pub use state::State;
pub use wallbar::{Desktop, Listing, Random, Wallbar};

use std::path::PathBuf;

/// The wallpaper folder: `$WALLBAR_DIR`, else `~/Pictures/wallpapers`.
pub fn default_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("WALLBAR_DIR").filter(|v| !v.is_empty()) {
        return PathBuf::from(dir);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .join("Pictures")
        .join("wallpapers")
}

/// `~/Library/Application Support/wallbar/state.json`.
pub fn default_state_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("wallbar")
        .join("state.json")
}
