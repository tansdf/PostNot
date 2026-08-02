import { expect, test } from "playwright/test";

const routes = [
  { path: "/", name: "requests" },
  { path: "/websockets", name: "websockets" },
  { path: "/collections", name: "collections" },
  { path: "/environments", name: "environments" },
  { path: "/playbooks", name: "playbooks" },
  { path: "/activity", name: "activity" },
  { path: "/settings", name: "settings" }
];

const viewports = [
  { name: "desktop", width: 1440, height: 1000, panelInset: "20px" },
  { name: "compact", width: 720, height: 1200, panelInset: "16px" }
];

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    localStorage.clear();
    localStorage.setItem(
      "postnot.settings",
      JSON.stringify({
        theme: "light",
        uiScale: 1,
        realtimeConnectTimeoutMs: 30_000,
        realtimeMaxConcurrentSessions: 20,
        realtimeMaxMessageBytes: 64 * 1024 * 1024,
        realtimeTranscriptMaxEntries: 2_000,
        realtimeTranscriptMaxBytes: 64 * 1024 * 1024
      })
    );
  });
});

test("sidebar reserves its flexible middle region for collections", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto("/");

  const workspaceNavigation = page.getByRole("navigation", { name: "Workspaces" });
  await expect(workspaceNavigation.getByRole("link")).toHaveCount(3);
  await expect(workspaceNavigation.getByRole("link", { name: "Requests" })).toHaveAttribute("aria-current", "page");

  const collectionsLink = page.getByRole("link", { name: "Collections" });
  await expect(collectionsLink).toHaveAttribute("href", "/collections");
  const createCollectionButton = page.getByRole("button", { name: "Create collection" });
  await expect(createCollectionButton.locator("svg")).toBeVisible();
  await expect(createCollectionButton).toHaveText("");
  await expect.poll(() => createCollectionButton.evaluate((button) => {
    const rect = button.getBoundingClientRect();
    return { width: rect.width, height: rect.height };
  })).toEqual({ width: 32, height: 32 });

  const utilities = page.getByRole("navigation", { name: "Utilities" });
  await expect(utilities.getByRole("link")).toHaveCount(3);
  await expect(utilities.getByRole("link", { name: "MCP integration" })).toContainText("MCP");
  const settingsLink = utilities.getByRole("link", { name: "Settings" });
  await expect(settingsLink).toHaveAttribute("title", "Settings");
  await expect(settingsLink).toHaveText("");

  const geometry = await page.locator("aside.sidebar").evaluate((sidebar) => {
    const workspaceNav = sidebar.querySelector(".sidebar-nav")?.getBoundingClientRect();
    const collectionTree = sidebar.querySelector(".sidebar-section-scroll")?.getBoundingClientRect();
    const utilityNav = sidebar.querySelector(".sidebar-utility-nav")?.getBoundingClientRect();
    if (!workspaceNav || !collectionTree || !utilityNav) throw new Error("Sidebar regions are missing");
    return {
      workspaceHeight: workspaceNav.height,
      collectionHeight: collectionTree.height,
      collectionBottom: collectionTree.bottom,
      utilityTop: utilityNav.top
    };
  });

  expect(geometry.workspaceHeight).toBeLessThanOrEqual(40);
  expect(geometry.collectionHeight).toBeGreaterThan(geometry.workspaceHeight * 4);
  expect(geometry.collectionBottom).toBeLessThanOrEqual(geometry.utilityTop);
});

for (const viewport of viewports) {
  for (const route of routes) {
    test(`${route.name} follows panel spacing contract at ${viewport.name}`, async ({ page }, testInfo) => {
      await page.setViewportSize({ width: viewport.width, height: viewport.height });
      await page.goto(route.path);

      const panels = page.locator("main.workspace .panel");
      await panels.first().waitFor({ state: "attached" });

      const audit = await panels.evaluateAll((nodes) =>
        nodes.map((node) => {
          const style = getComputedStyle(node);
          const mode = node.classList.contains("panel-inset")
            ? "standard"
            : node.classList.contains("panel-inset-compact")
              ? "compact"
              : node.classList.contains("panel-flush")
                ? "flush"
                : node.classList.contains("panel-custom-inset")
                  ? "custom"
                  : "missing";
          return {
            classes: node.className,
            mode,
            paddingTop: style.paddingTop,
            paddingRight: style.paddingRight,
            paddingBottom: style.paddingBottom,
            paddingLeft: style.paddingLeft
          };
        })
      );

      expect(audit.filter((panel) => panel.mode === "missing"), JSON.stringify(audit, null, 2)).toEqual([]);
      for (const panel of audit) {
        if (panel.mode === "standard") {
          expect(
            [panel.paddingTop, panel.paddingRight, panel.paddingBottom, panel.paddingLeft],
            panel.classes
          ).toEqual(Array(4).fill(viewport.panelInset));
        } else if (panel.mode === "compact") {
          expect(
            [panel.paddingTop, panel.paddingRight, panel.paddingBottom, panel.paddingLeft],
            panel.classes
          ).toEqual(Array(4).fill("12px"));
        } else if (panel.mode === "flush") {
          expect(
            [panel.paddingTop, panel.paddingRight, panel.paddingBottom, panel.paddingLeft],
            panel.classes
          ).toEqual(Array(4).fill("0px"));
        }
      }

      const headingMargins = await page
        .locator(
          "main.workspace :is(.panel-heading > .eyebrow, .panel-heading > h1, .panel-heading > h2, .panel-heading > h3, .panel-heading > p, .editor-header > h1, .editor-header > h2, .editor-header > h3, .collections-column-header > h2)"
        )
        .evaluateAll((nodes) =>
          nodes.map((node) => ({
            text: node.textContent?.trim().slice(0, 80) ?? "",
            marginTop: getComputedStyle(node).marginTop,
            marginBottom: getComputedStyle(node).marginBottom
          }))
        );
      expect(
        headingMargins.filter((heading) => heading.marginTop !== "0px" || heading.marginBottom !== "0px"),
        JSON.stringify(headingMargins, null, 2)
      ).toEqual([]);

      const headingGaps = await page
        .locator("main.workspace .panel-heading")
        .evaluateAll((nodes) => nodes.map((node) => getComputedStyle(node).gap));
      expect(headingGaps.filter((gap) => gap !== "4px"), JSON.stringify(headingGaps, null, 2)).toEqual([]);

      expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth + 1)).toBe(true);

      const screenshotPath = testInfo.outputPath(`${route.name}-${viewport.name}.png`);
      await page.screenshot({ path: screenshotPath, fullPage: false });
      await testInfo.attach(`${route.name}-${viewport.name}`, {
        path: screenshotPath,
        contentType: "image/png"
      });
    });
  }
}
