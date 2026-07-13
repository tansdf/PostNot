import { createServer } from "node:http";
import { mkdir, readFile } from "node:fs/promises";
import { extname, join, normalize, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import assert from "node:assert/strict";

import { chromium } from "playwright";
import AxeBuilder from "@axe-core/playwright";

const repoRoot = resolve(fileURLToPath(new URL("..", import.meta.url)));
const docsRoot = join(repoRoot, "docs");
const outputRoot = join(repoRoot, ".tmp", "docs-site-visuals");
const mimeTypes = new Map([
  [".css", "text/css; charset=utf-8"],
  [".html", "text/html; charset=utf-8"],
  [".js", "text/javascript; charset=utf-8"],
  [".json", "application/json; charset=utf-8"],
  [".png", "image/png"],
  [".svg", "image/svg+xml"],
  [".webp", "image/webp"],
  [".xml", "application/xml; charset=utf-8"],
  [".txt", "text/plain; charset=utf-8"]
]);

const releaseFixture = {
  tag_name: "v9.8.7",
  published_at: "2026-07-12T00:16:38Z",
  html_url: "https://github.com/tansdf/PostNot/releases/tag/v9.8.7",
  assets: [
    "PostNot_9.8.7_x64-setup.exe",
    "PostNot_9.8.7_x64-setup.exe.sig",
    "PostNot_9.8.7_x64_en-US.msi",
    "PostNot_9.8.7_aarch64.dmg",
    "PostNot_9.8.7_x64.dmg",
    "PostNot_9.8.7_amd64.AppImage",
    "PostNot_9.8.7_amd64.AppImage.sig",
    "PostNot_9.8.7_amd64.deb",
    "PostNot-9.8.7-1.x86_64.rpm"
  ].map((name, index) => ({
    name,
    size: 7_500_000 + index * 100_000,
    digest: `sha256:${String(index + 1).repeat(64).slice(0, 64)}`,
    browser_download_url: `https://example.test/${name}`
  }))
};

function createStaticServer() {
  return createServer(async (request, response) => {
    const pathname = new URL(request.url || "/", "http://127.0.0.1").pathname;
    const relativePath = pathname === "/" ? "index.html" : pathname.replace(/^\//, "");
    const filePath = normalize(join(docsRoot, relativePath));
    if (!filePath.startsWith(docsRoot)) {
      response.writeHead(403).end("Forbidden");
      return;
    }
    try {
      const body = await readFile(filePath);
      response.writeHead(200, { "Content-Type": mimeTypes.get(extname(filePath)) || "application/octet-stream" });
      response.end(body);
    } catch {
      response.writeHead(404).end("Not found");
    }
  });
}

async function withFixtureRelease(page) {
  await page.route("https://api.github.com/repos/tansdf/PostNot/releases/latest", (route) =>
    route.fulfill({ status: 200, contentType: "application/json", body: JSON.stringify(releaseFixture) })
  );
}

async function assertNoOverflow(page, width) {
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
  assert.equal(overflow, 0, `${width}px layout has ${overflow}px horizontal overflow`);
}

async function captureThemes(browser, baseUrl) {
  for (const viewport of [
    { name: "desktop", width: 1440, height: 1000 },
    { name: "mobile", width: 390, height: 844 }
  ]) {
    const context = await browser.newContext({ viewport });
    const page = await context.newPage();
    await withFixtureRelease(page);
    await page.goto(baseUrl, { waitUntil: "networkidle" });
    for (const theme of ["light", "dark", "forest"]) {
      await page.locator(`[data-site-theme="${theme}"]`).click();
      const accessibility = await new AxeBuilder({ page }).analyze();
      assert.deepEqual(
        accessibility.violations,
        [],
        `${viewport.name}/${theme} accessibility violations:\n${accessibility.violations
          .map((violation) => `${violation.id}: ${violation.help}`)
          .join("\n")}`
      );
      await page.evaluate(() => window.scrollTo(0, 0));
      await page.waitForTimeout(100);
      await page.screenshot({ path: join(outputRoot, `home-${viewport.name}-${theme}.png`), fullPage: true });
    }
    await assertNoOverflow(page, viewport.width);
    await context.close();
  }
}

async function testReleaseEnhancement(browser, baseUrl) {
  const page = await browser.newPage({ viewport: { width: 960, height: 900 } });
  await withFixtureRelease(page);
  await page.goto(baseUrl, { waitUntil: "networkidle" });
  const platformIconGeometry = await page.locator(".platform-mark").evaluateAll((marks) =>
    marks.map((mark) => {
      const markRect = mark.getBoundingClientRect();
      const iconRect = mark.querySelector("svg").getBoundingClientRect();
      return {
        width: iconRect.width,
        height: iconRect.height,
        centerOffsetX: Math.abs(markRect.x + markRect.width / 2 - (iconRect.x + iconRect.width / 2)),
        centerOffsetY: Math.abs(markRect.y + markRect.height / 2 - (iconRect.y + iconRect.height / 2))
      };
    })
  );
  assert.equal(platformIconGeometry.length, 3);
  for (const geometry of platformIconGeometry) {
    assert.equal(geometry.width, platformIconGeometry[0].width);
    assert.equal(geometry.height, platformIconGeometry[0].height);
    assert.equal(geometry.centerOffsetX < 0.5, true);
    assert.equal(geometry.centerOffsetY < 0.5, true);
  }
  assert.equal(await page.locator("[data-release-version]").textContent(), "v9.8.7");
  const releaseLinkBoxes = await page.locator(".release-summary__links a").evaluateAll((links) =>
    links.map((link) => {
      const rect = link.getBoundingClientRect();
      return { left: rect.left, right: rect.right };
    })
  );
  assert.equal(releaseLinkBoxes.length, 2);
  assert.equal(releaseLinkBoxes[1].left - releaseLinkBoxes[0].right >= 12, true, "Release actions need a clear visual gap");
  assert.equal(await page.locator(".trust-strip span > strong").count(), 5);
  assert.equal(await page.locator(".trust-strip span > small").count(), 5);
  const setupLink = page.locator(".asset-row", { hasText: "Setup executable" });
  assert.match(await setupLink.getAttribute("href"), /PostNot_9\.8\.7_x64-setup\.exe$/);
  assert.match(await setupLink.locator("[data-asset-meta]").textContent(), /SHA-256/);
  assert.equal(await page.locator(".asset-signature").count() > 0, true);
  await page.close();
}

async function testMarketingNavigation(browser, baseUrl) {
  const page = await browser.newPage({ viewport: { width: 1080, height: 736 }, reducedMotion: "reduce" });
  await withFixtureRelease(page);
  await page.goto(baseUrl, { waitUntil: "networkidle" });
  const headerBottom = await page.locator(".site-header").evaluate((header) => header.getBoundingClientRect().bottom);

  for (const targetId of ["product", "privacy", "download"]) {
    await page.locator(`.nav-links a[href="#${targetId}"]`).click();
    const targetTop = await page.locator(`#${targetId}`).evaluate((target) => target.getBoundingClientRect().top);
    assert.equal(targetTop >= headerBottom - 1, true, `${targetId} must not scroll underneath the sticky header`);
    assert.equal(targetTop <= headerBottom + 12, true, `${targetId} should align closely with the sticky-header edge`);
  }
  await page.close();
}

async function testReleaseFallback(browser, baseUrl) {
  const page = await browser.newPage({ viewport: { width: 960, height: 900 } });
  await page.route("https://api.github.com/repos/tansdf/PostNot/releases/latest", (route) => route.abort());
  await page.goto(baseUrl, { waitUntil: "networkidle" });
  assert.match(await page.locator("[data-release-status]").textContent(), /unavailable/i);
  for (const link of await page.locator("[data-asset-match]").all()) {
    assert.equal(await link.getAttribute("href"), "https://github.com/tansdf/PostNot/releases/latest");
  }
  await page.close();
}

async function testMobileInteractions(browser, baseUrl) {
  const page = await browser.newPage({ viewport: { width: 390, height: 844 } });
  await withFixtureRelease(page);
  await page.goto(baseUrl, { waitUntil: "networkidle" });
  const menu = page.locator("[data-menu-toggle]");
  await menu.click();
  assert.equal(await menu.getAttribute("aria-expanded"), "true");
  await page.keyboard.press("Escape");
  assert.equal(await menu.getAttribute("aria-expanded"), "false");
  assert.equal(await menu.evaluate((element) => element === document.activeElement), true);

  const lazyImage = page.locator('img[loading="lazy"]').first();
  await lazyImage.scrollIntoViewIfNeeded();
  await lazyImage.evaluate((image) => image.decode());
  assert.equal(await lazyImage.evaluate((image) => image.naturalWidth > 0), true);

  const trigger = page.locator("[data-screenshot-src]").first();
  await trigger.click();
  assert.equal(await page.locator("main").evaluate((element) => element.inert), true);
  await page.keyboard.press("Shift+Tab");
  assert.equal(await page.locator(".screenshot-modal__backdrop").evaluate((element) => element === document.activeElement), true);
  await page.keyboard.press("Tab");
  assert.equal(await page.locator(".screenshot-modal__close").evaluate((element) => element === document.activeElement), true);
  await page.keyboard.press("Escape");
  assert.equal(await page.locator("main").evaluate((element) => element.inert), false);
  assert.equal(await trigger.evaluate((element) => element === document.activeElement), true);
  await assertNoOverflow(page, 390);
  await page.close();
}

async function testWithoutJavaScript(browser, baseUrl) {
  const context = await browser.newContext({ javaScriptEnabled: false, viewport: { width: 390, height: 844 } });
  const page = await context.newPage();
  await page.goto(baseUrl);
  assert.equal(await page.locator("[data-site-nav]").isVisible(), true);
  assert.equal(await page.locator("[data-release-fallback]").isVisible(), true);
  assert.equal(await page.locator("[data-asset-match]").first().getAttribute("href"), "https://github.com/tansdf/PostNot/releases/latest");
  await assertNoOverflow(page, 390);
  await context.close();
}

async function testDocumentation(browser, baseUrl) {
  const page = await browser.newPage({ viewport: { width: 390, height: 844 } });
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto(`${baseUrl}scripting.html`, { waitUntil: "networkidle" });
  assert.equal(await page.locator(".doc-mobile-toc").isVisible(), true);
  assert.equal(await page.locator(".heading-anchor").count() >= 12, true);
  await page.locator("[data-doc-mobile-toc]").selectOption("#examples");
  await page.waitForFunction(() => location.hash === "#examples" || document.querySelector("#examples").getBoundingClientRect().top < innerHeight);
  const headerBottom = await page.locator(".site-header").evaluate((header) => header.getBoundingClientRect().bottom);
  const examplesTop = await page.locator("#examples").evaluate((section) => section.getBoundingClientRect().top);
  assert.equal(examplesTop >= headerBottom - 1, true, `Scripting section top ${examplesTop}px is above sticky header bottom ${headerBottom}px`);
  assert.equal(examplesTop <= headerBottom + 12, true, `Scripting section top ${examplesTop}px is too far below sticky header bottom ${headerBottom}px`);
  await assertNoOverflow(page, 390);
  await page.close();
}

async function main() {
  await mkdir(outputRoot, { recursive: true });
  const server = createStaticServer();
  await new Promise((resolveListen) => server.listen(0, "127.0.0.1", resolveListen));
  const address = server.address();
  const baseUrl = `http://127.0.0.1:${address.port}/`;
  const browser = await chromium.launch();
  try {
    await captureThemes(browser, baseUrl);
    await testReleaseEnhancement(browser, baseUrl);
    await testMarketingNavigation(browser, baseUrl);
    await testReleaseFallback(browser, baseUrl);
    await testMobileInteractions(browser, baseUrl);
    await testWithoutJavaScript(browser, baseUrl);
    await testDocumentation(browser, baseUrl);
    for (const width of [720, 960]) {
      const page = await browser.newPage({ viewport: { width, height: 900 } });
      await withFixtureRelease(page);
      await page.goto(baseUrl, { waitUntil: "networkidle" });
      await assertNoOverflow(page, width);
      await page.close();
    }
    console.log(`Docs browser tests passed. Visual snapshots: ${outputRoot}`);
  } finally {
    await browser.close();
    await new Promise((resolveClose, rejectClose) => server.close((error) => error ? rejectClose(error) : resolveClose()));
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
