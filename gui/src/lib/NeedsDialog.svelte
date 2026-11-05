<script lang="ts">
  import { untrack } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import Dialog from "./Dialog.svelte";

  // The mods a mod needs (its `dependencies`), ticked from every mod in the
  // list, found by name.
  let {
    id,
    mods,
    current,
    blocked = new Map(),
    suggested = new Map(),
    save,
    close,
  }: {
    id: string;
    /** Every other mod: its id and title. */
    mods: { id: string; title: string }[];
    current: string[];
    /** The mods it may not need, each with why. */
    blocked?: Map<string, string>;
    /** The mods its plugins' masters come from, each with which: ticked to start. */
    suggested?: Map<string, string>;
    save: (needs: string[]) => Promise<boolean>;
    close: () => void;
  } = $props();

  // Starting at what the mod needs when the dialog opens.
  const ticked = new SvelteSet<string>(untrack(() => [...current, ...suggested.keys()]));
  let filter = $state("");
  let busy = $state(false);
  const words = $derived(filter.trim().toLowerCase());
  // The ticked first, then the rest, in list order.
  const shown = $derived(
    [...mods.filter((m) => ticked.has(m.id)), ...mods.filter((m) => !ticked.has(m.id))].filter(
      (m) => !words || m.id.toLowerCase().includes(words) || m.title.toLowerCase().includes(words),
    ),
  );
  const changed = $derived(ticked.size !== current.length || current.some((c) => !ticked.has(c)));

  async function ok() {
    busy = true;
    try {
      // In list order, as a reader would look for them.
      if (await save(mods.map((m) => m.id).filter((m) => ticked.has(m)))) close();
    } finally {
      busy = false;
    }
  }
</script>

<Dialog title="What {id} needs" {close} width={480}>
  <!-- svelte-ignore a11y_autofocus -->
  <input type="text" class="find" placeholder="Find a mod" bind:value={filter} autofocus />
  <ul>
    {#each shown as mod (mod.id)}
      {@const why = ticked.has(mod.id) ? undefined : blocked.get(mod.id)}
      <li>
        <label class:blocked={why} title={why}>
          <input
            type="checkbox"
            disabled={!!why}
            checked={ticked.has(mod.id)}
            onchange={(e) => (e.currentTarget.checked ? ticked.add(mod.id) : ticked.delete(mod.id))}
          />
          {mod.title}{#if mod.title !== mod.id}<span class="dim"> · {mod.id}</span>{/if}
          {#if why}<span class="dim"> · optional</span>{/if}
          {#if suggested.has(mod.id)}<span class="dim" title={suggested.get(mod.id)}> · a master</span>{/if}
        </label>
      </li>
    {/each}
    {#if !shown.length}<li class="dim">Nothing matches.</li>{/if}
  </ul>
  {#snippet foot()}
    <span class="dim count">{ticked.size} ticked</span>
    <button onclick={close}>Cancel</button>
    <button class="primary" onclick={ok} disabled={busy || !changed}>Save</button>
  {/snippet}
</Dialog>

<style>
  .find {
    width: 100%;
    box-sizing: border-box;
    margin-bottom: 8px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 50vh;
    overflow: auto;
  }
  li {
    padding: 2px 0;
  }
  label {
    display: flex;
    gap: 6px;
    align-items: baseline;
  }
  label.blocked {
    color: var(--dim);
  }
  .count {
    margin-right: auto;
  }
</style>
