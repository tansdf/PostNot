import { mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";

const tempRoot = process.platform === "win32" ? tmpdir() : "/tmp";
const vitestTemp = resolve(tempRoot, "postnot-vitest");
mkdirSync(vitestTemp, { recursive: true });

const vitestCli = resolve("node_modules", "vitest", "vitest.mjs");
const result = spawnSync(process.execPath, [vitestCli, "run"], {
  stdio: "inherit",
  env: {
    ...process.env,
    TMPDIR: vitestTemp,
    TEMP: vitestTemp,
    TMP: vitestTemp
  }
});

process.exit(result.status ?? 1);
