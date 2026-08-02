import { existsSync, readFileSync } from "node:fs";
import { dirname, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const docsRoot = join(repoRoot, "docs");
const failures = [];

function fail(message) {
  failures.push(message);
}

function read(relativePath) {
  const path = join(repoRoot, relativePath);
  if (!existsSync(path)) {
    fail(`${relativePath}: file is missing`);
    return "";
  }
  return readFileSync(path, "utf8");
}

function requirePattern(source, pattern, message) {
  if (!pattern.test(source)) fail(message);
}

function validateHeadings(html, relativePath) {
  const headings = [...html.matchAll(/<h([1-6])\b/gi)].map((match) => Number(match[1]));
  if (headings.filter((level) => level === 1).length !== 1) {
    fail(`${relativePath}: expected exactly one h1`);
  }
  for (let index = 1; index < headings.length; index += 1) {
    if (headings[index] > headings[index - 1] + 1) {
      fail(`${relativePath}: heading level jumps from h${headings[index - 1]} to h${headings[index]}`);
    }
  }
}

function validateLocalReferences(html, relativePath) {
  for (const match of html.matchAll(/(?:href|src)="([^"]+)"/g)) {
    const reference = match[1];
    if (
      !reference ||
      reference.startsWith("#") ||
      reference.startsWith("http://") ||
      reference.startsWith("https://") ||
      reference.startsWith("data:") ||
      reference.startsWith("mailto:")
    ) {
      continue;
    }
    const cleanReference = reference.split(/[?#]/)[0];
    if (!cleanReference || extname(cleanReference) === ".html" && cleanReference === relativePath.split("/").at(-1)) {
      continue;
    }
    if (!existsSync(join(docsRoot, cleanReference))) {
      fail(`${relativePath}: missing local reference ${reference}`);
    }
  }
}

const indexHtml = read("docs/index.html");
const scriptingHtml = read("docs/scripting.html");
const siteJs = read("docs/site.js");
const siteCss = read("docs/site.css");
const screenshotCapture = read("scripts/capture-docs-screenshots.mjs");

for (const [relativePath, html] of [
  ["docs/index.html", indexHtml],
  ["docs/scripting.html", scriptingHtml]
]) {
  validateHeadings(html, relativePath);
  validateLocalReferences(html, relativePath);
  requirePattern(html, /rel="canonical"/, `${relativePath}: canonical URL is missing`);
  requirePattern(html, /property="og:image"/, `${relativePath}: Open Graph image is missing`);
  requirePattern(html, /name="twitter:card"/, `${relativePath}: Twitter card metadata is missing`);
  requirePattern(html, /application\/ld\+json/, `${relativePath}: structured data is missing`);
}

requirePattern(indexHtml, /id="product"/, "docs/index.html: product section is missing");
requirePattern(indexHtml, /href="#product">Features</, "docs/index.html: feature navigation label is missing");
requirePattern(indexHtml, /id="realtime"/, "docs/index.html: realtime workflow section is missing");
requirePattern(indexHtml, /href="#realtime">WebSockets</, "docs/index.html: WebSocket navigation link is missing");
requirePattern(indexHtml, /websockets-page\.webp/, "docs/index.html: WebSocket workspace screenshot is missing");
requirePattern(indexHtml, /Transcript boundary:[^<]*<\/strong>\s*session-only/i, "docs/index.html: session-only transcript boundary is missing");
requirePattern(indexHtml, /id="agents"/, "docs/index.html: agent MCP section is missing");
requirePattern(
  indexHtml,
  /agents cannot execute requests or scripts, open realtime connections, or send traffic/i,
  "docs/index.html: MCP execution boundary is missing"
);
requirePattern(indexHtml, /agent-activity-page\.webp/, "docs/index.html: Agent Activity screenshot is missing");
requirePattern(indexHtml, /id="download"/, "docs/index.html: download section is missing");
requirePattern(indexHtml, /data-download-platform=/, "docs/index.html: platform download cards are missing");
if ((indexHtml.match(/class="platform-mark__icon"/g) ?? []).length !== 3) {
  fail("docs/index.html: download cards must use three consistently sized SVG platform icons");
}
if (/<span class="platform-mark"[^>]*>[⊞●⌁]/.test(indexHtml)) {
  fail("docs/index.html: download cards still use baseline-dependent Unicode platform glyphs");
}
requirePattern(indexHtml, /data-release-fallback/, "docs/index.html: release fallback is missing");
requirePattern(indexHtml, /data-menu-toggle[^>]+aria-expanded="false"/, "docs/index.html: accessible mobile menu toggle is missing");
requirePattern(indexHtml, /Free and open source[^<]*Apache-2\.0/i, "docs/index.html: permanent open-source message is missing");
requirePattern(indexHtml, /SoftwareApplication/, "docs/index.html: SoftwareApplication JSON-LD is missing");

requirePattern(scriptingHtml, /class="doc-mobile-toc"/, "docs/scripting.html: mobile table of contents is missing");
requirePattern(scriptingHtml, /Back to features/, "docs/scripting.html: back-to-features link is missing");
requirePattern(scriptingHtml, /<p class="eyebrow">Scripting<\/p>/, "docs/scripting.html: scripting eyebrow is unclear");
requirePattern(scriptingHtml, /class="heading-anchor"/, "docs/scripting.html: heading permalinks are missing");

if (/Dark theme/.test(indexHtml)) {
  fail("docs/index.html: redundant Dark theme label is still present in the hero preview");
}

requirePattern(siteJs, /api\.github\.com\/repos\/tansdf\/PostNot\/releases\/latest/, "docs/site.js: latest-release API integration is missing");
requirePattern(siteJs, /request\.signal|AbortSignal|AbortController/, "docs/site.js: release request timeout is missing");
requirePattern(siteJs, /\.inert\s*=/, "docs/site.js: modal background inert handling is missing");
requirePattern(siteJs, /event\.shiftKey/, "docs/site.js: focus trap does not handle reverse tabbing");
requirePattern(siteJs, /aria-expanded/, "docs/site.js: mobile menu state management is missing");
requirePattern(siteCss, /prefers-reduced-motion:\s*reduce/, "docs/site.css: reduced-motion treatment is missing");
requirePattern(siteCss, /@media\s*\(max-width:\s*719px\)/, "docs/site.css: mobile layout is missing");
requirePattern(
  screenshotCapture,
  /getByRole\("heading",\s*\{\s*name:\s*"PostNot API"/,
  "scripts/capture-docs-screenshots.mjs: Collections capture must wait for the visible collection heading"
);
requirePattern(
  screenshotCapture,
  /responsiveScreenshotWidth\s*=\s*960/,
  "scripts/capture-docs-screenshots.mjs: capture flow must generate 960px responsive screenshots"
);
requirePattern(
  screenshotCapture,
  /path:\s*"\/websockets\?profileId=mock-websocket-profile&messageId=mock-realtime-websocket-1"/,
  "scripts/capture-docs-screenshots.mjs: WebSocket workflow capture is missing"
);
if (/getByText\("Collection root"\)\.waitFor/.test(screenshotCapture)) {
  fail("scripts/capture-docs-screenshots.mjs: Collections capture waits for dialog-only text");
}

for (const relativePath of [
  "docs/robots.txt",
  "docs/sitemap.xml",
  "docs/site.webmanifest",
  "docs/social-preview.svg"
]) {
  if (!existsSync(join(repoRoot, relativePath))) fail(`${relativePath}: required production asset is missing`);
}

if (failures.length > 0) {
  console.error(`Docs validation failed with ${failures.length} issue(s):`);
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("Docs validation passed.");
