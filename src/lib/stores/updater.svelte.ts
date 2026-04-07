import { browser } from "$app/environment";

import { checkForUpdates, getSettings, hasTauriRuntime, installUpdate } from "$lib/api/commands";
import type { AvailableUpdate } from "$lib/api/types";
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

  get isChecking() {
    return this.phase === "checking";
  }

  get isInstalling() {
    return this.phase === "installing";
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

    try {
      notifications.info(
        `Installing v${targetVersion}. PostNot should close when the installer takes over.`,
        "Applying update"
      );
      await installUpdate();

      this.availableUpdate = null;
      notifications.success(
        `The update installer for v${targetVersion} has been handed off. If PostNot is still open, you can close it and let the installer finish.`,
        "Installer started"
      );
    } catch (error) {
      this.errorText = error instanceof Error ? error.message : String(error);
      notifications.error(this.errorText, "Update install failed");
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
}

export const updater = new UpdaterStore();
