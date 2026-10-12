// Reads the published docs from the repo's docs/ folder at build time. They're never copied
// into site/: there's one copy of each doc, and GitHub and the site both show it.
//
// The site always builds from site/ (the npm scripts, CI and Cloudflare Pages, which builds in
// site/ from a full checkout), so the repo root is one folder up.

import fs from "node:fs";
import path from "node:path";
import { type RenderedDoc, renderMarkdown } from "./markdown";
import { docsNav, type NavEntry } from "./nav";

export const REPO_ROOT = path.resolve(process.cwd(), "..");
const DOCS_ROOT = path.join(REPO_ROOT, "docs");

let rendering: Promise<Map<string, RenderedDoc>> | undefined;

async function renderAll(): Promise<Map<string, RenderedDoc>> {
  if (!fs.existsSync(path.join(REPO_ROOT, "Cargo.toml")) || !fs.existsSync(DOCS_ROOT)) {
    throw new Error(`expected the EasyVox repo at ${REPO_ROOT}: build the site from site/`);
  }
  const docs = new Map<string, RenderedDoc>();
  for (const entry of docsNav) {
    const source = fs.readFileSync(path.join(DOCS_ROOT, entry.file), "utf8");
    docs.set(entry.file, await renderMarkdown(source, entry.file, REPO_ROOT));
  }

  // Every #anchor that points into a published doc must name a heading there.
  const broken = [...docs.values()]
    .flatMap((doc) => doc.anchors)
    .filter((a) => !docs.get(a.file)?.ids.has(a.id))
    .map((a) => `${a.from}: no #${a.id} in docs/${a.file}`);
  if (broken.length > 0) {
    throw new Error(`broken anchors in the docs:\n  ${broken.join("\n  ")}`);
  }
  return docs;
}

// Every published doc is rendered once per build, and all their links and anchors are checked
// together: one broken link fails the build.
export async function docHtml(entry: NavEntry): Promise<string> {
  rendering ??= renderAll();
  const doc = (await rendering).get(entry.file);
  if (!doc) throw new Error(`docs/${entry.file} isn't in the nav`);
  return doc.html;
}
