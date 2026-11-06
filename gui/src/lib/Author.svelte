<script lang="ts">
  import { tick } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import {
    api,
    tomlString,
    type Change,
    type Changes,
    type Checked,
    type Conflicts as Found,
    type Edited,
    type MasterDependencies,
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
  import ModFields from "./ModFields.svelte";
  import { namedMods, neededBy, optionalGroups, optionalNeeds, optionalToml } from "./optional";
  import ModFiles, { type Archive as ModArchive } from "./ModFiles.svelte";
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
  // The mod's archives, each with its files once read.
  let files = $state<ModArchive[]>([]);
  let actions = $state<string[]>([]);
  let order = $state<Order | null>(null);
  let found = $state<Found | null>(null);
  let conflictsError = $state<string | null>(null);
  let checked = $state<Checked | null>(null);
  let checking = $state(false);
  let busy = $state(false);
  // The masters, read for the load order (`check --masters-only`).
  let readingMasters = $state(false);
  let mastersError = $state<string | null>(null);
  // The mods of the plugin chosen in the load order, marked in the list.
  const marked = new SvelteSet<string>();

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
  // In its "Make optional" submenu: a new group's name, or an `if`, being typed.
  let menuAsk = $state<"group" | "if" | null>(null);
  let menuText = $state("");

  // From the menu, many mods can be made optional quickly: each pick is
  // written at once, as in the mod's Optional dialog. A group keeps the mod's
  // `if`, and whether it starts ticked when it stays in the same group; an
  // `if` of its own takes it out of its group.
  function setOptional(mod: ModDoc, group: string) {
    const ticked = !!mod.optional && (mod.optional.group ?? "") === group.trim() && mod.optional.default;
    modMenu = null;
    run(() => api.setFields(mod.id, null, [`optional=${optionalToml(group, ticked)}`], []));
  }
  function setIf(mod: ModDoc, condition: string) {
    modMenu = null;
    run(() => api.setFields(mod.id, null, [`if=${tomlString(condition)}`], mod.optional ? ["optional"] : []));
  }
  function alwaysInstall(mod: ModDoc) {
    modMenu = null;
    run(() => api.setFields(mod.id, null, [], [...(mod.condition ? ["if"] : []), ...(mod.optional ? ["optional"] : [])]));
  }
  // The "+" menu by the search box.
  let addMenu = $state(false);

  async function removeMod(id: string) {
    modMenu = null;
    if (await run(() => api.removeMod(id))) {
      if (selected === id) {
        selected = null;
        view = null;
        files = [];
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
  const allMods = $derived([...mods.values()].map((m) => m.mod));
  const groups = $derived(optionalGroups(allMods));
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
    files = [];
    await readMod(id, 1);
  }

  async function readMod(id: string, number: number) {
    try {
      view = await api.modView(id);
      actions = await api.actions(id);
      await readFiles(id);
    } catch (err) {
      error = String(err);
    }
  }

  // Every archive's files, read side by side; quietly, after an edit, the
  // old ones stay shown until the new are read.
  async function readFiles(id: string, quietly = false) {
    if (!view) return;
    const base: ModArchive[] = view.archives.map((a) => ({ number: a.number, label: a.label, condition: a.condition, tree: null, error: null }));
    if (!quietly) files = base;
    const read = await Promise.all(
      base.map(async (a) => {
        try {
          return { ...a, tree: await api.modFiles(id, a.number) };
        } catch (err) {
          return { ...a, error: String(err) };
        }
      }),
    );
    if (selected === id) files = read;
  }

  async function show(next: Tab) {
    tab = next;
    error = null;
    try {
      if (next === "load") {
        if (!order) order = await api.loadOrder();
        if (!order.masters_read) readMasters();
      }
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

  async function readMasters() {
    if (readingMasters) return;
    readingMasters = true;
    mastersError = null;
    try {
      await api.readMasters();
      order = await api.loadOrder();
    } catch (err) {
      mastersError = String(err);
    } finally {
      readingMasters = false;
    }
  }

  // Mark a plugin's mods in the list, their sections opened, the first
  // scrolled to.
  async function mark(ids: string[]) {
    marked.clear();
    for (const id of ids) {
      marked.add(id);
      for (const anchor of mods.get(id)?.path ?? []) opened.add(anchor);
    }
    if (!ids.length) return;
    await tick();
    scroller
      ?.querySelector(`[data-drop="mod:${CSS.escape(ids[0])}"]`)
      ?.scrollIntoView({ block: "nearest" });
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
  // Side by side: the list (large, for a large list) need not hold up the
  // mod shown.
  async function refresh() {
    const id = selected && mods.has(selected) ? selected : null;
    const reads: Promise<unknown>[] = [api.reloadList().then((read) => (list = read))];
    if (order)
      reads.push(
        api.loadOrder().then((read) => {
          order = read;
          // A mod added brings plugins whose masters are not read yet.
          if (tab === "load") readMasters();
        }),
      );
    if (tab === "changes") reads.push(api.changes().then((read) => (changes = read)));
    if (id)
      reads.push(
        (async () => {
          const [read, acts] = await Promise.all([api.modView(id), api.actions(id)]);
          view = read;
          actions = acts;
          await readFiles(id, true);
        })(),
      );
    await Promise.all(reads);
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

  // Each mod's plugins' masters into its `dependencies` (every mod's, or
  // one's), reading the masters first if they are not yet; what it did is
  // shown until the next.
  let masterDeps = $state<MasterDependencies | null>(null);
  let addingDeps = $state(false);
  async function addMasterDeps(id: string | null) {
    modMenu = null;
    if (editing) return;
    editing = true;
    addingDeps = true;
    error = null;
    try {
      const done = await api.addMasterDependencies(id);
      masterDeps = done;
      if (done.edited) {
        last = done.edited.change;
        canUndo = done.edited.can_undo;
        canRedo = done.edited.can_redo;
      }
      if (!order) order = await api.loadOrder();
      await refresh();
    } catch (err) {
      error = String(err);
    } finally {
      editing = false;
      addingDeps = false;
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
    if (selected && view?.archives.length) readFiles(selected, true);
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
      {@const doc = mods.get(modMenu.id)?.mod}
      {#if doc}
        {@render optionalMenu(doc)}
      {/if}
      <button
        role="menuitem"
        onclick={() => addMasterDeps(modMenu!.id)}
        title="List the mods its plugins' masters come from in its dependencies (reading the masters from the archives first, if need be)"
        >Add its plugins' masters as dependencies</button
      >
      <button role="menuitem" class="danger" onclick={() => (confirmRemove = true)} title="mudcrab remove">Remove from the list…</button>
    {/if}
  </div>
{/if}

{#snippet masterDepsDone(done: MasterDependencies)}
  <div class="depsdone">
    <button class="link close" onclick={() => (masterDeps = null)} title="Hide this">×</button>
    {#if done.mods}
      Added dependencies to {done.mods} mod{done.mods === 1 ? "" : "s"}, from their plugins' masters{#if checked}; check again
        to see the list as it is now{/if}.
    {:else}
      No dependencies to add: every mod already lists the mods its plugins' masters come from.
    {/if}
    {#if done.refused.length}
      <div class="bad">
        Not added, as the player may untick them -- make each mod optional too, or have its <code>if</code> follow what it
        needs:
        <ul>
          {#each done.refused as r}
            <li>
              <button class="link" onclick={() => select(r.mod_id)}>{r.mod_id}</button> needs {r.needs}
              <span class="dim">({r.plugin} has {r.master} as a master)</span>
            </li>
          {/each}
        </ul>
      </div>
    {/if}
    {#if done.skipped.length}
      <div class="dim">
        Left as they were:
        <ul>{#each done.skipped as s}<li>{s}</li>{/each}</ul>
      </div>
    {/if}
  </div>
{/snippet}

{#snippet optionalMenu(doc: ModDoc)}
  {@const blockers = neededBy(doc.id, allMods)}
  {@const needed = optionalNeeds(doc, allMods)}
  {@const unfollowed = menuAsk === "if" ? needed.filter((n) => !namedMods(menuText).includes(n.id)) : []}
  {@const blocked = blockers.length
    ? `${blockers.map((m) => m.title).join(", ")} ${blockers.length === 1 ? "needs it and is" : "need it and are"} not optional`
    : undefined}
  <div class="sub" class:flip={modMenu!.x > window.innerWidth - 470} class:asking={menuAsk}>
    <button role="menuitem" aria-haspopup="menu" class="opens"
      >{doc.optional || doc.condition ? "Optional" : "Make optional"}<span class="dim">▸</span></button
    >
    <div class="menu subm" role="menu">
      {#each groups as group}
        {@const here = !!doc.optional && (doc.optional.group ?? "") === group}
        <button role="menuitem" class:here disabled={!!blocked || here} title={blocked} onclick={() => setOptional(doc, group)}
          >{group || "The list's optional mods"}</button
        >
      {/each}
      {#if menuAsk === "group"}
        <form
          class="ask"
          onsubmit={(e) => {
            e.preventDefault();
            if (menuText.trim()) setOptional(doc, menuText);
          }}
        >
          <!-- svelte-ignore a11y_autofocus -->
          <input type="text" bind:value={menuText} placeholder="The new group's name" autofocus
            onkeydown={(e) => e.key === "Escape" && (e.stopPropagation(), (menuAsk = null))} />
        </form>
      {:else}
        <button role="menuitem" disabled={!!blocked} title={blocked} onclick={() => ((menuAsk = "group"), (menuText = ""))}
          >Add new group…</button
        >
      {/if}
      <hr />
      {#if menuAsk === "if"}
        <form
          class="ask"
          onsubmit={(e) => {
            e.preventDefault();
            if (menuText.trim() && !unfollowed.length) setIf(doc, menuText.trim());
          }}
        >
          <!-- svelte-ignore a11y_autofocus -->
          <input type="text" class="code" bind:value={menuText} placeholder="e.g. big_textures && !mod:Other" autofocus
            onkeydown={(e) => e.key === "Escape" && (e.stopPropagation(), (menuAsk = null))} />
          {#if unfollowed.length}
            <div class="bad">
              It needs {unfollowed.map((n) => n.title).join(", ")}, so its <code>if</code> has to name
              {#each unfollowed as n, i}{i ? ", " : ""}<code>mod:{JSON.stringify(n.id)}</code>{/each}.
            </div>
          {:else}
            <div class="dim">Enter to write it as its <code>if</code>{doc.optional ? ", out of its group" : ""}.</div>
          {/if}
        </form>
      {:else}
        <button role="menuitem" onclick={() => ((menuAsk = "if"), (menuText = doc.condition?.expr ?? ""))}>Custom condition…</button>
      {/if}
      {#if doc.optional || doc.condition}
        <button
          role="menuitem"
          disabled={!!needed.length}
          title={needed.length ? `It needs ${needed.map((m) => m.title).join(", ")}, which the player may untick` : undefined}
          onclick={() => alwaysInstall(doc)}>Always install it</button
        >
      {/if}
    </div>
  </div>
{/snippet}

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
  <section class="left" bind:this={scroller} data-tour="author-list">
    <nav class="panes" data-tour="author-archives">
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
          data-tour="author-add"
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
              class:marked={marked.has(e.id)}
              class:off={e.enabled === false}
              style="--depth: {depth}"
              data-drop={"mod:" + e.id}
              role="listitem"
              oncontextmenu={(ev) => {
                ev.preventDefault();
                ev.stopPropagation();
                confirmRemove = false;
                menuAsk = null;
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
                {#if e.optional}<span class="tag cond" title={e.optional.query}>optional</span>{/if}
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
      <nav data-tour="author-tabs">
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
      <span class="edits" data-tour="author-undo">
        <button onclick={undo} disabled={!canUndo || editing} title="Undo the last edit (Ctrl+Z)">↶ Undo</button>
        <button onclick={redo} disabled={!canRedo || editing} title="Redo (Ctrl+Shift+Z)">↷ Redo</button>
        <button onclick={reload} title="Read the list again, if it has changed on disk">Reload</button>
      </span>
    </div>

    <div class="content">
      {#if error}<pre class="error">{error}</pre>{/if}
      {#if addingDeps}
        <p class="dim">Reading plugins' masters, and adding them as dependencies…</p>
      {:else if masterDeps}
        {@render masterDepsDone(masterDeps)}
      {/if}

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
          <ModFields
            mod={chosen}
            inputs={list.document.inputs}
            mods={allMods}
            {order}
            {readingMasters}
            readMasters={async () => {
              if (!order) order = await api.loadOrder();
              await readMasters();
            }}
            {run}
          />
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
            <ModFiles
              id={chosen.id}
              archives={files}
              edit={(number, key, add, drop) => run(() => api.editFilter(chosen.id, number, key, add, drop))}
              move={(from, to) => run(() => api.addAction(chosen.id, "file_move", [`from=${JSON.stringify(from)}`, `to=${JSON.stringify(to)}`], null))}
              problem={(message) => (error = message)}
              choose={(number, label) => (choosing = { id: chosen.id, number, label })}
              inputs={list.document.inputs}
              {run}
            />
          {/if}
        {/if}
      {:else if tab === "load"}
        {#if order}
          <LoadOrder
            {order}
            {select}
            {mark}
            reading={readingMasters}
            readError={mastersError}
            move={(plugin, before) => run(() => api.movePlugin(plugin, before))}
            setActive={(plugin, active) => run(() => api.setPluginActive(plugin, active))}
          />
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
          <button
            onclick={() => addMasterDeps(null)}
            disabled={editing}
            title="For every mod in the plan: list the mods its plugins' masters come from in its dependencies. One edit, undone at once."
            >Add plugin masters as dependencies to all mods</button
          >
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
  .mod.marked {
    background: var(--accent-soft);
    box-shadow: inset 3px 0 0 var(--accent);
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
  .menu button:disabled {
    color: var(--dim);
    background: none;
  }
  .sub {
    position: relative;
  }
  .opens {
    display: flex !important;
    justify-content: space-between;
  }
  .subm {
    display: none;
    position: absolute;
    left: calc(100% + 2px);
    top: -4px;
    max-width: 280px;
  }
  .sub.flip .subm {
    left: auto;
    right: calc(100% + 2px);
  }
  .sub:hover > .subm,
  .sub:focus-within > .subm,
  .sub.asking > .subm {
    display: block;
  }
  .subm .here::before {
    content: "✓ ";
  }
  .subm hr {
    border: none;
    border-top: 1px solid var(--line);
    margin: 4px 0;
  }
  .ask {
    padding: 2px 8px 4px;
    font-size: 12px;
  }
  .ask input {
    width: 100%;
    box-sizing: border-box;
    margin-bottom: 3px;
  }
  .ask input.code {
    font-family: var(--mono);
  }
  .ask .bad {
    color: var(--problem);
  }
  .depsdone {
    position: relative;
    font-size: 13px;
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 8px 28px 8px 12px;
    margin: 0 0 10px;
  }
  .depsdone .close {
    position: absolute;
    top: 4px;
    right: 8px;
  }
  .depsdone ul {
    margin: 4px 0 0;
    padding-left: 18px;
  }
  .depsdone .bad {
    color: var(--problem);
    margin-top: 6px;
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
