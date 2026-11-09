<script lang="ts">
  import { api, tomlString, type InputDoc, type LoadOrder, type ModDoc } from "./api";
  import NeedsDialog from "./NeedsDialog.svelte";
  import OptionalDialog, { type Fresh } from "./OptionalDialog.svelte";
  import { masterNeeds, neededBy, optionalGroups, optionalToml } from "./optional";

  // A mod's own settings: whether it is optional (its `if`), whether it is
  // installed inactive (`disable`), and what it needs (`dependencies`). Each
  // change is written at once, as its command would.
  let {
    mod,
    inputs,
    mods,
    order,
    readingMasters,
    readMasters,
    run,
    rename,
  }: {
    mod: ModDoc;
    inputs: InputDoc[];
    /** Every mod in the list. */
    mods: ModDoc[];
    /** The load order, with each plugin's masters once read. */
    order: LoadOrder | null;
    readingMasters: boolean;
    /** Read every plugin's masters (`check --masters-only`). */
    readMasters: () => void;
    /** Run an edit; resolves true when written. */
    run: (edit: () => Promise<unknown>) => Promise<boolean>;
    /** Rename the mod (its id); resolves true when written. */
    rename: (id: string) => Promise<boolean>;
  } = $props();

  // The mod's new name, while one is typed.
  let renaming = $state<string | null>(null);
  async function saveName() {
    const name = renaming?.trim();
    if (!name || name === mod.id) return void (renaming = null);
    if (await rename(name)) renaming = null;
  }

  // `dependencies` as its file writes it: a list (the author's own), or a
  // table of them by group, `masters` and `resources` being what `check`
  // found.
  type DepGroups = { masters: string[]; resources: string[]; custom: string[] };
  let deps = $state<DepGroups>({ masters: [], resources: [], custom: [] });
  /** Every mod it needs, whatever the reason. */
  const needs = $derived([...new Set([...deps.custom, ...deps.masters, ...deps.resources])]);
  const foundAs = (id: string) =>
    deps.masters.includes(id) ? "a master" : deps.resources.includes(id) ? "its files" : null;
  let error = $state<string | null>(null);
  async function readNeeds() {
    try {
      const value = new Map(await api.fields(mod.id, null)).get("dependencies");
      const ids = (v: unknown) => (Array.isArray(v) ? v.map(String) : []);
      if (Array.isArray(value)) deps = { masters: [], resources: [], custom: ids(value) };
      else if (value && typeof value === "object") {
        const table = value as Record<string, unknown>;
        deps = { masters: ids(table.masters), resources: ids(table.resources), custom: ids(table.custom) };
      } else deps = { masters: [], resources: [], custom: [] };
      error = null;
    } catch (err) {
      error = String(err);
    }
  }
  /** As TOML: a plain list while only the author's own are there. */
  function dependenciesToml(g: DepGroups): string {
    const list = (ids: string[]) => `[${ids.map(tomlString).join(", ")}]`;
    if (!g.masters.length && !g.resources.length) return list(g.custom);
    const parts = (["masters", "resources", "custom"] as const).filter((k) => g[k].length).map((k) => `${k} = ${list(g[k])}`);
    return `{ ${parts.join(", ")} }`;
  }
  $effect(() => {
    void mod;
    readNeeds();
  });

  let optional = $state(false);
  let choosing = $state(false);
  const title = (id: string) => mods.find((m) => m.id === id)?.title ?? id;

  // What would leave a mod that is not optional needing one; see ./optional.
  const isOptional = (id: string) => !!mods.find((m) => m.id === id)?.optional;
  const dependents = $derived(neededBy(mod.id, mods));
  /** The optional mods this one needs (as read just now). */
  const optionalNeeds = $derived(needs.filter(isOptional));

  // Optional by a question: out of its group of optional mods, if it was in one.
  async function setCondition(condition: string, fresh: Fresh | null): Promise<boolean> {
    if (fresh && !(await run(() => api.addInput(fresh.name, fresh.kind, fresh.query, fresh.choices, fresh.fallback)))) return false;
    return run(() => api.setFields(mod.id, null, [`if=${tomlString(condition)}`], mod.optional ? ["optional"] : []));
  }
  const groups = $derived(optionalGroups(mods));

  // The mods its plugins' masters come from, that `dependencies` does not
  // list yet. One it may not need (optional, while it is not and does not
  // follow it) is shown, not offered.
  const fromMasters = $derived(masterNeeds(mod.id, order).filter((n) => !needs.includes(n.id)));
  const mayNotNeed = (id: string) => !mod.optional && isOptional(id) && !(mod.condition?.mods ?? []).includes(id);
  const offered = $derived(fromMasters.filter((n) => !mayNotNeed(n.id)));
  const refused = $derived(fromMasters.filter((n) => mayNotNeed(n.id)));
  const why = (n: { plugin: string; master: string }) => `Its ${n.plugin} has ${n.master} as a master`;
  // Into `masters`; `custom` is the author's, and left as it is.
  const addFromMasters = () => {
    const masters = new Set([...deps.masters, ...offered.map((n) => n.id)]);
    return writeGroups({ ...deps, masters: mods.map((m) => m.id).filter((id) => masters.has(id)) });
  };
  const setGroup = (optional: string) => run(() => api.setFields(mod.id, null, [`optional=${optional}`], []));
  const always = () =>
    run(() =>
      api.setFields(
        mod.id,
        null,
        [],
        [...(mod.condition ? ["if"] : []), ...(mod.optional ? ["optional"] : [])],
      ),
    );
  // Chosen by hand: one taken off goes from whichever group had it; one
  // added is the author's own.
  function setNeeds(next: string[]): Promise<boolean> {
    const masters = deps.masters.filter((id) => next.includes(id));
    const resources = deps.resources.filter((id) => next.includes(id));
    const custom = next.filter((id) => !masters.includes(id) && !resources.includes(id));
    return writeGroups({ masters, resources, custom });
  }
  async function writeGroups(next: DepGroups): Promise<boolean> {
    const empty = !next.masters.length && !next.resources.length && !next.custom.length;
    const done = await run(() =>
      empty
        ? api.setFields(mod.id, null, [], ["dependencies"])
        : api.setFields(mod.id, null, [`dependencies=${dependenciesToml(next)}`], []),
    );
    if (done) await readNeeds();
    return done;
  }
</script>

{#if error}<pre class="error">{error}</pre>{/if}
<div class="fields">
  <div class="row">
    <span class="name">Name</span>
    <span>
      {#if renaming === null}
        <code>{mod.id}</code>
        <button class="link" onclick={() => (renaming = mod.id)} title="Its id, which is also its folder in MO2: what names it in the list is renamed too"
          >Rename…</button
        >
      {:else}
        <form
          class="rename"
          onsubmit={(e) => {
            e.preventDefault();
            saveName();
          }}
        >
          <input bind:value={renaming} spellcheck="false" />
          <button type="submit" class="primary" disabled={!renaming.trim() || renaming.trim() === mod.id}>Rename</button>
          <button type="button" onclick={() => (renaming = null)}>Cancel</button>
        </form>
        <div class="dim small">
          Its folder in MO2 too: what names it in the list (dependencies, conditions, merges, notes) is renamed with
          it. An installed copy keeps its old folder until installed again.
        </div>
      {/if}
    </span>
  </div>
  <div class="row">
    <span class="name">Optional</span>
    <span>
      {#if mod.optional}
        {@const opt = mod.optional}
        One of <strong>{opt.group ?? "the list's optional mods"}</strong>,
        <label
          class="tick"
          title="Whether it starts ticked: the player can still untick it, or tick it (optional = {'{'} default = true {'}'})"
          ><input
            type="checkbox"
            checked={opt.default}
            onchange={(e) => run(() => api.setFields(mod.id, null, [`optional=${optionalToml(opt.group ?? "", e.currentTarget.checked)}`], []))}
          />
          ticked to start</label
        >{#if mod.condition}, and only if <strong>{mod.condition.text}</strong>{/if}
        <button class="link" onclick={() => (optional = true)}>Change…</button>
        · {@render alwaysButton()}
      {:else if mod.condition}
        Installed only if <strong>{mod.condition.text}</strong>
        <code class="dim">{mod.condition.expr}</code>
        <button class="link" onclick={() => (optional = true)}>Change…</button>
        · {@render alwaysButton()}
      {:else}
        <button onclick={() => (optional = true)}>Make this mod optional…</button>
      {/if}
    </span>
  </div>
  <label class="row" title="Installed, but unticked in MO2 until the player ticks it; its plugins inactive">
    <span class="name">Inactive</span>
    <span>
      <input
        type="checkbox"
        checked={mod.enabled === false}
        onchange={(e) => run(() => api.setEnabled(mod.id, !e.currentTarget.checked))}
      />
      <span class="dim">unticked in MO2</span>
    </span>
  </label>
  <div class="row">
    <span class="name">Needs</span>
    <span class="needs">
      {#each needs as need}
        {@const why = foundAs(need)}
        <span class="chip" title={why ? `Found by check: ${why === "a master" ? "its plugins' masters" : "its plugins use its files"}` : "Listed by hand"}
          >{title(need)}{#if why}<span class="dim"> · {why}</span>{/if}</span
        >
      {/each}
      {#if !needs.length}<span class="dim">nothing</span>{/if}
      <button class="link" onclick={() => (choosing = true)}>Choose…</button>
      {#if !order?.masters_read}
        ·
        <button
          class="link"
          onclick={readMasters}
          disabled={readingMasters}
          title="Read its plugins' masters from the archives (check --masters-only), to see which mods they come from"
          >{readingMasters ? "Reading masters…" : "Find from its masters"}</button
        >
      {/if}
    </span>
  </div>
  {#if fromMasters.length}
    <div class="row">
      <span></span>
      <span class="masters">
        {#if offered.length}
          Its plugins' masters also need
          {#each offered as n, i}{i ? ", " : ""}<strong title={why(n)}>{title(n.id)}</strong>{/each}
          · <button class="link" onclick={addFromMasters}>Add {offered.length === 1 ? "it" : "them"}</button>
        {/if}
        {#if refused.length}
          <span class="bad">
            {#each refused as n, i}{i ? ", " : ""}<strong title={why(n)}>{title(n.id)}</strong>{/each}
            {refused.length === 1 ? "has" : "have"} a master its plugins need, but
            {refused.length === 1 ? "is" : "are"} optional: make this mod optional too, or have its <code>if</code> follow
            {refused.length === 1 ? "it" : "them"}.
          </span>
        {/if}
      </span>
    </div>
  {/if}
</div>

{#snippet alwaysButton()}
  <button
    class="link"
    onclick={always}
    disabled={!!optionalNeeds.length}
    title={optionalNeeds.length ? `It needs ${optionalNeeds.map(title).join(", ")}, which the player may untick` : undefined}
    >Always install it</button
  >
{/snippet}

{#if optional}
  <OptionalDialog
    id={mod.id}
    {inputs}
    current={mod.condition?.expr ?? null}
    save={setCondition}
    {groups}
    dependents={dependents.map((m) => m.title)}
    optionalNeeds={optionalNeeds.map((id) => ({ id, title: title(id) }))}
    group={mod.optional ? { name: mod.optional.group ?? null, default: mod.optional.default } : null}
    saveGroup={setGroup}
    close={() => (optional = false)}
  />
{/if}
{#if choosing}
  <NeedsDialog
    id={mod.id}
    mods={mods.filter((m) => m.id !== mod.id)}
    current={needs}
    suggested={new Map(offered.map((n) => [n.id, why(n)]))}
    blocked={mod.optional
      ? new Map()
      : new Map(
          mods
            .filter((m) => m.optional && !(mod.condition?.mods ?? []).includes(m.id))
            .map((m) => [m.id, `${m.title} is optional and ${mod.title} is not: make ${mod.title} optional too, or have its if follow ${m.title}`]),
        )}
    save={setNeeds}
    close={() => (choosing = false)}
  />
{/if}

<style>
  .rename {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .rename input {
    flex: 1;
    min-width: 0;
  }
  .tick {
    display: inline-flex;
    gap: 4px;
    align-items: center;
  }
  .fields {
    font-size: 13px;
    margin: 6px 0 10px;
  }
  .row {
    display: grid;
    grid-template-columns: 90px minmax(0, 1fr);
    gap: 8px;
    align-items: baseline;
    margin: 5px 0;
  }
  .name {
    color: var(--dim);
  }
  code {
    font-size: 11px;
    margin: 0 4px;
    overflow-wrap: anywhere;
  }
  .needs {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    align-items: baseline;
  }
  .masters {
    font-size: 12px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .bad {
    color: var(--problem);
  }
  .chip {
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 0 6px;
    font-size: 12px;
  }
  input[type="checkbox"] {
    margin: 0 4px 0 0;
    vertical-align: -2px;
  }
  .error {
    color: var(--problem);
    white-space: pre-wrap;
    font-family: inherit;
  }
</style>
