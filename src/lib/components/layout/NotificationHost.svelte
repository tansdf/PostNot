<script lang="ts">
  import { onMount } from "svelte";
  import { fly, fade } from "svelte/transition";
  import { notifications } from "$lib/stores/notifications.svelte";

  let prefersReducedMotion = $state(false);

  onMount(() => {
    const query = window.matchMedia("(prefers-reduced-motion: reduce)");
    const sync = () => (prefersReducedMotion = query.matches);
    sync();
    query.addEventListener("change", sync);
    return () => query.removeEventListener("change", sync);
  });
</script>

{#if notifications.items.length > 0}
  <section class="notification-host" aria-live="polite" aria-relevant="additions removals">
    {#each notifications.items as notification (notification.id)}
      <article
        class={`notification-card notification-${notification.tone}`}
        role={notification.tone === "error" ? "alert" : "status"}
        style={`--notification-duration: ${notification.durationMs}ms;`}
        in:fly={{ y: prefersReducedMotion ? 0 : 18, duration: prefersReducedMotion ? 0 : 180 }}
        out:fade={{ duration: prefersReducedMotion ? 0 : 140 }}
      >
        <div class="notification-body">
          {#if notification.title}
            <strong>{notification.title}</strong>
          {/if}
          <p>{notification.message}</p>
        </div>

        <button
          class="notification-close"
          type="button"
          aria-label="Dismiss notification"
          onclick={() => notifications.dismiss(notification.id)}
        >
          x
        </button>

        <div class="notification-progress-track" aria-hidden="true">
          <div class="notification-progress" onanimationend={() => notifications.dismiss(notification.id)}></div>
        </div>
      </article>
    {/each}
  </section>
{/if}
