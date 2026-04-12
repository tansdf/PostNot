<script lang="ts">
  import type { CollectionSummary } from "$lib/api/types";

  let {
    collection,
    isSavingCollection = false,
    pendingDeleteCollectionId = "",
    onSaveCollection = () => false,
    onDeleteCollection = () => {}
  }: {
    collection: CollectionSummary;
    isSavingCollection?: boolean;
    pendingDeleteCollectionId?: string;
    onSaveCollection?: (name: string, description: string) => Promise<boolean> | boolean;
    onDeleteCollection?: (collectionId: string) => Promise<void> | void;
  } = $props();

  // Drafts reset when parent remounts this component via `{#key collection.id}`.
  // svelte-ignore state_referenced_locally
  let draftName = $state(collection.name);
  // svelte-ignore state_referenced_locally
  let draftDescription = $state(collection.description);

  function formatUpdatedAt(value: string) {
    try {
      return new Intl.DateTimeFormat(undefined, {
        dateStyle: "medium",
        timeStyle: "short"
      }).format(new Date(value));
    } catch {
      return value;
    }
  }

  async function handleSubmit() {
    await onSaveCollection(draftName, draftDescription);
  }
</script>

<form class="collections-detail-form" onsubmit={(e) => { e.preventDefault(); void handleSubmit(); }}>
  <div class="detail-facts">
    <div class="detail-kv-item">
      <span class="field-label">Saved requests</span>
      <strong>{collection.requestCount}</strong>
    </div>
    <div class="detail-kv-item detail-wide">
      <span class="field-label">Last updated</span>
      <strong>{formatUpdatedAt(collection.updatedAt)}</strong>
    </div>
  </div>

  <label>
    <span class="field-label">Collection name</span>
    <input class="text-input" bind:value={draftName} placeholder="Untitled collection" required />
  </label>

  <label>
    <span class="field-label">Description</span>
    <textarea
      class="text-input collection-description-input"
      bind:value={draftDescription}
      placeholder="Describe what this collection is for"
    ></textarea>
  </label>

  <div class="collections-page-actions">
    <button class="send-button" type="submit" disabled={isSavingCollection}>
      {isSavingCollection ? "Saving..." : "Save collection"}
    </button>
    <button
      class="icon-button"
      type="button"
      onclick={() => onDeleteCollection(collection.id)}
      disabled={pendingDeleteCollectionId === collection.id}
    >
      {pendingDeleteCollectionId === collection.id ? "Deleting..." : "Delete collection"}
    </button>
  </div>
</form>
