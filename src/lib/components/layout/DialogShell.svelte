<script lang="ts">
  import type { Snippet } from "svelte";
  import { modalBackdropDismiss, modalFocusTrap } from "$lib/modal-focus-trap";

  let {
    ariaLabelledby,
    onDismiss,
    sizeClass = "save-dialog",
    dismissible = true,
    children
  }: {
    ariaLabelledby: string;
    onDismiss: () => void;
    sizeClass?: string;
    dismissible?: boolean;
    children: Snippet;
  } = $props();

  function dismiss() {
    if (dismissible) {
      onDismiss();
    }
  }
</script>

<div
  class="modal-backdrop"
  use:modalFocusTrap={{ onEscape: dismiss }}
  use:modalBackdropDismiss={{ onDismiss: dismiss }}
>
  <div
    class={["panel", "panel-custom-inset", sizeClass]}
    role="dialog"
    tabindex="-1"
    aria-modal="true"
    aria-labelledby={ariaLabelledby}
  >
    {@render children()}
  </div>
</div>
