<script lang="ts">
  import type { Document, Entry } from "./api";
  import { inline, markdown } from "./markdown";
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
    <nav>
      <button class:on={tab === "guide"} onclick={() => (tab = "guide")}>Guide · {document.mod_count} mods</button>
      <button class:on={tab === "load"} onclick={() => (tab = "load")}>Load order · {document.load_order.length}</button>
    </nav>
    {#if tab === "guide"}
      <input type="text" placeholder="Find a mod or plugin" bind:value={filter} />
    {/if}
  </div>

  {#if tab === "guide"}
    <div class="content">
      {#if !words}
        {#if document.docs.summary}<p class="lead">{@html inline(document.docs.summary)}</p>{/if}
        {#if document.docs.body}<div class="prose">{@html markdown(document.docs.body)}</div>{/if}
        {#if document.inputs.length}
          <p class="dim small">This list asks {document.inputs.length} question(s); answer them on the right before installing.</p>
        {/if}
      {/if}
      {#each shown as entry (entry.kind + ("anchor" in entry ? entry.anchor : ""))}
        <EntryView {entry} {words} {matches} />
      {/each}
      {#if words && !shown.length}<p class="dim">Nothing matches.</p>{/if}
      {#if !words && document.docs.footer}<div class="prose footer">{@html markdown(document.docs.footer)}</div>{/if}
    </div>
  {:else}
    <div class="content">
      <p class="dim small">{active} active of {document.load_order.length}, in the order the game loads them. Read only here; author mode will let you drag them.</p>
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
  nav {
    display: flex;
    gap: 4px;
  }
  nav button {
    background: none;
    border-color: transparent;
    color: var(--dim);
  }
  nav button.on {
    color: var(--text);
    background: var(--card);
    border-color: var(--line);
  }
  .bar input {
    margin-left: auto;
    width: 260px;
  }
  .content {
    padding: 16px 20px 40px;
    max-width: 980px;
  }
  .lead {
    font-size: 16px;
    margin: 0 0 8px;
  }
  .small {
    font-size: 12px;
  }
  .footer {
    margin-top: 24px;
    padding-top: 12px;
    border-top: 1px solid var(--line);
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
