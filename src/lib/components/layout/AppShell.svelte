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

    <div class="sidebar-card">
      <h2>Milestone 1</h2>
      <p>Single request runner, native execution, local settings, and persisted history.</p>
    </div>

    <div class="sidebar-card muted">
      <h2>Next Up</h2>
      <p>Collections, environments, Postman import, and request tabs.</p>
    </div>
  </aside>

  <main class="workspace">
    <slot />
  </main>
</div>
