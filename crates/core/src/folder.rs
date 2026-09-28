//! The wallpaper folder, in folder order.

use std::path::{Path, PathBuf};

/// Extensions wallbar treats as wallpapers.
pub const EXTENSIONS: [&str; 4] = ["jpg", "jpeg", "png", "heic"];

/// Whether a filename is an image wallbar will use.
pub fn is_image(file: &str) -> bool {
    if file.starts_with('.') {
        return false;
    }
    Path::new(file)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

/// Sort filenames into folder order: case-insensitive, with the exact bytes
/// breaking ties so the order is total.
pub fn sort_folder_order(files: &mut [String]) {
    files.sort_by(|a, b| {
        a.to_lowercase()
            .cmp(&b.to_lowercase())
            .then_with(|| a.cmp(b))
    });
}

/// The images directly in `dir`, in folder order, as absolute paths.
/// A missing folder is an empty list rather than an error.
pub fn list(dir: &Path) -> Vec<PathBuf> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<String> = read
        .filter_map(Result::ok)
        .filter(|e| {
            e.file_type()
                .map(|t| t.is_file() || t.is_symlink())
                .unwrap_or(false)
        })
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|f| is_image(f))
        .collect();
    sort_folder_order(&mut files);
    files.into_iter().map(|f| dir.join(f)).collect()
}

/// Where `path` sits in `files`: by exact path, then by the same filename
/// in the same (canonical) folder, which absorbs `/private` and symlinked
/// home directories.
pub fn position(files: &[PathBuf], path: &Path) -> Option<usize> {
    if let Some(i) = files.iter().position(|f| f == path) {
        return Some(i);
    }
    let name = path.file_name()?;
    let parent = std::fs::canonicalize(path.parent()?).ok()?;
    files.iter().position(|f| {
        f.file_name() == Some(name)
            && f.parent()
                .and_then(|p| std::fs::canonicalize(p).ok())
                .is_some_and(|p| p == parent)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_images_count() {
        assert!(is_image("Light - A - B.jpg"));
        assert!(is_image("x.JPEG"));
        assert!(is_image("x.heic"));
        assert!(is_image("x.png"));
        assert!(!is_image("paintings.json"));
        assert!(!is_image("x.gif"));
        assert!(!is_image(".hidden.jpg"));
        assert!(!is_image("jpg"));
    }

    #[test]
    fn folder_order_ignores_case() {
        let mut files: Vec<String> = [
            "light - b.jpg",
            "Dark - z.jpg",
            "Light - a.jpg",
            "dark - a.jpg",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        sort_folder_order(&mut files);
        assert_eq!(
            files,
            [
                "dark - a.jpg",
                "Dark - z.jpg",
                "Light - a.jpg",
                "light - b.jpg"
            ]
        );
    }

    #[test]
    fn listing_a_folder_sorts_and_filters() {
        let dir = crate::wallbar::tests::temp_dir("listing");
        for f in ["b.jpg", "A.png", "c.txt", "paintings.json", ".d.jpg"] {
            std::fs::write(dir.join(f), b"x").unwrap();
        }
        std::fs::create_dir(dir.join("sub.jpg")).unwrap();
        let names: Vec<String> = list(&dir)
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["A.png", "b.jpg"]);
        assert!(list(&dir.join("missing")).is_empty());
        assert_eq!(position(&list(&dir), &dir.join("b.jpg")), Some(1));
        assert_eq!(position(&list(&dir), &dir.join("zzz.jpg")), None);
    }
}
