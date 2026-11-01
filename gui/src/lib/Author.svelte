<script lang="ts">
  import { SvelteSet } from "svelte/reactivity";
  import {
    api,
    type Change,
    type Changes,
    type Checked,
    type Conflicts as Found,
    type Edited,
    type Entry,
    type ListView,
    type LoadOrder as Order,
    type ModDoc,
    type ModView,
    type Tree,
  } from "./api";
  import AddDialog from "./AddDialog.svelte";
  import ChooseDialog from "./ChooseDialog.svelte";
  import Archives from "./Archives.svelte";
  import Conflicts from "./Conflicts.svelte";
  import { startDrag, type Place } from "./drag";
  import EntryView from "./EntryView.svelte";
  import Fields from "./Fields.svelte";
  import FileTree from "./FileTree.svelte";
  import Inputs from "./Inputs.svelte";
  import LoadOrder from "./LoadOrder.svelte";
  import Notes from "./Notes.svelte";
  import Text from "./Text.svelte";

  // Author mode: the list as a tree on the left; on the right, the chosen mod
  // and its files, the load order with masters, conflicts, check, the
  // questions, and this session's changes. Every edit is one of the editing
  // commands, written at once (as the CLI writes it), shown with its command
  // and diff, and undoable. See docs/design/gui.md.
  let {
    list = $bindable(),
    active,
    locations,
  }: { list: ListView; active: boolean; locations: import("./api").Locations | null } = $props();

  type Tab = "mod" | "load" | "conflicts" | "check" | "inputs" | "changes";
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
  let actions = $state<string[]>([]);
  let order = $state<Order | null>(null);
  let found = $state<Found | null>(null);
  let conflictsError = $state<string | null>(null);
  let checked = $state<Checked | null>(null);
  let checking = $state(false);
  let busy = $state(false);

  // The last edit, and whether there is one to undo or redo.
  let last = $state<Change | null>(null);
  let lastOpen = $state(false);
  let canUndo = $state(false);
  let canRedo = $state(false);
  let changes = $state<Changes | null>(null);
  let editing = $state(false);
  let scroller = $state<HTMLElement | null>(null);

  // The left pane: the list, or the archives to add from; and the archive
  // being added.
  let pane = $state<"list" | "archives">("list");
  let adding = $state<string | null>(null);
  // The archive whose installer is being walked again.
  let choosing = $state<{ id: string; number: number; label: string } | null>(null);
  // A mod's right-click menu, and whether its removal is being confirmed.
  let modMenu = $state<{ x: number; y: number; id: string; title: string; enabled: boolean } | null>(null);
  let confirmRemove = $state(false);
  // The "+" menu by the search box.
  let addMenu = $state(false);

  async function removeMod(id: string) {
    modMenu = null;
    if (await run(() => api.removeMod(id))) {
      if (selected === id) {
        selected = null;
        view = null;
        tree = null;
      }
      await archivesPane?.refresh();
    }
  }
  let archivesPane = $state<Archives | null>(null);

  // Every mod in the document by id, the sections above it (by anchor, to
  // open them) and its section path (for moving it), in list order.
  const mods = $derived.by(() => {
    const out = new Map<string, { mod: ModDoc; path: string[]; section: string[] }>();
    const walk = (entries: Entry[], path: string[], section: string[]) => {
      for (const e of entries) {
        if (e.kind === "mod") out.set(e.id, { mod: e, path, section });
        else if (e.kind === "section") walk(e.entries, [...path, e.anchor], e.path);
      }
    };
    walk(list.document.entries, [], []);
    return out;
  });
  const sections = $derived.by(() => {
    const out = new Map<string, string[]>();
    const walk = (entries: Entry[]) => {
      for (const e of entries)
        if (e.kind === "section") {
          out.set(e.anchor, e.path);
          walk(e.entries);
        }
    };
    walk(list.document.entries);
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
    await readMod(id, 1);
  }

  async function readMod(id: string, number: number) {
    try {
      view = await api.modView(id);
      actions = await api.actions(id);
      if (view.archives.length) await readTree(id, Math.min(number, view.archives.length));
    } catch (err) {
      error = String(err);
    }
  }

  async function readTree(id: string, number: number, quietly = false) {
    archive = number;
    if (!quietly) tree = null;
    treeError = null;
    loadingTree = !quietly;
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
      if (next === "changes") changes = await api.changes();
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

  // Everything shown, read again: after an edit, or when the list may have
  // changed outside. What has to be asked for again (conflicts) is dropped.
  async function refresh() {
    list = await api.reloadList();
    if (order) order = await api.loadOrder();
    if (tab === "changes") changes = await api.changes();
    if (selected && mods.has(selected)) {
      view = await api.modView(selected);
      actions = await api.actions(selected);
      if (view.archives.length) await readTree(selected, Math.min(archive, view.archives.length), true);
    }
  }

  async function reload() {
    error = null;
    found = null;
    try {
      await refresh();
      if (tab === "conflicts") await show("conflicts");
    } catch (err) {
      error = String(err);
    }
  }

  // Run an edit, show what it did, and read everything again.
  async function run(edit: () => Promise<Edited | unknown>): Promise<boolean> {
    if (editing) return false;
    editing = true;
    error = null;
    try {
      const done = (await edit()) as Edited;
      last = done.change;
      canUndo = done.can_undo;
      canRedo = done.can_redo;
      await refresh();
      return true;
    } catch (err) {
      error = String(err);
      return false;
    } finally {
      editing = false;
    }
  }

  const undo = () => canUndo && run(api.undo);
  const redo = () => canRedo && run(api.redo);

  // Ctrl+Z and Ctrl+Shift+Z (or Ctrl+Y), in author mode, unless typing.
  function onkeydown(event: KeyboardEvent) {
    const target = event.target as HTMLElement;
    if (!active || target.closest("input, textarea, select") || !(event.ctrlKey || event.metaKey)) return;
    const key = event.key.toLowerCase();
    if (key === "z" && !event.shiftKey) {
      event.preventDefault();
      undo();
    } else if ((key === "z" && event.shiftKey) || key === "y") {
      event.preventDefault();
      redo();
    }
  }

  // Moving a mod: before or after another (in that mod's section), or into a
  // section, at its end (`move`).
  function acceptsMod(key: string, place: Place) {
    return key.startsWith("sec:") ? place === "into" : place !== "into";
  }
  function dropMod(id: string, key: string, place: Place) {
    if (key.startsWith("sec:")) {
      const section = sections.get(key.slice(4));
      if (section) run(() => api.moveMod(id, section, null));
      return;
    }
    const target = key.slice(4);
    const at = mods.get(target);
    if (!at || target === id) return;
    let before: string | null = target;
    if (place === "after") {
      // The next mod in the same section, or its end.
      const ids = [...mods.keys()];
      const next = ids.slice(ids.indexOf(target) + 1).find((m) => m !== id);
      before = next && mods.get(next)!.section.join("\u0000") === at.section.join("\u0000") ? next : null;
    }
    if (before === id) return;
    run(() => api.moveMod(id, at.section, before));
  }

  const chosen = $derived(selected ? mods.get(selected)?.mod : undefined);

  // The folders changed (in the Folders dialog): what is read from them --
  // the archives, a mod's files, conflicts -- is read again.
  let seenFolders = "";
  $effect(() => {
    const key = JSON.stringify(locations);
    if (!locations || key === seenFolders) return;
    const first = !seenFolders;
    seenFolders = key;
    if (first) return;
    found = null;
    archivesPane?.refresh();
    if (selected && view?.archives.length) readTree(selected, archive, true);
  });

  // Add the mod an archive makes, then show it.
  async function addMod(request: import("./api").AddRequest): Promise<boolean> {
    const ok = await run(() => api.addMod(request));
    if (ok) {
      await archivesPane?.refresh();
      await select(request.id);
    }
    return ok;
  }
</script>

<svelte:window
  {onkeydown}
  onclick={() => {
    modMenu = null;
    addMenu = false;
  }}
/>

{#if modMenu}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="menu" style="left: {modMenu.x}px; top: {modMenu.y}px" role="menu" tabindex="-1" onclick={(e) => e.stopPropagation()}>
    <div class="mtitle">{modMenu.title}</div>
    {#if confirmRemove}
      <div class="confirm">
        Remove it from the list, with its plugins and notes?
        <div>
          <button class="danger" onclick={() => removeMod(modMenu!.id)}>Remove</button>
          <button onclick={() => (confirmRemove = false)}>Keep it</button>
        </div>
      </div>
    {:else}
      <button
        role="menuitem"
        onclick={() => {
          const { id, enabled } = modMenu!;
          modMenu = null;
          run(() => api.setEnabled(id, !enabled));
        }}
        title={modMenu.enabled
          ? "Installed, but left unticked in MO2; its plugins leave the load order (mudcrab disable)"
          : "Ticked in MO2 again; its plugins go back in the load order (mudcrab enable)"}
        >{modMenu.enabled ? "Untick it in MO2" : "Tick it in MO2"}</button
      >
      <button role="menuitem" class="danger" onclick={() => (confirmRemove = true)} title="mudcrab remove">Remove from the list…</button>
    {/if}
  </div>
{/if}

{#if choosing}
  <ChooseDialog
    id={choosing.id}
    number={choosing.number}
    label={choosing.label}
    write={(answers) => run(() => api.chooseWrite(choosing!.id, choosing!.number, answers))}
    close={() => (choosing = null)}
  />
{/if}

{#if adding}
  <AddDialog
    path={adding}
    document={list.document}
    section={selected ? (mods.get(selected)?.section ?? []) : []}
    add={addMod}
    close={() => (adding = null)}
  />
{/if}

<div class="author">
  <section class="left" bind:this={scroller}>
    <nav class="panes">
      <button class:on={pane === "list"} onclick={() => (pane = "list")}>The list</button>
      <button class:on={pane === "archives"} onclick={() => (pane = "archives")} title="The archives in the downloads folders, to add mods from">Archives</button>
    </nav>
    <div class:hidden={pane !== "archives"}>
      <Archives bind:this={archivesPane} add={(path) => (adding = path)} {select} />
    </div>
    <div class="bar" class:hidden={pane !== "list"}>
      <input type="text" placeholder="Find a mod or plugin" bind:value={filter} />
      <div class="addwrap">
        <button
          class="plus"
          onclick={(e) => {
            e.stopPropagation();
            addMenu = !addMenu;
          }}
          title="Add a mod to the list">+</button
        >
        {#if addMenu}
          <div class="menu right" role="menu">
            <button
              role="menuitem"
              onclick={() => {
                addMenu = false;
                pane = "archives";
              }}>From an archive in the downloads folders…</button
            >
            <button
              role="menuitem"
              onclick={async () => {
                addMenu = false;
                pane = "archives";
                await Promise.resolve();
                archivesPane?.focusLink();
              }}>From a Nexus link…</button
            >
          </div>
        {/if}
      </div>
    </div>
    <div class="list" class:hidden={pane !== "list"}>
      {#snippet entries(items: Entry[], depth: number)}
        {#each items.filter((e) => matches(e, words)) as e (e.kind + e.anchor)}
          {#if e.kind === "section"}
            {@const open = opened.has(e.anchor) || !!words}
            <button
              class="section"
              style="--depth: {depth}"
              data-drop={"sec:" + e.anchor}
              data-drop-into
              onclick={() => (opened.has(e.anchor) ? opened.delete(e.anchor) : opened.add(e.anchor))}
            >
              <span class="twisty">{open ? "▾" : "▸"}</span>
              <span class="title">{e.title}</span>
              <span class="dim count">{count(e)}</span>
            </button>
            {#if open}{@render entries(e.entries, depth + 1)}{/if}
          {:else if e.kind === "mod"}
            <div
              class="mod"
              class:on={selected === e.id}
              class:off={e.enabled === false}
              style="--depth: {depth}"
              data-drop={"mod:" + e.id}
              role="listitem"
              oncontextmenu={(ev) => {
                ev.preventDefault();
                ev.stopPropagation();
                confirmRemove = false;
                modMenu = { x: ev.clientX, y: ev.clientY, id: e.id, title: e.title, enabled: e.enabled !== false };
              }}
            >
              <span
                class="grip"
                role="button"
                tabindex="-1"
                title="Drag to move it: onto a mod to put it beside that one, onto a section to put it at its end"
                onpointerdown={(ev) =>
                  startDrag(ev, {
                    scroller: scroller!,
                    accepts: acceptsMod,
                    onDrop: (key, place) => dropMod(e.id, key, place),
                  })}>⠿</span
              >
              <button class="pick" onclick={() => select(e.id)}>
                <span class="title">{e.title}</span>
                {#if e.condition}<span class="tag cond" title={e.condition.text}>if</span>{/if}
                {#if e.enabled === false}<span class="tag">off</span>{/if}
                {#if e.mod_type}<span class="tag">{e.mod_type}</span>{/if}
                {#if e.plugins.length}<span class="dim count">{e.plugins.length}p</span>{/if}
              </button>
            </div>
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
        <button class:on={tab === "inputs"} onclick={() => show("inputs")}>Questions</button>
        <button class:on={tab === "changes"} onclick={() => show("changes")}>Changes</button>
      </nav>
      <span class="edits">
        <button onclick={undo} disabled={!canUndo || editing} title="Undo the last edit (Ctrl+Z)">↶ Undo</button>
        <button onclick={redo} disabled={!canRedo || editing} title="Redo (Ctrl+Shift+Z)">↷ Redo</button>
        <button onclick={reload} title="Read the list again, if it has changed on disk">Reload</button>
      </span>
    </div>

    <div class="content">
      {#if error}<pre class="error">{error}</pre>{/if}

      {#if tab === "mod"}
        {#if !chosen}
          <p class="dim">
            Choose a mod on the left to see what it installs and change it. Drag a mod by its handle to move it.
            Every change is written to the list at once, as its command would, and can be undone.
          </p>
        {:else}
          <EntryView entry={{ kind: "mod", ...chosen }} words="" matches={() => true} prose={false} />
          {#if view?.used}
            <p class="dim small">
              Defined by the shim <code>{view.used}</code>: what the entry here sets is written over it.
            </p>
          {/if}
          {#if view?.origin}
            <p class="dim small">
              Written in <code>{view.origin}</code>, which the list includes: changes here are made there.
              {#if view.overridden.length}
                The include overrides its <code>{view.overridden.join(", ")}</code>, which are changed in the include
                instead.
              {/if}
            </p>
          {/if}
          <Notes id={chosen.id} docs={chosen.docs} save={(fields) => run(() => api.setDocs(chosen.id, fields))} />
          <details class="form">
            <summary>The mod's fields</summary>
            <Fields id={chosen.id} archive={null} save={(sets, unsets) => run(() => api.setFields(chosen.id, null, sets, unsets))} />
          </details>
          {#if actions.length}
            <h4>Actions</h4>
            <ol class="actions">
              {#each actions as action, i}
                <li>
                  <code>{action}</code>
                  <button class="link danger" onclick={() => run(() => api.removeAction(chosen.id, i + 1))}>Remove</button>
                </li>
              {/each}
            </ol>
          {/if}
          {#if view?.left_out}
            <p class="dim">The answers given in Install leave this mod out, so its files are not planned.</p>
          {:else if view && !view.archives.length}
            <p class="dim">No archives: this mod is made at install.</p>
          {:else if view}
            {#if view.archives.length > 1}
              <nav class="archives">
                {#each view.archives as a}
                  <button class:on={archive === a.number} onclick={() => readTree(view!.id, a.number)}
                    >{a.number}. {a.label}{#if a.condition}<span class="dim"> (if {a.condition})</span>{/if}</button
                  >
                {/each}
              </nav>
            {/if}
            {@const shownArchive = view.archives.find((a) => a.number === archive)}
            {@const fomod = tree?.layout === "fomod" || !!tree?.rows.some((r) => r.path.toLowerCase().endsWith("fomod/moduleconfig.xml"))}
            {#if tree && (fomod || tree.layout === "bain")}
              <button
                class="chooser"
                onclick={() => (choosing = { id: chosen.id, number: archive, label: shownArchive?.label ?? "" })}
                title="Walk its installer again, starting at what the list chooses now (mudcrab choose)"
                >Choose its {fomod ? "FOMOD options" : "BAIN packages"}…</button
              >
            {/if}
            <details class="form">
              <summary>Archive {archive}'s fields</summary>
              <Fields id={chosen.id} {archive} save={(sets, unsets) => run(() => api.setFields(chosen.id, archive, sets, unsets))} />
            </details>
            {#if loadingTree}
              <p class="dim">Reading the archive…</p>
            {:else if treeError}
              <pre class="error">{treeError}</pre>
            {:else if tree}
              <FileTree
                {tree}
                edit={(key, add, drop) => run(() => api.editFilter(chosen.id, archive, key, add, drop))}
                problem={(message) => (error = message)}
                move={(from, to) => run(() => api.addAction(chosen.id, "file_move", [`from=${JSON.stringify(from)}`, `to=${JSON.stringify(to)}`], null))}
              />
            {/if}
          {/if}
        {/if}
      {:else if tab === "load"}
        {#if order}
          <LoadOrder {order} {select} {runCheck} {checking} move={(plugin, before) => run(() => api.movePlugin(plugin, before))} />
        {:else}<p class="dim">Reading…</p>{/if}
      {:else if tab === "conflicts"}
        {#if busy}
          <p class="dim">Comparing every installed mod's files…</p>
        {:else if conflictsError}
          <pre class="error">{conflictsError}</pre>
        {:else if found}
          <Conflicts
            {found}
            {select}
            focus={selected}
            hide={(id, other) => run(() => api.addAction(id, "file_hide", [`conflicts_with=[${JSON.stringify(other)}]`], null))}
          />
        {/if}
      {:else if tab === "check"}
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
      {:else if tab === "inputs"}
        <Inputs document={list.document} {select} {run} />
      {:else if changes}
        {#if !changes.edits.length}
          <p class="dim">No changes yet in this session.</p>
        {:else}
          <p class="dim small">
            {changes.edits.length} edit(s) since the list was opened here. The commands, run in order on another copy of the
            list, make the same changes.
          </p>
          <pre class="script">{changes.edits.map((e) => e.command).join("\n")}</pre>
          <button onclick={() => navigator.clipboard.writeText(changes!.edits.map((e) => e.command).join("\n"))}>Copy the commands</button>
          <h4>The session as one diff</h4>
          <pre class="diff">{#each changes.diff.split("\n") as line}<span class:add={line.startsWith("+")} class:del={line.startsWith("-")}>{line}
</span>{/each}</pre>
        {/if}
      {/if}
    </div>

    {#if last}
      <div class="last">
        <button class="link" onclick={() => (lastOpen = !lastOpen)} title="Show the change it made">
          {lastOpen ? "▾" : "▸"} <code>{last.command}</code>
        </button>
        {#if lastOpen}
          <pre class="diff">{#each last.diff.split("\n") as line}<span class:add={line.startsWith("+")} class:del={line.startsWith("-")}>{line}
</span>{/each}</pre>
        {/if}
      </div>
    {/if}
  </aside>
</div>

<style>
  .author {
    display: grid;
    grid-template-columns: minmax(260px, 380px) minmax(0, 1fr);
    flex: 1;
    min-height: 0;
  }
  .left {
    overflow: auto;
    min-height: 0;
  }
  .right {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .content {
    flex: 1;
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
    padding-left: calc(10px + var(--depth) * 14px);
  }
  .mod .grip {
    width: 12px;
    flex: none;
    visibility: hidden;
  }
  .mod:hover .grip {
    visibility: visible;
  }
  .pick {
    display: flex;
    align-items: baseline;
    gap: 6px;
    flex: 1;
    min-width: 0;
    text-align: left;
    background: none;
    border: none;
    padding: 0;
    color: inherit;
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
  .addwrap {
    position: relative;
  }
  .plus {
    font-weight: 700;
    padding: 2px 10px;
  }
  .menu {
    position: fixed;
    z-index: 60;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
    padding: 4px;
    min-width: 200px;
    font-size: 13px;
  }
  .menu.right {
    position: absolute;
    right: 0;
    top: calc(100% + 4px);
    white-space: nowrap;
  }
  .menu button {
    display: block;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
  }
  .menu button:hover {
    background: var(--accent-soft);
  }
  .menu .danger {
    color: var(--problem);
  }
  .mtitle {
    font-weight: 600;
    padding: 2px 12px 4px;
    overflow-wrap: anywhere;
  }
  .confirm {
    padding: 4px 12px;
    max-width: 260px;
  }
  .confirm div {
    display: flex;
    gap: 6px;
    margin-top: 6px;
  }
  .confirm button {
    width: auto;
    border: 1px solid var(--line);
  }
  .mod.off .title {
    color: var(--dim);
    font-style: italic;
  }
  .panes {
    display: flex;
    gap: 4px;
    padding: 6px 10px 0;
    background: var(--bg);
  }
  .panes button {
    background: none;
    border-color: transparent;
    color: var(--dim);
    font-size: 13px;
  }
  .panes button.on {
    color: var(--text);
    background: var(--card);
    border-color: var(--line);
  }
  .hidden {
    display: none;
  }
  .edits {
    margin-left: auto;
    display: flex;
    gap: 4px;
  }
  .edits button {
    font-size: 12px;
    padding: 3px 8px;
  }
  .chooser {
    font-size: 12px;
    margin: 6px 0;
  }
  details.form {
    margin: 8px 0;
  }
  details.form summary {
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    margin-bottom: 4px;
  }
  h4 {
    font-size: 13px;
    margin: 12px 0 4px;
  }
  .actions {
    margin: 0;
    padding-left: 22px;
    font-size: 12px;
  }
  .actions li {
    margin: 2px 0;
  }
  .danger {
    color: var(--problem);
    font-size: 12px;
    margin-left: 6px;
  }
  .script,
  .diff {
    background: var(--card);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 8px 10px;
    overflow: auto;
    white-space: pre;
    max-height: 50vh;
  }
  .diff .add {
    color: var(--good);
  }
  .diff .del {
    color: var(--problem);
  }
  .last {
    border-top: 1px solid var(--line);
    background: var(--panel);
    padding: 6px 12px;
    font-size: 12px;
    max-height: 40vh;
    overflow: auto;
  }
  .last .link {
    color: var(--text);
    text-align: left;
  }
  .last code {
    overflow-wrap: anywhere;
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
