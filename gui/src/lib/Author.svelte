<script lang="ts">
  import { SvelteSet } from "svelte/reactivity";
  import {
    api,
    type Checked,
    type Conflicts as Found,
    type Entry,
    type ListView,
    type LoadOrder as Order,
    type ModDoc,
    type ModView,
    type Tree,
  } from "./api";
  import Conflicts from "./Conflicts.svelte";
  import EntryView from "./EntryView.svelte";
  import FileTree from "./FileTree.svelte";
  import LoadOrder from "./LoadOrder.svelte";
  import Text from "./Text.svelte";

  // Author mode, reading: the list as a tree on the left; on the right, the
  // chosen mod and its files, the load order with masters, conflicts and
  // check. Each view is what a command shows, and says which.
  let { list }: { list: ListView } = $props();

  type Tab = "mod" | "load" | "conflicts" | "check";
  let tab = $state<Tab>("mod");
  let filter = $state("");
  let selected = $state<string | null>(null);
  const opened = new SvelteSet<string>();

  let error = $state<string | null>(null);
  let view = $state<ModView | null>(null);
  let archive = $state(1);
  let tree = $state<Tree | null>(null);
  let treeError = $state<string | null>(null);
  let loadingTree = $state(false);
  let order = $state<Order | null>(null);
  let found = $state<Found | null>(null);
  let conflictsError = $state<string | null>(null);
  let checked = $state<Checked | null>(null);
  let checking = $state(false);
  let busy = $state(false);

  // Every mod in the document by id, and the sections above it.
  const mods = $derived.by(() => {
    const out = new Map<string, { mod: ModDoc; path: string[] }>();
    const walk = (entries: Entry[], path: string[]) => {
      for (const e of entries) {
        if (e.kind === "mod") out.set(e.id, { mod: e, path });
        else if (e.kind === "section") walk(e.entries, [...path, e.anchor]);
      }
    };
    walk(list.document.entries, []);
    return out;
  });

  function matches(entry: Entry, words: string): boolean {
    if (!words) return true;
    if (entry.kind === "section")
      return entry.title.toLowerCase().includes(words) || entry.entries.some((e) => matches(e, words));
    if (entry.kind === "mod")
      return (
        entry.title.toLowerCase().includes(words) ||
        entry.id.toLowerCase().includes(words) ||
        entry.plugins.some((p) => p.toLowerCase().includes(words))
      );
    return false;
  }
  const words = $derived(filter.trim().toLowerCase());
  function count(e: Entry): number {
    if (e.kind === "mod") return 1;
    if (e.kind === "section") return e.entries.reduce((n, c) => n + count(c), 0);
    return 0;
  }

  // Choose a mod: open its sections, show it, read its first archive.
  async function select(id: string) {
    selected = id;
    tab = "mod";
    for (const anchor of mods.get(id)?.path ?? []) opened.add(anchor);
    view = null;
    tree = null;
    treeError = null;
    try {
      view = await api.modView(id);
      archive = 1;
      if (view.archives.length) await readTree(id, 1);
    } catch (err) {
      error = String(err);
    }
  }

  async function readTree(id: string, number: number) {
    archive = number;
    tree = null;
    treeError = null;
    loadingTree = true;
    try {
      tree = await api.modFiles(id, number);
    } catch (err) {
      treeError = String(err);
    } finally {
      loadingTree = false;
    }
  }

  async function show(next: Tab) {
    tab = next;
    error = null;
    try {
      if (next === "load" && !order) order = await api.loadOrder();
      if (next === "conflicts" && !found) {
        busy = true;
        conflictsError = null;
        found = await api.conflicts();
      }
    } catch (err) {
      if (next === "conflicts") conflictsError = String(err);
      else error = String(err);
    } finally {
      busy = false;
    }
  }

  async function runCheck() {
    checking = true;
    error = null;
    try {
      checked = await api.check();
      order = await api.loadOrder();
    } catch (err) {
      error = String(err);
    } finally {
      checking = false;
    }
  }

  // Read everything again: the list may have been changed outside.
  async function reload() {
    order = null;
    found = null;
    if (selected) await select(selected);
    await show(tab === "mod" ? "mod" : tab);
  }

  const chosen = $derived(selected ? mods.get(selected)?.mod : undefined);
</script>

<div class="author">
  <section class="left">
    <div class="bar">
      <input type="text" placeholder="Find a mod or plugin" bind:value={filter} />
    </div>
    <div class="list">
      {#snippet entries(items: Entry[], depth: number)}
        {#each items.filter((e) => matches(e, words)) as e (e.kind + e.anchor)}
          {#if e.kind === "section"}
            {@const open = opened.has(e.anchor) || !!words}
            <button
              class="section"
              style="--depth: {depth}"
              onclick={() => (opened.has(e.anchor) ? opened.delete(e.anchor) : opened.add(e.anchor))}
            >
              <span class="twisty">{open ? "▾" : "▸"}</span>
              <span class="title">{e.title}</span>
              <span class="dim count">{count(e)}</span>
            </button>
            {#if open}{@render entries(e.entries, depth + 1)}{/if}
          {:else if e.kind === "mod"}
            <button class="mod" class:on={selected === e.id} style="--depth: {depth}" onclick={() => select(e.id)}>
              <span class="title">{e.title}</span>
              {#if e.condition}<span class="tag cond" title={e.condition.text}>if</span>{/if}
              {#if e.enabled === false}<span class="tag">off</span>{/if}
              {#if e.mod_type}<span class="tag">{e.mod_type}</span>{/if}
              {#if e.plugins.length}<span class="dim count">{e.plugins.length}p</span>{/if}
            </button>
          {/if}
        {/each}
      {/snippet}
      {@render entries(list.document.entries, 0)}
    </div>
  </section>

  <aside class="right">
    <div class="bar">
      <nav>
        <button class:on={tab === "mod"} onclick={() => show("mod")}>Mod</button>
        <button class:on={tab === "load"} onclick={() => show("load")}>Load order</button>
        <button class:on={tab === "conflicts"} onclick={() => show("conflicts")}>Conflicts</button>
        <button class:on={tab === "check"} onclick={() => show("check")}>
          Check{#if checked}<span class="n" class:bad={checked.problems > 0} class:warn={!checked.problems && checked.warnings > 0}
              >{checked.problems || checked.warnings || "✔"}</span
            >{/if}
        </button>
      </nav>
      <button class="reload" onclick={reload} title="Read the list again, if it has changed on disk">Reload</button>
    </div>

    <div class="content">
      {#if error}<pre class="error">{error}</pre>{/if}

      {#if tab === "mod"}
        {#if !chosen}
          <p class="dim">Choose a mod on the left to see what it installs. Editing comes next; for now this reads the list as it is.</p>
        {:else}
          <EntryView entry={{ kind: "mod", ...chosen }} words="" matches={() => true} />
          {#if view?.left_out}
            <p class="dim">The answers given in Install leave this mod out, so it is not planned.</p>
          {:else if view && !view.archives.length}
            <p class="dim">No archives: this mod is made at install.</p>
          {:else if view}
            {#if view.archives.length > 1}
              <nav class="archives">
                {#each view.archives as a}
                  <button class:on={archive === a.number} onclick={() => readTree(view!.id, a.number)}>{a.number}. {a.label}</button>
                {/each}
              </nav>
            {/if}
            {#if loadingTree}
              <p class="dim">Reading the archive…</p>
            {:else if treeError}
              <pre class="error">{treeError}</pre>
            {:else if tree}
              <FileTree {tree} />
            {/if}
          {/if}
        {/if}
      {:else if tab === "load"}
        {#if order}<LoadOrder {order} {select} {runCheck} {checking} />{:else}<p class="dim">Reading…</p>{/if}
      {:else if tab === "conflicts"}
        {#if busy}
          <p class="dim">Comparing every installed mod's files…</p>
        {:else if conflictsError}
          <pre class="error">{conflictsError}</pre>
        {:else if found}
          <Conflicts {found} {select} focus={selected} />
        {/if}
      {:else}
        <div class="checkbar">
          <button class="primary" onclick={runCheck} disabled={checking}>{checking ? "Checking…" : checked ? "Check again" : "Run check"}</button>
          {#if checked}<span>{checked.done.summary}</span>{/if}
        </div>
        {#if checked}
          {#each checked.groups as group}
            <section class="group {group.tone}">
              <h3>{group.heading} <span class="dim">({group.items.length})</span></h3>
              <ul>
                {#each group.items as item}
                  <li>
                    {#if item.mod_id}
                      <button class="link" onclick={() => select(item.mod_id!)} title="Show this mod"><Text spans={item.text} /></button>
                    {:else}
                      <Text spans={item.text} />
                    {/if}
                  </li>
                {/each}
              </ul>
            </section>
          {/each}
          <p class="dim small">Same from a terminal: <code>{checked.done.command}</code></p>
        {:else}
          <p class="dim">Checks every archive, layout, action, master, the load order and BSAs, as <code>mudcrab check</code> does. It reads every archive, so it takes a minute on a large list.</p>
        {/if}
      {/if}
    </div>
  </aside>
</div>

<style>
  .author {
    display: grid;
    grid-template-columns: minmax(260px, 380px) minmax(0, 1fr);
    min-height: 0;
    height: 100%;
  }
  .left,
  .right {
    overflow: auto;
    min-height: 0;
  }
  .left {
    border-right: 1px solid var(--line);
    background: var(--panel);
  }
  .bar {
    position: sticky;
    top: 0;
    z-index: 1;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    background: var(--bg);
    border-bottom: 1px solid var(--line);
  }
  .left .bar input {
    width: 100%;
  }
  .list {
    padding: 4px 0 30px;
  }
  .section,
  .mod {
    display: flex;
    align-items: baseline;
    gap: 6px;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    border-radius: 0;
    padding: 3px 10px 3px calc(10px + var(--depth) * 14px);
    min-width: 0;
  }
  .section .title {
    font-weight: 600;
  }
  .mod .title,
  .section .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }
  .mod {
    padding-left: calc(28px + var(--depth) * 14px);
  }
  .mod:hover,
  .section:hover {
    background: var(--card-hi);
  }
  .mod.on {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .twisty {
    width: 12px;
    color: var(--dim);
    flex: none;
  }
  .count {
    margin-left: auto;
    font-size: 11px;
    flex: none;
  }
  .tag {
    font-size: 10px;
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 0 4px;
    color: var(--dim);
    flex: none;
  }
  .tag.cond {
    color: var(--note);
    border-color: var(--note);
  }
  nav {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
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
  .n {
    margin-left: 6px;
    font-size: 11px;
    color: var(--good);
  }
  .n.warn {
    color: var(--warning);
  }
  .n.bad {
    color: var(--problem);
  }
  .reload {
    margin-left: auto;
  }
  .content {
    padding: 12px 18px 40px;
  }
  .archives {
    margin: 8px 0;
  }
  .archives button {
    font-size: 12px;
    overflow-wrap: anywhere;
    text-align: left;
  }
  .error {
    color: var(--problem);
    white-space: pre-wrap;
    font-family: inherit;
    font-size: 13px;
  }
  .checkbar {
    display: flex;
    gap: 12px;
    align-items: center;
    flex-wrap: wrap;
    margin-bottom: 10px;
  }
  .group h3 {
    font-size: 14px;
    margin: 14px 0 4px;
  }
  .group.problem h3 {
    color: var(--problem);
  }
  .group.warning h3 {
    color: var(--warning);
  }
  .group ul {
    margin: 0;
    padding-left: 18px;
    font-size: 13px;
  }
  .group li .link {
    color: inherit;
    text-align: left;
  }
  .group li .link:hover {
    text-decoration: underline;
  }
  .small {
    font-size: 12px;
  }
  code {
    overflow-wrap: anywhere;
  }
</style>
