<script lang="ts">
  import type { Span } from "./api";

  // A message from mudcrab's library, with its names set off.
  let { spans }: { spans: Span[] } = $props();
</script>

{#each spans as span}{#if span.kind === "name"}<strong>{span.text}</strong>{:else if span.kind === "dim"}<span class="dim">{span.text}</span>{:else}{span.text}{/if}{/each}
