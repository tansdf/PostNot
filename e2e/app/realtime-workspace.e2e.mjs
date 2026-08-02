import AxeBuilder from "@axe-core/playwright";
import { expect, test } from "playwright/test";

// Enterprise UX gates: task clarity, state visibility and recovery,
// accessibility, data safety, cross-feature consistency, responsive density,
// and actionable error handling.

async function expectNoSeriousAccessibilityViolations(page) {
  const results = await new AxeBuilder({ page }).analyze();
  const blocking = results.violations.filter((violation) =>
    violation.impact === "critical" || violation.impact === "serious"
  );
  expect(blocking, JSON.stringify(blocking, null, 2)).toEqual([]);
}

async function capture(page, testInfo, name) {
  const path = testInfo.outputPath(`${name}.png`);
  await page.screenshot({ path, fullPage: true });
  await testInfo.attach(name, { path, contentType: "image/png" });
}

async function resolveCssColor(locator, customProperty) {
  return locator.evaluate((node, propertyName) => {
    const probe = document.createElement("span");
    probe.style.backgroundColor = `var(${propertyName})`;
    node.append(probe);
    const resolvedColor = getComputedStyle(probe).backgroundColor;
    probe.remove();
    return resolvedColor;
  }, customProperty);
}

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    if (localStorage.getItem("postnot.settings") !== null) {
      return;
    }
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

test("Requests and realtime share the tokenized JSON editor behavior", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("button", { name: "Body", exact: true }).click();
  await page.getByLabel("Body type").selectOption("json");
  const requestJsonEditor = page.getByLabel("JSON request body");

  await requestJsonEditor.fill("{}");
  await requestJsonEditor.evaluate((node) => {
    node.setSelectionRange(1, 1);
  });
  await requestJsonEditor.press("Enter");
  await expect(requestJsonEditor).toHaveValue("{\n  \n}");

  await requestJsonEditor.fill('{"tenant":"{{tenant_id}}","enabled":true}');
  await expect(requestJsonEditor).toHaveClass(/variable-input-highlighted/);
  const requestPanel = page.locator(".request-panel");
  await expect(requestPanel.locator(".json-editor-overlay .jt-key").first()).toContainText('"tenant"');
  await expect(requestPanel.locator(".json-editor-overlay .jt-variable")).toContainText("{{tenant_id}}");
  await requestPanel.getByRole("button", { name: "Format", exact: true }).click();
  await expect(requestJsonEditor).toHaveValue('{\n  "tenant": "{{tenant_id}}",\n  "enabled": true\n}');

  await page.getByRole("button", { name: "Auth", exact: true }).click();
  const requestAuthEditor = requestPanel.locator(".auth-editor");
  await requestAuthEditor.getByLabel("Auth type").selectOption("oauth2");
  await expect(requestAuthEditor.getByLabel("Access token")).toBeVisible();
  await expect(requestAuthEditor.getByRole("button", { name: "Fetch token" })).toBeVisible();
});

test("Requests and realtime share query and header editing standards", async ({ page }, testInfo) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Headers", exact: true }).click();

  const requestHeaders = page.locator(".request-panel .key-value-editor");
  const requestHeaderName = requestHeaders.locator('input[placeholder="Header"]').first();
  await expect(requestHeaders.getByRole("heading", { name: "Headers" })).toBeVisible();
  await expect(requestHeaderName).toHaveAttribute("list", /key-value-name-suggestions-/);
  const requestNameListId = await requestHeaderName.getAttribute("list");
  const requestHeaderNames = await page.locator(`#${requestNameListId} option`).evaluateAll((options) =>
    options.map((option) => option.value)
  );
  expect(requestHeaderNames).toContain("Content-Type");

  await requestHeaderName.fill("Content-Type");
  const requestHeaderValue = requestHeaders.locator('input[placeholder="Value"]').first();
  const requestValueListId = await requestHeaderValue.getAttribute("list");
  const requestHeaderValues = await page.locator(`#${requestValueListId} option`).evaluateAll((options) =>
    options.map((option) => option.value)
  );
  expect(requestHeaderValues).toContain("application/json");
  await capture(page, testInfo, "request-shared-header-editor");

  await page.goto("/websockets");
  const settingsTabs = page.getByRole("tablist", { name: "Connection settings" });
  await settingsTabs.getByRole("tab", { name: "Headers & cookies" }).click();

  const realtimeHeaders = page.locator("#realtime-settings-panel .key-value-editor");
  const realtimeHeaderName = realtimeHeaders.locator('input[placeholder="Header"]').first();
  await expect(realtimeHeaders.getByRole("heading", { name: "Headers" })).toBeVisible();
  await expect(realtimeHeaders.locator(".editor-header")).toHaveCSS("align-items", "center");
  await expect(realtimeHeaders.getByRole("button", { name: "Add Cookie header" })).toHaveCount(0);
  await expect(realtimeHeaderName).toHaveAttribute("list", /key-value-name-suggestions-/);
  const realtimeNameListId = await realtimeHeaderName.getAttribute("list");
  const realtimeHeaderNames = await page.locator(`#${realtimeNameListId} option`).evaluateAll((options) =>
    options.map((option) => option.value)
  );
  expect(realtimeHeaderNames).toEqual(requestHeaderNames);

  await realtimeHeaderName.fill("Content-Type");
  const realtimeHeaderValue = realtimeHeaders.locator('input[placeholder="Value"]').first();
  const realtimeValueListId = await realtimeHeaderValue.getAttribute("list");
  const realtimeHeaderValues = await page.locator(`#${realtimeValueListId} option`).evaluateAll((options) =>
    options.map((option) => option.value)
  );
  expect(realtimeHeaderValues).toEqual(requestHeaderValues);

  await realtimeHeaders.getByRole("button", { name: "Add row" }).click();
  await expect(realtimeHeaders.locator('input[placeholder="Header"]')).toHaveCount(2);
  await realtimeHeaders.locator('input[placeholder="Header"]').nth(1).fill("Cookie");
  await expect(realtimeHeaders.locator('input[placeholder="Header"]').nth(1)).toHaveValue("Cookie");
  await capture(page, testInfo, "realtime-shared-header-editor");

  await settingsTabs.getByRole("tab", { name: "Query" }).click();
  await expect(page.locator("#realtime-settings-panel .key-value-editor")).toBeVisible();
});

test("WebSocket workspace supports tabs, protocol editing, mock sessions, transcript tools, and safe close", async ({
  page
}, testInfo) => {
  await page.goto("/websockets");

  await expect(page.getByRole("heading", { name: "WebSocket connection" })).toBeVisible();
  await expect(page.locator(".realtime-editor")).toHaveCSS("padding", "20px");
  await expect(page.locator(".realtime-transcript-panel")).toHaveCSS("padding", "20px");
  await expect(page.getByRole("heading", { name: "WebSocket connection" })).toHaveCSS("margin", "0px");
  await expect(page.getByText("Disconnected", { exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: "Connect" })).toBeEnabled();

  await page.getByRole("button", { name: "Connect" }).click();
  await expect(page.getByText("Connected", { exact: true }).first()).toBeVisible();
  const closeOptionsButton = page.getByRole("button", { name: "Close options" });
  await closeOptionsButton.click();
  await expect(closeOptionsButton).toHaveAttribute("aria-expanded", "true");
  await expect(page.locator("#realtime-close-options")).toBeVisible();
  await page.getByRole("button", { name: "Disconnect" }).click();
  await expect(page.getByText("Disconnected", { exact: true }).first()).toBeVisible();
  await expect(page.locator("#realtime-close-options")).toHaveCount(0);

  const settingsPanel = page.locator("#realtime-settings-panel");
  const queryEditor = settingsPanel.locator(".editor-block");
  await expect(queryEditor.getByRole("heading", { name: "Query Parameters" })).toBeVisible();
  await expect(queryEditor.locator(".editor-header").getByRole("button", { name: "Add row" })).toBeVisible();
  await expect(queryEditor.locator(".row-add-button")).toHaveCount(0);
  await queryEditor.getByRole("button", { name: "Add row" }).click();
  await expect(queryEditor.locator(".kv-row")).toHaveCount(2);
  await queryEditor.getByRole("button", { name: "Remove parameter row 2" }).click();
  await expect(queryEditor.locator(".kv-row")).toHaveCount(1);

  const realtimeComposer = page.locator(".realtime-composer");
  await expect(realtimeComposer.locator(".request-save-split")).toBeVisible();
  await expect(realtimeComposer.locator(".request-send-actions")).toBeVisible();
  await realtimeComposer.getByLabel("Payload type").selectOption("json");
  const jsonMessageEditor = realtimeComposer.getByLabel("JSON message");
  await jsonMessageEditor.fill('{"message":"{{api_token}}","attempt":1}');
  await expect(jsonMessageEditor).toHaveClass(/variable-input-highlighted/);
  await expect(realtimeComposer.locator(".json-editor-overlay .jt-key").first()).toContainText('"message"');
  await expect(realtimeComposer.locator(".json-editor-overlay .jt-variable")).toContainText("{{api_token}}");
  await realtimeComposer.getByRole("button", { name: "Format", exact: true }).click();
  await expect(jsonMessageEditor).toHaveValue('{\n  "message": "{{api_token}}",\n  "attempt": 1\n}');
  await capture(page, testInfo, "realtime-shared-json-editor");

  const settingsTabs = page.getByRole("tablist", { name: "Connection settings" });
  await settingsTabs.getByRole("tab", { name: "Auth" }).click();
  const authEditor = settingsPanel.locator(".editor-block");
  await expect(authEditor).toHaveClass(/auth-editor/);
  await expect(authEditor.getByRole("heading", { name: "Auth" })).toBeVisible();
  await expect(authEditor.getByLabel("Auth type")).toHaveClass(/body-mode-select/);
  await expect(authEditor.getByText("This connection will be opened without authentication.")).toBeVisible();
  await authEditor.getByLabel("Auth type").selectOption("bearer");
  await expect(authEditor.getByLabel("Token")).toBeVisible();
  const selectedControlBackground = await resolveCssColor(settingsTabs, "--control-selected-bg");
  await expect(settingsTabs.getByRole("tab", { name: "Auth" })).toHaveCSS("background-color", selectedControlBackground);
  await capture(page, testInfo, "realtime-shared-auth-editor");
  await authEditor.getByLabel("Auth type").selectOption("oauth2");
  await expect(authEditor.getByLabel("Access token")).toBeVisible();
  await expect(authEditor.getByRole("button", { name: "Fetch token" })).toHaveCount(0);

  await settingsTabs.getByRole("tab", { name: "Reconnect" }).click();
  const reconnectCheckbox = settingsPanel.getByLabel("Reconnect automatically");
  await expect(reconnectCheckbox).toHaveClass(/row-toggle/);
  await expect(reconnectCheckbox).toHaveClass(/settings-checkbox/);
  await expect(settingsPanel.getByLabel("Maximum attempts")).toBeDisabled();
  await reconnectCheckbox.check();
  await expect(reconnectCheckbox).toBeChecked();
  await expect(reconnectCheckbox).toHaveCSS("background-color", selectedControlBackground);
  const reconnectCheckmarkTransform = await reconnectCheckbox.evaluate((node) =>
    getComputedStyle(node, "::before").transform
  );
  expect(reconnectCheckmarkTransform).not.toBe("none");
  await expect(settingsPanel.getByLabel("Maximum attempts")).toBeEnabled();
  await expect(settingsTabs.getByRole("tab", { name: "Reconnect" })).toHaveCSS("background-color", selectedControlBackground);
  await capture(page, testInfo, "realtime-styled-reconnect-toggle");

  await page.getByLabel("Name", { exact: true }).fill("Billing events");
  await expect(page.getByText("Unsaved connection changes", { exact: true })).toBeVisible();
  await page.getByLabel("Connection URL").fill("wss://events.example.test/billing");
  await page.locator(".realtime-profile-manager").getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByLabel("Connection profile", { exact: true })).not.toHaveValue("");
  await expect(page.getByLabel("Connection profile", { exact: true }).locator('option[value=""]')).toHaveAttribute("disabled", "");
  await page.getByRole("button", { name: "Open a new WebSocket tab" }).click();
  const connectionTabs = page.getByRole("tablist", { name: "Open realtime connections" }).getByRole("tab");
  await expect(connectionTabs).toHaveCount(2);
  await expect(page.locator(".request-tab-chip").nth(1).locator('[title="Close Untitled WebSocket connection"]')).toBeVisible();
  await expect(page.locator(".request-tabs-strip > .request-tab-create")).toBeVisible();
  await connectionTabs.first().focus();
  await page.keyboard.press("ArrowRight");
  await expect(connectionTabs.nth(1)).toHaveAttribute("aria-selected", "true");
  await expectNoSeriousAccessibilityViolations(page);

  await page.getByLabel("Name", { exact: true }).fill("Uncommitted realtime draft");
  await page.locator('[title="Close Uncommitted realtime draft"]').click();
  const closeDialog = page.getByRole("dialog", { name: "Close connection tab?" });
  await expect(closeDialog).toBeVisible();
  expect(await closeDialog.evaluate((node) => node.contains(document.activeElement))).toBe(true);
  await expect(page.getByText("Unsaved connection or message changes will be discarded.")).toBeVisible();
  await page.getByRole("button", { name: "Cancel" }).click();

  await page.getByLabel("Connection protocol").selectOption("socketio");
  await expect(page.getByRole("heading", { name: "Socket.IO connection" })).toBeVisible();
  await expect(page.getByLabel("Message protocol")).toHaveValue("websocket");
  await expect(page.getByText(/message is incompatible with the selected Socket.IO connection/)).toBeVisible();
  await page.getByLabel("Message protocol").selectOption("socketio");
  await settingsTabs.getByRole("tab", { name: "Query" }).focus();
  await page.keyboard.press("ArrowRight");
  await expect(settingsTabs.getByRole("tab", { name: "Headers & cookies" })).toHaveAttribute("aria-selected", "true");
  await page.getByRole("tab", { name: "Protocol" }).click();
  await expect(page.getByLabel("Engine.IO path")).toHaveValue("/socket.io/");
  await page.getByLabel("Auth payload (JSON object)").fill("[");
  await expect(page.getByRole("alert")).toContainText("Unexpected");
  await expect(page.getByRole("button", { name: "Connect" })).toBeDisabled();
  await page.getByLabel("Auth payload (JSON object)").fill('{"tenant":"acme"}');
  await expect(page.getByLabel("Auth payload (JSON object)")).toHaveClass(/variable-input-highlighted/);
  await expect(settingsPanel.locator(".json-editor-overlay .jt-key")).toContainText('"tenant"');

  await page.getByLabel("Arguments (JSON array)").fill("{}");
  await expect(page.getByText("Event arguments must be a JSON array.")).toBeVisible();
  await page.getByLabel("Arguments (JSON array)").fill('[{"invoiceId":"inv_42"}]');
  await expect(page.getByLabel("Arguments (JSON array)")).toHaveClass(/variable-input-highlighted/);
  const ackCheckbox = page.getByLabel("Wait for acknowledgement");
  await expect(ackCheckbox).toHaveClass(/row-toggle/);
  await expect(ackCheckbox).toHaveClass(/settings-checkbox/);
  await page.getByLabel("Payload type").selectOption("binary");
  await expect(page.getByLabel("Binary source")).toBeVisible();
  await page.getByLabel("Payload type").selectOption("json");

  await page.getByRole("button", { name: "Connect" }).click();
  await expect(page.getByRole("region", { name: "Socket.IO connection" }).getByText("Connected", { exact: true })).toBeVisible();
  await expect(page.getByRole("log").getByText("Connected", { exact: true })).toBeVisible();
  await expect(page.getByRole("log")).toHaveAttribute("aria-label", "Realtime session messages");
  await expect(page.getByText(/Connection settings are locked while this session is active/)).toBeVisible();
  await expect(page.getByLabel("Connection profile", { exact: true })).toBeDisabled();
  await expect(page.getByLabel("Name", { exact: true })).toBeDisabled();
  await expect(page.getByLabel("Connection protocol")).toBeDisabled();
  await expect(page.getByLabel("Connection URL")).toBeDisabled();
  await expect(page.getByLabel("Message name")).toBeEnabled();
  await expect(page.getByRole("button", { name: "Send" })).toBeEnabled();
  await page.getByRole("button", { name: "Send" }).click();
  await expect(page.getByRole("log").getByText("Sent", { exact: true })).toBeVisible();
  await expect(page.getByRole("log").getByText("Received", { exact: true })).toBeVisible();
  await expect(page.getByRole("log").getByRole("button", { name: "Copy" })).toHaveCount(2);

  await page.getByRole("tab", { name: "All", exact: true }).focus();
  await page.keyboard.press("ArrowRight");
  await expect(page.getByRole("tab", { name: "Sent", exact: true })).toHaveAttribute("aria-selected", "true");
  await expect(page.getByRole("log").getByText("Sent", { exact: true })).toBeVisible();
  await expect(page.getByRole("log").getByText("Received", { exact: true })).toBeHidden();

  await page.getByRole("tab", { name: "Events", exact: true }).click();
  await expect(page.getByRole("log").getByText("Connected", { exact: true })).toBeVisible();
  await page.getByPlaceholder("Search messages and events").fill("does-not-match");
  await expect(page.getByText("No matching messages")).toBeVisible();
  await page.getByPlaceholder("Search messages and events").fill("");
  for (let index = 0; index < 14; index += 1) {
    await page.getByRole("button", { name: "Send" }).click();
  }
  await page.getByRole("log").evaluate((node) => {
    node.scrollTop = 0;
    node.dispatchEvent(new Event("scroll"));
  });
  await expect(page.getByRole("button", { name: "Follow latest" })).toBeVisible();
  await page.getByRole("button", { name: "Follow latest" }).click();
  await page.getByRole("log").scrollIntoViewIfNeeded();
  await capture(page, testInfo, "realtime-workspace-transcript");
  await page.getByRole("button", { name: "Clear" }).click();
  await expect(page.getByText("No session messages yet")).toBeVisible();

  await realtimeComposer.getByRole("button", { name: "More save actions" }).click();
  await realtimeComposer.getByRole("menuitem", { name: "Save as" }).click();
  const saveDialog = page.getByRole("dialog", { name: "Save realtime message" });
  await expect(saveDialog).toBeVisible();
  await expect(saveDialog).toHaveClass(/panel-custom-inset/);
  await expect(saveDialog).toHaveCSS("padding", "20px");
  await expect(saveDialog.getByRole("heading", { name: "Save realtime message" })).toHaveCSS("margin", "0px");
  await expect(page.getByRole("listbox", { name: "Choose a collection" })).toBeVisible();
  await page.getByRole("button", { name: "Cancel" }).click();

  await capture(page, testInfo, "realtime-workspace-desktop-light");
});

test("WebSockets navigation preserves the active draft and defers external message replacement", async ({ page }) => {
  await page.goto("/websockets");

  const sidebar = page.locator("aside.sidebar");
  await sidebar.getByRole("button", { name: "Expand collection" }).first().click();
  await sidebar.getByRole("button", { name: /Live order events/ }).click();
  await expect(page.getByLabel("Message name")).toHaveValue("Live order events");

  await page.getByLabel("Message name").fill("Unsaved local message");
  await expect(page.getByText("Unsaved message changes", { exact: true })).toBeVisible();

  await page.getByRole("link", { name: "Requests", exact: true }).click();
  await page.getByRole("link", { name: "WebSockets", exact: true }).click();
  await expect(page.getByRole("heading", { name: "WebSocket connection" })).toBeVisible();
  await expect(page.getByLabel("Message name")).toHaveValue("Unsaved local message");
  await expect(page.getByRole("dialog", { name: "Replace message draft?" })).toHaveCount(0);

  await page.getByRole("link", { name: "Requests", exact: true }).click();
  await sidebar.getByRole("button", { name: /Support presence/ }).click();
  await expect(page.getByRole("heading", { name: "WebSocket connection" })).toBeVisible();
  const replaceDialog = page.getByRole("dialog", { name: "Replace message draft?" });
  await expect(replaceDialog).toBeVisible();
  await expect(replaceDialog.getByText("The connection and session transcript will be preserved.")).toBeVisible();
  await replaceDialog.getByRole("button", { name: "Cancel" }).click();
  await expect(page.getByLabel("Message name")).toHaveValue("Unsaved local message");
  await expect(page).toHaveURL(/messageId=mock-realtime-websocket-1/);

  await page.getByRole("link", { name: "Requests", exact: true }).click();
  await sidebar.getByRole("button", { name: /Support presence/ }).click();
  await expect(replaceDialog).toBeVisible();
  await replaceDialog.getByRole("button", { name: "Discard and open" }).click();
  await expect(page.getByLabel("Message name")).toHaveValue("Support presence");
  await expect(page.getByLabel("Connection URL")).toHaveValue("ws://localhost:8080");
});

test("WebSockets deep links load a profile and message independently", async ({ page }) => {
  await page.goto("/websockets?profileId=mock-websocket-profile&messageId=mock-realtime-websocket-1");

  await expect(page.getByLabel("Connection profile", { exact: true })).toHaveValue("mock-websocket-profile");
  await expect(page.getByLabel("Connection URL")).toHaveValue("wss://events.example.test/orders");
  await expect(page.getByLabel("Message name")).toHaveValue("Live order events");
  await expect(page.getByLabel("Message protocol")).toHaveValue("websocket");
  await expect(page).toHaveURL(/profileId=mock-websocket-profile/);
  await expect(page).toHaveURL(/messageId=mock-realtime-websocket-1/);
});

test("New message keeps the current protocol and live session", async ({ page }) => {
  await page.goto("/websockets");

  await page.getByRole("button", { name: "Connect" }).click();
  await expect(page.getByText("Connected", { exact: true }).first()).toBeVisible();
  const composer = page.locator(".realtime-composer");
  await composer.getByRole("button", { name: "Send", exact: true }).click();
  await expect(page.getByRole("log").getByText("Sent", { exact: true })).toBeVisible();

  const sidebar = page.locator("aside.sidebar");
  await sidebar.getByRole("button", { name: "Expand collection" }).first().click();
  await sidebar.getByRole("button", { name: /Support presence/ }).click();
  await expect(page.getByLabel("Message protocol")).toHaveValue("socketio");
  await expect(composer.getByRole("button", { name: "Update", exact: true })).toBeVisible();
  await expect(page).toHaveURL(/messageId=mock-realtime-socketio-1/);

  await page.getByLabel("Message name").fill("Unsaved Socket.IO edits");
  await composer.getByRole("button", { name: "New", exact: true }).click();
  const newMessageDialog = page.getByRole("dialog", { name: "Start a new message?" });
  await expect(newMessageDialog).toBeVisible();
  await expect(newMessageDialog.getByText("The connection and session transcript will be preserved.")).toBeVisible();
  await newMessageDialog.getByRole("button", { name: "Cancel" }).click();
  await expect(page.getByLabel("Message name")).toHaveValue("Unsaved Socket.IO edits");

  await composer.getByRole("button", { name: "New", exact: true }).click();
  await newMessageDialog.getByRole("button", { name: "Discard and create" }).click();
  await expect(page.getByLabel("Message name")).toHaveValue("Untitled Socket.IO message");
  await expect(page.getByLabel("Message protocol")).toHaveValue("socketio");
  await expect(page.getByLabel("Event")).toHaveValue("message");
  await expect(composer.getByRole("button", { name: "Save", exact: true })).toBeVisible();
  await expect(page).not.toHaveURL(/messageId=/);
  await expect(page.getByText("Connected", { exact: true }).first()).toBeVisible();
  await expect(page.getByRole("log").getByText("Sent", { exact: true })).toBeVisible();
});

test("settings expose bounded realtime controls and persist saved preferences", async ({ page }, testInfo) => {
  await page.goto("/settings");

  await expect(page.getByLabel("Connect timeout (seconds)")).toHaveValue("30");
  await expect(page.getByLabel("Maximum live sessions")).toHaveValue("20");
  await expect(page.getByLabel("Maximum message (MiB)")).toHaveValue("64");
  await expect(page.getByLabel("Transcript entries per session")).toHaveValue("2000");
  await expect(page.getByLabel("Transcript retained data per session (MiB)")).toHaveValue("64");
  await expect(page.getByText("Transcripts are session-only and never restored; large payloads use temporary files cleared on release or startup.")).toBeVisible();
  await expectNoSeriousAccessibilityViolations(page);
  await page.getByRole("heading", { name: "WebSockets" }).scrollIntoViewIfNeeded();
  await capture(page, testInfo, "realtime-settings-light");
  await expect.poll(() => page.locator("form.settings-form").evaluate((form) => form.checkValidity())).toBe(true);

  await page.getByLabel("Request timeout (ms)").fill("45000");
  await page.getByLabel("Follow redirects automatically").uncheck();
  await page.getByRole("button", { name: "Save settings" }).click();
  await expect(page.getByText("Settings saved", { exact: true })).toBeVisible();
  await expect.poll(() => page.evaluate(() => {
    const cached = JSON.parse(localStorage.getItem("postnot.settings") ?? "{}");
    return { requestTimeoutMs: cached.requestTimeoutMs, followRedirects: cached.followRedirects };
  })).toEqual({ requestTimeoutMs: 45_000, followRedirects: false });

  await page.reload();
  await expect(page.getByLabel("Request timeout (ms)")).toHaveValue("45000");
  await expect(page.getByLabel("Follow redirects automatically")).not.toBeChecked();
});

test("collections explain lossless PostNot portability and Postman realtime omissions", async ({ page }, testInfo) => {
  await page.goto("/collections");
  await page.getByRole("button", { name: "Export", exact: true }).click();
  const exportDialog = page.getByRole("dialog", { name: "Export collection" });
  await expect(exportDialog.getByText("Lossless export for HTTP, WebSocket, Socket.IO, folders, and scripts.")).toBeVisible();
  await exportDialog.getByRole("radio", { name: /Postman Collection v2.1/ }).check();
  await expect(exportDialog.getByText("WebSocket and Socket.IO definitions cannot be represented by the Postman export format.")).toBeVisible();
  await capture(page, testInfo, "collection-export-portability");
  await exportDialog.getByRole("button", { name: "Cancel" }).click();

  await page.getByRole("button", { name: "Import", exact: true }).first().click();
  const importDialog = page.getByRole("dialog", { name: "Import" });
  await importDialog.getByRole("tab", { name: "PostNot" }).click();
  await expect(importDialog.getByText("Import a lossless PostNot collection, including WebSocket and Socket.IO definitions.")).toBeVisible();
  await expectNoSeriousAccessibilityViolations(page);
  await importDialog.getByRole("button", { name: "Cancel" }).click();

  const sidebar = page.locator("aside.sidebar");
  await sidebar.getByRole("button", { name: "Expand collection" }).first().click();
  await expect(sidebar.getByText("WS", { exact: true })).toBeVisible();
  await expect(sidebar.getByText("S.IO", { exact: true })).toBeVisible();
  await sidebar.getByRole("button", { name: /Live order events/ }).click();
  await expect(page).toHaveURL(/\/websockets\?.*messageId=mock-realtime-websocket-1/);
  await expect(page.getByLabel("Message name")).toHaveValue("Live order events");
  await expect(page.getByLabel("Connection URL")).toHaveValue("ws:\/\/localhost:8080");
  await expect(page.locator(".realtime-composer").getByRole("button", { name: "Update", exact: true })).toBeVisible();
  await capture(page, testInfo, "collection-realtime-routing");
});

for (const scenario of [
  { name: "desktop-dark", width: 1440, height: 1000, theme: "dark", scale: 1 },
  { name: "compact-980", width: 980, height: 1100, theme: "light", scale: 1 },
  { name: "compact-720", width: 720, height: 1200, theme: "dark", scale: 1 },
  { name: "desktop-125-scale", width: 1440, height: 1000, theme: "light", scale: 1.25 }
]) {
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
    await expectNoSeriousAccessibilityViolations(page);
    await capture(page, testInfo, `realtime-${scenario.name}`);
  });
}
