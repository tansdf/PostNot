<script lang="ts">
  import type { Snippet } from "svelte";
  import { onMount } from "svelte";
  import { getSettings } from "$lib/api/commands";
  import "$lib/styles/app.css";
  import { applyTheme, applyUiScale, watchSystemTheme } from "$lib/theme";
  import AppShell from "$lib/components/layout/AppShell.svelte";
  import { notifications } from "$lib/stores/notifications.svelte";

  let { children }: { children?: Snippet } = $props();

  onMount(() => {
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
      } catch {
        themePreference = "system";
        applyUiScale(1);
        notifications.setDefaultDuration(5_000);
      }

      applyTheme(themePreference);
    };

    void loadTheme();

    return () => {
      stopWatching();
    };
  });
</script>

<AppShell>
  {@render children?.()}
</AppShell>
