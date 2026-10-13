// The launch switches, set when the repo went public and the first DMG shipped.

export const SITE_NAME = "EasyVox";

// While true, every page carries <meta name="robots" content="noindex"> and robots.txt
// disallows everything. Off since the first release.
export const NOINDEX = false;

// While false, nothing links to GitHub. Links in the docs to repo files outside the published
// set render as plain text instead (src/lib/markdown.ts). Set to true when the repo goes public.
export const REPO_PUBLIC = true;

export const REPO_URL = "https://github.com/brandonbews/easyvox";

// The latest release on GitHub, whose page has the DMG: every Download link goes here.
export const DOWNLOAD_URL = `${REPO_URL}/releases/latest`;

// The site's origin, attached at launch: "https://easyvox.dev". While it's null, links and the
// social image stay relative, and there's no canonical URL or og:url.
export const SITE_URL: string | null = "https://easyvox.dev";

// A root-relative path made absolute once SITE_URL is set (og:image must be absolute to work).
export function siteUrl(sitePath: string): string {
  return SITE_URL ? new URL(sitePath, SITE_URL).href : sitePath;
}

// A GitHub URL for a file or folder in the repo, or null while the repo is private.
export function repoUrl(repoPath: string, kind: "blob" | "tree" = "blob"): string | null {
  return REPO_PUBLIC ? `${REPO_URL}/${kind}/main/${repoPath}` : null;
}
