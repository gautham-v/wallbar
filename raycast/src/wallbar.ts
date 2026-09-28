import { getPreferenceValues, openExtensionPreferences, showHUD } from "@raycast/api";
import { showFailureToast } from "@raycast/utils";
import { DEFAULT_CLI, WallbarError, expandHome, label, runList, runPainting, type Listing, type Painting } from "./cli";

function bin(): string {
  const { cliPath } = getPreferenceValues<Preferences>();
  return expandHome(cliPath?.trim() || DEFAULT_CLI);
}

export const current = () => runPainting(bin(), ["current"]);
export const next = () => runPainting(bin(), ["next"]);
export const previous = () => runPainting(bin(), ["prev"]);
export const shuffle = () => runPainting(bin(), ["shuffle"]);
export const setWallpaper = (p: Painting) => runPainting(bin(), ["set", p.path]);
export const list = (): Promise<Listing> => runList(bin());

export async function fail(error: unknown, title = "wallbar failed") {
  const notFound = error instanceof WallbarError && error.notFound;
  await showFailureToast(error, {
    title: notFound ? "Install wallbar" : title,
    primaryAction: notFound
      ? { title: "Open Extension Preferences", onAction: () => openExtensionPreferences() }
      : undefined,
  });
}

// Body of the three no-view commands.
export async function runAndAnnounce(action: () => Promise<Painting>, failTitle: string) {
  try {
    await showHUD(label(await action()));
  } catch (error) {
    await fail(error, failTitle);
  }
}
