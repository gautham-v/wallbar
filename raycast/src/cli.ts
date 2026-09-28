// Talks to the wallbar CLI (docs/cli.md in the repo is the contract).
// No @raycast/api imports here so it can be exercised from plain node.
import { execFile } from "node:child_process";
import os from "node:os";
import { pathToFileURL } from "node:url";

export type Mood = "Light" | "Dark";

export type Painting = {
  file: string;
  path: string;
  mood: Mood | null;
  title: string;
  artist: string;
  life: string | null;
  year: string | null;
  where: string | null;
  about: string | null;
  link: string | null;
  index: number;
  count: number;
};

export type Listing = { current: string | null; items: Painting[] };

export const DEFAULT_CLI = "~/.local/bin/wallbar";

export class WallbarError extends Error {
  constructor(
    message: string,
    readonly notFound = false,
  ) {
    super(message);
    this.name = "WallbarError";
  }
}

export function expandHome(p: string): string {
  return p === "~" || p.startsWith("~/") ? os.homedir() + p.slice(1) : p;
}

function exec(bin: string, args: string[]): Promise<string> {
  return new Promise((resolve, reject) => {
    execFile(bin, args, { timeout: 20_000, maxBuffer: 16 * 1024 * 1024 }, (err, stdout, stderr) => {
      if (!err) return resolve(stdout);
      const code = (err as NodeJS.ErrnoException).code;
      if (code === "ENOENT") {
        return reject(
          new WallbarError(
            `wallbar not found at ${bin}. Install wallbar, or set its path in the extension preferences.`,
            true,
          ),
        );
      }
      if (code === "EACCES") return reject(new WallbarError(`wallbar at ${bin} isn't executable.`, true));
      const msg = String(stderr).trim();
      reject(new WallbarError(msg || err.message));
    });
  });
}

function parse<T>(stdout: string): T {
  try {
    return JSON.parse(stdout) as T;
  } catch {
    throw new WallbarError(`wallbar printed something that isn't JSON: ${stdout.trim().slice(0, 120)}`);
  }
}

export async function runPainting(bin: string, args: string[]): Promise<Painting> {
  const p = parse<Painting>(await exec(bin, [...args, "--json"]));
  if (!p || typeof p.path !== "string") throw new WallbarError("wallbar returned no painting.");
  return p;
}

export async function runList(bin: string): Promise<Listing> {
  const l = parse<Listing>(await exec(bin, ["list", "--json"]));
  if (!l || !Array.isArray(l.items)) throw new WallbarError("wallbar list returned no items.");
  return { current: l.current ?? null, items: l.items };
}

export function label(p: Painting): string {
  return p.artist ? `${p.artist} — ${p.title}` : p.title;
}

export function openUrl(p: Painting): string {
  return p.link || `https://en.wikipedia.org/w/index.php?search=${encodeURIComponent(`${p.artist} ${p.title}`.trim())}`;
}

// file:// URL safe to drop into markdown image syntax.
export function fileUrl(path: string): string {
  return pathToFileURL(path).href.replace(/\(/g, "%28").replace(/\)/g, "%29");
}

function mdEscape(s: string): string {
  return s.replace(/([\\`*_[\]#<>|])/g, "\\$1");
}

export function markdown(p: Painting): string {
  const parts = [`![${mdEscape(p.title)}](${fileUrl(p.path)})`, `## ${mdEscape(p.title)}`];
  if (p.about) parts.push(mdEscape(p.about));
  return parts.join("\n\n");
}

export function details(p: Painting): string {
  const lines = [p.title];
  const who = [p.artist, p.life && `(${p.life})`].filter(Boolean).join(" ");
  if (who) lines.push(who);
  const when = [p.year, p.where].filter(Boolean).join(", ");
  if (when) lines.push(when);
  if (p.about) lines.push("", p.about);
  lines.push("", openUrl(p));
  return lines.join("\n");
}
