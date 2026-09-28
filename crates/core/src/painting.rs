//! What a wallpaper is called: parsed from its filename, filled in from
//! `paintings.json`, and printed as the object `docs/cli.md` specifies.

use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

/// `Light` or `Dark`, from the first part of the filename.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Mood {
    Light,
    Dark,
}

/// What a filename alone says about a painting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedName {
    pub mood: Option<Mood>,
    /// The short name in the filename ("Corot"), or empty.
    pub artist: String,
    pub title: String,
}

/// `<Mood> - <Artist> - <Title>.<ext>`. Anything else is mood = None,
/// artist = "", title = the file stem.
///
/// The title keeps any further " - " in it, so a title that itself has a
/// dash in it is not cut short.
pub fn parse_filename(file: &str) -> ParsedName {
    let stem = Path::new(file)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| file.to_owned());
    let mut parts = stem.splitn(3, " - ");
    let (Some(mood), Some(artist), Some(title)) = (parts.next(), parts.next(), parts.next()) else {
        return unparsed(stem);
    };
    let mood = match mood.trim() {
        m if m.eq_ignore_ascii_case("light") => Mood::Light,
        m if m.eq_ignore_ascii_case("dark") => Mood::Dark,
        _ => return unparsed(stem),
    };
    let (artist, title) = (artist.trim(), title.trim());
    if artist.is_empty() || title.is_empty() {
        return unparsed(stem);
    }
    ParsedName {
        mood: Some(mood),
        artist: artist.to_owned(),
        title: title.to_owned(),
    }
}

fn unparsed(stem: String) -> ParsedName {
    ParsedName {
        mood: None,
        artist: String::new(),
        title: stem,
    }
}

/// One entry of `paintings.json`. Every field is optional.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Details {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub life: Option<String>,
    pub year: Option<String>,
    pub where_: Option<String>,
    pub about: Option<String>,
    pub link: Option<String>,
}

/// Parse `paintings.json`. Tolerant on purpose: the file is written by hand
/// (or by another tool while wallbar is running), so a malformed file is no
/// details at all, an entry that is not an object is skipped, and a field
/// that is not a non-empty string is treated as missing.
pub fn parse_details(json: &str) -> HashMap<String, Details> {
    let Ok(Value::Object(map)) = serde_json::from_str::<Value>(json) else {
        return HashMap::new();
    };
    map.into_iter()
        .filter_map(|(file, entry)| {
            let Value::Object(entry) = entry else {
                return None;
            };
            let field = |key: &str| {
                entry
                    .get(key)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
            };
            Some((
                file,
                Details {
                    title: field("title"),
                    artist: field("artist"),
                    life: field("life"),
                    year: field("year"),
                    where_: field("where"),
                    about: field("about"),
                    link: field("link"),
                },
            ))
        })
        .collect()
}

/// `<folder>/paintings.json`, or nothing if it is missing or unreadable.
pub fn load_details(dir: &Path) -> HashMap<String, Details> {
    std::fs::read_to_string(dir.join("paintings.json"))
        .map(|json| parse_details(&json))
        .unwrap_or_default()
}

/// The object every painting-printing command prints with `--json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Painting {
    pub file: String,
    pub path: String,
    pub mood: Option<Mood>,
    pub title: String,
    pub artist: String,
    pub life: Option<String>,
    pub year: Option<String>,
    #[serde(rename = "where")]
    pub where_: Option<String>,
    pub about: Option<String>,
    pub link: Option<String>,
    /// 1-based position within the pool; 0 when it is not in it.
    pub index: usize,
    pub count: usize,
}

impl Painting {
    /// Build the object for `path`. `details` is `None` for an image that is
    /// not in the folder, whose details are then all null.
    pub fn new(path: &Path, details: Option<&Details>, index: usize, count: usize) -> Self {
        let file = path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default();
        let parsed = parse_filename(&file);
        let empty = Details::default();
        let d = details.unwrap_or(&empty);
        Painting {
            path: path.to_string_lossy().into_owned(),
            mood: parsed.mood,
            title: d.title.clone().unwrap_or(parsed.title),
            artist: d.artist.clone().unwrap_or(parsed.artist),
            life: d.life.clone(),
            year: d.year.clone(),
            where_: d.where_.clone(),
            about: d.about.clone(),
            link: d.link.clone(),
            index,
            count,
            file,
        }
    }

    /// "Artist — Title", or just the title when there is no artist.
    pub fn one_line(&self) -> String {
        if self.artist.is_empty() {
            self.title.clone()
        } else {
            format!("{} \u{2014} {}", self.artist, self.title)
        }
    }

    /// Where the popover's link goes: the painting's own link, or a
    /// Wikipedia search for artist and title.
    pub fn link_url(&self) -> String {
        match &self.link {
            Some(link) => link.clone(),
            None => {
                let query = format!("{} {}", self.artist, self.title);
                format!(
                    "https://en.wikipedia.org/w/index.php?search={}",
                    percent_encode(query.trim())
                )
            }
        }
    }

    /// What the link is called: "Wikipedia" when it goes there, else "Details".
    pub fn link_label(&self) -> &'static str {
        match &self.link {
            Some(link) if !link.contains("wikipedia.org") => "Details",
            _ => "Wikipedia",
        }
    }
}

/// Percent-encode a query string value (RFC 3986 unreserved kept, space as +).
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_well_formed_name_splits_into_mood_artist_title() {
        let p = parse_filename("Light - Corot - The Bridge at Narni.jpg");
        assert_eq!(p.mood, Some(Mood::Light));
        assert_eq!(p.artist, "Corot");
        assert_eq!(p.title, "The Bridge at Narni");
    }

    #[test]
    fn dark_and_case_are_recognised() {
        let p = parse_filename("dark - Munch - Starry Night.HEIC");
        assert_eq!(p.mood, Some(Mood::Dark));
        assert_eq!(p.title, "Starry Night");
    }

    #[test]
    fn a_title_keeps_its_own_dashes_and_commas() {
        let p = parse_filename("Light - Turner - Rain - Steam, and Speed.jpg");
        assert_eq!(p.artist, "Turner");
        assert_eq!(p.title, "Rain - Steam, and Speed");
    }

    #[test]
    fn anything_else_is_the_stem_with_no_mood_or_artist() {
        for (file, title) in [
            ("sunset.png", "sunset"),
            (
                "Dusk - Monet - Water Lilies.jpg",
                "Dusk - Monet - Water Lilies",
            ),
            ("Light - Monet.jpg", "Light - Monet"),
            ("Light -  - x.jpg", "Light -  - x"),
        ] {
            let p = parse_filename(file);
            assert_eq!(p.mood, None, "{file}");
            assert_eq!(p.artist, "", "{file}");
            assert_eq!(p.title, title, "{file}");
        }
    }

    #[test]
    fn details_are_tolerant_of_partial_and_broken_files() {
        let json = r#"{
            "a.jpg": {"title": "A", "year": 1826, "where": "", "about": "  Note. "},
            "b.jpg": "not an object",
            "c.jpg": {}
        }"#;
        let d = parse_details(json);
        assert_eq!(d.len(), 2);
        let a = &d["a.jpg"];
        assert_eq!(a.title.as_deref(), Some("A"));
        assert_eq!(a.year, None, "a number is not a string");
        assert_eq!(a.where_, None, "empty is missing");
        assert_eq!(a.about.as_deref(), Some("Note."));
        assert_eq!(d["c.jpg"], Details::default());
        // Truncated mid-write, or not JSON at all: no details, no error.
        assert!(parse_details(r#"{"a.jpg": {"title": "A""#).is_empty());
        assert!(parse_details("[]").is_empty());
    }

    #[test]
    fn a_painting_falls_back_to_the_filename() {
        let path = Path::new("/w/Light - Corot - The Bridge at Narni.jpg");
        let p = Painting::new(path, None, 0, 15);
        assert_eq!(p.file, "Light - Corot - The Bridge at Narni.jpg");
        assert_eq!(p.title, "The Bridge at Narni");
        assert_eq!(p.artist, "Corot");
        assert_eq!(p.year, None);
        assert_eq!(p.one_line(), "Corot \u{2014} The Bridge at Narni");

        let details = Details {
            artist: Some("Jean-Baptiste-Camille Corot".into()),
            year: Some("1826".into()),
            ..Default::default()
        };
        let p = Painting::new(path, Some(&details), 7, 15);
        assert_eq!(p.artist, "Jean-Baptiste-Camille Corot");
        assert_eq!(p.title, "The Bridge at Narni");
        assert_eq!(p.year.as_deref(), Some("1826"));
    }

    #[test]
    fn the_json_object_has_the_contract_field_names() {
        let p = Painting::new(Path::new("/w/x.jpg"), None, 0, 3);
        let v: Value = serde_json::to_value(&p).unwrap();
        let keys: Vec<&str> = v.as_object().unwrap().keys().map(|k| k.as_str()).collect();
        for key in [
            "file", "path", "mood", "title", "artist", "life", "year", "where", "about", "link",
            "index", "count",
        ] {
            assert!(keys.contains(&key), "missing {key}");
        }
        assert_eq!(v["mood"], Value::Null);
        assert_eq!(
            serde_json::to_value(Mood::Dark).unwrap(),
            Value::String("Dark".into())
        );
    }

    #[test]
    fn the_link_falls_back_to_a_wikipedia_search() {
        let mut p = Painting::new(Path::new("/w/Light - Monet - The Magpie.jpg"), None, 1, 1);
        assert_eq!(
            p.link_url(),
            "https://en.wikipedia.org/w/index.php?search=Monet+The+Magpie"
        );
        assert_eq!(p.link_label(), "Wikipedia");
        p.link = Some("https://www.musee-orsay.fr/x".into());
        assert_eq!(p.link_label(), "Details");
        assert_eq!(percent_encode("Autumn – é"), "Autumn+%E2%80%93+%C3%A9");
    }
}
