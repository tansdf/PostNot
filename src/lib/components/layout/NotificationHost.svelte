<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { fly, fade } from "svelte/transition";
  import DialogShell from "$lib/components/layout/DialogShell.svelte";
  import { notifications } from "$lib/stores/notifications.svelte";

  let prefersReducedMotion = $state(false);

  function handleAction(notification: (typeof notifications.items)[number], action: (typeof notification.actions)[number]) {
    if (action.kind === "navigate" && action.href) {
      notifications.dismiss(notification.id);
      void goto(action.href);
      return;
    }
    notifications.openDetails(notification);
  }

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
          {#if notification.actions.length > 0}
            <div class="notification-actions">
              {#each notification.actions as action}
                <button
                  class="notification-action-button"
                  type="button"
                  onclick={() => handleAction(notification, action)}
                >
                  {action.label}
                </button>
              {/each}
            </div>
          {/if}
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

{#if notifications.activeDetails}
  <DialogShell ariaLabelledby="notification-details-title" onDismiss={() => notifications.closeDetails()}>
    <div class="modal-header">
      <div>
        <p class="eyebrow">Details</p>
        <h2 id="notification-details-title">{notifications.activeDetails.title}</h2>
      </div>
      <button
        class="icon-button row-action-button button-compact"
        type="button"
        aria-label="Close details"
        title="Close details"
        onclick={() => notifications.closeDetails()}
      >
        x
      </button>
    </div>
    <div class="modal-scroll-body notification-details-body">
      {#if notifications.activeDetails.summary}
        <p>{notifications.activeDetails.summary}</p>
      {/if}
      {#if notifications.activeDetails.items?.length}
        <h3>Imported items</h3>
        <ul>
          {#each notifications.activeDetails.items as item}
            <li>{item}</li>
          {/each}
        </ul>
      {/if}
      {#if notifications.activeDetails.warnings?.length}
        <h3>Warnings</h3>
        <ul>
          {#each notifications.activeDetails.warnings as warning}
            <li>{warning}</li>
          {/each}
        </ul>
      {/if}
      {#if notifications.activeDetails.errors?.length}
        <h3>Errors</h3>
        <ul>
          {#each notifications.activeDetails.errors as error}
            <li>{error}</li>
          {/each}
        </ul>
      {/if}
      {#if notifications.activeDetails.rawText}
        <pre>{notifications.activeDetails.rawText}</pre>
      {/if}
    </div>
    <div class="modal-actions">
      <button class="button-secondary" type="button" onclick={() => notifications.closeDetails()}>
        Close
      </button>
    </div>
  </DialogShell>
{/if}
