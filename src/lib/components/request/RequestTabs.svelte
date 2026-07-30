<script lang="ts">
  import { tick } from "svelte";
  import type { Attachment } from "svelte/attachments";

  import type { RequestWorkspaceTab } from "$lib/api/types";

  let {
    tabs = [],
    activeTabId = "",
    inFlightTabId = "",
    scrollRequest = { n: 0, tabId: "" },
    onIsDirty = () => false,
    onActivate = () => {},
    onClose = () => {},
    onCreate = () => {}
  }: {
    tabs?: RequestWorkspaceTab[];
    activeTabId?: string;
    inFlightTabId?: string;
    /** Bumped with a concrete tab id when that tab should scroll into view (e.g. opened from collections). */
    scrollRequest?: { n: number; tabId: string };
    onIsDirty?: (tab: RequestWorkspaceTab) => boolean;
    onActivate?: (tabId: string) => Promise<void> | void;
    onClose?: (tabId: string) => Promise<void> | void;
    onCreate?: () => Promise<void> | void;
  } = $props();

  let tabsStripEl: HTMLDivElement | null = $state(null);

  /** Horizontal tab strip: map vertical wheel to scrollLeft (needs non-passive listener). */
  const attachTabsStrip: Attachment<HTMLDivElement> = (node) => {
    tabsStripEl = node;

    const onWheel = (event: WheelEvent) => {
      if (node.scrollWidth <= node.clientWidth + 1) {
        return;
      }

      let delta = 0;
      if (event.shiftKey) {
        delta = event.deltaY;
      } else if (Math.abs(event.deltaX) > Math.abs(event.deltaY)) {
        delta = event.deltaX;
      } else {
        delta = event.deltaY;
      }

      if (delta === 0) {
        return;
      }

      node.scrollLeft += delta;
      event.preventDefault();
    };

    node.addEventListener("wheel", onWheel, { passive: false });
    return () => {
      if (tabsStripEl === node) {
        tabsStripEl = null;
      }
      node.removeEventListener("wheel", onWheel);
    };
  };

  $effect(() => {
    const { n, tabId } = scrollRequest;
    if (!n || !tabId || !tabsStripEl) {
      return;
    }

    void tick().then(() => {
      const strip = tabsStripEl;
      if (!strip) {
        return;
      }
      const chip = strip.querySelector(`[data-request-tab-id="${tabId}"]`);
      chip?.scrollIntoView({ behavior: "smooth", inline: "nearest", block: "nearest" });
    });
  });

  function tabLabel(tab: RequestWorkspaceTab) {
    return tab.request.name.trim() || "Untitled request";
  }

  function tabMethodClass(tab: RequestWorkspaceTab) {
    return `method-${tab.request.method.toLowerCase()}`;
  }

  function tabMethodLetter(tab: RequestWorkspaceTab) {
    return tab.request.method.charAt(0) || "?";
  }
</script>

<section class="panel panel-inset-compact request-tabs-panel" aria-label="Request tabs">
  <div class="request-tabs-strip scrollbar-invisible" role="tablist" aria-label="Open requests" {@attach attachTabsStrip}>
    {#each tabs as tab (tab.id)}
      <div
        class={["request-tab-chip", activeTabId === tab.id && "request-tab-chip-active"]}
        data-request-tab-id={tab.id}
      >
        <button
          class="request-tab-chip-button"
          type="button"
          role="tab"
          aria-selected={activeTabId === tab.id}
          onclick={() => onActivate(tab.id)}
        >
          <span class={["request-tab-chip-method", tabMethodClass(tab)]} aria-hidden="true">{tabMethodLetter(tab)}</span>
          <span class="request-tab-chip-label">{tabLabel(tab)}</span>
          {#if onIsDirty(tab)}
            <span class="request-tab-chip-dirty" aria-label="Unsaved changes" title="Unsaved changes"></span>
          {/if}
          {#if inFlightTabId === tab.id}
            <span class="request-tab-chip-status">Sending</span>
          {/if}
        </button>

        <button
          class="request-tab-chip-close"
          type="button"
          aria-label={`Close ${tabLabel(tab)}`}
          onclick={(event) => {
            event.stopPropagation();
            void onClose(tab.id);
          }}
        >
          x
        </button>
      </div>
    {/each}

    <button class="request-tab-create" type="button" onclick={onCreate} aria-label="Open a new request tab" title="New tab">
      +
    </button>
  </div>
</section>
