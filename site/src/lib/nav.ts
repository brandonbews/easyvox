// The published docs: a file in ../docs is on the site only if it's listed here. This list is
// the publish boundary, so no doc has to move, and it sets the sidebar order and the
// previous/next links. Titles and descriptions live here, not in the docs, which stay plain
// GitHub Markdown with no frontmatter.
//
// `file` is relative to the repo's docs/ folder. `slug` is the route below /docs.

export type NavEntry = {
  slug: string;
  file: string;
  title: string;
  description: string;
};

export const docsNav: NavEntry[] = [
  {
    slug: "building",
    file: "building.md",
    title: "Build from source",
    description:
      "Build EasyVox on an Apple silicon Mac: the tools, the speech model, the tests and the app bundle.",
  },
  {
    slug: "settings",
    file: "settings.md",
    title: "Settings",
    description:
      "EasyVox's settings file: where it is, how edits apply, and every key with its default, its unit and what it trades off.",
  },
  {
    slug: "model-bench",
    file: "model-bench.md",
    title: "Model bench",
    description:
      "The full October 2026 bench: accuracy, speed, memory and energy for each model and setting, with the methods.",
  },
  {
    slug: "bench",
    file: "bench/README.md",
    title: "Bench data",
    description:
      "The per-run numbers behind the model bench: what each CSV file holds, and how they fit together.",
  },
];

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
