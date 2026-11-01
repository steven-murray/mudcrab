<script lang="ts">
  import type { Snippet } from "svelte";

  // A dialog over the window: a title, its body, and its buttons at the foot.
  // A click outside it, or Escape, closes it.
  let {
    title,
    close,
    width = 560,
    children,
    foot,
  }: {
    title: string;
    close: () => void;
    width?: number;
    children: Snippet;
    foot: Snippet;
  } = $props();
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && close()} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="veil" onclick={close}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="dialog" role="dialog" tabindex="-1" aria-label={title} style:width="min({width}px, 94vw)" onclick={(e) => e.stopPropagation()}>
    <div class="dhead">
      <strong>{title}</strong>
      <button class="link" onclick={close}>Cancel</button>
    </div>
    <div class="dbody">{@render children()}</div>
    <div class="dfoot">{@render foot()}</div>
  </div>
</div>

<style>
  .veil {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: rgba(0, 0, 0, 0.45);
    display: grid;
    place-items: center;
  }
  .dialog {
    max-height: 90vh;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 10px;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.45);
  }
  .dhead,
  .dfoot {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
    padding: 10px 16px;
  }
  .dhead {
    border-bottom: 1px solid var(--line);
  }
  .dhead strong {
    overflow-wrap: anywhere;
  }
  .dfoot {
    border-top: 1px solid var(--line);
    justify-content: flex-end;
  }
  .dbody {
    padding: 12px 18px 16px;
    overflow: auto;
    font-size: 13px;
  }
</style>
