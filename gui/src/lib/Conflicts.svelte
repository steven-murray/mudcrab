<script lang="ts">
  import { api, type Between, type Conflicts, type Partner } from "./api";

  // Which installed mods provide the same files, and who wins: a later mod
  // in MO2's order wins a file an earlier one also provides.
  let {
    found,
    select,
    focus,
    hide,
  }: {
    found: Conflicts;
    select: (mod: string) => void;
    focus: string | null;
    /** Hide in `id` the files `other` also provides (`file_hide` with `conflicts_with`). */
    hide: (id: string, other: string) => void;
  } = $props();

  let filter = $state("");
  // The pair whose files are open, and them.
  let pair = $state<{ id: string; other: string } | null>(null);
  let between = $state<Between | null>(null);
  let loading = $state(false);
  let error = $state<string | null>(null);

  const words = $derived((focus && !filter ? focus : filter).trim().toLowerCase());
  const mods = $derived(
    found.sweep.mods.filter(
      (m) =>
        !words ||
        m.id.toLowerCase().includes(words) ||
        m.wins.some((p) => p.mod_id.toLowerCase().includes(words)) ||
        m.losses.some((p) => p.mod_id.toLowerCase().includes(words)),
    ),
  );

  async function open(id: string, other: string) {
    if (pair?.id === id && pair.other === other) {
      pair = null;
      return;
    }
    pair = { id, other };
    between = null;
    error = null;
    loading = true;
    try {
      between = await api.conflictsBetween(id, other);
    } catch (err) {
      error = String(err);
    } finally {
      loading = false;
    }
  }

  const total = (list: Partner[]) => list.reduce((n, p) => n + p.files, 0);
</script>

<div class="bar">
  <input type="text" placeholder={focus ? `Showing ${focus}; type to find another` : "Find a mod"} bind:value={filter} />
  <span class="dim small">
    {found.sweep.mods.length} of {found.sweep.installed} installed mods share files{#if found.sweep.missing}; {found.sweep.missing} not installed, so not compared{/if}
  </span>
</div>

{#each mods as m (m.id)}
  <article>
    <div class="top">
      <button class="link name" onclick={() => select(m.id)}>{m.id}</button>
      {#if m.section.length}<span class="dim small">{m.section.join(" - ")}</span>{/if}
    </div>
    {#each [{ list: m.wins, verb: "wins", cls: "win" }, { list: m.losses, verb: "loses", cls: "loss" }] as side}
      {#if side.list.length}
        <div class="side">
          <span class={side.cls}>{side.verb} {total(side.list)}</span>
          <span class="dim">{side.verb === "wins" ? "over" : "to"}</span>
          {#each side.list as p}
            <button
              class="chip"
              class:on={pair?.id === m.id && pair.other === p.mod_id}
              onclick={() => open(m.id, p.mod_id)}
              title="The files both provide">{p.mod_id} <span class="dim">({p.files})</span></button
            >
          {/each}
        </div>
      {/if}
    {/each}
    {#if pair?.id === m.id}
      <div class="files">
        {#if loading}
          <span class="dim">Reading…</span>
        {:else if error}
          <pre class="error">{error}</pre>
        {:else if between}
          <div class="dim small">
            {between.shared.overlap.length} file(s) both {m.id} and {pair.other} provide{#if between.shared.already_hidden}; {between.shared.already_hidden} more already hidden in {m.id}{/if}
          </div>
          <ul>{#each between.shared.overlap as path}<li><code>{path}</code></li>{/each}</ul>
          {#if between.shared.overlap.length}
            <button
              class="hide"
              onclick={() => hide(m.id, pair!.other)}
              title="Adds a file_hide action with conflicts_with to {m.id}: at install, its copies of these files are hidden, so {pair.other}'s are used whatever the order"
              >Hide these in {m.id}</button
            >
          {/if}
          <div class="dim small cmd">Same from a terminal: <code>{between.command}</code></div>
        {/if}
      </div>
    {/if}
  </article>
{/each}
{#if !mods.length}<p class="dim">{found.sweep.mods.length ? "Nothing matches." : "No two installed mods share a file."}</p>{/if}
<p class="dim small cmd">Same from a terminal: <code>{found.command}</code></p>

<style>
  .bar {
    display: flex;
    gap: 14px;
    align-items: center;
    flex-wrap: wrap;
    margin-bottom: 8px;
  }
  .bar input {
    flex: 1;
    max-width: 320px;
  }
  .small {
    font-size: 12px;
  }
  article {
    border-bottom: 1px solid var(--line);
    padding: 6px 2px;
  }
  .top {
    display: flex;
    gap: 10px;
    align-items: baseline;
    flex-wrap: wrap;
  }
  .name {
    font-weight: 600;
    text-align: left;
  }
  .side {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 6px;
    align-items: baseline;
    font-size: 12px;
    margin-top: 3px;
  }
  .win {
    color: var(--good);
  }
  .loss {
    color: var(--warning);
  }
  .chip {
    font-size: 12px;
    padding: 0 6px;
  }
  .chip.on {
    border-color: var(--accent);
    background: var(--accent-soft);
  }
  .files {
    margin: 6px 0 4px 12px;
    font-size: 12px;
  }
  .files ul {
    margin: 4px 0;
    padding-left: 16px;
    max-height: 300px;
    overflow: auto;
  }
  .files code,
  .cmd code {
    overflow-wrap: anywhere;
  }
  .hide {
    font-size: 12px;
    margin: 2px 0 6px;
  }
  .error {
    color: var(--problem);
    white-space: pre-wrap;
  }
</style>
