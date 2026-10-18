<script lang="ts">
  import { SvelteSet } from "svelte/reactivity";
  import type { FileRow, Tree } from "./api";

  // A mod's archive as a tree, marked with what install takes from it. Read
  // only: phase 6 turns the marks into ticks.
  let { tree }: { tree: Tree } = $props();

  // Folders start closed, except the top level of a small archive.
  const open = new SvelteSet<string>();
  $effect(() => {
    open.clear();
    if (tree.rows.length <= 60) for (const row of tree.rows) if (row.folder) open.add(row.path);
  });

  // A row shows when every folder above it is open.
  function shown(row: FileRow): boolean {
    let at = row.path.lastIndexOf("/");
    while (at > 0) {
      if (!open.has(row.path.slice(0, at))) return false;
      at = row.path.lastIndexOf("/", at - 1);
    }
    return true;
  }
  const rows = $derived(tree.rows.filter(shown));

  function toggle(row: FileRow) {
    if (open.has(row.path)) open.delete(row.path);
    else open.add(row.path);
  }

  function mark(row: FileRow): string {
    if (!row.offered) return "·";
    return row.state === "all" ? "✔" : row.state === "some" ? "◐" : "○";
  }

  function why(row: FileRow): string {
    if (row.package) return row.state === "none" ? "A BAIN package not chosen" : "A BAIN package chosen";
    if (!row.offered) return "Left out by the layout: a package or option not chosen";
    if (row.folder) return `${row.taken} of ${row.files} file(s) installed`;
    if (row.destination) return `Installed as ${row.destination}`;
    return `Left out by the archive's include or exclude (filters see it as ${row.filtered_as})`;
  }
</script>

<div class="tree">
  <div class="meta">
    <span><strong>{tree.taken}</strong> of {tree.entries} file(s) installed</span>
    {#if tree.layout}<span class="dim">layout: {tree.layout}</span>{/if}
    {#if tree.bain_subpackages.length}<span class="dim">packages: {tree.bain_subpackages.join(", ")}</span>{/if}
    {#if tree.include.length}<span class="dim">include: <code>{tree.include.join(", ")}</code></span>{/if}
    {#if tree.exclude.length}<span class="dim">exclude: <code>{tree.exclude.join(", ")}</code></span>{/if}
  </div>
  <div class="legend dim">✔ installed · ◐ partly · ○ filtered out · · not chosen by the layout</div>
  <ul>
    {#each rows as row (row.path)}
      <li
        class:out={row.state === "none"}
        class:unoffered={!row.offered}
        class:package={row.package}
        style="--depth: {row.depth}"
        title={why(row)}
      >
        <span class="mark">{mark(row)}</span>
        {#if row.folder}
          <button class="name folder" onclick={() => toggle(row)}>
            <span class="twisty">{open.has(row.path) ? "▾" : "▸"}</span>{row.name}/
          </button>
          <span class="count dim">{row.taken}/{row.files}</span>
        {:else}
          <span class="name">{row.name}</span>
          {#if row.destination && row.destination.toLowerCase() !== row.path.toLowerCase()}
            <span class="dest dim" title="Where it goes in the mod's folder">→ {row.destination}</span>
          {/if}
        {/if}
      </li>
    {/each}
  </ul>
  <p class="cmd dim">Same from a terminal: <code>{tree.command}</code></p>
</div>

<style>
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 14px;
    font-size: 13px;
  }
  .legend {
    font-size: 11px;
    margin: 4px 0 8px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    font-size: 13px;
  }
  li {
    display: flex;
    align-items: baseline;
    gap: 6px;
    padding: 1px 0 1px calc(var(--depth) * 16px);
    min-width: 0;
  }
  li:hover {
    background: var(--card-hi);
  }
  .mark {
    width: 14px;
    color: var(--good);
    flex: none;
    text-align: center;
  }
  li.out .mark {
    color: var(--warning);
  }
  li.unoffered .mark,
  li.unoffered .name {
    color: var(--faint);
  }
  li.package > .name {
    font-weight: 600;
  }
  .name {
    overflow-wrap: anywhere;
  }
  button.folder {
    background: none;
    border: none;
    padding: 0;
    text-align: left;
  }
  .twisty {
    display: inline-block;
    width: 12px;
    color: var(--dim);
  }
  .count,
  .dest {
    font-size: 11px;
    overflow-wrap: anywhere;
  }
  .cmd {
    font-size: 12px;
    margin-top: 10px;
    overflow-wrap: anywhere;
  }
</style>
