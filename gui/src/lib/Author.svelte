<script lang="ts">
  import { tick, untrack } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import {
    api,
    tomlString,
    type Change,
    type Changes,
    type Checked,
    type Conflicts as Found,
    type Edited,
    type AuthorProfile,
    noteSlow,
    type FoundDependencies,
    type FoundGroup,
    type FindingGroup,
    type ResourceScan,
    type Entry,
    type ListView,
    type LoadOrder as Order,
    type ModDoc,
    type ModView,
    type Tree,
  } from "./api";
  import AddDialog from "./AddDialog.svelte";
  import ChooseDialog from "./ChooseDialog.svelte";
  import IniEditor from "./IniEditor.svelte";
  import ProfileDialog from "./ProfileDialog.svelte";
  import AddActionDialog from "./AddActionDialog.svelte";
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
  import ListFilters from "./ListFilters.svelte";
  import { passes, type Context, type ModScan, type Want } from "./listFilters";

  // Author mode: the list as a tree on the left; on the right, the chosen mod
  // and its files, the load order with masters, conflicts, check, the
  // questions, and this session's changes. Every edit is one of the editing
  // commands, written at once (as the CLI writes it), shown with its command
  // and diff, and undoable. See docs/design/gui.md.
  let {
    list = $bindable(),
    active,
    locations = $bindable(),
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
  // Author mode's profile: the answers it plans the list with, apart from
  // Install mode's. Changing it plans the list again.
  let profile = $state<AuthorProfile | null>(null);
  let answering = $state(false);
  let naming = $state(false);
  let newProfile = $state("");
  async function readProfile() {
    try {
      profile = await api.authorProfile();
    } catch (err) {
      error = String(err);
    }
  }
  $effect(() => {
    if (active && !profile) untrack(readProfile);
  });
  // A profile chosen or its answers changed: what is shown follows them.
  async function profileChanged(next: AuthorProfile) {
    profile = next;
    // Profiles are shared: Install mode's list of them, and its answers when
    // it has the same one, follow.
    api
      .locations()
      .then((read) => (locations = read))
      .catch(() => {});
    order = null;
    checked = null;
    await refresh();
    rescan();
  }
  async function chooseProfile(name: string) {
    try {
      await profileChanged(await api.setAuthorProfile(name));
    } catch (err) {
      error = String(err);
    }
  }

  // The mod whose INIs are being edited (and the file and condition it
  // opens on), or an action added to.
  let editingIni = $state<string | null>(null);
  let iniStart = $state<{ file: string; scope: "mod" | "game"; condition: string | null } | null>(null);
  // An INI action's file, scope and condition, read from its line, to open
  // the INI editor on it.
  function iniActionOf(line: string): { file: string; scope: "mod" | "game"; condition: string | null } | null {
    if (!/(?:^|, )action = (["'])ini_(set|comment|append_block)\1/.test(line)) return null;
    const field = (name: string) => {
      const m = new RegExp(`(?:^|, )${name} = (?:"((?:[^"\\\\]|\\\\.)*)"|'([^']*)')`).exec(line);
      if (!m) return null;
      return m[1] !== undefined ? (JSON.parse(`"${m[1]}"`) as string) : m[2];
    };
    const file = field("file");
    if (!file) return null;
    return { file, scope: field("scope") === "game" ? "game" : "mod", condition: field("if") };
  }
  let addingAction = $state<string | null>(null);
  // An edit from a dialog, which shows why it was refused itself.
  async function attempt(edit: () => Promise<Edited | unknown>): Promise<string | null> {
    if (await run(edit)) return null;
    const why = error ?? "Nothing was written.";
    error = null;
    return why;
  }
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

  // Removing is refused while another mod names it; the menu stays open to
  // say why.
  let removeError = $state<string | null>(null);
  async function removeMod(id: string) {
    removeError = null;
    // Not selected while the list is read again: it would be looked up gone.
    const was = selected;
    if (selected === id) selected = null;
    if (!(await run(() => api.removeMod(id)))) {
      selected = was;
      removeError = error;
      error = null;
      return;
    }
    modMenu = null;
    if (was === id) {
      view = null;
      files = [];
    }
    await archivesPane?.refresh();
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

  // The list scanned (`scan_list`): what each mod installs, read from its
  // archives when the list opens and again after each edit, so the badges
  // and filters follow the list without a Check.
  let scan = $state<Map<string, ModScan> | null>(null);
  let listScanning = $state(false);
  let scanError = $state<string | null>(null);
  let scanTimer: ReturnType<typeof setTimeout> | undefined;
  let scanRun = 0;
  function rescan() {
    clearTimeout(scanTimer);
    scanTimer = setTimeout(async () => {
      const mine = ++scanRun;
      listScanning = true;
      try {
        const read = await api.scanList();
        if (mine === scanRun) {
          scan = new Map(read.mods.map((m) => [m.mod_id, m]));
          scanError = null;
        }
      } catch (err) {
        if (mine === scanRun) scanError = String(err);
      } finally {
        if (mine === scanRun) listScanning = false;
      }
    }, 400);
  }
  $effect(() => {
    void list;
    if (active) untrack(rescan);
  });

  // The filters, kept between sessions; each mod's non-optional dependents.
  const wantsKey = "mudcrab.author.filters";
  let wants = $state<Record<string, Want>>(
    (() => {
      try {
        return JSON.parse(localStorage.getItem(wantsKey) ?? "{}");
      } catch {
        return {};
      }
    })(),
  );
  $effect(() => {
    try {
      localStorage.setItem(wantsKey, JSON.stringify(wants));
    } catch {
      // Not kept, then.
    }
  });
  let filtersOpen = $state(false);
  // Where the filters open: under their button, kept on screen. Fixed, as
  // the list pane would cut off anything wider than itself.
  let filtersAt = $state("");
  function openFilters(button: HTMLElement) {
    const at = button.getBoundingClientRect();
    const width = 340;
    const left = Math.max(8, Math.min(at.left, window.innerWidth - width - 8));
    filtersAt = `left: ${left}px; top: ${at.bottom + 4}px; width: ${width}px; max-height: ${window.innerHeight - at.bottom - 16}px`;
    filtersOpen = true;
  }
  const filtering = $derived(Object.keys(wants).length > 0);
  const needers = $derived(new Map(allMods.map((m) => [m.id, neededBy(m.id, allMods)])));
  const cx = $derived<Context>({
    scan,
    neededBy: (id) => needers.get(id)?.length ?? 0,
    mergeSources: new Set(allMods.flatMap((m) => m.merge?.sources.map((s) => s.mod) ?? [])),
  });

  function matches(entry: Entry, words: string): boolean {
    if (!words && !filtering) return true;
    if (entry.kind === "section")
      return (!filtering && entry.title.toLowerCase().includes(words)) || entry.entries.some((e) => matches(e, words));
    if (entry.kind === "mod")
      return (
        (!words ||
          entry.title.toLowerCase().includes(words) ||
          entry.id.toLowerCase().includes(words) ||
          entry.plugins.some((p) => p.toLowerCase().includes(words))) &&
        (!filtering || passes(entry, wants, cx))
      );
    return false;
  }
  const words = $derived(filter.trim().toLowerCase());
  // Mods in a section: those shown, when filtered.
  function count(e: Entry): number {
    if (e.kind === "mod") return !filtering || passes(e, wants, cx) ? 1 : 0;
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
      checked = await api.check(false, true);
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
      const start = performance.now();
      const done = (await edit()) as Edited;
      const edited = performance.now();
      last = done.change;
      canUndo = done.can_undo;
      canRedo = done.can_redo;
      await refresh();
      const refreshed = performance.now();
      await new Promise((r) => setTimeout(r, 0));
      void document.body.offsetHeight;
      const shown = performance.now();
      noteSlow(
        `edit: ${Math.round(edited - start)} ms writing, ${Math.round(refreshed - edited)} ms reading back, ${Math.round(shown - refreshed)} ms showing`,
        shown - start,
      );
      return true;
    } catch (err) {
      error = String(err);
      return false;
    } finally {
      editing = false;
    }
  }

  // One group of each mod's found `dependencies` (every mod's, or one's) set
  // as last found: `masters` read first if they are not yet, `resources`
  // once looked for. What it did is shown until the next.
  let foundDeps = $state<{ group: FoundGroup; done: FoundDependencies } | null>(null);
  let addingDeps = $state<FoundGroup | null>(null);
  async function addFound(group: FoundGroup, id: string | null) {
    modMenu = null;
    if (editing) return;
    editing = true;
    addingDeps = group;
    error = null;
    try {
      const done = await api.addFoundDependencies(group, id);
      foundDeps = { group, done };
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
      addingDeps = null;
    }
  }

  // What each mod's plugins need of other mods for the files they name
  // (`check --resources`): kept for adding them, until looked for again.
  let resources = $state<ResourceScan | null>(null);
  let scanning = $state(false);
  async function findResources() {
    scanning = true;
    error = null;
    try {
      resources = await api.findResourceDependencies();
    } catch (err) {
      error = String(err);
    } finally {
      scanning = false;
    }
  }
  /** Resource dependencies found that can be added: not listed, not refused. */
  const resourcesToAdd = $derived(resources?.needs.filter((n) => !n.listed && !n.optional).length ?? 0);

  const undo = () => canUndo && run(api.undo);
  const redo = () => canRedo && run(api.redo);

  // Ctrl+Z and Ctrl+Shift+Z (or Ctrl+Y), in author mode, unless typing.
  function onkeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && filtersOpen) {
      filtersOpen = false;
      return;
    }
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
  onresize={() => (filtersOpen = false)}
  onclick={() => {
    modMenu = null;
    addMenu = false;
    filtersOpen = false;
  }}
/>

{#if modMenu}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="menu" style="left: {modMenu.x}px; top: {modMenu.y}px" role="menu" tabindex="-1" onclick={(e) => e.stopPropagation()}>
    <div class="mtitle">{modMenu.title}</div>
    {#if confirmRemove}
      {@const naming = allMods.filter(
        (m) => m.id !== modMenu!.id && ((m.dependencies ?? []).includes(modMenu!.id) || (m.condition?.mods ?? []).includes(modMenu!.id)),
      )}
      <div class="confirm">
        {#if naming.length}
          It can't be removed while other mods name it, in their <code>dependencies</code> or <code>if</code>:
          <ul>
            {#each naming as m (m.id)}<li>{m.title}</li>{/each}
          </ul>
          <div><button onclick={() => (confirmRemove = false)}>Back</button></div>
        {:else}
          Remove it from the list, with its plugins and notes?
          {#if removeError}<p class="error">{removeError}</p>{/if}
          <div>
            <button class="danger" disabled={editing} onclick={() => removeMod(modMenu!.id)}>Remove</button>
            <button onclick={() => (confirmRemove = false)}>Keep it</button>
          </div>
        {/if}
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
        onclick={() => addFound("masters", modMenu!.id)}
        title="List the mods its plugins' masters come from in its dependencies.masters (reading the masters from the archives first, if need be)"
        >Add its plugins' masters as dependencies</button
      >
      {#if resources}
        <button
          role="menuitem"
          onclick={() => addFound("resources", modMenu!.id)}
          title="List the mods that alone provide files its plugins name in its dependencies.resources, as last looked for in Check"
          >Add its resource dependencies</button
        >
      {/if}
      <button role="menuitem" class="danger" onclick={() => (confirmRemove = true)} title="mudcrab remove">Remove from the list…</button>
    {/if}
  </div>
{/if}

{#snippet foundDepsDone(group: FoundGroup, done: FoundDependencies)}
  {@const why = group === "masters" ? "their plugins' masters" : "the files their plugins use"}
  <div class="depsdone">
    <button class="link close" onclick={() => (foundDeps = null)} title="Hide this">×</button>
    {#if done.mods}
      Updated <code>dependencies.{group}</code> of {done.mods} mod{done.mods === 1 ? "" : "s"}, from {why}{#if checked}; check
        again to see the list as it is now{/if}.
    {:else}
      Nothing to change: every mod's <code>dependencies.{group}</code> already lists the mods {why} need.
    {/if}
    {#if done.refused.length}
      <div class="bad">
        Not added, as the player may untick them -- make each mod optional too, or have its <code>if</code> follow what it
        needs:
        <ul>
          {#each done.refused as r}
            <li>
              <button class="link" onclick={() => select(r.mod_id)}>{r.mod_id}</button> needs {r.needs}
              <span class="dim">({r.plugin} {group === "masters" ? "has" : "uses"} {r.named}{group === "masters" ? " as a master" : ""})</span>
            </li>
          {/each}
        </ul>
      </div>
    {/if}
    {#if done.skipped.length}
      <div class="dim">
        Left out:
        <ul>{#each done.skipped as s}<li>{s}</li>{/each}</ul>
      </div>
    {/if}
  </div>
{/snippet}

{#snippet findingGroups(groups: FindingGroup[])}
  {#each groups as group}
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

{#if answering && profile}
  <ProfileDialog {profile} inputs={list.document.inputs} saved={profileChanged} close={() => (answering = false)} />
{/if}

{#if editingIni}
  <IniEditor
    id={editingIni}
    archives={files}
    inputs={list.document.inputs}
    start={iniStart}
    run={attempt}
    close={() => {
      editingIni = null;
      iniStart = null;
    }}
  />
{/if}
{#if addingAction}
  <AddActionDialog
    id={addingAction}
    archives={files}
    inputs={list.document.inputs}
    known={[...new Set(actions.flatMap((a) => [...a.matchAll(/\bif = "([^"]*)"/g)].map((m) => m[1])))]}
    run={attempt}
    close={() => (addingAction = null)}
  />
{/if}

{#if choosing}
  <ChooseDialog
    id={choosing.id}
    number={choosing.number}
    label={choosing.label}
    inputs={list.document.inputs}
    current={profile?.answers ?? {}}
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
    <div class="profilebar">
      <span class="dim">Profile</span>
      <select
        value={profile?.name ?? ""}
        onchange={async (e) => {
          const name = e.currentTarget.value;
          if (name === "\u0000new") {
            e.currentTarget.value = profile?.name ?? "";
            newProfile = "";
            naming = true;
          } else await chooseProfile(name);
        }}
        title="The answers Author mode plans the list with"
      >
        {#each profile?.profiles ?? [] as name (name)}<option value={name}>{name}</option>{/each}
        <option value={"\u0000new"}>New profile…</option>
      </select>
      <button class="link" onclick={() => (answering = true)} disabled={!profile}>Answers…</button>
    </div>
    {#if naming}
      <form
        class="profilebar"
        onsubmit={async (e) => {
          e.preventDefault();
          naming = false;
          await chooseProfile(newProfile);
        }}
      >
        <input bind:value={newProfile} placeholder="a new profile's name" />
        <button type="submit" disabled={!newProfile.trim()}>Make</button>
        <button type="button" class="link" onclick={() => (naming = false)}>Cancel</button>
      </form>
    {/if}
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
          class="plus filt"
          class:on={filtering || filtersOpen}
          onclick={(e) => {
            e.stopPropagation();
            if (filtersOpen) filtersOpen = false;
            else openFilters(e.currentTarget);
            addMenu = false;
          }}
          title={filtering ? `${Object.keys(wants).length} filter(s) on` : "Filter the list"}
          >⚲{#if filtering}<span class="n">{Object.keys(wants).length}</span>{/if}</button
        >
        {#if filtersOpen}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <div class="filterpop" style={filtersAt} onclick={(e) => e.stopPropagation()}>
            <ListFilters bind:wants scanning={listScanning} error={scanError} scanned={scan?.size ?? null} {rescan} />
          </div>
        {/if}
      </div>
      <div class="addwrap">
        <button
          class="plus"
          data-tour="author-add"
          onclick={(e) => {
            e.stopPropagation();
            addMenu = !addMenu;
            filtersOpen = false;
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
            {@const open = opened.has(e.anchor) || !!words || filtering}
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
                removeError = null;
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
                {#if needers.get(e.id)?.length}
                  {@const need = needers.get(e.id)!}
                  <span class="tag need" title={`Needed by ${need.map((m) => m.title).join(", ")}: it can't be made optional while they aren't`}
                    >needed ×{need.length}</span
                  >
                {/if}
                {#if scan?.get(e.id)?.inis.length}
                  <span class="tag ini" title={scan.get(e.id)!.inis.join("\n")}>INI</span>
                {/if}
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
        <p class="dim">
          {addingDeps === "masters" ? "Reading plugins' masters, and adding them" : "Adding resource dependencies"}…
        </p>
      {:else if foundDeps}
        {@render foundDepsDone(foundDeps.group, foundDeps.done)}
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
          {#if view && !view.unplanned && view.archives.length}
            <p>
              <button onclick={() => (editingIni = chosen.id)} title="Change the settings in the mod's INIs, or the game's, as ini_set actions"
                >INI editor…</button
              >
              <button onclick={() => (addingAction = chosen.id)} title="Add any action: QAC, xEdit record removal, files, BSAs (action add)"
                >Add an action…</button
              >
            </p>
          {/if}
          {#if actions.length}
            <h4>Actions</h4>
            <ol class="actions">
              {#each actions as action, i}
                {@const ini = iniActionOf(action)}
                <li>
                  <code>{action}</code>
                  {#if ini}
                    <button
                      class="link"
                      onclick={() => {
                        iniStart = ini;
                        editingIni = chosen.id;
                      }}
                      title="Open it in the INI editor">Edit</button
                    >
                  {/if}
                  <button class="link danger" onclick={() => run(() => api.removeAction(chosen.id, i + 1))}>Remove</button>
                </li>
              {/each}
            </ol>
          {/if}
          {#if view?.left_out}
            <p class="note small">
              Not installed in profile <b>{profile?.name}</b>{#if chosen.condition}
                (it needs <code>{chosen.condition.text}</code>){:else if chosen.optional}
                (it is optional, and not ticked){/if}: shown as it would install.
            </p>
          {/if}
          {#if view?.unplanned}
            <p class="dim">Can't be planned with profile <b>{profile?.name}</b>'s answers: {view.unplanned}</p>
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
            onclick={() => addFound("masters", null)}
            disabled={editing}
            title="For every mod in the plan: set dependencies.masters to the mods its plugins' masters come from, taking out those it no longer needs. One edit, undone at once."
            >Add plugin masters as dependencies to all mods</button
          >
        </div>
        <div class="checkbar">
          <button
            onclick={findResources}
            disabled={scanning}
            title="Read every plugin whole for the meshes and sounds it names, and every BSA for its files (check --masters-only --resources). Slow the first time; what is read is kept."
            >{scanning ? "Reading plugins and BSAs…" : resources ? "Find resource dependencies again" : "Find resource dependencies"}</button
          >
          {#if resources}
            <span>{resources.needs.length} found, {resourcesToAdd} not yet listed</span>
            <button
              onclick={() => addFound("resources", null)}
              disabled={editing}
              title="For every mod in the plan: set dependencies.resources to the mods that alone provide files its plugins name, taking out those it no longer needs. One edit, undone at once."
              >Add resource dependencies to all mods</button
            >
          {/if}
        </div>
        {#if resources}
          {@render findingGroups(resources.groups)}
          {#if !resources.groups.length}<p class="dim">Every mod lists the mods it needs for its files.</p>{/if}
        {:else}
          <p class="dim small">
            A file a mod's plugins name -- a mesh, a sound -- that only one other mod provides makes that mod a dependency.
            One the game has, or several mods have, does not: any would do, and the order only decides whose wins.
          </p>
        {/if}
        {#if checked}
          {@render findingGroups(checked.groups)}
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
  .tag.need {
    color: var(--warning);
    border-color: var(--warning);
  }
  .tag.ini {
    color: var(--good);
    border-color: var(--good);
  }
  .filt.on {
    color: var(--accent);
    border-color: var(--accent);
  }
  .filt .n {
    font-size: 10px;
    margin-left: 2px;
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
  .filterpop {
    position: fixed;
    z-index: 60;
    overflow-y: auto;
    background: var(--panel);
    border-radius: 8px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
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
  .confirm ul {
    margin: 4px 0;
    padding-left: 18px;
    max-height: 200px;
    overflow-y: auto;
  }
  .confirm .error {
    margin: 6px 0 0;
    white-space: pre-wrap;
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
  .profilebar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px 0;
    font-size: 13px;
  }
  .profilebar select,
  .profilebar input {
    flex: 1;
    min-width: 0;
  }
  .note {
    color: var(--note);
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
