<script lang="ts">
  import type { Snippet } from "svelte";
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { getSettings, hasTauriRuntime, listAgentActivity } from "$lib/api/commands";
  import "$lib/styles/app.css";
  import { applyTheme, applyUiScale, watchSystemTheme } from "$lib/theme";
  import AppShell from "$lib/components/layout/AppShell.svelte";
  import { installEditableUndoFallback, shouldInstallEditableUndoFallback } from "$lib/editable-undo";
  import { notifications } from "$lib/stores/notifications.svelte";
  import { updater } from "$lib/stores/updater.svelte";
  import { collections } from "$lib/stores/collections.svelte";
  import { resolve } from "$app/paths";

  let { children }: { children?: Snippet } = $props();

  onMount(() => {
    let unlistenHistoryPersistence: (() => void) | undefined;
    let activityCursor = 0;
    let activityPoll: ReturnType<typeof setInterval> | undefined;
    const uninstallEditableUndoFallback = shouldInstallEditableUndoFallback()
      ? installEditableUndoFallback()
      : undefined;

    if (hasTauriRuntime()) {
      void listen<{ message: string }>("history-persistence-error", (event) => {
        const text = event.payload.message?.trim();
        if (text) {
          notifications.warning(
            `The request failed, and the failure could not be written to history: ${text}`,
            "History not saved"
          );
        }
      }).then((unlisten) => {
        unlistenHistoryPersistence = unlisten;
      });

      void listAgentActivity(undefined, 1).then((page) => {
        activityCursor = page.latestId;
        activityPoll = setInterval(() => void pollAgentActivity(), 2_000);
      });
    }

    async function pollAgentActivity() {
      if (document.visibilityState !== "visible") return;
      try {
        const activity = await listAgentActivity(activityCursor, 250);
        activityCursor = Math.max(activityCursor, activity.latestId);
        if (activity.entries.length === 0) return;
        const changedCollections = new Set(activity.entries.filter((entry) => entry.outcome === "succeeded").map((entry) => entry.collectionId).filter((id): id is string => Boolean(id)));
        if (changedCollections.size > 0) {
          await collections.loadCollections(collections.selectedCollectionId);
          await Promise.all(Array.from(changedCollections).map((id) => collections.loadCollectionItems(id)));
        }
        window.dispatchEvent(new CustomEvent("postnot-agent-activity", { detail: activity.entries }));
        const count = activity.entries.filter((entry) => entry.outcome === "succeeded").length;
        if (count > 0) {
          notifications.success(`${count} saved ${count === 1 ? "change is" : "changes are"} now visible.`, "PostNot MCP updated collections", {
            action: { label: "Open integration", kind: "navigate", href: resolve("/activity") }
          });
        }
      } catch {
        // Polling is best-effort; the Activity page presents explicit load errors.
      }
    }

    let themePreference = "system";
    const stopWatching = watchSystemTheme(() => {
      if (themePreference === "system") {
        applyTheme(themePreference);
      }
    });

    const loadTheme = async () => {
      try {
        const settings = await getSettings();
        themePreference = settings.theme;
        applyUiScale(settings.uiScale);
        notifications.setDefaultDuration(settings.notificationTimeoutMs);
        await updater.initialize();
      } catch {
        themePreference = "system";
        applyUiScale(1);
        notifications.setDefaultDuration(5_000);
      }

      applyTheme(themePreference);
    };

    void loadTheme();
    void updater.ensureSilentCheck();

    return () => {
      unlistenHistoryPersistence?.();
      if (activityPoll) clearInterval(activityPoll);
      uninstallEditableUndoFallback?.();
      stopWatching();
    };
  });
</script>

<AppShell>
  {@render children?.()}
</AppShell>
