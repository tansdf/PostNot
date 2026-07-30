import { defineConfig, devices } from "playwright/test";

const appE2ePort = process.env.POSTNOT_APP_E2E_PORT ?? "1420";
const appE2eBaseUrl = `http://127.0.0.1:${appE2ePort}`;

export default defineConfig({
  testDir: "./e2e/app",
  testMatch: "**/*.e2e.mjs",
  outputDir: "./test-results/app-e2e",
  fullyParallel: false,
  forbidOnly: true,
  retries: 0,
  workers: 1,
  reporter: [["list"], ["html", { outputFolder: "playwright-report/app-e2e", open: "never" }]],
  use: {
    baseURL: appE2eBaseUrl,
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
    video: "retain-on-failure"
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] }
    }
  ],
  webServer: {
    command: `npm run dev -- --host 127.0.0.1 --port ${appE2ePort}`,
    url: `${appE2eBaseUrl}/websockets`,
    reuseExistingServer: false,
    timeout: 120_000,
    stdout: "pipe",
    stderr: "pipe"
  }
});
