// The docs pipeline, which builds an HTML string:
//
//   remark-parse → remark-gfm → remark-rehype → rehype-slug
//     → rewriteLinks (repo-relative links → site routes, GitHub URLs or plain text)
//     → docElements (heading anchors, scrolling tables, code-block wrappers)
//     → rehype-stringify
//
// GFM, never MDX: what renders on GitHub renders here. Raw HTML in a doc is dropped
// (remark-rehype's default). rehype-slug uses github-slugger, so heading ids match GitHub's and
// `file.md#anchor` links work the same in both places.

import fs from "node:fs";
import path from "node:path";
import type { Element, Root } from "hast";
import rehypeSlug from "rehype-slug";
import rehypeStringify from "rehype-stringify";
import remarkGfm from "remark-gfm";
import remarkParse from "remark-parse";
import remarkRehype from "remark-rehype";
import { unified } from "unified";
import { SKIP, visit } from "unist-util-visit";
import { repoUrl } from "../config";
import { findByFile, routeFor } from "./nav";

// A #fragment a link points at, in a published doc, to check once every doc is rendered.
export type AnchorRef = { file: string; id: string; from: string };

export type RenderedDoc = {
  html: string;
  ids: Set<string>;
  anchors: AnchorRef[];
};

function isAbsoluteUrl(url: string): boolean {
  return /^[a-z][a-z0-9+.-]*:/i.test(url) || url.startsWith("//");
}

// What a link in a published doc becomes on the site. Relative links resolve against the doc's
// own folder, as on GitHub:
//
//   https://…, mailto:…                → unchanged
//   #anchor                            → unchanged (checked against the doc's headings)
//   a published doc (settings.md#x)    → its route (/docs/settings#x), anchor kept and checked
//   any other repo file or folder      → its GitHub URL, or null (plain text) while the repo
//                                        is private (REPO_PUBLIC in src/config.ts)
//   a path that doesn't exist          → the build fails
//   a root-relative path (/x)          → the build fails: write it relative, as GitHub resolves it
export function resolveLink(
  url: string,
  docFile: string,
  repoRoot: string,
): { href: string | null; anchor?: AnchorRef } {
  const from = `docs/${docFile}`;
  if (isAbsoluteUrl(url)) return { href: url };
  if (url.startsWith("#")) {
    return { href: url, anchor: { file: docFile, id: decodeURIComponent(url.slice(1)), from } };
  }
  if (url.startsWith("/")) {
    throw new Error(`${from}: root-relative link "${url}"; make it relative to the doc`);
  }

  const hashAt = url.indexOf("#");
  const filePart = decodeURIComponent((hashAt === -1 ? url : url.slice(0, hashAt)).split("?")[0]);
  const hash = hashAt === -1 ? "" : url.slice(hashAt);

  const repoPath = path.posix
    .normalize(path.posix.join("docs", path.posix.dirname(docFile), filePart))
    .replace(/\/$/, "");
  if (repoPath === ".." || repoPath.startsWith("../")) {
    throw new Error(`${from}: link "${url}" points outside the repo`);
  }
  const onDisk = path.join(repoRoot, repoPath);
  if (!fs.existsSync(onDisk)) {
    throw new Error(`${from}: broken link "${url}" (no ${repoPath} in the repo)`);
  }
  const isDir = fs.statSync(onDisk).isDirectory();

  if (repoPath.startsWith("docs/")) {
    const docRel = repoPath.slice("docs/".length);
    const entry = findByFile(isDir ? `${docRel}/README.md` : docRel);
    if (entry) {
      const anchor = hash
        ? { file: entry.file, id: decodeURIComponent(hash.slice(1)), from }
        : undefined;
      return { href: routeFor(entry) + hash, anchor };
    }
  }

  const github = repoUrl(repoPath, isDir ? "tree" : "blob");
  return { href: github === null ? null : github + hash };
}

function rehypeRewriteLinks(options: { docFile: string; repoRoot: string; anchors: AnchorRef[] }) {
  return (tree: Root) => {
    visit(tree, "element", (node, index, parent) => {
      if (node.tagName === "img") {
        // No doc has images yet. When one does, the site will need to copy it; until then a
        // relative image would be broken on the site, so it fails the build.
        const src = String(node.properties.src ?? "");
        if (!isAbsoluteUrl(src)) {
          throw new Error(`docs/${options.docFile}: relative image "${src}" isn't supported yet`);
        }
        return;
      }
      if (node.tagName !== "a" || typeof node.properties.href !== "string") return;

      const { href, anchor } = resolveLink(node.properties.href, options.docFile, options.repoRoot);
      if (anchor) options.anchors.push(anchor);
      if (href !== null) {
        node.properties.href = href;
        return;
      }
      // Private repo: keep the link's text, drop the link that would 404.
      if (!parent || index === undefined) return;
      parent.children.splice(index, 1, ...node.children);
      return [SKIP, index];
    });
  };
}

function hasLink(node: Element): boolean {
  let found = false;
  visit(node, "element", (child) => {
    if (child.tagName === "a") found = true;
  });
  return found;
}

function wrap(node: Element, properties: Element["properties"]): Element {
  return { type: "element", tagName: "div", properties, children: [node] };
}

function rehypeDocElements(options: { ids: Set<string> }) {
  return (tree: Root) => {
    visit(tree, "element", (node, index, parent) => {
      if (typeof node.properties.id === "string") options.ids.add(node.properties.id);
      if (!parent || index === undefined) return;

      // Headings link to themselves. The heading text stays the accessible name.
      if (/^h[1-6]$/.test(node.tagName) && typeof node.properties.id === "string") {
        if (hasLink(node)) return;
        node.children = [
          {
            type: "element",
            tagName: "a",
            properties: { className: ["anchor"], href: `#${node.properties.id}` },
            children: node.children,
          },
        ];
        return SKIP;
      }

      // Wide tables scroll inside their own box, not the page. A scrolling box needs a
      // tabindex, or it can't be scrolled from the keyboard.
      if (node.tagName === "table") {
        parent.children[index] = wrap(node, { className: ["table-scroll"], tabIndex: 0 });
        return [SKIP, index + 1];
      }

      // Fenced code: the <pre> scrolls, and its wrapper shows the language as a badge.
      if (node.tagName === "pre") {
        const code = node.children.find(
          (c): c is Element => c.type === "element" && c.tagName === "code",
        );
        const classes = code?.properties.className;
        const lang = Array.isArray(classes)
          ? classes
              .map(String)
              .find((c) => c.startsWith("language-"))
              ?.slice("language-".length)
          : undefined;
        node.properties.tabIndex = 0;
        parent.children[index] = wrap(node, { className: ["codeblock"], dataLang: lang });
        return [SKIP, index + 1];
      }
    });
  };
}

// Renders one doc. `docFile` is its path in docs/, for resolving its relative links.
export async function renderMarkdown(
  source: string,
  docFile: string,
  repoRoot: string,
): Promise<RenderedDoc> {
  const ids = new Set<string>();
  const anchors: AnchorRef[] = [];
  const file = await unified()
    .use(remarkParse)
    .use(remarkGfm)
    .use(remarkRehype)
    .use(rehypeSlug)
    .use(rehypeRewriteLinks, { docFile, repoRoot, anchors })
    .use(rehypeDocElements, { ids })
    .use(rehypeStringify)
    .process(source);
  return { html: String(file), ids, anchors };
}
