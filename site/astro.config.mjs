// EasyVox's website: static HTML and CSS, no JavaScript on any page.
// The docs are read from ../docs at build time (src/lib/docs.ts); nothing is copied.
import { defineConfig } from "astro/config";

export default defineConfig({
  output: "static",
  // Pages build to /docs/settings/index.html, served at /docs/settings.
  build: { format: "directory" },
  // Astro 7 defaults to 'jsx', which drops the space between inline elements
  // (`<b>a</b> <i>b</i>` renders as "ab"). true keeps one space, as Astro 6 did.
  compressHTML: true,
  // Off: no prefetch script on any page.
  prefetch: false,
  devToolbar: { enabled: false },
});
