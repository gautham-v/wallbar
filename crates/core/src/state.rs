//! `~/Library/Application Support/wallbar/state.json`:
//! `{ "history": ["<abs path>", ...], "match_appearance": true }`.
//!
//! Both the menu bar app and the CLI read and write it, so every command
//! loads it fresh and saves it with a rename, never holding it in memory
//! across commands.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// History is capped at this many entries; the oldest fall off.
pub const HISTORY_CAP: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    /// Earlier wallpapers, oldest first; `prev` pops the last one.
    pub history: Vec<PathBuf>,
    pub match_appearance: bool,
}

impl Default for State {
    fn default() -> Self {
        State {
            history: Vec::new(),
            match_appearance: true,
        }
    }
}

impl State {
    /// Read the file; a missing or unreadable one is the default state.
    pub fn load(path: &Path) -> State {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    /// Write the file atomically (temp file + rename), creating the folder.
    pub fn save(&self, path: &Path) -> Result<()> {
        let dir = path.parent().context("state path has no folder")?;
        std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        let tmp = dir.join(format!(".state.{}.tmp", std::process::id()));
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&tmp, json + "\n").with_context(|| format!("writing {}", tmp.display()))?;
        std::fs::rename(&tmp, path).with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }

    /// Remember `path` as the wallpaper being replaced. A repeat of the last
    /// entry is not pushed twice.
    pub fn push(&mut self, path: PathBuf) {
        if self.history.last() == Some(&path) {
            return;
        }
        self.history.push(path);
        if self.history.len() > HISTORY_CAP {
            let extra = self.history.len() - HISTORY_CAP;
            self.history.drain(..extra);
        }
    }

    /// Pop entries until one is usable: it still exists and is not the
    /// wallpaper already showing.
    pub fn pop_usable(&mut self, current: Option<&Path>) -> Option<PathBuf> {
        while let Some(path) = self.history.pop() {
            if Some(path.as_path()) != current && path.is_file() {
                return Some(path);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_or_partial_file_is_the_default() {
        let dir = crate::wallbar::tests::temp_dir("state-default");
        let path = dir.join("state.json");
        assert_eq!(State::load(&path), State::default());
        assert!(State::default().match_appearance);
        std::fs::write(&path, r#"{"match_appearance": false}"#).unwrap();
        let s = State::load(&path);
        assert!(!s.match_appearance);
        assert!(s.history.is_empty());
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(State::load(&path), State::default());
    }

    #[test]
    fn it_round_trips_in_the_contract_shape() {
        let dir = crate::wallbar::tests::temp_dir("state-roundtrip");
        let path = dir.join("nested").join("state.json");
        let mut s = State::default();
        s.push("/w/a.jpg".into());
        s.match_appearance = false;
        s.save(&path).unwrap();
        assert_eq!(State::load(&path), s);
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(v["history"][0], "/w/a.jpg");
        assert_eq!(v["match_appearance"], false);
    }

    #[test]
    fn history_is_capped_and_does_not_repeat() {
        let mut s = State::default();
        for i in 0..60 {
            s.push(format!("/w/{i}.jpg").into());
            s.push(format!("/w/{i}.jpg").into());
        }
        assert_eq!(s.history.len(), HISTORY_CAP);
        assert_eq!(s.history[0], PathBuf::from("/w/10.jpg"));
        assert_eq!(s.history.last().unwrap(), &PathBuf::from("/w/59.jpg"));
    }

    #[test]
    fn pop_skips_missing_files_and_the_current_one() {
        let dir = crate::wallbar::tests::temp_dir("state-pop");
        let a = dir.join("a.jpg");
        let b = dir.join("b.jpg");
        std::fs::write(&a, b"x").unwrap();
        std::fs::write(&b, b"x").unwrap();
        let mut s = State::default();
        s.push(a.clone());
        s.push(dir.join("gone.jpg"));
        s.push(b.clone());
        assert_eq!(s.pop_usable(Some(&b)), Some(a));
        assert_eq!(s.pop_usable(None), None);
    }
}
