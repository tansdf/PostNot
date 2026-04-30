import { browser } from "$app/environment";
import { listen } from "@tauri-apps/api/event";

import { checkForUpdates, getSettings, hasTauriRuntime, installUpdate } from "$lib/api/commands";
import type { AvailableUpdate, UpdateDownloadProgress } from "$lib/api/types";
import { notifications } from "$lib/stores/notifications.svelte";

type UpdateCheckMode = "silent" | "manual";

class UpdaterStore {
  initialized = $state(false);
  isBootstrapping = $state(false);
  phase = $state<"idle" | "checking" | "installing">("idle");
  configured = $state<boolean | null>(null);
  availableUpdate = $state<AvailableUpdate | null>(null);
  lastCheckedAt = $state<string | null>(null);
  errorText = $state("");
  installProgress = $state<UpdateDownloadProgress | null>(null);
  private unlistenDownloadProgress: (() => void) | null = null;

  get isChecking() {
    return this.phase === "checking";
  }

  get isInstalling() {
    return this.phase === "installing";
  }

  get installProgressPercent() {
    const progress = this.installProgress;

    if (!progress?.contentLength) {
      return progress?.finished ? 100 : null;
    }

    return Math.min(
      100,
      Math.max(0, Math.round((progress.downloadedBytes / progress.contentLength) * 100))
    );
  }

  get installProgressLabel() {
    const progress = this.installProgress;

    if (!progress) {
      return "";
    }

    if (progress.contentLength) {
      return `${formatBytes(progress.downloadedBytes)} of ${formatBytes(progress.contentLength)}`;
    }

    if (progress.downloadedBytes > 0) {
      return `${formatBytes(progress.downloadedBytes)} downloaded`;
    }

    return "Preparing download...";
  }

  get isRefreshInFlight() {
    return this.phase === "checking" && !!this.availableUpdate;
  }

  get checkButtonLabel() {
    if (this.phase !== "checking") {
      return this.availableUpdate ? "Refresh" : "Check now";
    }

    return this.availableUpdate ? "Refreshing..." : "Checking...";
  }

  async initialize() {
    if (!browser || this.initialized || this.isBootstrapping) {
      return;
    }

    this.isBootstrapping = true;

    try {
      const settings = await getSettings();
      this.lastCheckedAt = settings.lastUpdateCheckedAt;
      this.errorText = "";
      this.initialized = true;
      await this.listenForDownloadProgress();
    } catch {
      this.initialized = true;
    } finally {
      this.isBootstrapping = false;
    }
  }

  async ensureSilentCheck() {
    if (!browser || !hasTauriRuntime()) {
      return;
    }

    await this.initialize();

    if (this.isChecking || this.isInstalling) {
      return;
    }

    await this.runCheck("silent");
  }

  async checkManually() {
    await this.runCheck("manual");
  }

  async installAvailableUpdate() {
    if (!this.availableUpdate) {
      return;
    }

    const targetVersion = this.availableUpdate.version;
    this.phase = "installing";
    this.errorText = "";
    this.installProgress = {
      downloadedBytes: 0,
      contentLength: null,
      finished: false
    };

    try {
      notifications.info(
        `Downloading v${targetVersion}. PostNot will close when the installer takes over.`,
        "Downloading update"
      );
      await installUpdate();

      this.availableUpdate = null;
      this.installProgress = null;
      notifications.success(
        `The update installer for v${targetVersion} has been handed off. If PostNot is still open, you can close it and let the installer finish.`,
        "Installer started"
      );
    } catch (error) {
      this.errorText = error instanceof Error ? error.message : String(error);
      this.installProgress = null;
      notifications.error(
        `${this.errorText} The update is still available, so you can retry without checking again.`,
        "Update install failed"
      );
    } finally {
      this.phase = "idle";
    }
  }

  private async runCheck(mode: UpdateCheckMode) {
    if (!browser || !hasTauriRuntime() || this.isChecking || this.isInstalling) {
      return;
    }

    this.phase = "checking";
    if (mode === "manual") {
      this.errorText = "";
    }

    try {
      const result = await this.withTimeout(
        checkForUpdates(),
        30_000,
        "Update check timed out. Please try again."
      );

      this.configured = result.configured;
      this.availableUpdate = result.update;
      this.lastCheckedAt = new Date().toISOString();
      this.installProgress = null;
      this.phase = "idle";

      if (mode === "manual") {
        if (result.update) {
          notifications.success(`Version ${result.update.version} is available.`, "Update found");
        } else {
          notifications.info("No newer signed release is available.", "No update found");
        }
      }
    } catch (error) {
      if (mode === "manual") {
        this.errorText = error instanceof Error ? error.message : String(error);
      }
      this.phase = "idle";

      if (mode === "manual") {
        notifications.error(this.errorText, "Update check failed");
      }
    }
  }

  private withTimeout<T>(promise: Promise<T>, timeoutMs: number, message: string) {
    return new Promise<T>((resolve, reject) => {
      const timeoutId = window.setTimeout(() => {
        reject(new Error(message));
      }, timeoutMs);

      promise.then(
        (value) => {
          window.clearTimeout(timeoutId);
          resolve(value);
        },
        (error) => {
          window.clearTimeout(timeoutId);
          reject(error);
        }
      );
    });
  }

  private async listenForDownloadProgress() {
    if (!hasTauriRuntime() || this.unlistenDownloadProgress) {
      return;
    }

    this.unlistenDownloadProgress = await listen<UpdateDownloadProgress>(
      "update-download-progress",
      (event) => {
        this.installProgress = event.payload;
      }
    );
  }
}

export const updater = new UpdaterStore();

function formatBytes(bytes: number) {
  if (!Number.isFinite(bytes) || bytes <= 0) {
    return "0 B";
  }

  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unitIndex = 0;

  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex += 1;
  }

  const precision = value >= 10 || unitIndex === 0 ? 0 : 1;
  return `${value.toFixed(precision)} ${units[unitIndex]}`;
}
