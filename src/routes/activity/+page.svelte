<script lang="ts">
  import { resolve } from "$app/paths";
  import { onMount } from "svelte";
  import { getMcpSetupInfo, listAgentActivity } from "$lib/api/commands";
  import type { AgentActivityEntry, McpSetupInfo } from "$lib/api/types";
  import McpGlyph from "$lib/components/icons/McpGlyph.svelte";
  import DialogShell from "$lib/components/layout/DialogShell.svelte";
  import { notifications } from "$lib/stores/notifications.svelte";

  type ActivityGroup = { batchId: string; entries: AgentActivityEntry[] };
  type ConfigKind = "codex" | "claude" | "cursor" | "generic";

  const configOptions: { value: ConfigKind; label: string }[] = [
    { value: "codex", label: "Codex" },
    { value: "claude", label: "Claude Desktop" },
    { value: "cursor", label: "Cursor" },
    { value: "generic", label: "Generic" }
  ];

  let groups: ActivityGroup[] = $state([]);
  let isLoading = $state(true);
  let errorText = $state("");
  let mcpSetup: McpSetupInfo | null = $state(null);
  let setupErrorText = $state("");
  let isSetupOpen = $state(false);
  let configKind: ConfigKind = $state("codex");

  const changeCount = $derived(groups.reduce((count, group) => count + group.entries.length, 0));

  onMount(() => {
    void Promise.all([loadActivity(), loadMcpSetup()]);
  });

  async function loadActivity() {
    isLoading = true;
    try {
      const page = await listAgentActivity(undefined, 250);
      const grouped = new Map<string, AgentActivityEntry[]>();
      for (const entry of page.entries) {
        const existing = grouped.get(entry.batchId) ?? [];
        existing.push(entry);
        grouped.set(entry.batchId, existing);
      }
      groups = Array.from(grouped, ([batchId, entries]) => ({ batchId, entries }));
      errorText = "";
    } catch (error) {
      errorText = error instanceof Error ? error.message : String(error);
    } finally {
      isLoading = false;
    }
  }

  async function loadMcpSetup() {
    try {
      mcpSetup = await getMcpSetupInfo();
      setupErrorText = "";
    } catch (error) {
      setupErrorText = error instanceof Error ? error.message : String(error);
    }
  }

  function selectedMcpConfig() {
    if (!mcpSetup) return "";
    if (configKind === "codex") return mcpSetup.codexConfigToml;
    if (configKind === "claude") return mcpSetup.claudeConfigJson;
    if (configKind === "cursor") return mcpSetup.cursorConfigJson;
    return mcpSetup.genericConfigJson;
  }

  async function copyMcpConfig() {
    try {
      await navigator.clipboard.writeText(selectedMcpConfig());
      notifications.success("Paste it into your agent's MCP server configuration.", "MCP configuration copied");
    } catch (error) {
      notifications.error(error instanceof Error ? error.message : String(error), "Configuration was not copied");
    }
  }

  function formatDate(value: string) {
    const date = new Date(value);
    return Number.isNaN(date.getTime())
      ? value
      : new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" }).format(date);
  }

  function formatOperation(value: string) {
    return value.replaceAll("_", " ").replace(/^./, (letter) => letter.toUpperCase());
  }

  function formatTargetKind(value: string) {
    return value.replaceAll("_", " ");
  }

  function targetHref(entry: AgentActivityEntry) {
    if (!entry.collectionId) return "";
    const params = new URLSearchParams({ collectionId: entry.collectionId });
    if (entry.targetKind !== "collection" && entry.targetId) params.set("itemId", entry.targetId);
    return resolve(`/collections?${params.toString()}`);
  }
</script>

<svelte:head><title>PostNot MCP Integration</title></svelte:head>

<section class="panel mcp-page">
  <header class="mcp-page-header">
    <div class="mcp-page-heading">
      <p class="eyebrow">Local agent bridge</p>
      <h1>MCP Integration</h1>
      <p>Let compatible AI agents inspect your workspace and prepare reusable requests while you keep control of execution.</p>
    </div>
    <div class="mcp-page-actions">
      <button class="button-secondary" type="button" onclick={loadActivity} disabled={isLoading}>
        {isLoading ? "Refreshing…" : "Refresh"}
      </button>
      <button class="button-primary" type="button" onclick={() => (isSetupOpen = true)}>
        Configure MCP…
      </button>
    </div>
  </header>

  <div class="mcp-safety-summary" aria-label="MCP integration properties">
    <div>
      <span class="mcp-summary-icon"><McpGlyph name="local" /></span>
      <span><strong>Local connection</strong>Runs over stdio on this device</span>
    </div>
    <div>
      <span class="mcp-summary-icon"><McpGlyph name="authoring" /></span>
      <span><strong>Authoring only</strong>No send or delete tools exposed</span>
    </div>
    <div>
      <span class="mcp-summary-icon"><McpGlyph name="activity" /></span>
      <span><strong>Reviewable changes</strong>Every mutation is recorded below</span>
    </div>
  </div>

  <section class="mcp-activity-section" aria-labelledby="mcp-activity-title">
    <div class="mcp-section-heading">
      <div>
        <h2 id="mcp-activity-title">Recent agent changes</h2>
        <p>{changeCount === 0 ? "Activity will appear here after an agent changes your workspace." : `${changeCount} recorded ${changeCount === 1 ? "change" : "changes"}`}</p>
      </div>
    </div>

    {#if isLoading && groups.length === 0}
      <div class="empty-state mcp-empty-state" role="status">Loading recent agent changes…</div>
    {:else if errorText}
      <div class="feedback feedback-error" role="alert">{errorText}</div>
    {:else if groups.length === 0}
      <div class="empty-state mcp-empty-state">
        <span class="mcp-empty-mark"><McpGlyph name="activity" /></span>
        <strong>No agent changes yet</strong>
        <p>Configure an MCP client, then ask it to create or update a saved request.</p>
        <button class="button-secondary button-compact" type="button" onclick={() => (isSetupOpen = true)}>Configure MCP…</button>
      </div>
    {:else}
      <div class="activity-list">
        {#each groups as group (group.batchId)}
          {@const first = group.entries[0]}
          {@const failed = first.outcome === "failed"}
          <article class={["activity-card", failed && "activity-card-failed"]}>
            <div class="activity-marker">
              <McpGlyph name={failed ? "failed" : "success"} />
            </div>
            <div class="activity-card-content">
              <div class="activity-card-heading">
                <div class="activity-title-row">
                  <h3>{formatOperation(first.operation)}</h3>
                  <span class={["activity-status-badge", failed && "activity-status-badge-failed"]}>
                    {failed ? "Failed" : group.entries.length > 1 ? `${group.entries.length} changes` : "Completed"}
                  </span>
                </div>
                <time datetime={first.occurredAt}>{formatDate(first.occurredAt)}</time>
              </div>
              <p
                class="activity-actor"
                title={`Session ${first.sessionId}`}
                aria-label={`${first.actorName}${first.actorVersion ? ` ${first.actorVersion}` : ""}, session ${first.sessionId}`}
              >
                {first.actorName}{first.actorVersion ? ` ${first.actorVersion}` : ""}
              </p>
              <ul class="activity-targets">
                {#each group.entries as entry (entry.id)}
                  <li>
                    <span class="activity-kind">{formatTargetKind(entry.targetKind)}</span>
                    <div class="activity-target-copy">
                      <strong>{entry.targetName || entry.targetKind}</strong>
                      <span>{entry.changedFields.length ? `Changed ${entry.changedFields.join(", ")}` : entry.errorMessage ?? "No fields changed"}</span>
                    </div>
                    {#if targetHref(entry)}
                      <a class="button-ghost button-compact" href={targetHref(entry)}>Open</a>
                    {/if}
                  </li>
                {/each}
              </ul>
            </div>
          </article>
        {/each}
      </div>
    {/if}
  </section>
</section>

{#if isSetupOpen}
  <DialogShell ariaLabelledby="mcp-setup-title" onDismiss={() => (isSetupOpen = false)} sizeClass="save-dialog mcp-setup-dialog">
    <div class="editor-header mcp-dialog-header">
      <div>
        <p class="eyebrow">Agent configuration</p>
        <h2 id="mcp-setup-title">Connect PostNot over MCP</h2>
        <p>Choose your client, copy the generated snippet, and add it to that client's MCP server configuration.</p>
      </div>
      <button class="icon-button button-compact" type="button" aria-label="Close MCP setup" title="Close" onclick={() => (isSetupOpen = false)}>×</button>
    </div>

    <div class="modal-scroll-body mcp-dialog-body">
      {#if setupErrorText}
        <div class="feedback feedback-error" role="alert">{setupErrorText}</div>
      {:else if !mcpSetup}
        <div class="empty-state" role="status">Loading the installed PostNot path…</div>
      {:else}
        <div class="panel-tabs mcp-client-tabs" role="tablist" aria-label="MCP client">
          {#each configOptions as option (option.value)}
            <button
              class={["tab-button", configKind === option.value && "active"]}
              type="button"
              role="tab"
              aria-selected={configKind === option.value}
              onclick={() => (configKind = option.value)}
            >
              {option.label}
            </button>
          {/each}
        </div>

        <div class="mcp-config-block">
          <div class="mcp-config-heading">
            <span class="field-label">Configuration</span>
            <span>Launches PostNot in headless mode</span>
          </div>
          <pre class="history-preview mcp-config-code">{selectedMcpConfig()}</pre>
        </div>

        <div class="mcp-executable-row">
          <span class="field-label">PostNot executable</span>
          <code>{mcpSetup.executablePath}</code>
        </div>

        <p class="mcp-setup-note">
          The desktop window does not need to stay open. Agents can author saved requests, but they cannot send or delete them.
        </p>
      {/if}
    </div>

    <div class="collections-page-actions mcp-dialog-actions">
      <button class="button-secondary" type="button" onclick={() => (isSetupOpen = false)}>Close</button>
      <button class="button-primary" type="button" onclick={copyMcpConfig} disabled={!mcpSetup}>Copy configuration</button>
    </div>
  </DialogShell>
{/if}
