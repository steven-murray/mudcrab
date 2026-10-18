<script lang="ts">
  import { SvelteSet } from "svelte/reactivity";
  import type { InstalledFile } from "./api";
  import { startDrag } from "./drag";

  // What an archive puts in the mod's folder, where it ends up once the
  // mod's `file_move`s have run. Drag a file or folder onto a folder (or the
  // top of the mod), or give it a new path with "move…", to add a
  // `file_move`; the view then shows it in its new place.
  let {
    files,
    move,
    problem,
  }: {
    files: InstalledFile[];
    /** Move `from` to `to`, both in the mod's folder (`file_move`). */
    move: (from: string, to: string) => void;
    problem: (message: string) => void;
  } = $props();

  type Row = { path: string; name: string; depth: number; folder: boolean; moved: boolean; hidden: boolean; count: number };

  // Folders before files, each folder's contents after it.
  const all = $derived.by(() => {
    const folders = new Map<string, { moved: boolean; count: number }>();
    for (const file of files) {
      const parts = file.path.split("/");
      for (let i = 1; i < parts.length; i++) {
        const folder = parts.slice(0, i).join("/");
        const known = folders.get(folder) ?? { moved: false, count: 0 };
        known.count += 1;
        known.moved ||= file.path !== file.laid_out;
        folders.set(folder, known);
      }
    }
    const out: Row[] = [];
    const walk = (under: string, depth: number) => {
      const prefix = under ? `${under}/` : "";
      const inside = (p: string) => p.startsWith(prefix) && !p.slice(prefix.length).includes("/");
      for (const [folder, about] of [...folders].filter(([f]) => inside(f)).sort(([a], [b]) => a.localeCompare(b))) {
        out.push({ path: folder, name: folder.slice(prefix.length), depth, folder: true, moved: about.moved, hidden: false, count: about.count });
        walk(folder, depth + 1);
      }
      for (const file of files.filter((f) => inside(f.path)))
        out.push({
          path: file.path,
          name: file.path.slice(prefix.length),
          depth,
          folder: false,
          moved: file.path !== file.laid_out,
          hidden: file.hidden,
          count: 1,
        });
    };
    walk("", 0);
    return out;
  });

  const open = new SvelteSet<string>();
  let shownFor = "";
  $effect(() => {
    // Open everything in a small mod; keep what is open across a move.
    const key = files.map((f) => f.laid_out).join("\n");
    if (key === shownFor) return;
    const first = !shownFor;
    shownFor = key;
    if (first && files.length <= 60) for (const row of all) if (row.folder) open.add(row.path);
  });
  function shown(row: Row): boolean {
    let at = row.path.lastIndexOf("/");
    while (at > 0) {
      if (!open.has(row.path.slice(0, at))) return false;
      at = row.path.lastIndexOf("/", at - 1);
    }
    return true;
  }
  const rows = $derived(all.filter(shown));
  const toggle = (path: string) => (open.has(path) ? open.delete(path) : open.add(path));

  let list = $state<HTMLElement | null>(null);
  const base = (path: string) => path.slice(path.lastIndexOf("/") + 1);

  // A drop into a folder (or the mod's top, keyed ""): it keeps its name.
  function dropped(row: Row, key: string) {
    const to = key ? `${key}/${base(row.path)}` : base(row.path);
    if (to.toLowerCase() === row.path.toLowerCase()) return;
    if (`${to.toLowerCase()}/`.startsWith(`${row.path.toLowerCase()}/`)) return problem("A folder cannot go inside itself.");
    if (files.some((f) => f.path.toLowerCase() === to.toLowerCase())) return problem(`The mod already has ${to}.`);
    move(row.path, to);
  }

  // Hovering a closed folder while dragging opens it, after a moment.
  let opening: ReturnType<typeof setTimeout> | undefined;
  function hover(key: string | null) {
    clearTimeout(opening);
    if (key && !open.has(key) && all.some((r) => r.folder && r.path === key)) opening = setTimeout(() => open.add(key), 600);
  }

  // "move…": a new path typed for a row.
  let moving = $state<string | null>(null);
  let movingTo = $state("");
  function moveTyped(event: SubmitEvent, row: Row) {
    event.preventDefault();
    const to = movingTo.trim().replace(/^\/+|\/+$/g, "");
    if (to && to !== row.path) move(row.path, to);
    moving = null;
  }
</script>

<div class="top dim" data-drop="" data-drop-into="only">The top of the mod: drop here to move something out of its folder</div>
<ul bind:this={list}>
  {#each rows as row (row.path)}
    <li class:moved={row.moved} class:hidden={row.hidden} style="--depth: {row.depth}" data-drop={row.folder ? row.path : undefined} data-drop-into={row.folder ? "only" : undefined}>
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
            hover,
          })}>⠿</span
      >
      {#if row.folder}
        <button class="name folder" onclick={() => toggle(row.path)}>
          <span class="twisty">{open.has(row.path) ? "▾" : "▸"}</span>{row.name}/
        </button>
        <span class="count dim">{row.count}</span>
      {:else}
        <span class="name">{row.name}</span>
      {/if}
      {#if row.moved && !row.folder}<span class="tag" title="Moved here by a file_move">moved</span>{/if}
      {#if row.hidden}<span class="tag" title="Hidden by a file_hide: kept, but out of the game's view">hidden</span>{/if}
      <button
        class="act"
        onclick={() => {
          moving = row.path;
          movingTo = row.path;
        }}
        title="Move it within the mod, to a path you type (file_move)">move…</button
      >
    </li>
    {#if moving === row.path}
      <li class="moveto" style="--depth: {row.depth}">
        <form onsubmit={(e) => moveTyped(e, row)}>
          <span class="dim">move to</span>
          <!-- svelte-ignore a11y_autofocus -->
          <input type="text" bind:value={movingTo} autofocus />
          <button type="submit">Move</button>
          <button type="button" onclick={() => (moving = null)}>Cancel</button>
        </form>
      </li>
    {/if}
  {/each}
</ul>
{#if !files.length}<p class="dim small">This archive puts nothing in the mod's folder.</p>{/if}

<style>
  .top {
    font-size: 11px;
    padding: 3px 6px;
    border: 1px dashed var(--line);
    border-radius: 4px;
    margin-bottom: 4px;
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
  .grip {
    width: 10px;
    flex: none;
    visibility: hidden;
  }
  li:hover .grip {
    visibility: visible;
  }
  .name {
    overflow-wrap: anywhere;
  }
  li.moved > .name,
  li.moved > .folder {
    color: var(--note);
  }
  li.hidden > .name {
    color: var(--faint);
    text-decoration: line-through;
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
  .count {
    font-size: 11px;
  }
  .tag {
    font-size: 10px;
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 0 4px;
    color: var(--dim);
  }
  .act {
    font-size: 10px;
    padding: 0 5px;
    visibility: hidden;
  }
  li:hover .act {
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
  .small {
    font-size: 12px;
  }
</style>
