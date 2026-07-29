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
</script>

<section class="panel request-tabs-panel" aria-label="Realtime connection tabs">
  <div class="request-tabs-strip scrollbar-invisible" role="tablist" aria-label="Open realtime connections">
    {#each tabs as tab (tab.id)}
      <div class={["request-tab-chip", activeTabId === tab.id && "request-tab-chip-active"]}>
        <button
          class="request-tab-chip-button"
          type="button"
          role="tab"
          aria-selected={activeTabId === tab.id}
          onclick={() => onActivate(tab.id)}
        >
          <span class={["realtime-status-dot", `realtime-status-${tab.status}`]} aria-hidden="true"></span>
          <span class="sr-only">{tab.statusMessage}.</span>
          <span class="request-tab-chip-label">{label(tab)}</span>
          <span class="protocol-badge">{tab.draft.requestType === "socketio" ? "S.IO" : "WS"}</span>
          {#if onIsDirty(tab)}
            <span class="request-tab-chip-dirty" aria-label="Unsaved changes" title="Unsaved changes"></span>
          {/if}
        </button>
        <button
          class="request-tab-chip-close"
          type="button"
          aria-label={`Close ${label(tab)}`}
          onclick={(event) => {
            event.stopPropagation();
            void onClose(tab.id);
          }}
        >×</button>
      </div>
    {/each}
    <button class="request-tab-create" type="button" onclick={onCreate} aria-label="Open a new WebSocket tab" title="New tab">+</button>
  </div>
</section>
