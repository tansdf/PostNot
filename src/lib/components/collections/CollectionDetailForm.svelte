<script lang="ts">
  import type { CollectionSummary, EnvironmentVariable } from "$lib/api/types";
  import ScriptEditor from "$lib/components/request/ScriptEditor.svelte";

  const COLLECTION_PRE_REQUEST_PLACEHOLDER = "pn.request.upsertHeader('X-Collection', 'PostNot');";
  const COLLECTION_TEST_PLACEHOLDER = `pn.test('collection responds', () => {
  pn.expect(pn.response.code).toBe(200);
});`;

  let {
    collection,
    isSavingCollection = false,
    pendingDeleteCollectionId = "",
    environmentVariables = [],
    onSaveCollection = () => false,
    onDeleteCollection = () => {}
  }: {
    collection: CollectionSummary;
    isSavingCollection?: boolean;
    pendingDeleteCollectionId?: string;
    environmentVariables?: EnvironmentVariable[];
    onSaveCollection?: (
      name: string,
      description: string,
      preRequestScript: string,
      testScript: string
    ) => Promise<boolean> | boolean;
    onDeleteCollection?: (collectionId: string) => Promise<void> | void;
  } = $props();

  // Drafts reset when parent remounts this component via `{#key collection.id}`.
  // svelte-ignore state_referenced_locally
  let draftName = $state(collection.name);
  // svelte-ignore state_referenced_locally
  let draftDescription = $state(collection.description);
  // svelte-ignore state_referenced_locally
  let draftPreRequestScript = $state(collection.preRequestScript);
  // svelte-ignore state_referenced_locally
  let draftTestScript = $state(collection.testScript);

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
    await onSaveCollection(draftName, draftDescription, draftPreRequestScript, draftTestScript);
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

  <details class="folder-script-details collection-script-details">
    <summary>Collection scripts</summary>

    <div class="folder-script-form">
      <div class="request-script-grid">
        <section class="request-script-card">
          <div class="request-script-card-header">
            <h3 class="request-script-card-title">Pre-request Script</h3>
            <p class="field-help">Runs before each saved request in this collection.</p>
          </div>
          <ScriptEditor
            bind:value={draftPreRequestScript}
            placeholder={COLLECTION_PRE_REQUEST_PLACEHOLDER}
            scriptKind="preRequest"
            {environmentVariables}
          />
        </section>

        <section class="request-script-card">
          <div class="request-script-card-header">
            <h3 class="request-script-card-title">Test Script</h3>
            <p class="field-help">Runs after each response from this collection.</p>
          </div>
          <ScriptEditor
            bind:value={draftTestScript}
            placeholder={COLLECTION_TEST_PLACEHOLDER}
            scriptKind="test"
            {environmentVariables}
          />
        </section>
      </div>
    </div>
  </details>

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
