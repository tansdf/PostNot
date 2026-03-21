<script lang="ts">
  import { page } from "$app/stores";

  import SidebarCollections from "$lib/components/layout/SidebarCollections.svelte";

  export let title = "PostNot";
  export let subtitle = "A local-first desktop API client.";
</script>

<div class="app-shell">
  <aside class="sidebar">
    <div class="brand-block">
      <p class="eyebrow">Desktop API Client</p>
      <h1>{title}</h1>
      <p>{subtitle}</p>
    </div>

    <nav class="sidebar-nav" aria-label="Primary">
      <a class:sidebar-link-active={$page.url.pathname === "/"} class="sidebar-link" href="/">Requests</a>
      <a class:sidebar-link-active={$page.url.pathname.startsWith("/collections")} class="sidebar-link" href="/collections">Collections</a>
      <a class:sidebar-link-active={$page.url.pathname.startsWith("/environments")} class="sidebar-link" href="/environments">Environments</a>
      <a class:sidebar-link-active={$page.url.pathname.startsWith("/settings")} class="sidebar-link" href="/settings">Settings</a>
    </nav>

    <SidebarCollections />
  </aside>

  <main class="workspace">
    <slot />
  </main>
</div>
