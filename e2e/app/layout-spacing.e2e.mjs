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

const deepSidebarCollection = {
  id: "mock-deep-collection",
  name: "Payments platform",
  description: "Deep collection tree fixture",
  preRequestScript: "",
  testScript: "",
  requestCount: 1,
  updatedAt: "2026-08-13T12:00:00.000Z"
};

const longRequestName = "Replay failed invoice webhook after exhausting every retry for the international payments processing pipeline";
const longRequestUrl = "{{base_url}}/v2/invoices/webhooks/replay/after-all-retries/for-a-very-long-tenant-and-processing-pipeline";

function deepFolder(id, name, parentId, children) {
  return {
    id,
    collectionId: deepSidebarCollection.id,
    parentId,
    kind: "folder",
    name,
    method: null,
    url: null,
    preRequestScript: "",
    testScript: "",
    updatedAt: "2026-08-13T12:00:00.000Z",
    children
  };
}

const deepSidebarItems = [
  deepFolder("mock-deep-core", "Core services", null, [
    deepFolder("mock-deep-api", "API", "mock-deep-core", [
      deepFolder("mock-deep-v2", "v2", "mock-deep-api", [
        deepFolder("mock-deep-billing", "Billing", "mock-deep-v2", [
          deepFolder("mock-deep-invoices", "Invoices", "mock-deep-billing", [
            deepFolder("mock-deep-webhooks", "Webhooks", "mock-deep-invoices", [
              {
                id: "mock-deep-request",
                collectionId: deepSidebarCollection.id,
                parentId: "mock-deep-webhooks",
                kind: "request",
                requestType: "http",
                name: longRequestName,
                method: "POST",
                url: longRequestUrl,
                preRequestScript: "",
                testScript: "",
                updatedAt: "2026-08-13T12:00:00.000Z",
                children: []
              }
            ])
          ])
        ])
      ])
    ])
  ]),
  ...Array.from({ length: 48 }, (_, index) =>
    deepFolder(
      `mock-dense-folder-${index + 1}`,
      index === 7
        ? "A deliberately very long folder name for regional payment reconciliation and settlement archives"
        : `Regional archive ${String(index + 1).padStart(2, "0")}`,
      null,
      []
    )
  )
];

async function seedDeepSidebar(page) {
  await page.evaluate(
    async ({ collection, items }) => {
      const { collections } = await import("/src/lib/stores/collections.svelte.ts");
      collections.collections = [collection];
      collections.collectionItemsByCollection = { [collection.id]: items };
      collections.selectedCollectionId = collection.id;
      collections.initialized = true;
      collections.errorText = "";
    },
    { collection: deepSidebarCollection, items: deepSidebarItems }
  );
}

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

test("collection search follows the compact sidebar navigation language", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto("/");

  const sidebar = page.locator(".app-shell > aside.sidebar");
  const search = sidebar.getByRole("searchbox", { name: "Search collections, folders, and saved requests" });
  const searchShell = sidebar.getByRole("search", { name: "Filter collection navigation" });

  await expect(search).toHaveAttribute("placeholder", "Filter collections");
  await expect(search).toHaveAttribute("aria-keyshortcuts", "Control+K Meta+K");
  await expect(searchShell.locator(".sidebar-search-shortcut")).toHaveText("Ctrl K");

  const idleGeometry = await searchShell.evaluate((shell) => {
    const input = shell.querySelector("input")?.getBoundingClientRect();
    const rect = shell.getBoundingClientRect();
    return {
      shellHeight: rect.height,
      inputHeight: input?.height ?? 0
    };
  });
  expect(idleGeometry.shellHeight).toBe(32);
  expect(idleGeometry.inputHeight).toBeGreaterThanOrEqual(31);
  expect(idleGeometry.inputHeight).toBeLessThanOrEqual(32);

  await page.keyboard.press("Control+K");
  await expect(search).toBeFocused();
  await search.fill("note");

  await expect(searchShell.locator(".sidebar-search-shortcut")).toHaveCount(0);
  await expect(sidebar.getByRole("button", { name: "Clear collection search" })).toBeVisible();
  await expect(sidebar.locator(".sidebar-search-status")).toHaveText("1 result Enter to open");

  const result = sidebar.getByRole("option", { name: /request: PostNot API \/ Examples \/ Create onboarding note/ });
  await expect(result).toBeVisible();
  await expect(result).toContainText("Create onboarding note");
  await expect(result).toContainText("{{base_url}}/notes");
  await expect(result).toContainText("PostNot API / Examples");
  await expect(result.locator(".sidebar-search-updated")).toHaveCount(0);

  const resultGeometry = await result.evaluate((row) => {
    const style = getComputedStyle(row);
    return {
      height: row.getBoundingClientRect().height,
      columns: style.gridTemplateColumns.split(" ").length,
      borderLeftWidth: style.borderLeftWidth,
      backgroundIsTransparent: style.backgroundColor === "rgba(0, 0, 0, 0)"
    };
  });
  expect(resultGeometry.height).toBeLessThanOrEqual(62);
  expect(resultGeometry.columns).toBe(3);
  expect(resultGeometry.borderLeftWidth).toBe("2px");
  expect(resultGeometry.backgroundIsTransparent).toBe(false);

  await sidebar.getByRole("button", { name: "Clear collection search" }).click();
  await expect(search).toHaveValue("");
  await expect(searchShell.locator(".sidebar-search-shortcut")).toHaveText("Ctrl K");
  await expect(sidebar.getByText("PostNot API", { exact: true })).toBeVisible();
});

test("deep and dense collection paths keep long request rows readable without overlay chrome", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 1000 });
  await page.goto("/?savedRequestId=mock-deep-request");
  const sidebar = page.locator(".app-shell > aside.sidebar");
  await expect(sidebar.getByRole("button", { name: "Expand collection" }).first()).toBeVisible();

  await seedDeepSidebar(page);
  await expect(sidebar.getByText("Payments platform", { exact: true })).toBeVisible();
  await sidebar.getByRole("button", { name: "Expand collection" }).click();

  for (const itemId of [
    "mock-deep-core",
    "mock-deep-api",
    "mock-deep-v2",
    "mock-deep-billing",
    "mock-deep-invoices",
    "mock-deep-webhooks"
  ]) {
    await sidebar.locator(`[data-sidebar-item-id="${itemId}"]`).click();
  }

  await expect(sidebar.locator(".sidebar-depth-lens")).toHaveCount(0);
  await expect(sidebar.locator(".sidebar-folder-button")).toHaveCount(54);

  const requestRow = sidebar.locator('[data-sidebar-item-id="mock-deep-request"]');
  await expect(requestRow).toBeVisible();
  await expect(requestRow).toHaveClass(/sidebar-request-active/);
  await expect(requestRow).toContainText(longRequestName);
  await expect(requestRow).toContainText(longRequestUrl);
  await expect(requestRow).toHaveAttribute(
    "title",
    `Payments platform / Core services / API / v2 / Billing / Invoices / Webhooks / ${longRequestName} — ${longRequestUrl}`
  );

  const geometry = await sidebar.evaluate((node) => {
    const itemIds = [
      "mock-deep-core",
      "mock-deep-api",
      "mock-deep-v2",
      "mock-deep-billing",
      "mock-deep-invoices",
      "mock-deep-webhooks",
      "mock-deep-request"
    ];
    const identityLefts = itemIds.map((itemId) => {
      const row = node.querySelector(`[data-sidebar-item-id="${itemId}"]`);
      const identity = row?.querySelector(".sidebar-folder-icon, .sidebar-request-kind");
      return identity?.getBoundingClientRect().left ?? 0;
    });
    const request = node.querySelector('[data-sidebar-item-id="mock-deep-request"]')?.getBoundingClientRect();
    const requestNode = node.querySelector('[data-sidebar-item-id="mock-deep-request"]');
    const requestName = requestNode?.querySelector(".sidebar-request-name");
    const requestUrl = requestNode?.querySelector(".sidebar-request-url");
    const requestCopy = requestNode?.querySelector(".sidebar-request-copy")?.getBoundingClientRect();
    const requestStyle = requestNode ? getComputedStyle(requestNode) : null;
    const nameStyle = requestName ? getComputedStyle(requestName) : null;
    const scrollRegion = node.querySelector(".sidebar-section-scroll");
    return {
      identitySpread: Math.max(...identityLefts) - Math.min(...identityLefts),
      requestWidth: request?.width ?? 0,
      requestHeight: request?.height ?? 0,
      visibleNameLines: requestName && nameStyle
        ? requestName.getBoundingClientRect().height / Number.parseFloat(nameStyle.lineHeight)
        : 0,
      urlIsEllipsized: requestUrl ? requestUrl.scrollWidth > requestUrl.clientWidth : false,
      copyFitsVertically: request && requestCopy && requestStyle
        ? requestCopy.top >= request.top + Number.parseFloat(requestStyle.paddingTop) - 0.5 &&
          requestCopy.bottom <= request.bottom - Number.parseFloat(requestStyle.paddingBottom) + 0.5
        : false,
      hasVerticalOverflow: scrollRegion ? scrollRegion.scrollHeight > scrollRegion.clientHeight + 1 : false,
      hasHorizontalOverflow: scrollRegion ? scrollRegion.scrollWidth > scrollRegion.clientWidth + 1 : true
    };
  });

  expect(geometry.identitySpread).toBeLessThanOrEqual(24);
  expect(geometry.requestWidth).toBeGreaterThan(240);
  expect(geometry.requestHeight).toBeGreaterThan(60);
  expect(geometry.requestHeight).toBeLessThan(76);
  expect(geometry.visibleNameLines).toBeGreaterThan(1.8);
  expect(geometry.visibleNameLines).toBeLessThan(2.2);
  expect(geometry.urlIsEllipsized).toBe(true);
  expect(geometry.copyFitsVertically).toBe(true);
  expect(geometry.hasVerticalOverflow).toBe(true);
  expect(geometry.hasHorizontalOverflow).toBe(false);
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
