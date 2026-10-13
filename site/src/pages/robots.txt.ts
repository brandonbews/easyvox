// robots.txt, from the same flag as the noindex tag (NOINDEX in src/config.ts): allow everything,
// or, with NOINDEX on, ask crawlers to stay out (any page they reach anyway says noindex).
import type { APIRoute } from "astro";
import { NOINDEX } from "../config";

export const GET: APIRoute = () =>
  new Response(NOINDEX ? "User-agent: *\nDisallow: /\n" : "User-agent: *\nAllow: /\n", {
    headers: { "Content-Type": "text/plain; charset=utf-8" },
  });
