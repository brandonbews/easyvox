// The published docs: a file in ../docs is on the site only if it's listed here. This list is
// the publish boundary, so no doc has to move, and it sets the sidebar order and the
// previous/next links. Titles and descriptions live here, not in the docs, which stay plain
// GitHub Markdown with no frontmatter.
//
// The bench write-ups (model-bench.md, bench/README.md) aren't published: they stay in the repo
// as the full record, and /benchmarks is the site's bench page.
//
// `file` is relative to the repo's docs/ folder. `slug` is the route below /docs. `group` is the
// sidebar heading it sits under; entries of one group stay together, in this order.

export type NavEntry = {
  slug: string;
  file: string;
  title: string;
  description: string;
  group: string;
};

export const docsNav: NavEntry[] = [
  {
    slug: "building",
    group: "Guides",
    file: "building.md",
    title: "Build from source",
    description: "The tools, the speech model, the tests and the app bundle.",
  },
  {
    slug: "settings",
    group: "Guides",
    file: "settings.md",
    title: "Settings",
    description:
      "The settings file: where it is, the keys you're likely to change, and how errors are handled.",
  },
];

// The entries under each sidebar heading, in nav order.
export function navGroups(): { group: string; entries: NavEntry[] }[] {
  const groups: { group: string; entries: NavEntry[] }[] = [];
  for (const entry of docsNav) {
    const last = groups.at(-1);
    if (last?.group === entry.group) last.entries.push(entry);
    else groups.push({ group: entry.group, entries: [entry] });
  }
  return groups;
}

export function routeFor(entry: NavEntry): string {
  return `/docs/${entry.slug}`;
}

export function findByFile(file: string): NavEntry | undefined {
  return docsNav.find((e) => e.file === file);
}

export function prevNext(entry: NavEntry): { prev?: NavEntry; next?: NavEntry } {
  const i = docsNav.indexOf(entry);
  return { prev: docsNav[i - 1], next: docsNav[i + 1] };
}
