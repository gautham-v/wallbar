//! The real desktop: `NSWorkspace` desktop images on every `NSScreen`.
//!
//! On macOS 15 `setDesktopImageURL:forScreen:options:` updates the display's
//! wallpaper for every Space on it, not just the Space in front (see the
//! README), so setting each screen is all it takes.
//!
//! Two things worth knowing about reading the wallpaper back:
//!
//! - In the process that just set it, `desktopImageURLForScreen:` keeps
//!   returning the old image for a few hundred milliseconds. A fresh process
//!   sees the new one straight away, so the CLI is fine; the menu bar app
//!   remembers what it set (see `crates/app`).
//! - A long-running process does see changes made by other processes, so the
//!   popover can poll it.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use objc2::rc::Retained;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSScreen, NSWorkspace};
use objc2_foundation::{NSString, NSUserDefaults, NSURL};

use crate::pool::Appearance;
use crate::wallbar::Desktop;

/// The desktop of this Mac. AppKit's screen list is main-thread only, so
/// this needs the marker; both the CLI and the app run commands on the main
/// thread.
#[derive(Clone, Copy)]
pub struct MacDesktop {
    mtm: MainThreadMarker,
}

impl MacDesktop {
    pub fn new(mtm: MainThreadMarker) -> Self {
        MacDesktop { mtm }
    }
}

impl Desktop for MacDesktop {
    fn current(&self) -> Result<Option<PathBuf>> {
        Ok(current_wallpaper(self.mtm))
    }

    fn set(&self, path: &Path) -> Result<()> {
        set_wallpaper(self.mtm, path)
    }

    fn appearance(&self) -> Appearance {
        system_appearance()
    }
}

/// The main display's wallpaper, when it is a file.
pub fn current_wallpaper(mtm: MainThreadMarker) -> Option<PathBuf> {
    let screen = NSScreen::mainScreen(mtm).or_else(|| NSScreen::screens(mtm).firstObject())?;
    let url = NSWorkspace::sharedWorkspace().desktopImageURLForScreen(&screen)?;
    url_path(&url)
}

/// Show `path` on every display, keeping each display's own scaling options.
pub fn set_wallpaper(mtm: MainThreadMarker, path: &Path) -> Result<()> {
    let path = std::path::absolute(path)?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
    let workspace = NSWorkspace::sharedWorkspace();
    let screens = NSScreen::screens(mtm);
    if screens.count() == 0 {
        return Err(anyhow!("no displays to set a wallpaper on"));
    }
    for screen in screens.iter() {
        let options = workspace
            .desktopImageOptionsForScreen(&screen)
            .unwrap_or_default();
        unsafe { workspace.setDesktopImageURL_forScreen_options_error(&url, &screen, &options) }
            .map_err(|e| anyhow!("could not set the wallpaper: {}", e.localizedDescription()))?;
    }
    Ok(())
}

/// Dark when the global `AppleInterfaceStyle` is "Dark" — which macOS keeps
/// current in Auto mode too — else light.
pub fn system_appearance() -> Appearance {
    let defaults = NSUserDefaults::standardUserDefaults();
    let style = defaults.stringForKey(&NSString::from_str("AppleInterfaceStyle"));
    match style {
        Some(s) if s.to_string().eq_ignore_ascii_case("dark") => Appearance::Dark,
        _ => Appearance::Light,
    }
}

fn url_path(url: &Retained<NSURL>) -> Option<PathBuf> {
    if !url.isFileURL() {
        return None;
    }
    url.path().map(|p| PathBuf::from(p.to_string()))
}
