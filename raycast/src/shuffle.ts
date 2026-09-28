import { runAndAnnounce, shuffle } from "./wallbar";

export default async function Command() {
  await runAndAnnounce(shuffle, "Couldn't shuffle");
}
