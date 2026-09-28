import { Action, ActionPanel, Color, Detail, Icon, Keyboard } from "@raycast/api";
import { useEffect, useState } from "react";
import { details, markdown, openUrl, type Painting } from "./cli";
import { current, fail, next, previous, setWallpaper, shuffle } from "./wallbar";

type Props = {
  // Show this painting instead of loading the current wallpaper (used by Browse).
  painting?: Painting;
  isCurrent?: boolean;
  onChange?: () => void;
};

const TBD = { value: "To confirm", color: Color.SecondaryText };

function host(url: string): string {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return url;
  }
}

export function AboutView(props: Props) {
  const [painting, setPainting] = useState<Painting | undefined>(props.painting);
  const [isCurrent, setIsCurrent] = useState(props.painting ? !!props.isCurrent : true);
  const [loading, setLoading] = useState(!props.painting);
  const [error, setError] = useState<string>();

  async function run(action: () => Promise<Painting>, failTitle: string) {
    setLoading(true);
    try {
      setPainting(await action());
      setIsCurrent(true);
      setError(undefined);
      props.onChange?.();
    } catch (e) {
      await fail(e, failTitle);
      if (!painting) setError(e instanceof Error ? e.message : String(e));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    if (!props.painting) run(current, "Couldn't read the current wallpaper");
  }, []);

  const change = (
    <ActionPanel.Section>
      {painting && !isCurrent && (
        <Action
          title="Set as Wallpaper"
          icon={Icon.Desktop}
          shortcut={{ modifiers: ["cmd"], key: "return" }}
          onAction={() => run(() => setWallpaper(painting), "Couldn't set the wallpaper")}
        />
      )}
      <Action
        title="Shuffle"
        icon={Icon.Shuffle}
        // ⌘S is Shuffle here on purpose; there is nothing to save.
        // eslint-disable-next-line @raycast/prefer-common-shortcut
        shortcut={{ modifiers: ["cmd"], key: "s" }}
        onAction={() => run(shuffle, "Couldn't shuffle")}
      />
      <Action
        title="Next"
        icon={Icon.ArrowRight}
        shortcut={{ modifiers: ["cmd"], key: "arrowRight" }}
        onAction={() => run(next, "Couldn't go to the next wallpaper")}
      />
      <Action
        title="Previous"
        icon={Icon.ArrowLeft}
        shortcut={{ modifiers: ["cmd"], key: "arrowLeft" }}
        onAction={() => run(previous, "Couldn't go back")}
      />
    </ActionPanel.Section>
  );

  if (!painting) {
    return (
      <Detail
        isLoading={loading}
        markdown={error ? `Couldn't read the current wallpaper.\n\n\`\`\`\n${error}\n\`\`\`` : ""}
        actions={
          <ActionPanel>
            <Action
              title="Try Again"
              icon={Icon.ArrowClockwise}
              onAction={() => run(current, "Couldn't read the current wallpaper")}
            />
            {change}
          </ActionPanel>
        }
      />
    );
  }

  const inFolder = painting.index > 0;
  return (
    <Detail
      isLoading={loading}
      navigationTitle={props.painting ? painting.title : undefined}
      markdown={markdown(painting)}
      metadata={
        <Detail.Metadata>
          {painting.artist && <Detail.Metadata.Label title="Artist" text={painting.artist} />}
          {inFolder ? (
            <>
              <Detail.Metadata.Label title="Lived" text={painting.life ?? TBD} />
              <Detail.Metadata.Label title="Year" text={painting.year ?? TBD} />
              <Detail.Metadata.Label title="Where" text={painting.where ?? TBD} />
            </>
          ) : (
            <Detail.Metadata.Label
              title="Folder"
              text={{ value: "Not in the wallpaper folder", color: Color.SecondaryText }}
            />
          )}
          <Detail.Metadata.Separator />
          <Detail.Metadata.Link
            title="Link"
            text={painting.link ? host(painting.link) : "Search Wikipedia"}
            target={openUrl(painting)}
          />
          {inFolder && isCurrent && (
            <Detail.Metadata.Label title="In Rotation" text={`${painting.index} of ${painting.count}`} />
          )}
        </Detail.Metadata>
      }
      actions={
        <ActionPanel>
          <Action.OpenInBrowser title={painting.link ? "Open Link" : "Search Wikipedia"} url={openUrl(painting)} />
          {change}
          <ActionPanel.Section>
            <Action.CopyToClipboard
              title="Copy Details"
              content={details(painting)}
              shortcut={Keyboard.Shortcut.Common.Copy}
            />
            <Action.ShowInFinder path={painting.path} shortcut={{ modifiers: ["cmd", "shift"], key: "f" }} />
          </ActionPanel.Section>
        </ActionPanel>
      }
    />
  );
}
