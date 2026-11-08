<script lang="ts" module>
  import type { ChoiceSetView, ChoiceTarget, Conditional, Tree } from "./api";
  import IfTag from "./IfTag.svelte";
  /** One of the mod's archives, and its tree once read. */
  export type Archive = {
    number: number;
    label: string;
    condition: string | null;
    /** Whether its `if` holds in Author mode's profile. */
    applies?: boolean | null;
    /** Its FOMOD rows and BAIN packages as written, with their `if`s. */
    choices?: Conditional[];
    /** Its sets of choices, its own, and the one installed. */
    sets?: ChoiceSetView[];
    own?: ChoiceSetView;
    installed?: number | null;
    tree: Tree | null;
    error: string | null;
  };
</script>

<script lang="ts">
  import { tick as rendered } from "svelte";
  import ChoiceSets from "./ChoiceSets.svelte";
  import { api } from "./api";
  import { SvelteMap, SvelteSet } from "svelte/reactivity";
  import { startDrag } from "./drag";
  import type { InputDoc } from "./api";
  import ArchiveFields from "./ArchiveFields.svelte";

  // A mod's files, from all its archives, as one tree of the mod's folder as
  // installed. Each file is ticked when install takes it: untick it to leave
  // it out (an `exclude` on its archive, `files --exclude`), tick it to bring
  // it back. Drag a file or folder onto a folder (or the top of the mod), or
  // give it a new path with "move…", to move it (`file_move`). A file left
  // out stays, greyed, where it would go.
  //
  // Above it, each archive: what its installer chooses (BAIN packages, FOMOD
  // options), its filters, and its fields.
  let {
    id,
    archives,
    edit,
    move,
    problem,
    choose,
    inputs,
    run,
  }: {
    id: string;
    archives: Archive[];
    /** Add and drop values of an archive's `include`, `exclude` or
     *  `bain_subpackages`; resolves true when written. */
    edit: (number: number, key: string, add: string[], drop: string[]) => Promise<boolean>;
    /** Move `from` to `to`, both in the mod's folder (`file_move`);
     *  resolves true when written. */
    move: (from: string, to: string) => Promise<boolean>;
    problem: (message: string) => void;
    /** Walk an archive's installer again. */
    /** Walk an archive's FOMOD installer for one of its sets of choices. */
    choose: (number: number, label: string, to: ChoiceTarget, title: string) => void;
    /** The list's questions, for an archive's `if`. */
    inputs: InputDoc[];
    /** Run an edit; resolves true when written. */
    run: (edit: () => Promise<unknown>) => Promise<boolean>;
  } = $props();

  const lower = (list: string[]) => list.map((v) => v.toLowerCase());
  // The archive whose removal is being confirmed.
  let removing = $state<number | null>(null);
  // As globset escapes a path: each metacharacter in brackets.
  const escape = (path: string) => path.replace(/[*?[\]{}]/g, (c) => `[${c}]`);

  type File = {
    path: string;
    laid_out: string;
    hidden: boolean;
    source: string;
    taken: boolean;
    filtered_as: string;
    archive: Archive;
  };
  // A tick or move shows at once, before it is written and the archives
  // read again; what they then say replaces it.
  const ticked = new SvelteMap<string, boolean>();
  let moved = $state<[string, string][]>([]);
  $effect(() => {
    void archives;
    ticked.clear();
    moved = [];
  });
  function movedTo(path: string): string {
    for (const [from, to] of moved) {
      if (path.toLowerCase() === from.toLowerCase()) path = to;
      else if (path.toLowerCase().startsWith(`${from.toLowerCase()}/`)) path = `${to}${path.slice(from.length)}`;
    }
    return path;
  }
  const files = $derived.by<File[]>(
    () => {
      // One file per path: where two archives put the same one -- the same
      // archive twice, or two that overlap -- the later's is the one the
      // mod's folder ends up with.
      const byPath = new Map<string, File>();
      for (const a of archives)
        for (const f of a.tree?.installed ?? []) {
          const file = { ...f, path: movedTo(f.path), taken: ticked.get(f.path) ?? f.taken, archive: a };
          byPath.set(file.path.toLowerCase(), file);
        }
      return [...byPath.values()].sort((x, y) => x.path.localeCompare(y.path));
    },
  );
  // Shown at once; put back if the edit is not written.
  async function showing(paths: string[], taken: boolean, write: () => Promise<boolean>) {
    for (const path of paths) ticked.set(path, taken);
    if (!(await write())) for (const path of paths) ticked.delete(path);
  }
  const several = $derived(archives.length > 1);

  type Row = {
    path: string;
    name: string;
    depth: number;
    folder: boolean;
    moved: boolean;
    hidden: boolean;
    files: number;
    taken: number;
    file?: File;
  };

  // Folders before files, each folder's contents after it.
  const all = $derived.by(() => {
    const folders = new Map<string, { moved: boolean; files: number; taken: number }>();
    for (const file of files) {
      const parts = file.path.split("/");
      for (let i = 1; i < parts.length; i++) {
        const folder = parts.slice(0, i).join("/");
        const known = folders.get(folder) ?? { moved: false, files: 0, taken: 0 };
        known.files += 1;
        known.taken += Number(file.taken);
        known.moved ||= file.path !== file.laid_out;
        folders.set(folder, known);
      }
    }
    const out: Row[] = [];
    const walk = (under: string, depth: number) => {
      const prefix = under ? `${under}/` : "";
      const inside = (p: string) => p.startsWith(prefix) && !p.slice(prefix.length).includes("/");
      for (const [folder, about] of [...folders].filter(([f]) => inside(f)).sort(([a], [b]) => a.localeCompare(b))) {
        out.push({ path: folder, name: folder.slice(prefix.length), depth, folder: true, hidden: false, ...about });
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
          files: 1,
          taken: Number(file.taken),
          file,
        });
    };
    walk("", 0);
    return out;
  });

  // Folders start closed, except in a small mod, and stay as they were
  // across an edit.
  const open = new SvelteSet<string>();
  let shownFor = "";
  $effect(() => {
    // Once the mod's files are first read.
    if (id === shownFor || !files.length) return;
    shownFor = id;
    open.clear();
    if (files.length <= 60) for (const row of all) if (row.folder) open.add(row.path);
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

  // --- Ticks ------------------------------------------------------------------------

  // The exclude that leaves out a folder's files from one archive: the part
  // of their filtered paths above the folder, the same for all of them, or
  // null when they were moved in from different places.
  function folderGlob(folder: string, inside: File[]): string | null {
    let head: string | null = null;
    for (const file of inside) {
      const rel = file.path.slice(folder.length + 1).toLowerCase();
      const as = file.filtered_as;
      if (!as.toLowerCase().endsWith(`/${rel}`)) return null;
      const h = as.slice(0, as.length - rel.length - 1);
      if (head !== null && head.toLowerCase() !== h.toLowerCase()) return null;
      head = h;
    }
    return head ? `${escape(head)}/**` : null;
  }

  // An archive's own exclude matching `glob`, as written.
  const ownExclude = (archive: Archive, glob: string) => archive.tree?.exclude.find((g) => g.toLowerCase() === glob.toLowerCase());

  function widerFilter(path: string, archive: Archive) {
    const tree = archive.tree!;
    problem(
      `${path} is left out by a wider filter on ${archive.label} (${[
        ...tree.include.map((g) => `include ${g}`),
        ...tree.exclude.map((g) => `exclude ${g}`),
      ].join(", ")}); change that in its filters above.`,
    );
  }

  async function tick(row: Row) {
    if (row.file) {
      const file = row.file;
      const glob = escape(file.filtered_as);
      if (file.taken) return showing([file.path], false, () => edit(file.archive.number, "exclude", [glob], []));
      const own = ownExclude(file.archive, glob);
      return own ? showing([file.path], true, () => edit(file.archive.number, "exclude", [], [own])) : widerFilter(row.path, file.archive);
    }
    // A folder: per archive whose files are in it, one edit after another.
    const inside = files.filter((f) => f.path.startsWith(`${row.path}/`));
    const leaving = row.taken > 0;
    const edits: [number, string[], string[]][] = [];
    for (const archive of archives) {
      const theirs = inside.filter((f) => f.archive === archive);
      if (!theirs.length) continue;
      if (leaving) {
        const glob = folderGlob(row.path, theirs);
        if (!glob) return problem(`The files in ${row.path} were moved there from different places: untick them one by one.`);
        edits.push([archive.number, [glob], []]);
      } else {
        const glob = folderGlob(row.path, theirs);
        const drop = [
          ...new Set(
            [glob, ...theirs.map((f) => escape(f.filtered_as))]
              .filter((g): g is string => !!g)
              .map((g) => ownExclude(archive, g))
              .filter((g): g is string => !!g),
          ),
        ];
        if (!drop.length) return widerFilter(row.path, archive);
        edits.push([archive.number, [], drop]);
      }
    }
    await showing(
      inside.map((f) => f.path),
      !leaving,
      async () => {
        for (const [number, add, drop] of edits) if (!(await edit(number, "exclude", add, drop))) return false;
        return true;
      },
    );
  }

  // A click toggles the box itself; once what it does is shown (or refused,
  // or nothing to do), the box is put to what the tree says. Cancelling the
  // click instead would have the browser put the old tick back after
  // the new one was shown.
  async function settle(box: HTMLInputElement, work: unknown, state: () => { checked: boolean; indeterminate: boolean } | null) {
    await work;
    await rendered();
    const now = state();
    if (!now) return;
    box.checked = now.checked;
    box.indeterminate = now.indeterminate;
  }
  function rowState(path: string) {
    const row = all.find((r) => r.path === path);
    return row ? { checked: row.taken === row.files, indeterminate: row.taken > 0 && row.taken < row.files } : null;
  }

  function why(row: Row): string {
    if (row.folder) return `${row.taken} of ${row.files} file(s) installed`;
    const f = row.file!;
    const from = several ? `${f.archive.label}: ${f.source}` : f.source;
    if (!f.taken) return `${from}\nLeft out by its archive's include or exclude (filters see it as ${f.filtered_as})`;
    return f.path !== f.laid_out ? `${from}\nMoved here from ${f.laid_out}` : from;
  }

  // --- Moves ------------------------------------------------------------------------

  let list = $state<HTMLElement | null>(null);
  const base = (path: string) => path.slice(path.lastIndexOf("/") + 1);

  // A drop into a folder (or the mod's top, keyed ""): it keeps its name.
  function dropped(row: Row, key: string) {
    const to = key ? `${key}/${base(row.path)}` : base(row.path);
    if (to.toLowerCase() === row.path.toLowerCase()) return;
    if (`${to.toLowerCase()}/`.startsWith(`${row.path.toLowerCase()}/`)) return problem("A folder cannot go inside itself.");
    if (files.some((f) => f.path.toLowerCase() === to.toLowerCase())) return problem(`The mod already has ${to}.`);
    moveShown(row.path, to);
  }
  async function moveShown(from: string, to: string) {
    moved = [...moved, [from, to]];
    if (!(await move(from, to))) moved = moved.filter(([f, t]) => f !== from || t !== to);
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
    if (to && to !== row.path) moveShown(row.path, to);
    moving = null;
  }

  // --- Each archive -----------------------------------------------------------------

  const fomodOf = (tree: Tree) => tree.layout === "fomod" || tree.rows.some((r) => r.path.toLowerCase().endsWith("fomod/moduleconfig.xml"));
  const packagesOf = (tree: Tree) => (tree.layout === "bain" ? tree.rows.filter((r) => r.package) : []);

  function packageTick(archive: Archive, name: string) {
    const tree = archive.tree!;
    if (lower(tree.bain_conditional).includes(name.toLowerCase()))
      return problem(`${name} is chosen under a condition; change that in the list's file.`);
    const chosen = lower(tree.bain_subpackages).includes(name.toLowerCase());
    return edit(archive.number, "bain_subpackages", chosen ? [] : [name], chosen ? [name] : []);
  }

  let newGlob = $state<Record<number, string>>({});
  let newKey = $state<Record<number, "exclude" | "include">>({});
  function addGlob(event: SubmitEvent, archive: Archive) {
    event.preventDefault();
    const glob = (newGlob[archive.number] ?? "").trim();
    if (!glob) return;
    edit(archive.number, newKey[archive.number] ?? "exclude", [glob], []);
    newGlob[archive.number] = "";
  }
</script>

{#each archives as archive (archive.number)}
  <div class="archive">
    <div class="ahead">
      <strong>{several ? `${archive.number}. ` : ""}{archive.label}</strong>
      {#if archive.condition}<IfTag condition={archive.condition} applies={archive.applies} />{/if}
      {#if archive.tree}<span class="tag" title="Its layout">{archive.tree.layout ?? "automatic"}</span>{/if}
      <span class="grow"></span>
      {#if removing === archive.number}
        <span class="small">Remove this archive, and its choices?</span>
        <button class="small danger" onclick={() => run(() => api.removeArchive(id, archive.number)).then(() => (removing = null))}
          >Remove</button
        >
        <button class="small" onclick={() => (removing = null)}>Keep it</button>
      {:else}
        <button class="link small danger" onclick={() => (removing = archive.number)} title="Take this archive out of the mod">Remove</button>
      {/if}
    </div>
    {#if archive.error}
      <pre class="error">{archive.error}</pre>
    {:else if !archive.tree}
      <p class="dim small">Reading the archive…</p>
    {:else}
      {@const tree = archive.tree}
      {#if archive.own && (fomodOf(tree) || packagesOf(tree).length)}
        <ChoiceSets
          {id}
          number={archive.number}
          fomod={fomodOf(tree)}
          packages={packagesOf(tree).map((p) => p.name)}
          sets={archive.sets ?? []}
          own={archive.own}
          installed={archive.installed ?? null}
          {inputs}
          {run}
          choose={(to, title) => choose(archive.number, archive.label, to, title)}
        />
      {:else if packagesOf(tree).length}
        <div class="packages">
          <span class="dim">Packages:</span>
          {#each packagesOf(tree) as pkg}
            <label title={lower(tree.bain_conditional).includes(pkg.name.toLowerCase()) ? "Chosen under a condition, in the list's file" : ""}>
              <input
                type="checkbox"
                checked={lower(tree.bain_subpackages).includes(pkg.name.toLowerCase())}
                disabled={lower(tree.bain_conditional).includes(pkg.name.toLowerCase())}
                onclick={(e) =>
                  settle(e.currentTarget, packageTick(archive, pkg.name), () => {
                    const now = archives.find((a) => a.number === archive.number)?.tree;
                    return now ? { checked: lower(now.bain_subpackages).includes(pkg.name.toLowerCase()), indeterminate: false } : null;
                  })}
              />
              {pkg.name}
            </label>
          {/each}
        </div>
      {/if}
      <div class="filters">
        {#each [{ key: "include", list: tree.include }, { key: "exclude", list: tree.exclude }] as filter}
          {#each filter.list as glob}
            {@const matched = tree.matches.find((m) => m.key === filter.key && m.glob === glob)?.files}
            <span
              class="glob {filter.key}"
              class:idle={matched === 0}
              title={matched === 0
                ? `Matches nothing: filters match each file by its path in the mod as the archive lays it out (hover a file to see it), and case counts`
                : `${filter.key === "exclude" ? "Leaves out" : "Keeps"} ${matched} file(s)`}
              >{filter.key} <code>{glob}</code>
              <span class="n">{matched === 0 ? "matches nothing" : matched}</span><button class="x" title="Take it out" onclick={() => edit(archive.number, filter.key, [], [glob])}
                >✕</button
              ></span
            >
          {/each}
        {/each}
        <form onsubmit={(e) => addGlob(e, archive)}>
          <select
            value={newKey[archive.number] ?? "exclude"}
            onchange={(e) => (newKey[archive.number] = e.currentTarget.value as "exclude" | "include")}
            ><option>exclude</option><option>include</option></select
          >
          <input type="text" placeholder="a glob, e.g. docs/**" bind:value={newGlob[archive.number]} />
          <button type="submit" disabled={!(newGlob[archive.number] ?? "").trim()}>Add</button>
        </form>
      </div>
    {/if}
    <details class="form">
      <summary>Its fields</summary>
      <ArchiveFields {id} number={archive.number} label={archive.label} {inputs} {run} />
    </details>
  </div>
{/each}

<h4>Files</h4>
<p class="legend dim">
  Untick to leave a file out, tick to bring it back; drag one onto a folder to move it.
  {#if several}Hover a file for the archive it comes from.{/if}
  Files of packages or options not chosen are not listed.
</p>
<div class="top dim" data-drop="" data-drop-into="only">The top of the mod: drop here to move something out of its folder</div>
<ul bind:this={list}>
  {#each rows as row (row.path)}
    <li
      class:out={row.taken === 0}
      class:moved={row.moved}
      class:hidden={row.hidden}
      style="--depth: {row.depth}"
      data-drop={row.folder ? row.path : undefined}
      data-drop-into={row.folder ? "only" : undefined}
      title={why(row)}
    >
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
      <input
        type="checkbox"
        checked={row.taken === row.files}
        indeterminate={row.taken > 0 && row.taken < row.files}
        onclick={(e) => settle(e.currentTarget, tick(row), () => rowState(row.path))}
      />
      {#if row.folder}
        <button class="name folder" onclick={() => toggle(row.path)}>
          <span class="twisty">{open.has(row.path) ? "▾" : "▸"}</span>{row.name}/
        </button>
        <span class="count dim">{row.taken}/{row.files}</span>
      {:else}
        <span class="name">{row.name}</span>
        {#if several}<span class="from dim">{row.file!.archive.number}</span>{/if}
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
{#if !files.length && archives.every((a) => a.tree)}<p class="dim small">Nothing goes in the mod's folder.</p>{/if}

<style>
  .archive {
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 6px 10px;
    margin: 6px 0;
    font-size: 13px;
  }
  .ahead .grow {
    flex: 1;
  }
  .ahead {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 10px;
    align-items: baseline;
  }
  .ahead strong {
    overflow-wrap: anywhere;
  }
  .ahead button {
    margin-left: auto;
  }
  .small {
    font-size: 12px;
  }
  .packages {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 12px;
    margin: 6px 0 2px;
    font-size: 12px;
  }
  .packages label {
    display: flex;
    gap: 4px;
    align-items: baseline;
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
  .glob .n {
    font-size: 11px;
    margin-left: 2px;
  }
  .glob.idle {
    border-color: var(--warning);
  }
  .glob.idle .n {
    color: var(--warning);
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
  .form summary {
    font-size: 12px;
    color: var(--dim);
    cursor: pointer;
  }
  h4 {
    margin: 14px 0 2px;
  }
  .legend {
    font-size: 11px;
    margin: 0 0 6px;
  }
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
  input[type="checkbox"] {
    margin: 0;
    flex: none;
    align-self: center;
  }
  .grip {
    width: 10px;
    flex: none;
    visibility: hidden;
    cursor: grab;
  }
  li:hover .grip {
    visibility: visible;
  }
  .name {
    overflow-wrap: anywhere;
  }
  li.out > .name,
  li.out > .folder {
    color: var(--faint);
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
  .count,
  .from {
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
  .error {
    color: var(--problem);
    white-space: pre-wrap;
    font-family: inherit;
    font-size: 12px;
  }
</style>
