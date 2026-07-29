import AxeBuilder from "@axe-core/playwright";
import { expect, test, type Page, type TestInfo } from "playwright/test";

async function expectNoSeriousAccessibilityViolations(page: Page) {
  const results = await new AxeBuilder({ page }).analyze();
  const blocking = results.violations.filter((violation) =>
    violation.impact === "critical" || violation.impact === "serious"
  );
  expect(blocking, JSON.stringify(blocking, null, 2)).toEqual([]);
}

async function capture(page: Page, testInfo: TestInfo, name: string) {
  const path = testInfo.outputPath(`${name}.png`);
  await page.screenshot({ path, fullPage: true });
  await testInfo.attach(name, { path, contentType: "image/png" });
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

test("WebSocket workspace supports tabs, protocol editing, mock sessions, transcript tools, and safe close", async ({
  page
}, testInfo) => {
  await page.goto("/websockets");

  await expect(page.getByRole("heading", { name: "WebSocket connection" })).toBeVisible();
  await expect(page.getByText("Disconnected", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Connect" })).toBeEnabled();
  await page.getByLabel("Name").fill("Billing events");
  await page.getByLabel("Connection URL").fill("wss://events.example.test/billing");
  await page.getByRole("button", { name: "Open a new WebSocket tab" }).click();
  await expect(page.getByRole("tab", { name: /WebSocket/ })).toHaveCount(2);
  const connectionTabs = page.getByRole("tablist", { name: "Open realtime connections" }).getByRole("tab");
  await connectionTabs.first().focus();
  await page.keyboard.press("ArrowRight");
  await expect(connectionTabs.nth(1)).toHaveAttribute("aria-selected", "true");
  await expectNoSeriousAccessibilityViolations(page);

  await page.getByRole("button", { name: "Close Untitled WebSocket request" }).click();
  await expect(page.getByRole("dialog", { name: "Close connection tab?" })).toBeVisible();
  await expect(page.getByText("Unsaved changes in this draft will be discarded.")).toBeVisible();
  await page.getByRole("button", { name: "Cancel" }).click();

  await page.getByLabel("Mode").selectOption("socketio");
  await expect(page.getByRole("heading", { name: "Socket.IO connection" })).toBeVisible();
  await page.getByRole("tab", { name: "Protocol" }).click();
  await expect(page.getByLabel("Engine.IO path")).toHaveValue("/socket.io/");
  await page.getByLabel("Auth payload (JSON object)").fill("[");
  await expect(page.getByRole("alert")).toContainText("Unexpected");
  await expect(page.getByRole("button", { name: "Connect" })).toBeDisabled();
  await page.getByLabel("Auth payload (JSON object)").fill('{"tenant":"acme"}');

  await page.getByLabel("Arguments (JSON array)").fill("{}");
  await expect(page.getByText("Event arguments must be a JSON array.")).toBeVisible();
  await page.getByLabel("Arguments (JSON array)").fill('[{"invoiceId":"inv_42"}]');
  await page.getByLabel("Payload type").selectOption("binary");
  await expect(page.getByLabel("Binary source")).toBeVisible();
  await page.getByLabel("Payload type").selectOption("json");

  await page.getByRole("button", { name: "Connect" }).click();
  await expect(page.getByText("Connected", { exact: true })).toBeVisible();
  await expect(page.getByRole("log").getByText("Connected", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Send" })).toBeEnabled();

  await page.getByRole("tab", { name: "Events" }).click();
  await expect(page.getByRole("log").getByText("Connected", { exact: true })).toBeVisible();
  await page.getByPlaceholder("Search messages and events").fill("does-not-match");
  await expect(page.getByText("No matching messages")).toBeVisible();
  await page.getByPlaceholder("Search messages and events").fill("");
  await page.getByRole("button", { name: "Clear" }).click();
  await expect(page.getByText("No session messages yet")).toBeVisible();

  await page.getByRole("button", { name: "Save as…" }).click();
  await expect(page.getByRole("dialog", { name: "Save connection as" })).toBeVisible();
  await expect(page.getByRole("listbox", { name: "Choose a collection" })).toBeVisible();
  await page.getByRole("button", { name: "Cancel" }).click();

  await capture(page, testInfo, "realtime-workspace-desktop-light");
});

test("settings expose bounded realtime controls and persist the selected presentation", async ({ page }, testInfo) => {
  await page.goto("/settings");

  await expect(page.getByLabel("Connect timeout (seconds)")).toHaveValue("30");
  await expect(page.getByLabel("Maximum live sessions")).toHaveValue("20");
  await expect(page.getByLabel("Maximum message (MiB)")).toHaveValue("64");
  await expect(page.getByLabel("Transcript entries per session")).toHaveValue("2000");
  await expect(page.getByLabel("Transcript retained data per session (MiB)")).toHaveValue("64");
  await expect(page.getByText("Transcripts remain in memory only and are never restored after restart.")).toBeVisible();
  await expectNoSeriousAccessibilityViolations(page);
  await capture(page, testInfo, "realtime-settings-light");
});

test("collections explain lossless PostNot portability and Postman realtime omissions", async ({ page }, testInfo) => {
  await page.goto("/collections");
  await page.getByRole("button", { name: "Export", exact: true }).click();
  const exportDialog = page.getByRole("dialog", { name: "Export collection" });
  await expect(exportDialog.getByText("Lossless export for HTTP, WebSocket, Socket.IO, folders, and scripts.")).toBeVisible();
  await exportDialog.getByRole("radio", { name: /Postman Collection v2.1/ }).check();
  await expect(exportDialog.getByText("WebSocket and Socket.IO definitions cannot be represented by the Postman export format.")).toBeVisible();
  await exportDialog.getByRole("button", { name: "Cancel" }).click();

  await page.getByRole("button", { name: "Import", exact: true }).first().click();
  const importDialog = page.getByRole("dialog", { name: "Import" });
  await importDialog.getByRole("tab", { name: "PostNot" }).click();
  await expect(importDialog.getByText("Import a lossless PostNot collection, including WebSocket and Socket.IO definitions.")).toBeVisible();
  await expectNoSeriousAccessibilityViolations(page);
  await capture(page, testInfo, "collection-portability");
});

for (const scenario of [
  { name: "desktop-dark", width: 1440, height: 1000, theme: "dark", scale: 1 },
  { name: "compact-980", width: 980, height: 1100, theme: "light", scale: 1 },
  { name: "compact-720", width: 720, height: 1200, theme: "dark", scale: 1 },
  { name: "desktop-125-scale", width: 1440, height: 1000, theme: "light", scale: 1.25 }
] as const) {
  test(`workspace remains usable at ${scenario.name}`, async ({ page }, testInfo) => {
    await page.setViewportSize({ width: scenario.width, height: scenario.height });
    await page.addInitScript(({ theme, scale }) => {
      const settings = JSON.parse(localStorage.getItem("postnot.settings") ?? "{}");
      localStorage.setItem("postnot.settings", JSON.stringify({ ...settings, theme, uiScale: scale }));
    }, { theme: scenario.theme, scale: scenario.scale });
    await page.goto("/websockets");

    await expect(page.getByRole("heading", { name: "WebSocket connection" })).toBeVisible();
    await expect(page.getByLabel("Connection URL")).toBeVisible();
    await expect(page.getByRole("button", { name: "Connect" })).toBeVisible();
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth + 1)).toBe(true);
    await capture(page, testInfo, `realtime-${scenario.name}`);
  });
}
