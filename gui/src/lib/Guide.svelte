<script lang="ts">
  import type { Document, Entry } from "./api";
  import EntryView from "./EntryView.svelte";

  let { document }: { document: Document } = $props();
  let tab = $state<"guide" | "load">("guide");
  let filter = $state("");

  // Whether an entry, or anything in it, matches the filter.
  function matches(entry: Entry, words: string): boolean {
    if (!words) return true;
    if (entry.kind === "section") return entry.title.toLowerCase().includes(words) || entry.entries.some((e) => matches(e, words));
    if (entry.kind === "mod")
      return entry.title.toLowerCase().includes(words) || entry.plugins.some((p) => p.toLowerCase().includes(words));
    return false;
  }
  const words = $derived(filter.trim().toLowerCase());
  const shown = $derived(document.entries.filter((e) => matches(e, words)));
  const active = $derived(document.load_order.filter((p) => p.active).length);
</script>

<div class="guide">
  <div class="bar">
    <nav role="group" aria-label="Show" data-tour="guide-switch">
      <button class:on={tab === "guide"} onclick={() => (tab = "guide")}>Guide · {document.mod_count} mods</button>
      <button class:on={tab === "load"} onclick={() => (tab = "load")}>Load order · {active}/{document.load_order.length}</button>
    </nav>
    {#if tab === "guide"}
      <input type="text" placeholder="Find a mod or plugin" bind:value={filter} />
    {/if}
  </div>

  {#if tab === "guide"}
    <div class="content">
      {#each shown as entry (entry.kind + ("anchor" in entry ? entry.anchor : ""))}
        <EntryView {entry} {words} {matches} />
      {/each}
      {#if words && !shown.length}<p class="dim">Nothing matches.</p>{/if}
    </div>
  {:else}
    <div class="content">
      <ol class="load">
        {#each document.load_order as plugin, i}
          <li class:inactive={!plugin.active}>
            <span class="n">{i + 1}</span>
            <span class="plugin">{plugin.name}</span>
            <span class="dim">{plugin.mod ?? ""}</span>
            {#if !plugin.active}<span class="badge">inactive</span>{/if}
          </li>
        {/each}
      </ol>
    </div>
  {/if}
</div>

<style>
  .bar {
    position: sticky;
    top: 0;
    z-index: 1;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 16px;
    background: var(--bg);
    border-bottom: 1px solid var(--line);
  }
  /* A switch between the two, drawn as the Install / Author one is. */
  nav {
    display: flex;
    border: 1px solid var(--line);
    border-radius: 6px;
    overflow: hidden;
  }
  nav button {
    padding: 4px 12px;
    border: none;
    border-radius: 0;
    background: none;
    color: var(--dim);
  }
  nav button + button {
    border-left: 1px solid var(--line);
  }
  nav button:hover:not(.on) {
    color: var(--text);
    background: var(--card);
  }
  nav button.on {
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
  }
  .bar input {
    margin-left: auto;
    width: 260px;
  }
  .content {
    padding: 16px 20px 40px;
    max-width: 980px;
  }
  ol.load {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  ol.load li {
    display: grid;
    grid-template-columns: 44px minmax(0, 1fr) minmax(0, 1fr) 70px;
    gap: 8px;
    padding: 4px 8px;
    border-bottom: 1px solid var(--line);
  }
  ol.load li .n {
    color: var(--faint);
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  ol.load li.inactive .plugin {
    color: var(--dim);
  }
  .badge {
    font-size: 11px;
    color: var(--dim);
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 0 5px;
    justify-self: end;
  }
</style>
