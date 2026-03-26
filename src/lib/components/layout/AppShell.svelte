<script lang="ts">
  import type { Snippet } from "svelte";
  import { page } from "$app/state";
  import SidebarCollections from "$lib/components/layout/SidebarCollections.svelte";

  let {
    title = "PostNot",
    subtitle = "A local-first desktop API client.",
    children
  }: {
    title?: string;
    subtitle?: string;
    children?: Snippet;
  } = $props();
</script>

<div class="app-viewport">
  <div class="app-shell">
    <aside class="sidebar">
      <div class="brand-block">
        <p class="eyebrow">Desktop API Client</p>
        <h1>{title}</h1>
        <p>{subtitle}</p>
      </div>

      <nav class="sidebar-nav" aria-label="Primary">
        <a class={["sidebar-link", page.url.pathname === "/" && "sidebar-link-active"]} href="/">Requests</a>
        <a class={["sidebar-link", page.url.pathname.startsWith("/collections") && "sidebar-link-active"]} href="/collections">Collections</a>
        <a class={["sidebar-link", page.url.pathname.startsWith("/environments") && "sidebar-link-active"]} href="/environments">Environments</a>
        <a class={["sidebar-link", page.url.pathname.startsWith("/settings") && "sidebar-link-active"]} href="/settings">Settings</a>
      </nav>

      <SidebarCollections />
    </aside>

    <main class="workspace">
      {@render children?.()}
    </main>
  </div>
</div>
