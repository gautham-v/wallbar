//! Which images are in play, and how next / previous / shuffle move through
//! them.
//!
//! Everything here is over indices into the folder listing, so the rules can
//! be tested with plain filenames. The pool keeps folder order: it is the
//! folder with some images left out, never re-sorted.

use crate::painting::{parse_filename, Mood};

/// The system's light or dark mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    Light,
    Dark,
}

impl Appearance {
    pub fn mood(self) -> Mood {
        match self {
            Appearance::Light => Mood::Light,
            Appearance::Dark => Mood::Dark,
        }
    }
}

/// The pool, as indices into `files` in folder order.
///
/// With `match_appearance` (and no `--all`), only images whose mood matches
/// the appearance; if none do, every image. Otherwise every image.
pub fn select(files: &[String], match_appearance: bool, appearance: Appearance) -> Vec<usize> {
    let all: Vec<usize> = (0..files.len()).collect();
    if !match_appearance {
        return all;
    }
    let mood = appearance.mood();
    let matching: Vec<usize> = all
        .iter()
        .copied()
        .filter(|&i| parse_filename(&files[i]).mood == Some(mood))
        .collect();
    if matching.is_empty() {
        all
    } else {
        matching
    }
}

/// The next pool image after `current` in folder order, wrapping.
///
/// `current` is its index in the folder, or `None` when the wallpaper is not
/// in the folder (then the first pool image). A current image that is not in
/// the pool — a dark painting while the Mac is light — still has a place in
/// folder order, so next is the first pool image after that place.
pub fn next(pool: &[usize], current: Option<usize>) -> Option<usize> {
    let first = *pool.first()?;
    let Some(cur) = current else {
        return Some(first);
    };
    Some(pool.iter().copied().find(|&i| i > cur).unwrap_or(first))
}

/// The pool image before `current` in folder order, wrapping. Mirrors
/// [`next`]; with no current image, the last pool image.
pub fn prev(pool: &[usize], current: Option<usize>) -> Option<usize> {
    let last = *pool.last()?;
    let Some(cur) = current else {
        return Some(last);
    };
    Some(
        pool.iter()
            .rev()
            .copied()
            .find(|&i| i < cur)
            .unwrap_or(last),
    )
}

/// A random pool image other than `current`.
///
/// `pick(n)` returns a number in `0..n`. If the pool is only the current
/// image, the choice widens to every other image in `all`; with nothing else
/// at all there is nothing to shuffle to.
pub fn shuffle(
    pool: &[usize],
    all: usize,
    current: Option<usize>,
    pick: &mut dyn FnMut(usize) -> usize,
) -> Option<usize> {
    let mut choices: Vec<usize> = pool
        .iter()
        .copied()
        .filter(|&i| Some(i) != current)
        .collect();
    if choices.is_empty() {
        choices = (0..all).filter(|&i| Some(i) != current).collect();
    }
    if choices.is_empty() {
        return None;
    }
    let n = choices.len();
    Some(choices[pick(n).min(n - 1)])
}

/// 1-based position of `index` in the pool, 0 when it is not in it.
pub fn position(pool: &[usize], index: Option<usize>) -> usize {
    index
        .and_then(|i| pool.iter().position(|&p| p == i))
        .map_or(0, |p| p + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder() -> Vec<String> {
        // Already in folder order, as `folder::list` would return them.
        [
            "Dark - Homer - Moonlight.jpg",    // 0
            "Dark - Munch - Starry Night.jpg", // 1
            "Light - Corot - Narni.jpg",       // 2
            "Light - Monet - Magpie.jpg",      // 3
            "Light - Vermeer - Delft.jpg",     // 4
            "untitled.png",                    // 5
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }

    #[test]
    fn the_pool_follows_the_appearance() {
        let f = folder();
        assert_eq!(select(&f, true, Appearance::Light), vec![2, 3, 4]);
        assert_eq!(select(&f, true, Appearance::Dark), vec![0, 1]);
        assert_eq!(select(&f, false, Appearance::Dark), vec![0, 1, 2, 3, 4, 5]);
    }

    #[test]
    fn an_empty_mood_pool_falls_back_to_everything() {
        let f: Vec<String> = vec!["Light - A - B.jpg".into(), "x.png".into()];
        assert_eq!(select(&f, true, Appearance::Dark), vec![0, 1]);
        assert!(select(&[], true, Appearance::Light).is_empty());
    }

    #[test]
    fn next_and_prev_walk_the_pool_and_wrap() {
        let pool = vec![2, 3, 4];
        assert_eq!(next(&pool, Some(2)), Some(3));
        assert_eq!(next(&pool, Some(4)), Some(2));
        assert_eq!(prev(&pool, Some(3)), Some(2));
        assert_eq!(prev(&pool, Some(2)), Some(4));
    }

    #[test]
    fn a_current_image_outside_the_pool_keeps_its_folder_place() {
        let pool = vec![0, 1]; // dark pool
                               // Current is the Corot (2), a light painting: next wraps to 0, prev is 1.
        assert_eq!(next(&pool, Some(2)), Some(0));
        assert_eq!(prev(&pool, Some(2)), Some(1));
        let pool = vec![2, 3, 4];
        assert_eq!(next(&pool, Some(0)), Some(2));
        assert_eq!(prev(&pool, Some(0)), Some(4));
    }

    #[test]
    fn a_wallpaper_from_elsewhere_starts_at_the_ends() {
        let pool = vec![2, 3, 4];
        assert_eq!(next(&pool, None), Some(2));
        assert_eq!(prev(&pool, None), Some(4));
        assert_eq!(next(&[], None), None);
        assert_eq!(prev(&[], Some(1)), None);
    }

    #[test]
    fn shuffle_never_picks_the_current_image() {
        let pool = vec![2, 3, 4];
        // Every possible pick, from the lowest to past the end.
        for k in 0..5 {
            let got = shuffle(&pool, 6, Some(3), &mut |_| k).unwrap();
            assert_ne!(got, 3);
            assert!(pool.contains(&got));
        }
        let mut seen: Vec<usize> = (0..2)
            .map(|k| {
                shuffle(&pool, 6, Some(3), &mut |n| {
                    assert_eq!(n, 2);
                    k
                })
                .unwrap()
            })
            .collect();
        seen.sort();
        assert_eq!(seen, vec![2, 4]);
    }

    #[test]
    fn shuffle_widens_when_the_pool_is_only_the_current_image() {
        assert_eq!(shuffle(&[1], 3, Some(1), &mut |_| 0), Some(0));
        assert_eq!(shuffle(&[1], 3, Some(1), &mut |_| 1), Some(2));
        assert_eq!(shuffle(&[0], 1, Some(0), &mut |_| 0), None);
        // Not in the folder: anything in the pool.
        assert_eq!(shuffle(&[1, 2], 3, None, &mut |_| 1), Some(2));
    }

    #[test]
    fn position_is_one_based_and_zero_outside() {
        assert_eq!(position(&[2, 3, 4], Some(3)), 2);
        assert_eq!(position(&[2, 3, 4], Some(0)), 0);
        assert_eq!(position(&[2, 3, 4], None), 0);
    }
}
