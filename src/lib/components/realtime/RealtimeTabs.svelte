<script lang="ts">
  import type { RealtimeWorkspaceTab } from "$lib/api/types";

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
    return tab.draft.name.trim() || (tab.draft.requestType === "socketio" ? "Untitled Socket.IO" : "Untitled WebSocket");
  }

  function tabDomId(tabId: string) {
    return `realtime-tab-${tabId}`;
  }

  async function moveFocus(tabId: string) {
    await onActivate(tabId);
    document.getElementById(tabDomId(tabId))?.focus();
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
</script>

<section class="panel request-tabs-panel" aria-label="Realtime connection tabs">
  <div class="request-tabs-strip realtime-tabs-strip">
    <div class="realtime-tablist scrollbar-invisible" role="tablist" aria-label="Open realtime connections">
      {#each tabs as tab, index (tab.id)}
        <div class={["request-tab-chip", activeTabId === tab.id && "request-tab-chip-active"]}>
          <button
            id={tabDomId(tab.id)}
            class="request-tab-chip-button"
            type="button"
            role="tab"
            aria-selected={activeTabId === tab.id}
            aria-controls="realtime-connection-panel"
            tabindex={activeTabId === tab.id ? 0 : -1}
            onclick={() => onActivate(tab.id)}
            onkeydown={(event) => handleTabKeydown(event, index)}
          >
            <span class={["realtime-status-dot", `realtime-status-${tab.status}`]} aria-hidden="true"></span>
            <span class="sr-only">{tab.statusMessage}.</span>
            <span class="request-tab-chip-label">{label(tab)}</span>
            <span class="protocol-badge">{tab.draft.requestType === "socketio" ? "S.IO" : "WS"}</span>
            {#if onIsDirty(tab)}
              <span class="request-tab-chip-dirty" aria-label="Unsaved changes" title="Unsaved changes"></span>
            {/if}
          </button>
        </div>
      {/each}
    </div>
    <div class="realtime-tab-actions" role="group" aria-label="Connection tab actions">
      {#if activeTabId}
        {@const selectedTab = tabs.find((tab) => tab.id === activeTabId)}
        <button
          class="request-tab-create"
          type="button"
          onclick={() => onClose(activeTabId)}
          aria-label={`Close ${selectedTab ? label(selectedTab) : "active connection tab"}`}
          title="Close active tab"
        >×</button>
      {/if}
      <button class="request-tab-create" type="button" onclick={onCreate} aria-label="Open a new WebSocket tab" title="New tab">+</button>
    </div>
  </div>
</section>
