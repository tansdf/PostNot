<script lang="ts">
  let {
    name,
    className = ""
  }: {
    name: "local" | "authoring" | "activity" | "success" | "failed";
    className?: string;
  } = $props();
</script>

<svg
  class={className}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.8"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if name === "local"}
    <rect x="3" y="4" width="18" height="14" rx="2"></rect>
    <path d="m7 9 2.5 2.5L7 14"></path>
    <path d="M12 14h4"></path>
    <path d="M9 21h6"></path>
    <path d="M12 18v3"></path>
  {:else if name === "authoring"}
    <path d="M13.5 5.5 18.5 10.5"></path>
    <path d="m4 20 4.25-1.05L19.5 7.7a1.75 1.75 0 0 0 0-2.47l-.73-.73a1.75 1.75 0 0 0-2.47 0L5.05 15.75 4 20Z"></path>
    <path d="m5.25 15.55 3.2 3.2"></path>
  {:else if name === "activity"}
    <path d="M3.5 12a8.5 8.5 0 1 0 2.2-5.7"></path>
    <path d="M3.5 5.5v4h4"></path>
    <path d="M12 7.5V12l3 2"></path>
  {:else if name === "success"}
    <path d="m6.5 12.5 3.4 3.4 7.6-7.8"></path>
  {:else}
    <path d="M12 7.5v5.25"></path>
    <path d="M12 16.5h.01"></path>
  {/if}
</svg>
