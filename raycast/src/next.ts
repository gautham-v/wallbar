import { next, runAndAnnounce } from "./wallbar";

export default async function Command() {
  await runAndAnnounce(next, "Couldn't go to the next wallpaper");
}
