<script lang="ts">
  import type { Snippet } from "svelte";
  import { resolve } from "$app/paths";
  import { page } from "$app/state";
  import McpGlyph from "$lib/components/icons/McpGlyph.svelte";
  import NavigationGlyph from "$lib/components/icons/NavigationGlyph.svelte";
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
        <h1>{title} <span class={["version-pill", updater.availableUpdate && "version-pill-update-ready"]}>v{__APP_VERSION__}{#if updater.availableUpdate}<span class="version-pill-arrow" title={`Version ${updater.availableUpdate.version} is ready to install`} aria-label={`Version ${updater.availableUpdate.version} is ready to install`}>↑</span>{/if}</span></h1>
      </div>

      <nav class="sidebar-nav" aria-label="Workspaces">
        <a
          class={["sidebar-link", page.url.pathname === "/" && "sidebar-link-active"]}
          href={resolve("/")}
          aria-current={page.url.pathname === "/" ? "page" : undefined}
        >Requests</a>
        <a
          class={["sidebar-link", page.url.pathname.startsWith("/websockets") && "sidebar-link-active"]}
          href={resolve("/websockets")}
          aria-current={page.url.pathname.startsWith("/websockets") ? "page" : undefined}
        >
          WebSockets
        </a>
        <a
          class={["sidebar-link", page.url.pathname.startsWith("/playbooks") && "sidebar-link-active"]}
          href={resolve("/playbooks")}
          aria-current={page.url.pathname.startsWith("/playbooks") ? "page" : undefined}
        >
          Playbooks
        </a>
      </nav>

      <SidebarCollections />

      <nav class="sidebar-utility-nav" aria-label="Utilities">
        <a
          class={["sidebar-utility-link", page.url.pathname.startsWith("/environments") && "sidebar-utility-link-active"]}
          href={resolve("/environments")}
          aria-current={page.url.pathname.startsWith("/environments") ? "page" : undefined}
          title="Environments"
        >
          <NavigationGlyph name="environment" />
          <span>Env</span>
        </a>
        <a
          class={["sidebar-utility-link", page.url.pathname.startsWith("/activity") && "sidebar-utility-link-active"]}
          href={resolve("/activity")}
          aria-current={page.url.pathname.startsWith("/activity") ? "page" : undefined}
          aria-label="MCP integration"
          title="MCP integration"
        >
          <McpGlyph name="activity" />
          <span>MCP</span>
        </a>
        <a
          class={["sidebar-utility-link", "sidebar-settings-link", page.url.pathname.startsWith("/settings") && "sidebar-utility-link-active"]}
          href={resolve("/settings")}
          aria-current={page.url.pathname.startsWith("/settings") ? "page" : undefined}
          aria-label="Settings"
          title="Settings"
        >
          <NavigationGlyph name="settings" />
        </a>
      </nav>
    </aside>

    <main class="workspace">
      {@render children?.()}
    </main>

    <NotificationHost />
    <CollectionDragController />
  </div>
</div>
