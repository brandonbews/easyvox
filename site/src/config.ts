// The switches that change at launch (docs/plans/website.md, §9 step 10). Launch is the day the
// repo goes public, at or before the first DMG.

export const SITE_NAME = "EasyVox";

// While true, every page carries <meta name="robots" content="noindex"> and robots.txt
// disallows everything. Set to false at launch.
export const NOINDEX = true;

// While false, nothing links to GitHub: the repo is private, so every such link would 404.
// Links in the docs to repo files outside the published set render as plain text instead
// (src/lib/markdown.ts). Set to true when the repo goes public.
export const REPO_PUBLIC = false;

export const REPO_URL = "https://github.com/brandonbews/easyvox";

// A GitHub URL for a file or folder in the repo, or null while the repo is private.
export function repoUrl(repoPath: string, kind: "blob" | "tree" = "blob"): string | null {
  return REPO_PUBLIC ? `${REPO_URL}/${kind}/main/${repoPath}` : null;
}
