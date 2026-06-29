import { spawn } from "node:child_process";
import { mkdir, rm } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { chromium } from "playwright";

const __dirname = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(__dirname, "..");
const baseUrl = process.env.POSTNOT_SCREENSHOT_URL ?? "http://127.0.0.1:1420";
const imageDir = join(repoRoot, "docs", "images");
const tempDir = join(repoRoot, ".tmp", "docs-screenshots");
const viewport = { width: 1995, height: 1179 };

const now = new Date("2026-06-29T12:00:00.000Z").toISOString();

const settings = {
  theme: "dark",
  uiScale: 1,
  requestTimeoutMs: 30000,
  followRedirects: true,
  validateTls: true,
  historyLimit: 200,
  isHistoryCollapsed: false,
  environmentAutosave: true,
  notificationTimeoutMs: 5000,
  lastUpdateCheckedAt: "2026-06-29T09:30:00.000Z"
};

const requestDraft = {
  name: "Create onboarding note",
  method: "POST",
  url: "{{base_url}}/notes",
  queryParams: [
    { id: "query-1", key: "include", value: "author,workspace", enabled: true },
    { id: "query-2", key: "dry_run", value: "false", enabled: true }
  ],
  headers: [
    { id: "header-1", key: "Accept", value: "application/json", enabled: true },
    { id: "header-2", key: "X-Request-Id", value: "{{$guid}}", enabled: true }
  ],
  body: {
    mode: "json",
    raw: JSON.stringify(
      {
        title: "Welcome packet",
        labels: ["onboarding", "local-first"],
        published: false
      },
      null,
      2
    ),
    form: [{ id: "form-1", key: "", value: "", enabled: true }],
    files: []
  },
  auth: {
    type: "oauth2",
    basicUsername: "",
    basicPassword: "",
    bearerToken: "",
    apiKeyName: "",
    apiKeyValue: "",
    apiKeyIn: "header",
    oauth2AccessToken: "{{access_token}}",
    oauth2TokenUrl: "{{base_url}}/oauth/token",
    oauth2ClientId: "postnot-desktop",
    oauth2ClientSecret: "{{client_secret}}",
    oauth2Scope: "notes:write"
  },
  preRequestScript: "await pn.variables.set('request_nonce', 'docs-preview');",
  testScript: "pn.test('created note', () => {\n  pn.expect(pn.response.code).toBe(201);\n});"
};

const responsePayload = {
  statusCode: 201,
  statusText: "Created",
  durationMs: 128,
  sizeBytes: 214,
  headers: [
    { id: "resp-header-1", key: "content-type", value: "application/json", enabled: true },
    { id: "resp-header-2", key: "x-trace-id", value: "trace-docs-2026", enabled: true }
  ],
  bodyText: JSON.stringify(
    {
      id: "note_42",
      title: "Welcome packet",
      status: "draft",
      savedLocally: true
    },
    null,
    2
  ),
  errorText: "",
  executedAt: now
};

const workspaceTabs = [
  {
    id: "docs-tab-create-note",
    source: "saved",
    savedRequestId: "req-create-note",
    collectionId: "col-postnot-api",
    parentId: "folder-notes",
    request: requestDraft,
    baselineRequest: requestDraft,
    response: responsePayload,
    scriptExecution: {
      preRequestErrorText: "",
      testScriptErrorText: "",
      tests: [
        { id: "test-1", name: "created note", status: "passed", errorText: "" },
        { id: "test-2", name: "response is JSON", status: "passed", errorText: "" }
      ]
    },
    errorText: ""
  },
  {
    id: "docs-tab-list-notes",
    source: "saved",
    savedRequestId: "req-list-notes",
    collectionId: "col-postnot-api",
    parentId: "folder-notes",
    request: {
      ...requestDraft,
      name: "List notes",
      method: "GET",
      url: "{{base_url}}/notes",
      body: { ...requestDraft.body, mode: "none", raw: "" },
      auth: { ...requestDraft.auth, type: "bearer", bearerToken: "{{access_token}}" }
    },
    baselineRequest: null,
    response: null,
    scriptExecution: { preRequestErrorText: "", testScriptErrorText: "", tests: [] },
    errorText: ""
  }
];

const collections = [
  {
    id: "col-postnot-api",
    name: "PostNot API",
    description: "Saved local-first API workflows for docs screenshots.",
    preRequestScript: "await pn.variables.set('run_started_at', new Date().toISOString());",
    testScript: "pn.test('response finished', () => pn.expect(pn.response.durationMs).toBeLessThan(1000));",
    requestCount: 4,
    updatedAt: now
  },
  {
    id: "col-imports",
    name: "Import samples",
    description: "Postman, OpenAPI, and cURL examples.",
    preRequestScript: "",
    testScript: "",
    requestCount: 3,
    updatedAt: now
  }
];

const collectionItems = {
  "col-postnot-api": [
    {
      id: "folder-notes",
      collectionId: "col-postnot-api",
      parentId: null,
      kind: "folder",
      name: "Notes",
      method: null,
      url: null,
      preRequestScript: "await pn.variables.set('folder', 'notes');",
      testScript: "",
      updatedAt: now,
      children: [
        {
          id: "req-create-note",
          collectionId: "col-postnot-api",
          parentId: "folder-notes",
          kind: "request",
          name: "Create onboarding note",
          method: "POST",
          url: "{{base_url}}/notes",
          preRequestScript: requestDraft.preRequestScript,
          testScript: requestDraft.testScript,
          updatedAt: now,
          children: []
        },
        {
          id: "req-list-notes",
          collectionId: "col-postnot-api",
          parentId: "folder-notes",
          kind: "request",
          name: "List notes",
          method: "GET",
          url: "{{base_url}}/notes",
          preRequestScript: "",
          testScript: "",
          updatedAt: now,
          children: []
        }
      ]
    },
    {
      id: "folder-auth",
      collectionId: "col-postnot-api",
      parentId: null,
      kind: "folder",
      name: "Auth",
      method: null,
      url: null,
      preRequestScript: "",
      testScript: "",
      updatedAt: now,
      children: [
        {
          id: "req-token",
          collectionId: "col-postnot-api",
          parentId: "folder-auth",
          kind: "request",
          name: "Client credentials token",
          method: "POST",
          url: "{{base_url}}/oauth/token",
          preRequestScript: "",
          testScript: "",
          updatedAt: now,
          children: []
        }
      ]
    }
  ],
  "col-imports": []
};

const environments = [
  { id: "env-local", name: "Local dark demo", isActive: true, variableCount: 5, updatedAt: now },
  { id: "env-staging", name: "Staging", isActive: false, variableCount: 4, updatedAt: now }
];

const environmentDetail = {
  id: "env-local",
  name: "Local dark demo",
  isActive: true,
  updatedAt: now,
  variables: [
    { id: "env-var-1", key: "base_url", value: "https://api.post-not.local", enabled: true, isSecret: false },
    { id: "env-var-2", key: "access_token", value: "********", enabled: true, isSecret: true },
    { id: "env-var-3", key: "client_secret", value: "********", enabled: true, isSecret: true },
    { id: "env-var-4", key: "workspace_id", value: "wrk_local_docs", enabled: true, isSecret: false },
    { id: "env-var-5", key: "request_nonce", value: "generated-by-script", enabled: true, isSecret: false }
  ]
};

const cache = {
  "postnot.theme": "dark",
  "postnot.uiScale": "1",
  "postnot.settings": settings,
  "postnot.workspace.tabs": workspaceTabs,
  "postnot.workspace.activeTabId": "docs-tab-create-note",
  "postnot.collections.list": collections,
  "postnot.collections.selectedId": "col-postnot-api",
  "postnot.collections.itemsByCollection": collectionItems,
  "postnot.sidebar.expanded": {
    expandedCollectionIds: ["col-postnot-api", "col-imports"],
    expandedFolderIds: ["folder-notes", "folder-auth"]
  },
  "postnot.environments.list": environments,
  "postnot.environments.activeId": "env-local",
  "postnot.environments.activeVarCount": 5,
  "postnot.environments.activeDetailMeta": environmentDetail
};

const captures = [
  {
    path: "/",
    file: "requests-page.webp",
    waitFor: ".workspace-grid",
    beforeCapture: async (page) => {
      await page.getByRole("button", { name: "Send", exact: true }).click();
      await page.getByText("Created").first().waitFor({ timeout: 5000 });
    }
  },
  {
    path: "/",
    file: "request-preview.webp",
    waitFor: ".workspace-grid",
    beforeCapture: async (page) => {
      await page.getByLabel("Preview resolved request").click();
      await page.getByRole("heading", { name: "Resolved Request Preview" }).waitFor();
    }
  },
  {
    path: "/collections?collectionId=mock-collection-1",
    file: "collections-page.webp",
    waitFor: ".collections-page-panel",
    beforeCapture: async (page) => {
      await page.getByText("Collection root").waitFor();
      await page.getByText("Create onboarding note").first().waitFor();
    }
  },
  {
    path: "/playbooks",
    file: "playbooks-page.webp",
    waitFor: ".playbooks-page",
    beforeCapture: async (page) => {
      const run = page.locator(".run-history-item").first();
      if (await run.count()) {
        await run.click();
        await page.waitForTimeout(250);
      }
    }
  },
  { path: "/environments?environmentId=env-local", file: "environments-page.webp", waitFor: ".environment-list" },
  { path: "/settings", file: "settings-page.webp", waitFor: ".settings-page" }
];

async function canReachServer() {
  try {
    const response = await fetch(baseUrl, { signal: AbortSignal.timeout(1000) });
    return response.ok;
  } catch {
    return false;
  }
}

async function waitForServer() {
  const startedAt = Date.now();
  while (Date.now() - startedAt < 60000) {
    if (await canReachServer()) {
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 500));
  }
  throw new Error(`Timed out waiting for ${baseUrl}`);
}

function spawnDevServer() {
  const child = spawn("npm", ["run", "dev"], {
    cwd: repoRoot,
    stdio: "inherit",
    env: { ...process.env, BROWSER: "none" }
  });
  child.on("exit", (code) => {
    if (code !== null && code !== 0) {
      console.error(`Dev server exited with code ${code}`);
    }
  });
  return child;
}

async function convertToWebp(pngPath, webpPath) {
  await new Promise((resolve, reject) => {
    const child = spawn("cwebp", ["-quiet", "-q", "88", pngPath, "-o", webpPath], {
      cwd: repoRoot,
      stdio: "inherit"
    });
    child.on("error", reject);
    child.on("exit", (code) => {
      if (code === 0) resolve();
      else reject(new Error(`cwebp failed for ${pngPath} with code ${code}`));
    });
  });
}

async function main() {
  await mkdir(imageDir, { recursive: true });
  await mkdir(tempDir, { recursive: true });

  let serverProcess = null;
  if (!(await canReachServer())) {
    serverProcess = spawnDevServer();
    await waitForServer();
  }

  const browser = await chromium.launch();
  const context = await browser.newContext({
    viewport,
    deviceScaleFactor: 1,
    colorScheme: "dark"
  });

  await context.addInitScript((seed) => {
    window.localStorage.clear();
    for (const [key, value] of Object.entries(seed)) {
      window.localStorage.setItem(key, typeof value === "string" ? value : JSON.stringify(value));
    }
  }, cache);

  try {
    const page = await context.newPage();

    for (const capture of captures) {
      await page.goto(`${baseUrl}${capture.path}`, { waitUntil: "networkidle" });
      await page.addStyleTag({
        content: `
          * { caret-color: transparent !important; }
          html { scroll-behavior: auto !important; }
        `
      });
      await page.waitForSelector(capture.waitFor, { state: "visible" });
      await page.waitForTimeout(500);

      if (capture.beforeCapture) {
        await capture.beforeCapture(page);
        await page.waitForTimeout(500);
      }

      const pngPath = join(tempDir, capture.file.replace(/\.webp$/, ".png"));
      const webpPath = join(imageDir, capture.file);
      await page.screenshot({ path: pngPath, fullPage: false });
      await convertToWebp(pngPath, webpPath);
      console.log(`Captured docs/images/${capture.file}`);
    }
  } finally {
    await browser.close();
    await rm(tempDir, { recursive: true, force: true });
    if (serverProcess) {
      serverProcess.kill("SIGTERM");
    }
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
