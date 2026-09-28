//! Downscaled copies of the wallpapers for the popover's header image.
//!
//! A 5K JPEG takes long enough to decode that doing it on every open shows,
//! so the popover draws a ~1200px copy from `~/Library/Caches/wallbar/thumbs`.
//! The copy's name is a hash of the source path, size and modification time,
//! so an edited or replaced image gets a new thumbnail instead of a stale one
//! (and gpui's own image cache, keyed by path, never serves the old pixels).
//!
//! `sips` does the work: it goes through ImageIO, so HEIC is covered, and it
//! runs off the main thread.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

/// Longest edge of a thumbnail, in pixels: the popover is 280pt wide, so
/// this is a 2x header with room to crop.
const MAX_EDGE: u32 = 1200;

#[derive(Debug, Clone)]
pub struct Thumbs {
    dir: PathBuf,
}

impl Default for Thumbs {
    fn default() -> Self {
        Self::new()
    }
}

impl Thumbs {
    pub fn new() -> Self {
        let dir = dirs::cache_dir()
            .unwrap_or_else(std::env::temp_dir)
            .join("wallbar")
            .join("thumbs");
        Thumbs { dir }
    }

    /// Where the thumbnail for `source` lives, whether or not it exists yet.
    pub fn path_for(&self, source: &Path) -> Option<PathBuf> {
        let meta = std::fs::metadata(source).ok()?;
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos());
        let key = format!("{}\0{}\0{}", source.display(), meta.len(), mtime);
        Some(self.dir.join(format!("{:016x}.jpg", fnv1a(key.as_bytes()))))
    }

    /// The thumbnail if it has already been made.
    pub fn cached(&self, source: &Path) -> Option<PathBuf> {
        self.path_for(source).filter(|p| p.is_file())
    }

    /// Make the thumbnail if needed and return it. Blocking: call it from a
    /// background thread.
    pub fn ensure(&self, source: &Path) -> Result<PathBuf> {
        let out = self
            .path_for(source)
            .with_context(|| format!("cannot read {}", source.display()))?;
        if out.is_file() {
            return Ok(out);
        }
        std::fs::create_dir_all(&self.dir)?;
        let tmp = out.with_extension(format!("{}.tmp.jpg", std::process::id()));
        let status = Command::new("/usr/bin/sips")
            .arg("-Z")
            .arg(MAX_EDGE.to_string())
            .args(["-s", "format", "jpeg", "-s", "formatOptions", "82"])
            .arg(source)
            .arg("--out")
            .arg(&tmp)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .context("running sips")?;
        if !status.success() || !tmp.is_file() {
            let _ = std::fs::remove_file(&tmp);
            bail!("sips could not read {}", source.display());
        }
        std::fs::rename(&tmp, &out)?;
        Ok(out)
    }
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_changes_when_the_file_does() {
        let dir = std::env::temp_dir().join(format!("wallbar-thumbs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("a.jpg");
        std::fs::write(&src, b"one").unwrap();
        let thumbs = Thumbs {
            dir: dir.join("cache"),
        };
        let first = thumbs.path_for(&src).unwrap();
        assert_eq!(thumbs.path_for(&src).unwrap(), first, "stable");
        std::fs::write(&src, b"longer").unwrap();
        assert_ne!(thumbs.path_for(&src).unwrap(), first);
        assert!(thumbs.path_for(&dir.join("missing.jpg")).is_none());
        assert!(thumbs.cached(&src).is_none());
    }
}
