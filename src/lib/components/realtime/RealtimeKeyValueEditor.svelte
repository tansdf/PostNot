<script lang="ts">
  import { createKeyValueRow, type EnvironmentVariable, type KeyValueRow } from "$lib/api/types";
  import VariableField from "$lib/components/request/VariableField.svelte";

  let {
    rows = $bindable(),
    variables = [],
    title,
    description = "",
    keyLabel = "Key",
    valueLabel = "Value",
    addLabel = "Add row",
    auxiliaryActionLabel = "",
    onAuxiliaryAction = () => {}
  }: {
    rows: KeyValueRow[];
    variables?: EnvironmentVariable[];
    title: string;
    description?: string;
    keyLabel?: string;
    valueLabel?: string;
    addLabel?: string;
    auxiliaryActionLabel?: string;
    onAuxiliaryAction?: () => Promise<void> | void;
  } = $props();

  function update(index: number, patch: Partial<KeyValueRow>) {
    rows = rows.map((row, rowIndex) => rowIndex === index ? { ...row, ...patch } : row);
  }

  function remove(index: number) {
    const next = rows.filter((_, rowIndex) => rowIndex !== index);
    rows = next.length ? next : [createKeyValueRow()];
  }
</script>

<div class="editor-block">
  <div class="editor-header">
    {#if description}
      <div class="panel-heading">
        <h2>{title}</h2>
        <p class="field-help">{description}</p>
      </div>
    {:else}
      <h2>{title}</h2>
    {/if}

    {#if auxiliaryActionLabel}
      <div class="request-actions">
        <button class="button-secondary" type="button" onclick={onAuxiliaryAction}>{auxiliaryActionLabel}</button>
        <button class="button-secondary" type="button" onclick={() => (rows = [...rows, createKeyValueRow()])}>{addLabel}</button>
      </div>
    {:else}
      <button class="button-secondary" type="button" onclick={() => (rows = [...rows, createKeyValueRow()])}>{addLabel}</button>
    {/if}
  </div>

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
          class="icon-button row-action-button row-action-danger"
          type="button"
          onclick={() => remove(index)}
          aria-label={`Remove ${keyLabel.toLowerCase()} row ${index + 1}`}
          title={`Remove ${keyLabel.toLowerCase()} row`}
        >
          <svg viewBox="0 0 20 20" aria-hidden="true">
            <path d="M3 5h14" />
            <path d="M8 5V3h4v2" />
            <path d="M6 8v8" />
            <path d="M10 8v8" />
            <path d="M14 8v8" />
            <path d="M5 5l1 12h8l1-12" />
          </svg>
        </button>
      </div>
    {/each}
  </div>
</div>
