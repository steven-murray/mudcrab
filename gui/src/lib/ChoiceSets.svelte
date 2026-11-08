<script lang="ts">
  import { api, type ChoiceSetView, type ChoiceTarget, type InputDoc } from "./api";
  import ConditionPicker from "./ConditionPicker.svelte";
  import IfTag from "./IfTag.svelte";

  // An archive's installer choices as sets, in order: the first whose `if`
  // holds is installed, the archive's own when none does. The one Author
  // mode's profile installs is marked. A BAIN set's packages are ticked in
  // place; a FOMOD set is chosen by walking its installer.
  let {
    id,
    number,
    fomod,
    packages,
    sets,
    own,
    installed,
    inputs,
    run,
    choose,
  }: {
    id: string;
    number: number;
    /** A FOMOD installer; else BAIN packages. */
    fomod: boolean;
    /** The archive's BAIN packages, to tick. */
    packages: string[];
    sets: ChoiceSetView[];
    own: ChoiceSetView;
    installed: number | null;
    inputs: InputDoc[];
    run: (edit: () => Promise<unknown>) => Promise<boolean>;
    /** Walk the FOMOD installer for one set. */
    choose: (to: ChoiceTarget, title: string) => void;
  } = $props();

  const lower = (list: string[]) => list.map((s) => s.toLowerCase());
  const known = $derived([...new Set(sets.map((s) => s.condition).filter((c): c is string => !!c))]);

  function tick(to: ChoiceTarget, set: ChoiceSetView, name: string, on: boolean) {
    const now = on
      ? packages.filter((p) => lower(set.packages).includes(p.toLowerCase()) || p === name)
      : set.packages.filter((p) => p.toLowerCase() !== name.toLowerCase());
    run(() => api.writeBainSet(id, number, to, now));
  }

  // The set whose condition is being changed, or a new one being made.
  let conditioning = $state<number | null>(null);
  let draft = $state<string | null>(null);
  let adding = $state(false);
  function startCondition(n: number) {
    adding = false;
    conditioning = n;
    draft = sets[n].condition;
  }
  async function saveCondition() {
    if (conditioning === null) return;
    const n = conditioning;
    if (await run(() => api.setChoiceCondition(id, number, n, draft?.trim() || null))) conditioning = null;
  }
  async function addSet() {
    const condition = draft?.trim() || null;
    if (fomod) {
      adding = false;
      choose({ new: condition }, condition ? `a new set, if ${condition}` : "a new set");
    } else if (await run(() => api.writeBainSet(id, number, { new: condition }, own.packages))) adding = false;
  }
  const when = (c: string | null) => (c ? `if ${c}` : "always");
</script>

<div class="sets">
  <div class="shead">
    <strong class="small">Install choices</strong>
    <span class="dim small">the first set whose condition holds is installed; the archive's own when none does</span>
  </div>
  {#each [...sets.map((set, n) => ({ set, n })), { set: own, n: -1 }] as { set, n } (n)}
    {@const to: ChoiceTarget = n < 0 ? "default" : { set: n }}
    {@const on = n < 0 ? installed === null : installed === n}
    <div class="set" class:on>
      <div class="row">
        {#if n >= 0}
          <span class="order">
            <button class="link" disabled={n === 0} onclick={() => run(() => api.moveChoiceSet(id, number, n, n - 1))} title="Try it earlier">↑</button>
            <button class="link" disabled={n === sets.length - 1} onclick={() => run(() => api.moveChoiceSet(id, number, n, n + 1))} title="Try it later"
              >↓</button
            >
          </span>
          {#if set.condition}<IfTag condition={set.condition} applies={set.applies} />{:else}<span class="tag">always</span>{/if}
        {:else}
          <span class="tag">{sets.length ? "otherwise" : "always"}</span>
        {/if}
        {#if on}<span class="now" title="What Author mode's profile installs">installed</span>{/if}
        <span class="grow"></span>
        {#if fomod}
          <button class="link small" onclick={() => choose(to, n < 0 ? "the archive's own choices" : `set ${n + 1}, ${when(set.condition)}`)}
            >Choose…</button
          >
        {/if}
        {#if n >= 0}
          <button class="link small" onclick={() => startCondition(n)}>Condition…</button>
          <button class="link small danger" onclick={() => run(() => api.removeChoiceSet(id, number, n))} title="Take this set out">✕</button>
        {/if}
      </div>
      {#if conditioning === n && n >= 0}
        <div class="edit">
          <ConditionPicker bind:value={draft} {known} {inputs} />
          <div class="buttons">
            <button class="primary small" onclick={saveCondition} disabled={draft !== null && !draft.trim()}>Save</button>
            <button class="small" onclick={() => (conditioning = null)}>Cancel</button>
          </div>
        </div>
      {/if}
      {#if fomod}
        {#if set.fomod.length}
          <ul class="lines">{#each set.fomod as line, i (i)}<li>{line}</li>{/each}</ul>
        {:else}
          <p class="dim small">The installer's own defaults.</p>
        {/if}
      {:else}
        <div class="packages">
          {#each packages as pkg (pkg)}
            {@const conditional = lower(set.conditional).includes(pkg.toLowerCase())}
            <label title={conditional ? "Chosen under an if of its own, in the list's file" : ""}>
              <input
                type="checkbox"
                checked={lower(set.packages).includes(pkg.toLowerCase())}
                disabled={conditional}
                onchange={(e) => tick(to, set, pkg, e.currentTarget.checked)}
              />
              {pkg}
            </label>
          {/each}
        </div>
      {/if}
    </div>
  {/each}
  {#if adding}
    <div class="edit">
      <ConditionPicker bind:value={draft} {known} {inputs} />
      <div class="buttons">
        <button class="primary small" onclick={addSet} disabled={draft !== null && !draft.trim()}
          >{fomod ? "Choose its options…" : "Add, starting from the archive's own"}</button
        >
        <button class="small" onclick={() => (adding = false)}>Cancel</button>
      </div>
    </div>
  {:else}
    <button
      class="link small"
      onclick={() => {
        conditioning = null;
        draft = null;
        adding = true;
      }}>+ Add a set of choices</button
    >
  {/if}
</div>

<style>
  .sets {
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 6px 10px;
    margin: 6px 0;
  }
  .shead {
    display: flex;
    gap: 8px;
    align-items: baseline;
    flex-wrap: wrap;
    margin-bottom: 4px;
  }
  .set {
    border-left: 3px solid transparent;
    padding: 4px 8px;
    margin: 4px 0;
    border-radius: 4px;
  }
  .set.on {
    border-left-color: var(--accent);
    background: var(--accent-soft);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }
  .grow {
    flex: 1;
  }
  .order button {
    padding: 0 2px;
  }
  .now {
    font-size: 11px;
    color: var(--accent);
  }
  .tag {
    font-size: 11px;
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 0 4px;
    color: var(--dim);
  }
  .lines {
    margin: 2px 0;
    padding-left: 18px;
    font-size: 12px;
  }
  .packages {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 12px;
    font-size: 12px;
    margin-top: 2px;
  }
  .edit {
    margin: 6px 0;
  }
  .buttons {
    display: flex;
    gap: 6px;
    margin-top: 6px;
  }
  .small {
    font-size: 12px;
  }
</style>
