<script lang="ts">
  import { api, tomlString, type InputDoc, type ModDoc } from "./api";
  import NeedsDialog from "./NeedsDialog.svelte";
  import OptionalDialog, { type Fresh } from "./OptionalDialog.svelte";

  // A mod's own settings: whether it is optional (its `if`), whether it is
  // installed inactive (`disable`), and what it needs (`dependencies`). Each
  // change is written at once, as its command would.
  let {
    mod,
    inputs,
    mods,
    run,
  }: {
    mod: ModDoc;
    inputs: InputDoc[];
    /** Every mod in the list: its id and title. */
    mods: { id: string; title: string }[];
    /** Run an edit; resolves true when written. */
    run: (edit: () => Promise<unknown>) => Promise<boolean>;
  } = $props();

  let needs = $state<string[]>([]);
  let error = $state<string | null>(null);
  async function readNeeds() {
    try {
      const value = new Map(await api.fields(mod.id, null)).get("dependencies");
      needs = Array.isArray(value) ? value.map(String) : [];
      error = null;
    } catch (err) {
      error = String(err);
    }
  }
  $effect(() => {
    void mod;
    readNeeds();
  });

  let optional = $state(false);
  let choosing = $state(false);
  const title = (id: string) => mods.find((m) => m.id === id)?.title ?? id;

  async function setCondition(condition: string, fresh: Fresh | null): Promise<boolean> {
    if (fresh && !(await run(() => api.addInput(fresh.name, fresh.kind, fresh.query, fresh.choices, fresh.fallback)))) return false;
    return run(() => api.setFields(mod.id, null, [`if=${tomlString(condition)}`], []));
  }
  async function setNeeds(next: string[]): Promise<boolean> {
    const done = await run(() =>
      next.length
        ? api.setFields(mod.id, null, [`dependencies=[${next.map(tomlString).join(", ")}]`], [])
        : api.setFields(mod.id, null, [], ["dependencies"]),
    );
    if (done) await readNeeds();
    return done;
  }
</script>

{#if error}<pre class="error">{error}</pre>{/if}
<div class="fields">
  <div class="row">
    <span class="name">Optional</span>
    <span>
      {#if mod.condition}
        Installed only if <strong>{mod.condition.text}</strong>
        <code class="dim">{mod.condition.expr}</code>
        <button class="link" onclick={() => (optional = true)}>Change…</button>
        · <button class="link" onclick={() => run(() => api.setFields(mod.id, null, [], ["if"]))}>Always install it</button>
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
      {#each needs as need}<span class="chip">{title(need)}</span>{/each}
      {#if !needs.length}<span class="dim">nothing</span>{/if}
      <button class="link" onclick={() => (choosing = true)}>Choose…</button>
    </span>
  </div>
</div>

{#if optional}
  <OptionalDialog id={mod.id} {inputs} current={mod.condition?.expr ?? null} save={setCondition} close={() => (optional = false)} />
{/if}
{#if choosing}
  <NeedsDialog
    id={mod.id}
    mods={mods.filter((m) => m.id !== mod.id)}
    current={needs}
    save={setNeeds}
    close={() => (choosing = false)}
  />
{/if}

<style>
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
