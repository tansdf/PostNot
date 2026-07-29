<script lang="ts">
  import { createKeyValueRow, type EnvironmentVariable, type KeyValueRow } from "$lib/api/types";
  import VariableField from "$lib/components/request/VariableField.svelte";

  let {
    rows = $bindable(),
    variables = [],
    keyLabel = "Key",
    valueLabel = "Value",
    addLabel = "Add row"
  }: {
    rows: KeyValueRow[];
    variables?: EnvironmentVariable[];
    keyLabel?: string;
    valueLabel?: string;
    addLabel?: string;
  } = $props();

  function update(index: number, patch: Partial<KeyValueRow>) {
    rows = rows.map((row, rowIndex) => rowIndex === index ? { ...row, ...patch } : row);
  }

  function remove(index: number) {
    const next = rows.filter((_, rowIndex) => rowIndex !== index);
    rows = next.length ? next : [createKeyValueRow()];
  }
</script>

<div class="row-list">
  {#each rows as row, index (row.id)}
    <div class="kv-row realtime-kv-row">
      <input
        class="row-toggle"
        type="checkbox"
        checked={row.enabled}
        onchange={(event) => update(index, { enabled: event.currentTarget.checked })}
        aria-label={`Enable ${keyLabel.toLowerCase()} row ${index + 1}`}
      />
      <VariableField
        value={row.key}
        {variables}
        className="text-input"
        placeholder={keyLabel}
        onValueInput={(value) => update(index, { key: value })}
      />
      <VariableField
        value={row.value}
        {variables}
        className="text-input"
        placeholder={valueLabel}
        onValueInput={(value) => update(index, { value })}
      />
      <button
        class="row-action-button"
        type="button"
        onclick={() => remove(index)}
        aria-label={`Remove row ${index + 1}`}
        title="Remove row"
      >×</button>
    </div>
  {/each}
  <button class="button-secondary button-compact row-add-button" type="button" onclick={() => (rows = [...rows, createKeyValueRow()])}>
    {addLabel}
  </button>
</div>
