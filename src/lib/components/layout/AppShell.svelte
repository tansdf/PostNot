<script lang="ts">
  export let title = "PostNot";
  export let subtitle = "A local-first desktop API client.";
</script>

<div class="app-shell">
  <aside class="sidebar">
    <div class="brand-block">
      <p class="eyebrow">Desktop API Client</p>
      <h1>{title}</h1>
      <p>{subtitle}</p>
    </div>

    <nav class="sidebar-nav" aria-label="Primary">
      <a class="sidebar-link" href="/">Requests</a>
      <a class="sidebar-link" href="/settings">Settings</a>
    </nav>

    <div class="sidebar-card">
      <h2>Milestone 1</h2>
      <p>Single request runner, native execution, local settings, and persisted history.</p>
    </div>

    <div class="sidebar-card muted">
      <h2>Current Scope</h2>
      <p>Saved request settings now affect execution, and request history persists in SQLite.</p>
    </div>
  </aside>

  <main class="workspace">
    <slot />
  </main>
</div>
