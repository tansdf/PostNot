<script lang="ts">
  import type { CollectionItemSummary, EnvironmentVariable } from "$lib/api/types";
  import ScriptEditor from "$lib/components/request/ScriptEditor.svelte";

  const FOLDER_PRE_REQUEST_PLACEHOLDER = "pn.request.upsertHeader('X-Folder', 'Shared');";
  const FOLDER_TEST_PLACEHOLDER = `pn.test('folder status is ok', () => {
  pn.expect(pn.response.code).toBeLessThan(500);
});`;

  let {
    item,
    isSaving = false,
    environmentVariables = [],
    onSaveFolder = () => false
  }: {
    item: CollectionItemSummary;
    isSaving?: boolean;
    environmentVariables?: EnvironmentVariable[];
    onSaveFolder?: (
      itemId: string,
      name: string,
      preRequestScript: string,
      testScript: string
    ) => Promise<boolean> | boolean;
  } = $props();

  // Drafts reset when parent remounts this component via `{#key item.id}`.
  // svelte-ignore state_referenced_locally
  let draftName = $state(item.name);
  // svelte-ignore state_referenced_locally
  let draftPreRequestScript = $state(item.preRequestScript);
  // svelte-ignore state_referenced_locally
  let draftTestScript = $state(item.testScript);

  async function handleSubmit() {
    await onSaveFolder(item.id, draftName, draftPreRequestScript, draftTestScript);
  }
</script>

<details class="folder-script-details">
  <summary>Folder settings</summary>

  <form class="folder-script-form" onsubmit={(event) => { event.preventDefault(); void handleSubmit(); }}>
    <label>
      <span class="field-label">Folder name</span>
      <input class="text-input" bind:value={draftName} placeholder="Folder name" required />
    </label>

    <div class="request-script-grid">
      <section class="request-script-card">
        <div class="request-script-card-header">
          <h3 class="request-script-card-title">Pre-request Script</h3>
          <p class="field-help">Runs after collection scripts and before child request scripts.</p>
        </div>
        <ScriptEditor
          bind:value={draftPreRequestScript}
          placeholder={FOLDER_PRE_REQUEST_PLACEHOLDER}
          scriptKind="preRequest"
          {environmentVariables}
        />
      </section>

      <section class="request-script-card">
        <div class="request-script-card-header">
          <h3 class="request-script-card-title">Test Script</h3>
          <p class="field-help">Runs after each response for requests inside this folder.</p>
        </div>
        <ScriptEditor
          bind:value={draftTestScript}
          placeholder={FOLDER_TEST_PLACEHOLDER}
          scriptKind="test"
          {environmentVariables}
        />
      </section>
    </div>

    <div class="collections-page-actions">
      <button class="button-primary" type="submit" disabled={isSaving}>
        {isSaving ? "Saving..." : "Save folder"}
      </button>
    </div>
  </form>
</details>
