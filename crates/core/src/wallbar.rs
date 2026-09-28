//! The commands in `docs/cli.md`, over a [`Desktop`] and a [`Random`].
//!
//! Each command reads the folder, `paintings.json` and the state file fresh,
//! so the menu bar app and the CLI can both drive the same desktop without
//! either holding a stale copy.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Serialize;

use crate::folder;
use crate::painting::{Details, Painting};
use crate::pool::{self, Appearance};
use crate::state::State;

/// The desktop: what is showing, how to change it, and whether the Mac is
/// in dark mode. [`crate::macos::MacDesktop`] is the real one.
pub trait Desktop {
    /// The wallpaper on the main display, if it is a file.
    fn current(&self) -> Result<Option<PathBuf>>;
    /// Show `path` on every display.
    fn set(&self, path: &Path) -> Result<()>;
    fn appearance(&self) -> Appearance;
}

/// A source of choices for shuffle: a number in `0..n`.
pub trait Random {
    fn below(&mut self, n: usize) -> usize;
}

/// A small xorshift generator seeded from the clock and the pid — shuffle
/// needs variety, not cryptography, and this keeps `rand` out of the build.
pub struct ClockRandom(u64);

impl ClockRandom {
    pub fn new() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let seed = nanos ^ ((std::process::id() as u64) << 32) ^ 0x9E37_79B9_7F4A_7C15;
        ClockRandom(seed.max(1))
    }
}

impl Default for ClockRandom {
    fn default() -> Self {
        Self::new()
    }
}

impl Random for ClockRandom {
    fn below(&mut self, n: usize) -> usize {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        if n == 0 {
            0
        } else {
            (x % n as u64) as usize
        }
    }
}

/// What `wallbar list --json` prints.
#[derive(Debug, Clone, Serialize)]
pub struct Listing {
    /// The current wallpaper's filename, in the folder or not.
    pub current: Option<String>,
    pub items: Vec<Painting>,
}

/// The folder, its details and the state, read once per command.
struct Snapshot {
    files: Vec<PathBuf>,
    names: Vec<String>,
    details: HashMap<String, Details>,
    state: State,
}

pub struct Wallbar<D: Desktop> {
    pub dir: PathBuf,
    pub state_path: PathBuf,
    pub desktop: D,
}

impl<D: Desktop> Wallbar<D> {
    pub fn new(dir: PathBuf, state_path: PathBuf, desktop: D) -> Self {
        Wallbar {
            dir,
            state_path,
            desktop,
        }
    }

    fn snapshot(&self) -> Snapshot {
        let files = folder::list(&self.dir);
        let names = files
            .iter()
            .map(|p| {
                p.file_name()
                    .map(|f| f.to_string_lossy().into_owned())
                    .unwrap_or_default()
            })
            .collect();
        Snapshot {
            files,
            names,
            details: crate::painting::load_details(&self.dir),
            state: State::load(&self.state_path),
        }
    }

    fn pool(&self, snap: &Snapshot, all: bool) -> Vec<usize> {
        let matching = snap.state.match_appearance && !all;
        pool::select(&snap.names, matching, self.desktop.appearance())
    }

    fn painting(&self, snap: &Snapshot, path: &Path, pool: &[usize]) -> Painting {
        let index = folder::position(&snap.files, path);
        let details = index.map(|i| {
            snap.details
                .get(&snap.names[i])
                .cloned()
                .unwrap_or_default()
        });
        Painting::new(
            path,
            details.as_ref(),
            pool::position(pool, index),
            pool.len(),
        )
    }

    fn current_path(&self) -> Result<PathBuf> {
        self.desktop
            .current()?
            .context("the main display has no wallpaper image")
    }

    /// Whether the pool follows the system appearance.
    pub fn match_appearance(&self) -> bool {
        State::load(&self.state_path).match_appearance
    }

    /// `wallbar current`.
    pub fn current(&self, all: bool) -> Result<Painting> {
        let snap = self.snapshot();
        let pool = self.pool(&snap, all);
        Ok(self.painting(&snap, &self.current_path()?, &pool))
    }

    /// The painting object for any path, relative to the default pool — what
    /// the popover draws when it already knows what is showing.
    pub fn describe(&self, path: &Path) -> Painting {
        let snap = self.snapshot();
        let pool = self.pool(&snap, false);
        self.painting(&snap, path, &pool)
    }

    /// `wallbar list`: every image, index/count relative to the whole folder.
    pub fn list(&self) -> Result<Listing> {
        let snap = self.snapshot();
        let all: Vec<usize> = (0..snap.files.len()).collect();
        let current = self.desktop.current().ok().flatten();
        Ok(Listing {
            current: current
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|f| f.to_string_lossy().into_owned()),
            items: snap
                .files
                .iter()
                .map(|p| self.painting(&snap, p, &all))
                .collect(),
        })
    }

    /// `wallbar next`.
    pub fn next(&self, all: bool) -> Result<Painting> {
        let mut snap = self.snapshot();
        let pool = self.pool(&snap, all);
        let current = self.desktop.current()?;
        let at = current
            .as_deref()
            .and_then(|c| folder::position(&snap.files, c));
        let Some(target) = pool::next(&pool, at) else {
            bail!("no images in {}", self.dir.display());
        };
        let target = snap.files[target].clone();
        self.apply(&mut snap, current, &target, true)?;
        Ok(self.painting(&snap, &target, &pool))
    }

    /// `wallbar prev`: back through history, else the folder.
    pub fn prev(&self, all: bool) -> Result<Painting> {
        let mut snap = self.snapshot();
        let pool = self.pool(&snap, all);
        let current = self.desktop.current()?;
        let target = match snap.state.pop_usable(current.as_deref()) {
            Some(path) => path,
            None => {
                let at = current
                    .as_deref()
                    .and_then(|c| folder::position(&snap.files, c));
                let Some(i) = pool::prev(&pool, at) else {
                    bail!("no images in {}", self.dir.display());
                };
                snap.files[i].clone()
            }
        };
        self.apply(&mut snap, current, &target, false)?;
        Ok(self.painting(&snap, &target, &pool))
    }

    /// `wallbar shuffle`: anything in the pool but what is showing.
    pub fn shuffle(&self, all: bool, random: &mut dyn Random) -> Result<Painting> {
        let mut snap = self.snapshot();
        let pool = self.pool(&snap, all);
        let current = self.desktop.current()?;
        let at = current
            .as_deref()
            .and_then(|c| folder::position(&snap.files, c));
        let pick = pool::shuffle(&pool, snap.files.len(), at, &mut |n| random.below(n));
        let Some(i) = pick else {
            if snap.files.is_empty() {
                bail!("no images in {}", self.dir.display());
            }
            bail!("nothing to shuffle to: the only image is already showing");
        };
        let target = snap.files[i].clone();
        self.apply(&mut snap, current, &target, true)?;
        Ok(self.painting(&snap, &target, &pool))
    }

    /// `wallbar set <filename|path>`.
    pub fn set(&self, target: &str) -> Result<Painting> {
        let mut snap = self.snapshot();
        let pool = self.pool(&snap, false);
        let path = self.resolve(target)?;
        let current = self.desktop.current()?;
        self.apply(&mut snap, current, &path, true)?;
        Ok(self.painting(&snap, &path, &pool))
    }

    /// `wallbar match-appearance on|off`.
    pub fn set_match_appearance(&self, on: bool) -> Result<()> {
        let mut state = State::load(&self.state_path);
        state.match_appearance = on;
        state.save(&self.state_path)
    }

    /// A filename in the folder, or a path to any image.
    fn resolve(&self, target: &str) -> Result<PathBuf> {
        let in_folder = self.dir.join(target);
        let path = if !target.contains('/') && in_folder.is_file() {
            in_folder
        } else {
            let p = PathBuf::from(target);
            let p = if p.is_absolute() {
                p
            } else {
                std::env::current_dir()?.join(p)
            };
            if !p.is_file() {
                bail!("no such image: {target}");
            }
            p
        };
        let name = path.file_name().map(|f| f.to_string_lossy().into_owned());
        if !name.is_some_and(|n| folder::is_image(&n)) {
            bail!("not an image wallbar can use: {target}");
        }
        Ok(path)
    }

    /// Show `target`, then record the change: push what it replaced (for
    /// every command but prev, whose pop is already in `snap.state`) and save.
    fn apply(
        &self,
        snap: &mut Snapshot,
        current: Option<PathBuf>,
        target: &Path,
        push: bool,
    ) -> Result<()> {
        let unchanged = current.as_deref() == Some(target);
        if !unchanged {
            self.desktop.set(target)?;
        }
        if push && !unchanged {
            if let Some(current) = current {
                snap.state.push(current);
            }
        }
        snap.state.save(&self.state_path)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::cell::RefCell;

    /// A fresh, empty folder under the system temp dir.
    pub fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "wallbar-test-{}-{}-{name}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    struct FakeDesktop {
        current: RefCell<Option<PathBuf>>,
        appearance: Appearance,
        sets: RefCell<Vec<PathBuf>>,
    }

    impl Desktop for FakeDesktop {
        fn current(&self) -> Result<Option<PathBuf>> {
            Ok(self.current.borrow().clone())
        }
        fn set(&self, path: &Path) -> Result<()> {
            *self.current.borrow_mut() = Some(path.to_owned());
            self.sets.borrow_mut().push(path.to_owned());
            Ok(())
        }
        fn appearance(&self) -> Appearance {
            self.appearance
        }
    }

    /// Always picks choice `k` (clamped by the pool code).
    struct Fixed(usize);
    impl Random for Fixed {
        fn below(&mut self, n: usize) -> usize {
            assert!(n > 0);
            self.0 % n
        }
    }

    const FILES: [&str; 6] = [
        "Dark - Homer - Moonlight.jpg",
        "Dark - Munch - Starry Night.jpg",
        "Light - Corot - The Bridge at Narni.jpg",
        "Light - Monet - The Magpie.jpg",
        "light - Vermeer - View of Delft.png",
        "untitled.heic",
    ];

    fn setup(name: &str, appearance: Appearance, current: &str) -> Wallbar<FakeDesktop> {
        let root = temp_dir(name);
        let dir = root.join("wallpapers");
        std::fs::create_dir_all(&dir).unwrap();
        for f in FILES {
            std::fs::write(dir.join(f), b"img").unwrap();
        }
        std::fs::write(
            dir.join("paintings.json"),
            r#"{"Light - Corot - The Bridge at Narni.jpg": {
                "artist": "Jean-Baptiste-Camille Corot", "life": "French, 1796–1875",
                "year": "1826", "where": "Musée du Louvre, Paris"}}"#,
        )
        .unwrap();
        let desktop = FakeDesktop {
            current: RefCell::new(Some(dir.join(current))),
            appearance,
            sets: RefCell::new(Vec::new()),
        };
        Wallbar::new(dir, root.join("state").join("state.json"), desktop)
    }

    fn file(w: &Wallbar<FakeDesktop>) -> String {
        w.current(false).unwrap().file
    }

    #[test]
    fn current_reports_details_and_pool_position() {
        let w = setup("current", Appearance::Light, FILES[2]);
        let p = w.current(false).unwrap();
        assert_eq!(p.artist, "Jean-Baptiste-Camille Corot");
        assert_eq!(p.title, "The Bridge at Narni");
        assert_eq!(p.year.as_deref(), Some("1826"));
        assert_eq!(p.where_.as_deref(), Some("Musée du Louvre, Paris"));
        // Light pool is Corot, Monet, Vermeer (lower-case mood still counts).
        assert_eq!((p.index, p.count), (1, 3));
        let p = w.current(true).unwrap();
        assert_eq!((p.index, p.count), (3, 6));
    }

    #[test]
    fn a_wallpaper_from_outside_the_folder_has_no_details() {
        let w = setup("outside", Appearance::Light, FILES[2]);
        let elsewhere = w.dir.parent().unwrap().join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::fs::write(elsewhere.join("Sequoia.heic"), b"img").unwrap();
        *w.desktop.current.borrow_mut() = Some(elsewhere.join("Sequoia.heic"));
        let p = w.current(false).unwrap();
        assert_eq!(p.file, "Sequoia.heic");
        assert_eq!(p.index, 0);
        assert_eq!(p.count, 3);
        assert_eq!(p.year, None);
        // Next from outside starts at the top of the pool.
        assert_eq!(w.next(false).unwrap().file, FILES[2]);
        // ...and prev goes back to where it was.
        assert_eq!(w.prev(false).unwrap().file, "Sequoia.heic");
    }

    #[test]
    fn next_then_prev_returns_to_the_original() {
        let w = setup("next-prev", Appearance::Light, FILES[2]);
        assert_eq!(w.next(false).unwrap().file, FILES[3]);
        assert_eq!(w.next(false).unwrap().file, FILES[4]);
        assert_eq!(w.next(false).unwrap().file, FILES[2], "wraps");
        assert_eq!(w.prev(false).unwrap().file, FILES[4]);
        assert_eq!(w.prev(false).unwrap().file, FILES[3]);
        assert_eq!(w.prev(false).unwrap().file, FILES[2]);
        // History is empty now: prev walks the folder backwards.
        assert!(State::load(&w.state_path).history.is_empty());
        assert_eq!(w.prev(false).unwrap().file, FILES[4]);
    }

    #[test]
    fn next_all_walks_every_image_and_the_dark_pool_is_separate() {
        let w = setup("all", Appearance::Dark, FILES[1]);
        assert_eq!(w.next(false).unwrap().file, FILES[0], "dark pool wraps");
        assert_eq!(w.next(true).unwrap().file, FILES[1]);
        assert_eq!(w.next(true).unwrap().file, FILES[2]);
        let p = w.next(true).unwrap();
        assert_eq!((p.index, p.count), (4, 6));
        w.set_match_appearance(false).unwrap();
        assert_eq!(w.next(false).unwrap().file, FILES[4]);
        assert_eq!(w.current(false).unwrap().count, 6);
    }

    #[test]
    fn shuffle_excludes_the_current_image_and_is_undone_by_prev() {
        for k in 0..6 {
            let w = setup(&format!("shuffle-{k}"), Appearance::Light, FILES[3]);
            let p = w.shuffle(false, &mut Fixed(k)).unwrap();
            assert_ne!(p.file, FILES[3]);
            assert!(
                p.mood == Some(crate::Mood::Light),
                "stays in the light pool"
            );
            assert_eq!(w.prev(false).unwrap().file, FILES[3]);
        }
    }

    #[test]
    fn shuffle_with_a_one_image_pool_widens_or_fails() {
        let w = setup("shuffle-one", Appearance::Dark, FILES[0]);
        // Remove the other dark image: the pool is just the current one.
        std::fs::remove_file(w.dir.join(FILES[1])).unwrap();
        let p = w.shuffle(false, &mut Fixed(0)).unwrap();
        assert_ne!(p.file, FILES[0]);
        for f in &FILES[1..] {
            let _ = std::fs::remove_file(w.dir.join(f));
        }
        *w.desktop.current.borrow_mut() = Some(w.dir.join(FILES[0]));
        assert!(w.shuffle(false, &mut Fixed(0)).is_err());
    }

    #[test]
    fn set_takes_a_filename_or_a_path_and_pushes_history() {
        let w = setup("set", Appearance::Light, FILES[2]);
        assert_eq!(w.set(FILES[0]).unwrap().file, FILES[0]);
        let path = w.dir.join(FILES[4]);
        assert_eq!(w.set(path.to_str().unwrap()).unwrap().file, FILES[4]);
        assert!(w.set("nope.jpg").is_err());
        std::fs::write(w.dir.join("notes.txt"), b"x").unwrap();
        assert!(w.set("notes.txt").is_err());
        let history = State::load(&w.state_path).history;
        assert_eq!(history, vec![w.dir.join(FILES[2]), w.dir.join(FILES[0])]);
        // Setting what is already showing changes nothing.
        w.set(FILES[4]).unwrap();
        assert_eq!(w.desktop.sets.borrow().len(), 2);
        assert_eq!(State::load(&w.state_path).history.len(), 2);
    }

    #[test]
    fn prev_skips_history_entries_that_are_gone() {
        let w = setup("prev-gone", Appearance::Light, FILES[2]);
        w.next(false).unwrap(); // Corot -> Monet, history [Corot]
        w.next(false).unwrap(); // Monet -> Vermeer, history [Corot, Monet]
        std::fs::remove_file(w.dir.join(FILES[3])).unwrap();
        assert_eq!(w.prev(false).unwrap().file, FILES[2]);
        assert_eq!(file(&w), FILES[2]);
    }

    #[test]
    fn list_is_the_whole_folder_with_the_current_file() {
        let w = setup("list", Appearance::Light, FILES[2]);
        let l = w.list().unwrap();
        assert_eq!(l.current.as_deref(), Some(FILES[2]));
        assert_eq!(l.items.len(), 6);
        assert_eq!(l.items[0].file, FILES[0]);
        assert_eq!((l.items[5].index, l.items[5].count), (6, 6));
        assert_eq!(l.items[5].title, "untitled");
        assert_eq!(l.items[5].mood, None);
        let v = serde_json::to_value(&l).unwrap();
        assert_eq!(v["current"], FILES[2]);
        assert!(v["items"].is_array());
    }

    #[test]
    fn an_empty_folder_is_an_error_not_a_panic() {
        let w = setup("empty", Appearance::Light, FILES[2]);
        for f in FILES {
            std::fs::remove_file(w.dir.join(f)).unwrap();
        }
        assert!(w.next(false).is_err());
        assert!(w.prev(false).is_err());
        assert!(w.shuffle(false, &mut Fixed(0)).is_err());
        assert_eq!(w.list().unwrap().items.len(), 0);
    }

    #[test]
    fn clock_random_stays_in_range() {
        let mut r = ClockRandom::new();
        for n in 1..50 {
            assert!(r.below(n) < n);
        }
    }
}
