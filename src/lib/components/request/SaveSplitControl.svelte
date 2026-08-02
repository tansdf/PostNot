<script lang="ts">
  import type { Attachment } from "svelte/attachments";

  let {
    label = "Save",
    loadingLabel = "Saving...",
    disabled = false,
    isSaving = false,
    showMenu = false,
    menuLabel = "Save as",
    onSave = () => {},
    onSaveAs = () => {}
  }: {
    label?: string;
    loadingLabel?: string;
    disabled?: boolean;
    isSaving?: boolean;
    showMenu?: boolean;
    menuLabel?: string;
    onSave?: () => Promise<void> | void;
    onSaveAs?: () => Promise<void> | void;
  } = $props();

  let isMenuOpen = $state(false);
  let rootNode: HTMLDivElement | null = null;

  const attachRoot: Attachment<HTMLDivElement> = (node) => {
    rootNode = node;
    return () => {
      if (rootNode === node) rootNode = null;
    };
  };

  function closeOnDocumentClick(event: MouseEvent) {
    if (!isMenuOpen || rootNode?.contains(event.target as Node)) return;
    isMenuOpen = false;
  }

  function closeOnWindowKeydown(event: KeyboardEvent) {
    if (isMenuOpen && event.key === "Escape") isMenuOpen = false;
  }
</script>

<svelte:window onkeydown={closeOnWindowKeydown} />
<svelte:document onclickcapture={closeOnDocumentClick} />

<div
  class={[
    "request-save-split",
    !showMenu && "request-save-split-solo",
    (disabled || isSaving) && "request-save-split-disabled"
  ]}
  {@attach attachRoot}
>
  <button
    class="request-save-split-main"
    type="button"
    onclick={() => {
      isMenuOpen = false;
      void onSave();
    }}
    disabled={disabled || isSaving}
  >
    {isSaving ? loadingLabel : label}
  </button>
  {#if showMenu}
    <button
      class="request-save-split-chevron"
      type="button"
      aria-expanded={isMenuOpen}
      aria-haspopup="menu"
      aria-label="More save actions"
      disabled={disabled || isSaving}
      onclick={(event) => {
        event.stopPropagation();
        isMenuOpen = !isMenuOpen;
      }}
    >
      <svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
        <path d="M6 9l6 6 6-6" />
      </svg>
    </button>
    {#if isMenuOpen}
      <div class="request-save-menu" role="menu">
        <button
          class="request-save-menu-item"
          type="button"
          role="menuitem"
          onclick={() => {
            isMenuOpen = false;
            void onSaveAs();
          }}
        >
          {menuLabel}
        </button>
      </div>
    {/if}
  {/if}
</div>
