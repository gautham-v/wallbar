import { Action, ActionPanel, Color, Grid, Icon, showHUD } from "@raycast/api";
import { usePromise } from "@raycast/utils";
import { AboutView } from "./about-view";
import { label, type Painting } from "./cli";
import { fail, list, setWallpaper } from "./wallbar";

const SECTIONS = ["Light", "Dark", "Other"] as const;

async function set(p: Painting) {
  try {
    await showHUD(label(await setWallpaper(p)));
  } catch (e) {
    await fail(e, "Couldn't set the wallpaper");
  }
}

export default function Command() {
  const { data, isLoading, revalidate } = usePromise(list, [], {
    onError: (e) => fail(e, "Couldn't list wallpapers"),
  });
  const items = data?.items ?? [];
  const currentFile = data?.current ?? null;

  return (
    <Grid
      isLoading={isLoading}
      columns={4}
      aspectRatio="16/9"
      fit={Grid.Fit.Fill}
      searchBarPlaceholder="Search wallpapers"
      selectedItemId={currentFile ?? undefined}
    >
      {!isLoading && items.length === 0 && (
        <Grid.EmptyView icon={Icon.Image} title="No wallpapers" description="Add images to the wallpaper folder." />
      )}
      {SECTIONS.map((section) => {
        const group = items.filter((p) => (p.mood ?? "Other") === section);
        if (group.length === 0) return null;
        return (
          <Grid.Section key={section} title={section} subtitle={String(group.length)}>
            {group.map((p) => {
              const isCurrent = p.file === currentFile;
              return (
                <Grid.Item
                  key={p.file}
                  id={p.file}
                  content={{ value: { source: p.path }, tooltip: label(p) }}
                  title={p.title}
                  subtitle={p.artist}
                  keywords={[p.artist, p.file, ...(p.mood ? [p.mood] : [])]}
                  accessory={
                    isCurrent
                      ? { icon: { source: Icon.CheckCircle, tintColor: Color.Green }, tooltip: "Current wallpaper" }
                      : undefined
                  }
                  actions={
                    <ActionPanel>
                      <Action title="Set as Wallpaper" icon={Icon.Desktop} onAction={() => set(p)} />
                      <Action.Push
                        title="About This Painting"
                        icon={Icon.Info}
                        shortcut={{ modifiers: ["cmd"], key: "i" }}
                        target={<AboutView painting={p} isCurrent={isCurrent} onChange={revalidate} />}
                      />
                      <Action.ShowInFinder path={p.path} shortcut={{ modifiers: ["cmd", "shift"], key: "f" }} />
                    </ActionPanel>
                  }
                />
              );
            })}
          </Grid.Section>
        );
      })}
    </Grid>
  );
}
