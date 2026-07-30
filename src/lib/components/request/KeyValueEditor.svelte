<script lang="ts">
  import { createKeyValueRow, type EnvironmentVariable, type KeyValueRow } from "$lib/api/types";
  import VariableField from "$lib/components/request/VariableField.svelte";

  let {
    rows,
    variables = [],
    title,
    description = "",
    keyLabel = "Key",
    valueLabel = "Value",
    addLabel = "Add row",
    auxiliaryActionLabel = "",
    rowLabel = "",
    keySuggestions = [],
    getValueSuggestions = () => [],
    onRowsChange,
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
    rowLabel?: string;
    keySuggestions?: string[];
    getValueSuggestions?: (key: string) => string[];
    onRowsChange: (rows: KeyValueRow[]) => void;
    onAuxiliaryAction?: () => Promise<void> | void;
  } = $props();

  let accessibleRowLabel = $derived(rowLabel || keyLabel.toLowerCase());

  function update(index: number, patch: Partial<KeyValueRow>) {
    onRowsChange(rows.map((row, rowIndex) => rowIndex === index ? { ...row, ...patch } : row));
  }

  function add() {
    onRowsChange([...rows, createKeyValueRow()]);
  }

  function remove(index: number) {
    const next = rows.filter((_, rowIndex) => rowIndex !== index);
    onRowsChange(next.length ? next : [createKeyValueRow()]);
  }

  function keyListId(rowId: string) {
    return `key-value-name-suggestions-${rowId}`;
  }

  function valueListId(rowId: string) {
    return `key-value-value-suggestions-${rowId}`;
  }
</script>

<div class="editor-block key-value-editor">
  <div class="editor-header">
    <h2>{title}</h2>

    <div class="request-actions">
      {#if auxiliaryActionLabel}
        <button class="button-secondary" type="button" onclick={onAuxiliaryAction}>{auxiliaryActionLabel}</button>
      {/if}
      <button class="button-secondary" type="button" onclick={add}>{addLabel}</button>
    </div>
  </div>

  <div class="key-value-content">
    {#if description}<p class="field-help">{description}</p>{/if}
    <div class="row-list">
      {#each rows as row, index (row.id)}
        {@const valueSuggestions = getValueSuggestions(row.key)}
        <div class="kv-row key-value-row">
          <input
            class="row-toggle"
            type="checkbox"
            checked={row.enabled}
            onchange={(event) => update(index, { enabled: event.currentTarget.checked })}
            aria-label={`Enable ${accessibleRowLabel} row ${index + 1}`}
          />
          <input
            class="text-input"
            value={row.key}
            placeholder={keyLabel}
            list={keySuggestions.length ? keyListId(row.id) : undefined}
            oninput={(event) => update(index, { key: event.currentTarget.value })}
          />
          {#if keySuggestions.length}
            <datalist id={keyListId(row.id)}>
              {#each keySuggestions as suggestion (suggestion)}
                <option value={suggestion}></option>
              {/each}
            </datalist>
          {/if}
          <VariableField
            value={row.value}
            {variables}
            className="text-input"
            placeholder={valueLabel}
            list={valueSuggestions.length ? valueListId(row.id) : undefined}
            onValueInput={(value) => update(index, { value })}
          />
          {#if valueSuggestions.length}
            <datalist id={valueListId(row.id)}>
              {#each valueSuggestions as suggestion (suggestion)}
                <option value={suggestion}></option>
              {/each}
            </datalist>
          {/if}
          <button
            class="icon-button row-action-button row-action-danger"
            type="button"
            onclick={() => remove(index)}
            aria-label={`Remove ${accessibleRowLabel} row ${index + 1}`}
            title={`Remove ${accessibleRowLabel} row`}
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
</div>
