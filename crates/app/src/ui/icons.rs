//! The popover's SVG icons, compiled into the binary. gpui's `svg()` asks the
//! app's [`gpui::AssetSource`] for a path and draws it as an alpha mask
//! tinted with `.text_color(..)`.

use std::borrow::Cow;

use gpui::SharedString;

pub const CHEVRON_LEFT: &str = "icons/chevron-left.svg";
pub const CHEVRON_RIGHT: &str = "icons/chevron-right.svg";
pub const SHUFFLE: &str = "icons/shuffle.svg";
pub const CHECK: &str = "icons/check.svg";

const FILES: [(&str, &[u8]); 4] = [
    (
        CHEVRON_LEFT,
        include_bytes!("../../../../assets/icons/chevron-left.svg"),
    ),
    (
        CHEVRON_RIGHT,
        include_bytes!("../../../../assets/icons/chevron-right.svg"),
    ),
    (
        SHUFFLE,
        include_bytes!("../../../../assets/icons/shuffle.svg"),
    ),
    (CHECK, include_bytes!("../../../../assets/icons/check.svg")),
];

/// Register with `Application::new().with_assets(Assets)`.
pub struct Assets;

impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(FILES
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(FILES
            .iter()
            .filter(|(name, _)| name.starts_with(path))
            .map(|(name, _)| SharedString::from(*name))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::AssetSource;

    #[test]
    fn every_icon_loads_and_is_an_svg() {
        for (name, _) in FILES {
            let bytes = Assets.load(name).unwrap().unwrap();
            assert!(
                std::str::from_utf8(&bytes).unwrap().starts_with("<svg"),
                "{name}"
            );
        }
    }
}
