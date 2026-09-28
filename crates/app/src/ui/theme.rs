//! Visual tokens, as gpui types. The same family as claudebar and scorebar:
//! a translucent system-menu material, monochrome, greys as alpha over the
//! material, one 13px size with 11px captions, regular and medium weights.
//! Ink against grey is what carries state — there is no accent colour.

use gpui::{px, FontWeight, Pixels, Rgba, WindowAppearance};

const fn black(alpha: f32) -> Rgba {
    Rgba {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: alpha,
    }
}

const fn white(alpha: f32) -> Rgba {
    Rgba {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: alpha,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    /// The popover material, over a blurred window.
    pub bg: Rgba,
    /// Hairline rim around the popover.
    pub border: Rgba,
    /// Ink: titles, values, the link, a checked box.
    pub text: Rgba,
    /// Grey: labels, the artist's dates, the pool count.
    pub secondary: Rgba,
    /// Fainter grey: an unchecked box's outline, a disabled button's icon.
    pub tertiary: Rgba,
    pub separator: Rgba,
    /// Button fill, and its hover.
    pub button: Rgba,
    pub button_hover: Rgba,
    /// What sits on an ink fill (the checkmark).
    pub on_ink: Rgba,
    /// The header before its thumbnail has loaded.
    pub placeholder: Rgba,
}

pub const LIGHT: Theme = Theme {
    bg: Rgba {
        r: 236.0 / 255.0,
        g: 236.0 / 255.0,
        b: 238.0 / 255.0,
        a: BG_ALPHA,
    },
    border: black(0.06),
    text: black(0.85),
    secondary: black(0.50),
    tertiary: black(0.28),
    separator: black(0.09),
    button: black(0.055),
    button_hover: black(0.10),
    on_ink: white(0.95),
    placeholder: black(0.06),
};

pub const DARK: Theme = Theme {
    bg: Rgba {
        r: 40.0 / 255.0,
        g: 40.0 / 255.0,
        b: 42.0 / 255.0,
        a: BG_ALPHA,
    },
    border: white(0.10),
    text: white(0.85),
    secondary: white(0.55),
    tertiary: white(0.30),
    separator: white(0.12),
    button: white(0.08),
    button_hover: white(0.14),
    on_ink: black(0.90),
    placeholder: white(0.06),
};

/// Popover material opacity over the blurred window: what gives it the
/// system-menu wash. Checked against the Battery menu in claudebar.
pub const BG_ALPHA: f32 = 0.85;

impl Default for Theme {
    fn default() -> Self {
        LIGHT
    }
}

impl Theme {
    pub fn for_appearance(appearance: WindowAppearance) -> Self {
        match appearance {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => DARK,
            WindowAppearance::Light | WindowAppearance::VibrantLight => LIGHT,
        }
    }
}

// ── Sizes ────────────────────────────────────────────────────────────────────

pub const POPOVER_WIDTH_PX: f32 = 280.0;
pub const POPOVER_WIDTH: Pixels = px(POPOVER_WIDTH_PX);
pub const POPOVER_RADIUS_PX: f32 = 10.0;
pub const POPOVER_RADIUS: Pixels = px(POPOVER_RADIUS_PX);
/// Menus hang straight off the bar.
pub const POPOVER_TOP_GAP: Pixels = px(0.);
/// The header image is the popover's width, cropped to 16:7.
pub const HEADER_HEIGHT: Pixels = px((POPOVER_WIDTH_PX * 7.0 / 16.0) as i32 as f32);
/// The text inset every row shares.
pub const PAD_X: Pixels = px(12.);
/// Button height and corner radius.
pub const BUTTON_HEIGHT: Pixels = px(24.);
pub const BUTTON_MIN_WIDTH: Pixels = px(30.);
pub const BUTTON_RADIUS: Pixels = px(6.);
pub const ICON: Pixels = px(12.);
pub const SHUFFLE_ICON: Pixels = px(14.);
/// The Year / Where label column.
pub const LABEL_WIDTH: Pixels = px(46.);
/// The Match appearance checkbox.
pub const CHECKBOX: Pixels = px(14.);
pub const CHECKBOX_RADIUS: Pixels = px(4.);
pub const CHECK_ICON: Pixels = px(10.);
pub const HAIRLINE: Pixels = px(1.);

// ── Type ─────────────────────────────────────────────────────────────────────

pub const TEXT: Pixels = px(13.);
pub const LINE: Pixels = px(17.);
/// The about text gets a little more leading than a row.
pub const LINE_PROSE: Pixels = px(18.);
pub const CAPTION: Pixels = px(11.);
pub const LINE_CAPTION: Pixels = px(14.);
/// Medium, not semibold: semibold reads as shouting next to system menus.
pub const WEIGHT_EMPHASIS: FontWeight = FontWeight::MEDIUM;
pub const UI_FAMILY: &str = ".SystemUIFont";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_popover_is_in_the_family_range() {
        assert!((260.0..=300.0).contains(&POPOVER_WIDTH_PX));
        assert_eq!(HEADER_HEIGHT, px(122.));
    }

    #[test]
    fn greys_are_alpha_over_the_material() {
        for t in [LIGHT, DARK] {
            assert!(t.secondary.a < t.text.a);
            assert!(t.tertiary.a < t.secondary.a);
            assert_eq!((t.text.r, t.text.g), (t.secondary.r, t.secondary.g));
        }
    }

    #[test]
    fn appearance_picks_the_matching_set() {
        assert_eq!(Theme::for_appearance(WindowAppearance::Dark), DARK);
        assert_eq!(Theme::for_appearance(WindowAppearance::VibrantLight), LIGHT);
    }
}
