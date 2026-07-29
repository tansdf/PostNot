<script lang="ts">
  import type { Snippet } from "svelte";
  import { resolve } from "$app/paths";
  import { page } from "$app/state";
  import CollectionDragController from "$lib/components/layout/CollectionDragController.svelte";
  import NotificationHost from "$lib/components/layout/NotificationHost.svelte";
  import SidebarCollections from "$lib/components/layout/SidebarCollections.svelte";
  import { updater } from "$lib/stores/updater.svelte";

  let {
    title = "PostNot",
    children
  }: {
    title?: string;
    children?: Snippet;
  } = $props();
</script>

<div class="app-viewport">
  <div class="app-shell">
    <aside class="sidebar">
      <div class="brand-block">
        <p class="eyebrow">Desktop API Client</p>
        <h1>{title} <span class={["version-pill", updater.availableUpdate && "version-pill-update-ready"]}>v{__APP_VERSION__}{#if updater.availableUpdate}<span class="version-pill-arrow" title={`Version ${updater.availableUpdate.version} is ready to install`} aria-label={`Version ${updater.availableUpdate.version} is ready to install`}>↑</span>{/if}</span></h1>
      </div>

      <nav class="sidebar-nav" aria-label="Primary">
        <a class={["sidebar-link", page.url.pathname === "/" && "sidebar-link-active"]} href={resolve("/")}>Requests</a>
        <a
          class={["sidebar-link", page.url.pathname.startsWith("/websockets") && "sidebar-link-active"]}
          href={resolve("/websockets")}
        >
          WebSockets
        </a>
        <a
          class={["sidebar-link", page.url.pathname.startsWith("/collections") && "sidebar-link-active"]}
          href={resolve("/collections")}
        >
          Collections
        </a>
        <a
          class={["sidebar-link", page.url.pathname.startsWith("/environments") && "sidebar-link-active"]}
          href={resolve("/environments")}
        >
          Environments
        </a>
        <a
          class={["sidebar-link", page.url.pathname.startsWith("/playbooks") && "sidebar-link-active"]}
          href={resolve("/playbooks")}
        >
          Playbooks
        </a>
        <a
          class={["sidebar-link", page.url.pathname.startsWith("/activity") && "sidebar-link-active"]}
          href={resolve("/activity")}
        >
          MCP Integration
        </a>
        <a
          class={["sidebar-link", page.url.pathname.startsWith("/settings") && "sidebar-link-active"]}
          href={resolve("/settings")}
        >
          Settings
        </a>
      </nav>

      <SidebarCollections />
    </aside>

    <main class="workspace">
      {@render children?.()}
    </main>

    <NotificationHost />
    <CollectionDragController />
  </div>
</div>
