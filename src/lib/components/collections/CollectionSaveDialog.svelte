<script lang="ts">
  import DialogShell from "$lib/components/layout/DialogShell.svelte";

  let {
    title,
    titleId,
    confirmLabel,
    savingLabel = "Saving…",
    collections,
    folders,
    selectedCollectionId,
    selectedParentId,
    isSaving = false,
    onSelectCollection,
    onSelectFolder,
    onConfirm,
    onDismiss
  }: {
    title: string;
    titleId: string;
    confirmLabel: string;
    savingLabel?: string;
    collections: Array<{ id: string; name: string; requestCount: number }>;
    folders: Array<{ id: string | null; name: string; depth: number }>;
    selectedCollectionId: string;
    selectedParentId: string | null;
    isSaving?: boolean;
    onSelectCollection: (collectionId: string) => Promise<void> | void;
    onSelectFolder: (parentId: string | null) => void;
    onConfirm: () => Promise<void> | void;
    onDismiss: () => void;
  } = $props();
</script>

<DialogShell ariaLabelledby={titleId} {onDismiss} sizeClass="save-dialog request-save-dialog">
  <div class="editor-header">
    <h2 id={titleId}>{title}</h2>
  </div>

  <div class="editor-block request-save-dialog-body">
    <div class="request-save-target-section">
      <span class="field-label">Choose a collection</span>
      <div class="save-target-list save-collection-list" role="listbox" aria-label="Choose a collection">
        {#each collections as collection (collection.id)}
          <button
            class={["save-target-button", selectedCollectionId === collection.id && "save-target-active"]}
            type="button"
            role="option"
            aria-selected={selectedCollectionId === collection.id}
            onclick={() => onSelectCollection(collection.id)}
          >
            <strong>{collection.name}</strong>
            <span>{collection.requestCount} request{collection.requestCount === 1 ? "" : "s"}</span>
          </button>
        {/each}
      </div>
    </div>

    {#if selectedCollectionId}
      <div class="request-save-target-section">
        <span class="field-label">Choose a folder</span>
        <div class="save-target-list save-folder-list" role="listbox" aria-label="Choose a folder">
          {#each folders as folder (`${selectedCollectionId}-${folder.id ?? "root"}`)}
            <button
              class={[
                "save-target-button",
                folder.id ? "save-target-folder" : "save-target-root",
                selectedParentId === folder.id && "save-target-active"
              ]}
              type="button"
              role="option"
              aria-selected={selectedParentId === folder.id}
              onclick={() => onSelectFolder(folder.id)}
              style={`--tree-depth:${folder.depth};`}
            >
              <strong>{folder.name}</strong>
              <span>{folder.id ? "Folder" : "Collection root"}</span>
            </button>
          {/each}
        </div>
      </div>
    {/if}

    <div class="collections-page-actions">
      <button class="button-secondary" type="button" onclick={onDismiss}>Cancel</button>
      <button class="button-primary" type="button" onclick={onConfirm} disabled={isSaving}>
        {isSaving ? savingLabel : confirmLabel}
      </button>
    </div>
  </div>
</DialogShell>
