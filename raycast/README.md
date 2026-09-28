# Wallbar for Raycast

Local Raycast extension for wallbar. It only talks to the `wallbar` CLI
(`../docs/cli.md` is the contract), always with `--json`.

| Command | What it does |
| --- | --- |
| About This Wallpaper | The painting on the desktop, with artist, dates, where it hangs and a note. ⌘S shuffle, ⌘→ next, ⌘← previous, ↵ opens the link (or a Wikipedia search). |
| Shuffle Wallpaper | Random wallpaper that matches light or dark mode. Shows "Artist — Title". |
| Next Wallpaper | Next in folder order. |
| Previous Wallpaper | Back to the one before. |
| Browse Wallpapers | Grid of every wallpaper, split into Light and Dark. ↵ sets it, ⌘I shows its details. |

## Setup

Needs the CLI at `~/.local/bin/wallbar` (installed with Wallbar.app). If it lives
somewhere else, set **wallbar CLI** in the extension's preferences.

```sh
npm install
npx ray develop   # registers the extension with Raycast; Ctrl-C when it says ready
```

The commands stay installed after the dev session stops. Re-run `npx ray develop`
after editing anything in `src/`.

## Hotkeys

Extensions can't set their own hotkeys. In Raycast Settings → Extensions → Wallbar:

- Shuffle Wallpaper: ⌃⌥S
- Next Wallpaper: ⌃⌥→
- Previous Wallpaper: ⌃⌥←
- About This Wallpaper: ⌃⌥I

## Scripts

- `npm run build`: type-check and build into `dist/`
- `npm run lint`: ESLint + Prettier (`npx ray lint --fix` to format)
