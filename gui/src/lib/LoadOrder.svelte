<script lang="ts">
  import type { LoadOrder, PluginRow } from "./api";
  import { startDrag } from "./drag";

  // The load order with each plugin's masters, and a flag on any placed
  // above one (check's rule, worked out here from the order, so it follows a
  // drag). Drag a plugin by its handle to move it (`plugins move`); tick or
  // untick it to make it active or not. The chosen plugin is shaded, its
  // masters shaded differently, and its mods marked in the list.
  let {
    order,
    select,
    reading,
    readError,
    move,
    setActive,
    mark,
  }: {
    order: LoadOrder;
    select: (mod: string) => void;
    /** Whether the masters are being read (`check --masters-only`). */
    reading: boolean;
    readError: string | null;
    move: (plugin: string, before: string | null) => void;
    setActive: (plugin: string, active: boolean) => void;
    /** The chosen plugin's mods, to mark in the list on the left. */
    mark: (mods: string[]) => void;
  } = $props();

  let scroller = $state<HTMLElement | null>(null);

  // A drop before or after a plugin, as `plugins move --before` (or --last).
  function dropped(plugin: string, key: string, place: string) {
    const at = order.plugins.findIndex((p) => p.name === key);
    const before = place === "before" ? key : (order.plugins[at + 1]?.name ?? null);
    if (before === plugin) return;
    move(plugin, before);
  }

  let filter = $state("");
  let onlyFlagged = $state(false);
  let opened = $state<string | null>(null);
  function choose(plugin: PluginRow) {
    opened = opened === plugin.name ? null : plugin.name;
    mark(opened ? plugin.mods : []);
  }
  // The chosen plugin's masters, lowercased.
  const mastersOfOpened = $derived(
    new Set(
      (order.plugins.find((p) => p.name === opened)?.masters ?? []).map((m) => m.toLowerCase()),
    ),
  );

  const at = $derived(new Map(order.plugins.map((p, i) => [p.name.toLowerCase(), i])));
  const game = $derived(new Set(order.game));

  // What is wrong with each master of the plugin at `index`, if anything.
  function problems(plugin: PluginRow, index: number): { master: string; why: string }[] {
    const out: { master: string; why: string }[] = [];
    for (const master of plugin.masters ?? []) {
      const key = master.toLowerCase();
      const where = at.get(key);
      const merged = order.merged[key];
      if (where === undefined) {
        if (merged) out.push({ master, why: `merged into ${merged}, so not loaded` });
        else if (!game.has(key)) out.push({ master, why: "not in the list" });
      } else if (where > index) out.push({ master, why: `loads after it, at ${where + 1}` });
      else if (order.plugins[where].inactive) out.push({ master, why: "in the list, but inactive" });
    }
    return out;
  }

  const rows = $derived(
    order.plugins.map((plugin, index) => ({ plugin, index, problems: problems(plugin, index) })),
  );
  const flagged = $derived(rows.filter((r) => r.problems.length).length);
  const words = $derived(filter.trim().toLowerCase());
  const shown = $derived(
    rows.filter(
      (r) =>
        (!onlyFlagged || r.problems.length) &&
        (!words ||
          r.plugin.name.toLowerCase().includes(words) ||
          r.plugin.mods.some((m) => m.toLowerCase().includes(words)) ||
          (r.plugin.masters ?? []).some((m) => m.toLowerCase().includes(words))),
    ),
  );
</script>

<div class="bar">
  <input type="text" placeholder="Find a plugin, mod or master" bind:value={filter} />
  {#if order.masters_read}
    <label class:warn={flagged > 0}><input type="checkbox" bind:checked={onlyFlagged} /> Only flagged ({flagged})</label>
  {/if}
</div>

{#if readError}
  <p class="note bad">Masters not read: {readError}</p>
{:else if reading}
  <p class="note">Reading each plugin's masters from the archives… (quick after the first time)</p>
{/if}

<ol bind:this={scroller}>
  {#each shown as { plugin, index, problems } (plugin.name)}
    <li
      class:inactive={plugin.inactive}
      class:flagged={problems.length > 0}
      class:chosen={opened === plugin.name}
      class:master={mastersOfOpened.has(plugin.name.toLowerCase())}
      data-drop={plugin.name}
    >
      <div class="row">
        <span
          class="grip"
          role="button"
          tabindex="-1"
          title="Drag to move it"
          onpointerdown={(e) =>
            startDrag(e, {
              scroller: scroller!.closest(".right") as HTMLElement,
              onDrop: (key, place) => dropped(plugin.name, key, place),
            })}>⠿</span
        >
        <input
          type="checkbox"
          checked={!plugin.inactive}
          title={plugin.inactive ? "Inactive: unticked in MO2. Tick to make it active" : "Active. Untick to leave it unticked in MO2"}
          onchange={(e) => setActive(plugin.name, e.currentTarget.checked)}
        />
        <span class="n">{index + 1}</span>
        <button class="plugin" onclick={() => choose(plugin)}>
          {plugin.name}
        </button>
        <span class="mods">
          {#each plugin.mods as mod}<button class="link" onclick={() => select(mod)}>{mod}</button>{/each}
        </span>
        <span class="tags">
          {#if plugin.masters}<span class="badge" title={plugin.masters.join("\n")}>{plugin.masters.length} master(s)</span>{/if}
          {#if problems.length}<span class="badge bad">{problems.length} misplaced</span>{/if}
        </span>
      </div>
      {#if problems.length}
        <ul class="why">
          {#each problems as p}<li>needs <strong>{p.master}</strong>, which is {p.why}</li>{/each}
        </ul>
      {/if}
      {#if opened === plugin.name}
        <div class="masters">
          {#if plugin.masters === null}
            <span class="dim">Masters not read{order.masters_read ? " (made at install, or inside an archive in an archive)" : ""}.</span>
          {:else if plugin.masters.length === 0}
            <span class="dim">No masters.</span>
          {:else}
            Masters, in the order the plugin names them:
            {#each plugin.masters as master}<code>{master}</code>{/each}
          {/if}
        </div>
      {/if}
    </li>
  {/each}
</ol>
{#if !shown.length}<p class="dim">Nothing matches.</p>{/if}

<style>
  .bar {
    display: flex;
    gap: 14px;
    align-items: center;
    margin-bottom: 8px;
  }
  .bar input[type="text"] {
    flex: 1;
    max-width: 320px;
  }
  label {
    font-size: 13px;
    color: var(--dim);
  }
  label.warn {
    color: var(--warning);
  }
  .note {
    font-size: 13px;
    color: var(--dim);
  }
  .note.bad {
    color: var(--problem);
  }
  .row input[type="checkbox"] {
    margin: 0;
    align-self: center;
  }
  li.chosen {
    background: var(--accent-soft);
    box-shadow: inset 3px 0 0 var(--accent);
  }
  li.master {
    background: color-mix(in srgb, var(--note) 14%, transparent);
    box-shadow: inset 3px 0 0 var(--note);
  }
  ol {
    list-style: none;
    padding: 0;
    margin: 0;
    font-size: 13px;
  }
  ol > li {
    border-bottom: 1px solid var(--line);
    padding: 3px 4px;
  }
  .row {
    display: grid;
    grid-template-columns: 14px 16px 30px minmax(0, 1.2fr) minmax(0, 1fr) auto;
    gap: 8px;
    align-items: baseline;
  }
  .n {
    color: var(--faint);
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .plugin {
    background: none;
    border: none;
    padding: 0;
    text-align: left;
    overflow-wrap: anywhere;
  }
  li.inactive .plugin {
    color: var(--dim);
  }
  li.flagged .plugin {
    color: var(--problem);
  }
  .mods {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 8px;
    min-width: 0;
  }
  .mods .link {
    font-size: 12px;
    text-align: left;
    overflow-wrap: anywhere;
  }
  .tags {
    display: flex;
    gap: 4px;
  }
  .badge {
    font-size: 11px;
    color: var(--dim);
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 0 5px;
    white-space: nowrap;
  }
  .badge.bad {
    color: var(--problem);
    border-color: var(--problem);
  }
  .why {
    margin: 2px 0 2px 84px;
    padding: 0;
    list-style: none;
    font-size: 12px;
    color: var(--problem);
  }
  .masters {
    margin: 4px 0 4px 84px;
    font-size: 12px;
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    align-items: baseline;
  }
</style>
