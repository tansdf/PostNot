<script lang="ts">
  import type { FolderGlyphVariant } from "$lib/icons/folderPaths";
  import { folderPathForVariant } from "$lib/icons/folderPaths";

  let {
    variant,
    class: className = ""
  }: {
    variant: FolderGlyphVariant;
    class?: string;
  } = $props();

  const d = $derived(folderPathForVariant(variant));
</script>

<svg
  class={["tree-folder-icon-svg", className].filter(Boolean).join(" ")}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.75"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  focusable="false"
>
  <path {d} />
</svg>
