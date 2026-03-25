<script lang="ts">
  import { onMount } from "svelte";

  import { getSettings } from "$lib/api/commands";
  import "$lib/styles/app.css";
  import { applyTheme, applyUiScale, watchSystemTheme } from "$lib/theme";

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
      } catch {
        themePreference = "system";
        applyUiScale(1);
      }

      applyTheme(themePreference);
    };

    void loadTheme();

    return () => {
      stopWatching();
    };
  });
</script>

<slot />
