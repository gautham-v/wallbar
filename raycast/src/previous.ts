import { previous, runAndAnnounce } from "./wallbar";

export default async function Command() {
  await runAndAnnounce(previous, "Couldn't go back");
}
