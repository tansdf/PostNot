<script lang="ts">
  let {
    label = "Send",
    disabled = false,
    isSending = false,
    isCanceling = false,
    onPreview = undefined,
    onSend = () => {},
    onCancel = () => {}
  }: {
    label?: string;
    disabled?: boolean;
    isSending?: boolean;
    isCanceling?: boolean;
    onPreview?: (() => Promise<void> | void) | undefined;
    onSend?: () => Promise<void> | void;
    onCancel?: () => Promise<void> | void;
  } = $props();
</script>

<div class={["request-send-actions", isSending && "request-send-actions-cancel", !onPreview && "request-send-actions-solo"]}>
  {#if onPreview}
    <button
      class="request-send-preview-control"
      type="button"
      onclick={onPreview}
      aria-label="Preview resolved request"
      title="Preview resolved request"
      disabled={isSending || isCanceling}
    >
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M2.5 12s3.5-6 9.5-6 9.5 6 9.5 6-3.5 6-9.5 6-9.5-6-9.5-6Z" />
        <circle cx="12" cy="12" r="3" />
      </svg>
    </button>
  {/if}

  <button
    class="request-send-main-control"
    type="button"
    onclick={() => (isSending ? onCancel() : onSend())}
    disabled={isCanceling || disabled}
  >
    {#if isSending}
      {isCanceling ? "Canceling..." : "Cancel"}
    {:else}
      {label}
    {/if}
  </button>
</div>
