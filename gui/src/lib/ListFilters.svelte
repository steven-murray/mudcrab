<script lang="ts">
  import { FILTERS, type Want } from "./listFilters";

  // The list's filters, each any, yes or no; and how the scan they partly
  // rely on is going.
  let {
    wants = $bindable(),
    scanning,
    error,
    scanned,
    rescan,
  }: {
    wants: Record<string, Want>;
    scanning: boolean;
    error: string | null;
    /** How many mods the last scan covered; null before one. */
    scanned: number | null;
    rescan: () => void;
  } = $props();

  const groups = [...new Set(FILTERS.map((f) => f.group))];
  function set(key: string, want: Want | null) {
    const next = { ...wants };
    if (want) next[key] = want;
    else delete next[key];
    wants = next;
  }
</script>

<div class="filters">
  <div class="status small">
    {#if scanning}
      <span class="dim">Scanning the archives…</span>
    {:else if error}
      <span class="error" title={error}>The scan failed: {error.split("\n")[0]}</span>
    {:else if scanned !== null}
      <span class="dim">Scanned {scanned} mod(s)</span>
    {/if}
    <button class="link small" onclick={rescan} disabled={scanning} title="Read the archives again (what is kept is not read twice)">Scan again</button>
    {#if Object.keys(wants).length}<button class="link small" onclick={() => (wants = {})}>Clear filters</button>{/if}
  </div>
  {#each groups as group (group)}
    <div class="group">{group}</div>
    {#each FILTERS.filter((f) => f.group === group) as f (f.key)}
      <div class="row">
        <span class="label" class:waiting={f.scan && scanned === null}>{f.label}</span>
        <span class="seg">
          <button class:on={!wants[f.key]} onclick={() => set(f.key, null)}>any</button>
          <button class:on={wants[f.key] === "yes"} onclick={() => set(f.key, "yes")}>yes</button>
          <button class:on={wants[f.key] === "no"} onclick={() => set(f.key, "no")}>no</button>
        </span>
      </div>
    {/each}
  {/each}
</div>

<style>
  .filters {
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 6px 10px;
    margin: 4px 0 8px;
    font-size: 12px;
  }
  .status {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .group {
    font-weight: 600;
    margin: 8px 0 2px;
    color: var(--dim);
  }
  .row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
    padding: 1px 0;
  }
  .label.waiting {
    color: var(--faint);
  }
  .seg {
    display: inline-flex;
    border: 1px solid var(--line);
    border-radius: 6px;
    overflow: hidden;
    flex: none;
  }
  .seg button {
    border: none;
    background: none;
    padding: 0 7px;
    font-size: 11px;
    width: auto;
  }
  .seg button.on {
    background: var(--accent-soft);
    color: var(--accent);
  }
  .small {
    font-size: 12px;
  }
  .error {
    color: var(--problem);
  }
</style>
