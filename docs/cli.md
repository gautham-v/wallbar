# wallbar CLI contract

One Rust binary, `wallbar`. With no arguments it runs the menu bar app. With a
subcommand it does one thing, prints JSON (when asked), and exits. The Raycast
extension only ever talks to the CLI; the menu bar app notices changes on its own.

## Paths
- Wallpaper folder: `$WALLBAR_DIR`, default `~/Pictures/wallpapers`.
  Images: `*.jpg|*.jpeg|*.png|*.heic` directly in that folder, sorted by filename
  (case-insensitive). That order is "folder order".
- Details file: `<folder>/paintings.json` (optional; see schema below).
- State: `~/Library/Application Support/wallbar/state.json`
  `{ "history": ["<abs path>", ...], "match_appearance": true }` (history capped at 50).
- Installed CLI path: `~/.local/bin/wallbar` (symlink into Wallbar.app).

## Filenames
`<Mood> - <Artist> - <Title>.<ext>` where Mood is `Light` or `Dark`.
Anything else: mood = null, artist = "", title = file stem.

## paintings.json
```json
{
  "Light - Corot - The Bridge at Narni.jpg": {
    "title": "The Bridge at Narni",
    "artist": "Jean-Baptiste-Camille Corot",
    "life": "French, 1796–1875",
    "year": "1826",
    "where": "Musée du Louvre, Paris",
    "about": "One to three sentences.",
    "link": "https://… (museum or Wikipedia page)"
  }
}
```
Every field optional; missing ones fall back to the filename (title/artist) or null.

## Commands
All commands that print a painting print this object with `--json`
(and a one-line "Artist — Title" without it):
```json
{ "file": "Light - Corot - The Bridge at Narni.jpg", "path": "/Users/…/…jpg",
  "mood": "Light", "title": "…", "artist": "…", "life": "…"|null, "year": "…"|null,
  "where": "…"|null, "about": "…"|null, "link": "…"|null,
  "index": 7, "count": 15 }
```
`index` is 1-based position within the current pool, `count` the pool size.

- `wallbar current [--all] [--json]` — the wallpaper on the main display
  (`--all` only changes index/count to be relative to the whole folder).
  If it is not in the folder, `file`/`path` are still set and details are null, index 0.
- `wallbar list [--json]` — array of the object above for every image in the folder
  (index/count relative to the whole folder), plus top-level `"current": "<file>"` when
  using `--json`: `{ "current": "...", "items": [ ... ] }`.
- `wallbar next [--all] [--json]` — next in folder order within the pool, wraps.
  Pushes the old wallpaper onto history.
- `wallbar prev [--all] [--json]` — pops history if non-empty, else previous in folder order.
  History entries whose file is gone (or that are already showing) are skipped.
- `wallbar shuffle [--all] [--json]` — random from pool excluding current; pushes history.
  If the pool is only the current image, picks from the other images in the folder.
- `wallbar set <filename|path> [--json]` — sets that image; pushes history.
- `wallbar match-appearance on|off [--json]` — writes state; prints
  `Match appearance: on` (or `{"match_appearance": true}` with `--json`).

Pool: if `match_appearance` is true (default) and `--all` is not given, only images whose
mood matches the current system appearance (`Dark` when macOS is in dark mode, else `Light`);
if that pool would be empty, use all images. Otherwise all images.

Setting applies to every screen (NSWorkspace setDesktopImageURL:forScreen:options: for each
NSScreen). Exit code 0 on success; 1 with a message on stderr on failure; 2 for a usage
error (unknown command or option).

Spaces (checked on macOS 15.8): once the wallpaper store has per-Space entries — which the
first programmatic set creates — a set changes the Space in front only (on every display).
Other Spaces keep their own wallpaper, and `current` reports the Space in front.
