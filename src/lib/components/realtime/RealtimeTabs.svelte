<script lang="ts">
  import { tick } from "svelte";
  import type { RealtimeWorkspaceTab } from "$lib/api/types";
  import { horizontalWheelScroll } from "$lib/horizontal-scroll";

  let {
    tabs = [],
    activeTabId = "",
    onIsDirty = () => false,
    onActivate = () => {},
    onClose = () => {},
    onCreate = () => {}
  }: {
    tabs?: RealtimeWorkspaceTab[];
    activeTabId?: string;
    onIsDirty?: (tab: RealtimeWorkspaceTab) => boolean;
    onActivate?: (tabId: string) => Promise<void> | void;
    onClose?: (tabId: string) => Promise<void> | void;
    onCreate?: () => Promise<void> | void;
  } = $props();

  function label(tab: RealtimeWorkspaceTab) {
    return tab.connectionDraft.name.trim() || (tab.connectionDraft.protocol === "socketio" ? "Untitled Socket.IO" : "Untitled WebSocket");
  }

  function tabDomId(tabId: string) {
    return `realtime-tab-${tabId}`;
  }

  async function moveFocus(tabId: string) {
    await onActivate(tabId);
    document.getElementById(tabDomId(tabId))?.focus();
  }

  function handleTabClick(event: MouseEvent, tabId: string) {
    if ((event.target as HTMLElement).closest("[data-close-realtime-tab]")) {
      void onClose(tabId);
      return;
    }
    void onActivate(tabId);
  }

  function handleTabKeydown(event: KeyboardEvent, tabIndex: number) {
    let nextIndex = tabIndex;
    if (event.key === "ArrowRight") nextIndex = (tabIndex + 1) % tabs.length;
    else if (event.key === "ArrowLeft") nextIndex = (tabIndex - 1 + tabs.length) % tabs.length;
    else if (event.key === "Home") nextIndex = 0;
    else if (event.key === "End") nextIndex = tabs.length - 1;
    else if (event.key === "Delete") {
      event.preventDefault();
      void onClose(tabs[tabIndex].id);
      return;
    } else {
      return;
    }
    event.preventDefault();
    void moveFocus(tabs[nextIndex].id);
  }

  $effect(() => {
    const tabId = activeTabId;
    if (!tabId) return;
    void tick().then(() => {
      document.querySelector(`[data-realtime-tab-id="${tabId}"]`)?.scrollIntoView({
        behavior: "smooth",
        inline: "nearest",
        block: "nearest"
      });
    });
  });
</script>

<section class="panel panel-inset-compact request-tabs-panel" aria-label="Realtime connection tabs">
  <div class="request-tabs-strip scrollbar-invisible" {@attach horizontalWheelScroll}>
    <div class="realtime-tablist" role="tablist" aria-label="Open realtime connections">
      {#each tabs as tab, index (tab.id)}
        <div class={["request-tab-chip", activeTabId === tab.id && "request-tab-chip-active"]} data-realtime-tab-id={tab.id}>
          <button
            id={tabDomId(tab.id)}
            class="request-tab-chip-button"
            type="button"
            role="tab"
            aria-selected={activeTabId === tab.id}
            aria-controls="realtime-connection-panel"
            aria-keyshortcuts="Delete"
            tabindex={activeTabId === tab.id ? 0 : -1}
            onclick={(event) => handleTabClick(event, tab.id)}
            onkeydown={(event) => handleTabKeydown(event, index)}
          >
            <span class={["realtime-status-dot", `realtime-status-${tab.status}`]} aria-hidden="true"></span>
            <span class="sr-only">{tab.statusMessage}.</span>
            <span class="request-tab-chip-label">{label(tab)}</span>
            <span class="protocol-badge">{tab.connectionDraft.protocol === "socketio" ? "S.IO" : "WS"}</span>
            {#if onIsDirty(tab)}
              <span class="request-tab-chip-dirty" aria-label="Unsaved changes" title="Unsaved changes"></span>
            {/if}
            <span
              class="request-tab-chip-close"
              data-close-realtime-tab
              aria-hidden="true"
              title={`Close ${label(tab)}`}
            >
              x
            </span>
          </button>
        </div>
      {/each}
    </div>

    <button class="request-tab-create" type="button" onclick={onCreate} aria-label="Open a new WebSocket tab" title="New tab">
      +
    </button>
  </div>
</section>
