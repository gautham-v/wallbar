//! What the menu bar app does, over `wallbar-core`: the same commands the CLI
//! runs, plus the one thing only a long-running process needs.
//!
//! `NSWorkspace` answers `desktopImageURLForScreen:` with the *old* image for
//! a few hundred milliseconds after this process sets a new one, so a
//! popover that re-read the desktop straight after "next" would flick back.
//! [`AppDesktop`] remembers what it set and reports that until the system
//! catches up (or a few seconds pass, in case something else changed it).

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Result;
use objc2::MainThreadMarker;
use objc2_app_kit::NSApplication;
use wallbar_core::macos::MacDesktop;
use wallbar_core::wallbar::ClockRandom;
use wallbar_core::{Appearance, Desktop, Painting, Wallbar};

use crate::thumbs::Thumbs;

/// How long our own set is trusted over what `NSWorkspace` reports.
const PENDING_FOR: Duration = Duration::from_secs(3);

pub struct AppDesktop {
    mac: MacDesktop,
    mtm: MainThreadMarker,
    pending: RefCell<Option<(PathBuf, Instant)>>,
}

impl Desktop for AppDesktop {
    fn current(&self) -> Result<Option<PathBuf>> {
        let real = self.mac.current()?;
        let mut pending = self.pending.borrow_mut();
        if let Some((path, at)) = pending.as_ref() {
            if at.elapsed() < PENDING_FOR && real.as_ref() != Some(path) {
                return Ok(Some(path.clone()));
            }
            *pending = None;
        }
        Ok(real)
    }

    fn set(&self, path: &Path) -> Result<()> {
        self.mac.set(path)?;
        *self.pending.borrow_mut() = Some((path.to_owned(), Instant::now()));
        Ok(())
    }

    /// The app has an `NSApp`, whose effective appearance follows the
    /// system live; the defaults key the CLI reads can lag in a long-running
    /// process.
    fn appearance(&self) -> Appearance {
        let name = NSApplication::sharedApplication(self.mtm)
            .effectiveAppearance()
            .name()
            .to_string();
        if name.contains("Dark") {
            Appearance::Dark
        } else {
            Appearance::Light
        }
    }
}

/// Which way the popover, the keys or a scroll asked to go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    Next,
    Prev,
    Shuffle,
}

/// Everything the popover draws that is not the thumbnail.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub painting: Option<Painting>,
    pub match_appearance: bool,
    /// Why there is no painting, when there is none.
    pub problem: Option<String>,
}

pub struct Controller {
    wallbar: Wallbar<AppDesktop>,
    pub thumbs: Thumbs,
    random: RefCell<ClockRandom>,
}

impl Controller {
    pub fn new(mtm: MainThreadMarker) -> Self {
        let desktop = AppDesktop {
            mac: MacDesktop::new(mtm),
            mtm,
            pending: RefCell::new(None),
        };
        Controller {
            wallbar: Wallbar::new(
                wallbar_core::default_dir(),
                wallbar_core::default_state_path(),
                desktop,
            ),
            thumbs: Thumbs::new(),
            random: RefCell::new(ClockRandom::new()),
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        let match_appearance = self.wallbar.match_appearance();
        match self.wallbar.current(false) {
            Ok(painting) => Snapshot {
                painting: Some(painting),
                match_appearance,
                problem: None,
            },
            Err(err) => Snapshot {
                painting: None,
                match_appearance,
                problem: Some(format!("{err:#}")),
            },
        }
    }

    pub fn step(&self, nav: Nav) -> Result<Painting> {
        match nav {
            Nav::Next => self.wallbar.next(false),
            Nav::Prev => self.wallbar.prev(false),
            Nav::Shuffle => self.wallbar.shuffle(false, &mut *self.random.borrow_mut()),
        }
    }

    pub fn set_match_appearance(&self, on: bool) -> Result<()> {
        self.wallbar.set_match_appearance(on)
    }

    /// Every image in the folder, for warming the thumbnail cache.
    pub fn all_images(&self) -> Vec<PathBuf> {
        wallbar_core::folder::list(&self.wallbar.dir)
    }
}
