<script lang="ts">
  import { SvelteSet } from "svelte/reactivity";
  import type { FileRow, Tree } from "./api";
  import { startDrag } from "./drag";

  // A mod's archive as a tree, marked with what install takes from it, with
  // ticks that follow the installer an author already knows:
  //
  // - BAIN: packages only, at the top level, as Wrye Bash's installer tab
  //   ticks them (`files --package`); the files inside are shown read-only.
  // - FOMOD: no ticks here; its options are chosen in its installer's steps.
  // - Anything else: files and folders, unticking writing an `exclude`
  //   (`files --exclude`) and ticking back dropping it.
  //
  // For any layout, "exclude" on a row is the expert override: an `exclude`
  // for what the filters see it as. An installed file or folder can be moved
  // within the mod -- dragged onto a folder, or given a new path with
  // "move…" -- which writes a `file_move` action.
  let {
    tree,
    edit,
    problem,
    move,
  }: {
    tree: Tree;
    edit: (key: string, add: string[], drop: string[]) => void;
    problem: (message: string) => void;
    /** Move `from` to `to`, both in the mod's folder (`file_move`). */
    move: (from: string, to: string) => void;
  } = $props();

  let list = $state<HTMLElement | null>(null);
  const base = (path: string) => path.slice(path.lastIndexOf("/") + 1);

  // A drop into a folder (or the mod's top, keyed ""): the row keeps its
  // name there.
  function dropped(row: FileRow, key: string) {
    const from = row.destination!;
    const to = key ? `${key}/${base(from)}` : base(from);
    if (to.toLowerCase() === from.toLowerCase()) return;
    if (to.toLowerCase().startsWith(`${from.toLowerCase()}/`)) return problem("A folder cannot be moved into itself.");
    move(from, to);
  }

  // "move…": a new path typed for a row.
  let moving = $state<string | null>(null);
  let movingTo = $state("");
  function moveTyped(event: SubmitEvent, row: FileRow) {
    event.preventDefault();
    const to = movingTo.trim().replace(/^\/+|\/+$/g, "");
    if (to && to !== row.destination) move(row.destination!, to);
    moving = null;
  }

  const bain = $derived(tree.layout === "bain");
  const fomod = $derived(tree.layout === "fomod");
  const lower = (list: string[]) => list.map((v) => v.toLowerCase());

  // BAIN: a top-level package ticked when chosen.
  function packageTick(row: FileRow) {
    const name = row.name;
    if (lower(tree.bain_conditional).includes(name.toLowerCase()))
      return problem(`${name} is chosen under a condition; change that in the list's file.`);
    const chosen = lower(tree.bain_subpackages).includes(name.toLowerCase());
    edit("bain_subpackages", chosen ? [] : [name], chosen ? [name] : []);
  }

  // Anything else: untick to exclude it, tick to drop the exclude that left
  // it out.
  function fileTick(row: FileRow) {
    if (!row.offered || !row.exclude_glob)
      return problem(
        row.offered
          ? `${row.path} is the mod's data folder itself; untick what is in it instead.`
          : `${row.path} is left out by the layout, not a filter.`,
      );
    if (row.state !== "none") return edit("exclude", [row.exclude_glob], []);
    const own = tree.exclude.find((g) => g.toLowerCase() === row.exclude_glob!.toLowerCase());
    if (own) return edit("exclude", [], [own]);
    problem(
      `${row.path} is left out by a wider filter (${[
        ...tree.include.map((g) => `include ${g}`),
        ...tree.exclude.map((g) => `exclude ${g}`),
      ].join(", ")}); change that below.`,
    );
  }

  function exclude(row: FileRow) {
    if (row.exclude_glob) edit("exclude", [row.exclude_glob], []);
  }

  let newGlob = $state("");
  let newKey = $state<"exclude" | "include">("exclude");
  function addGlob(event: SubmitEvent) {
    event.preventDefault();
    const glob = newGlob.trim();
    if (!glob) return;
    edit(newKey, [glob], []);
    newGlob = "";
  }

  // Folders start closed, except in a small archive, and stay as they were
  // when the same archive is read again after an edit.
  const open = new SvelteSet<string>();
  let shownLabel = "";
  $effect(() => {
    if (tree.label === shownLabel) return;
    shownLabel = tree.label;
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
  <div class="filters">
    {#each [{ key: "include", list: tree.include }, { key: "exclude", list: tree.exclude }] as filter}
      {#each filter.list as glob}
        <span class="glob {filter.key}"
          >{filter.key} <code>{glob}</code><button class="x" title="Take it out" onclick={() => edit(filter.key, [], [glob])}
            >✕</button
          ></span
        >
      {/each}
    {/each}
    <form onsubmit={addGlob}>
      <select bind:value={newKey}><option>exclude</option><option>include</option></select>
      <input type="text" placeholder="a glob, e.g. docs/**" bind:value={newGlob} />
      <button type="submit" disabled={!newGlob.trim()}>Add</button>
    </form>
  </div>
  <div class="legend dim">
    ✔ installed · ◐ partly · ○ filtered out · · not chosen by the layout
    {#if bain}· tick a package to choose it, as in Wrye Bash{:else if fomod}· its options are chosen in the FOMOD installer, not here{:else}· untick to exclude{/if}
  </div>
  <div class="top dim" data-drop="" data-drop-into>Drop here to move it to the top of the mod</div>
  <ul bind:this={list}>
    {#each rows as row (row.path)}
      <li
        class:out={row.state === "none"}
        class:unoffered={!row.offered}
        class:package={row.package}
        style="--depth: {row.depth}"
        title={why(row)}
        data-drop={row.folder && row.destination ? row.destination : undefined}
        data-drop-into={row.folder && row.destination ? "" : undefined}
      >
        {#if row.destination}
          <span
            class="grip"
            role="button"
            tabindex="-1"
            title="Drag onto a folder to move it there (file_move)"
            onpointerdown={(e) =>
              startDrag(e, {
                scroller: list!.closest(".content") as HTMLElement,
                accepts: (_, place) => place === "into",
                onDrop: (key) => dropped(row, key),
              })}>⠿</span
          >
        {:else}<span class="nogrip"></span>{/if}
        {#if bain && row.package}
          <input
            type="checkbox"
            checked={lower(tree.bain_subpackages).includes(row.name.toLowerCase())}
            disabled={lower(tree.bain_conditional).includes(row.name.toLowerCase())}
            onclick={(e) => {
              e.preventDefault();
              packageTick(row);
            }}
          />
        {:else if !bain && !fomod && row.offered}
          <input
            type="checkbox"
            checked={row.state === "all"}
            indeterminate={row.state === "some"}
            onclick={(e) => {
              e.preventDefault();
              fileTick(row);
            }}
          />
        {:else}
          <span class="mark">{mark(row)}</span>
        {/if}
        {#if row.folder}
          <button class="name folder" onclick={() => toggle(row)}>
            <span class="twisty">{open.has(row.path) ? "▾" : "▸"}</span>{row.name}/
          </button>
          <span class="count dim">{row.taken}/{row.files}</span>
          {#if (bain || fomod) && row.taken && row.exclude_glob}
            <button class="override" onclick={() => exclude(row)} title={`Write exclude ${row.exclude_glob}${bain ? " -- it applies to every package" : ""}`}
              >exclude</button
            >
          {/if}
        {:else}
          <span class="name">{row.name}</span>
          {#if row.destination && row.destination.toLowerCase() !== row.path.toLowerCase()}
            <span class="dest dim" title="Where it goes in the mod's folder">→ {row.destination}</span>
          {/if}
          {#if (bain || fomod) && row.taken && row.exclude_glob}
            <button class="override" onclick={() => exclude(row)} title={`Write exclude ${row.exclude_glob}${bain ? " -- it applies to every package" : ""}`}
              >exclude</button
            >
          {/if}
        {/if}
        {#if row.destination}
          <button
            class="override"
            onclick={() => {
              moving = row.path;
              movingTo = row.destination ?? "";
            }}
            title="Move it within the mod, to a path you type (file_move)">move…</button
          >
        {/if}
      </li>
      {#if moving === row.path}
        <li class="moveto" style="--depth: {row.depth}">
          <form onsubmit={(e) => moveTyped(e, row)}>
            <span class="dim">move <code>{row.destination}</code> to</span>
            <!-- svelte-ignore a11y_autofocus -->
            <input type="text" bind:value={movingTo} autofocus />
            <button type="submit">Move</button>
            <button type="button" onclick={() => (moving = null)}>Cancel</button>
          </form>
        </li>
      {/if}
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
  .filters {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 8px;
    align-items: center;
    margin: 6px 0;
    font-size: 12px;
  }
  .glob {
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 0 0 0 6px;
    color: var(--dim);
  }
  .glob.include {
    border-color: var(--note);
  }
  .glob code {
    color: var(--text);
  }
  .x {
    border: none;
    background: none;
    padding: 0 5px;
    color: var(--dim);
  }
  .x:hover {
    color: var(--problem);
  }
  .filters form {
    display: flex;
    gap: 4px;
  }
  .filters input {
    width: 180px;
    font-size: 12px;
  }
  .filters select,
  .filters button {
    font-size: 12px;
    padding: 2px 6px;
  }
  input[type="checkbox"] {
    margin: 0;
    flex: none;
  }
  .override {
    font-size: 10px;
    padding: 0 5px;
    visibility: hidden;
  }
  li:hover .override {
    visibility: visible;
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
  .top {
    font-size: 11px;
    padding: 2px 6px;
    border: 1px dashed var(--line);
    border-radius: 4px;
    margin-bottom: 4px;
  }
  .grip,
  .nogrip {
    width: 10px;
    flex: none;
  }
  li .grip {
    visibility: hidden;
  }
  li:hover .grip {
    visibility: visible;
  }
  .moveto form {
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
    padding: 3px 0;
  }
  .moveto input {
    flex: 1;
    min-width: 160px;
    font-size: 12px;
  }
  .moveto button {
    font-size: 12px;
    padding: 1px 8px;
  }
  .cmd {
    font-size: 12px;
    margin-top: 10px;
    overflow-wrap: anywhere;
  }
</style>
