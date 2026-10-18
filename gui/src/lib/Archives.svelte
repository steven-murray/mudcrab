<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { api, type ArchiveRow, type NexusFileRow, type NexusMod } from "./api";

  // The archives in the downloads folder and the other folders to look in,
  // newest first. Double-click one (or right-click, "Add to the list") to add
  // it as a mod; those the list already uses say by which mod.
  let {
    add,
    select,
  }: {
    add: (path: string) => void;
    select: (mod: string) => void;
  } = $props();

  let rows = $state<ArchiveRow[] | null>(null);
  let error = $state<string | null>(null);
  let filter = $state("");
  let unused = $state(true);
  let menu = $state<{ x: number; y: number; row: ArchiveRow } | null>(null);

  export async function refresh() {
    error = null;
    try {
      rows = await api.archives();
    } catch (err) {
      error = String(err);
    }
  }
  onMount(refresh);

  // A Nexus link: the mod's files to choose from, then the chosen one
  // fetched (or waited for, downloaded by hand) and added.
  let link = $state("");
  let linkInput = $state<HTMLInputElement | null>(null);
  /** Put the cursor in the Nexus link box. */
  export function focusLink() {
    linkInput?.focus();
  }
  let nexus = $state<NexusMod | null>(null);
  let showOld = $state(false);
  let looking = $state(false);
  let waiting = $state<{ file: string; page: string | null } | null>(null);
  let timer: ReturnType<typeof setInterval> | undefined;
  onDestroy(() => clearInterval(timer));

  async function lookUp(event: SubmitEvent) {
    event.preventDefault();
    error = null;
    nexus = null;
    looking = true;
    try {
      nexus = await api.nexusLink(link);
    } catch (err) {
      error = String(err);
    } finally {
      looking = false;
    }
  }

  const old = (f: NexusFileRow) => ["OLD_VERSION", "ARCHIVED", "DELETED"].includes(f.category_name ?? "");
  const files = $derived((nexus?.files ?? []).filter((f) => showOld || !old(f) || f.file_id === nexus?.file_id));

  async function fetch(file: NexusFileRow) {
    if (file.here) return add(file.here);
    error = null;
    looking = true;
    try {
      const fetched = await api.nexusFetch(nexus!.mod_id, file.file_id, file.file_name);
      if (fetched.path) {
        await refresh();
        return add(fetched.path);
      }
      waiting = { file: file.file_name ?? file.name ?? String(file.file_id), page: fetched.page };
      if (fetched.page) await api.openPage(fetched.page).catch(() => {});
      clearInterval(timer);
      timer = setInterval(async () => {
        try {
          const arrived = await api.nexusLook();
          if (arrived) {
            clearInterval(timer);
            waiting = null;
            await refresh();
            add(arrived);
          }
        } catch (err) {
          clearInterval(timer);
          waiting = null;
          error = String(err);
        }
      }, 2000);
    } catch (err) {
      error = String(err);
    } finally {
      looking = false;
    }
  }

  const words = $derived(filter.trim().toLowerCase());
  const shown = $derived(
    (rows ?? []).filter(
      (r) =>
        (!unused || !r.used_by.length) &&
        (!words ||
          r.file_name.toLowerCase().includes(words) ||
          (r.mod_name ?? "").toLowerCase().includes(words)),
    ),
  );

  function size(bytes: number): string {
    if (bytes >= 1 << 30) return `${(bytes / (1 << 30)).toFixed(1)} GB`;
    if (bytes >= 1 << 20) return `${(bytes / (1 << 20)).toFixed(1)} MB`;
    return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  }

  function age(seconds: number | null): string {
    if (!seconds) return "";
    const days = Math.floor((Date.now() / 1000 - seconds) / 86400);
    return days < 1 ? "today" : days < 2 ? "yesterday" : days < 60 ? `${days} days ago` : new Date(seconds * 1000).toLocaleDateString();
  }
</script>

<svelte:window onclick={() => (menu = null)} onkeydown={(e) => e.key === "Escape" && (menu = null)} />

<form class="link" onsubmit={lookUp}>
  <input type="text" placeholder="Paste a Nexus link to add a mod from" bind:value={link} bind:this={linkInput} />
  <button type="submit" disabled={!link.trim() || looking}>{looking ? "…" : "Look up"}</button>
</form>
{#if waiting}
  <div class="waiting">
    Waiting for <strong>{waiting.file}</strong>: download it from the page that opened{#if waiting.page}
      (<button class="link" onclick={() => api.openPage(waiting!.page!)}>open it again</button>){/if}. It is
    picked up as it arrives, from the browser's downloads folder too.
    <button class="link" onclick={() => ((waiting = null), clearInterval(timer))}>Stop</button>
  </div>
{/if}
{#if nexus}
  <div class="nexus">
    <div class="nhead">
      <span>Nexus mod {nexus.mod_id}: choose a file</span>
      <span>
        <label><input type="checkbox" bind:checked={showOld} /> old</label>
        <button class="link" onclick={() => (nexus = null)}>Close</button>
      </span>
    </div>
    {#each files as file (file.file_id)}
      <button class="file" class:named={file.file_id === nexus.file_id} onclick={() => fetch(file)} disabled={looking}>
        <span class="fname">{file.name ?? file.file_name}</span>
        <span class="dim small">
          {file.category_name?.toLowerCase().replace("_", " ")}{#if file.version} · v{file.version}{/if}{#if file.size_kb} · {size(file.size_kb * 1024)}{/if}
          {#if file.here}· <span class="here">already downloaded</span>{/if}
        </span>
      </button>
    {/each}
  </div>
{/if}
<div class="bar">
  <input type="text" placeholder="Find an archive" bind:value={filter} />
</div>
<div class="opts">
  <label><input type="checkbox" bind:checked={unused} /> Only those not in the list</label>
  <button class="link" onclick={refresh} title="Look in the folders again">Refresh</button>
</div>

{#if error}
  <pre class="error">{error}</pre>
{:else if !rows}
  <p class="dim pad">Looking in the downloads folders…</p>
{:else if !rows.length}
  <p class="dim pad">No archives in the downloads folder or the other folders chosen in Install.</p>
{:else}
  <ul>
    {#each shown as row (row.path)}
      <li
        ondblclick={() => add(row.path)}
        oncontextmenu={(e) => {
          e.preventDefault();
          e.stopPropagation();
          menu = { x: e.clientX, y: e.clientY, row };
        }}
        title={`${row.path}\nDouble-click to add it to the list`}
      >
        <div class="name">{row.mod_name ?? row.file_name}</div>
        <div class="meta dim">
          {#if row.mod_name}<span class="file">{row.file_name}</span>{/if}
          {#if row.version}<span>v{row.version}</span>{/if}
          <span>{size(row.size)}</span>
          <span>{age(row.modified)}</span>
          {#if row.source?.startsWith("nexus:")}<span class="tag">Nexus</span>{:else if row.source}<span class="tag">web</span>{/if}
        </div>
        {#if row.used_by.length}
          <div class="used">
            in the list as
            {#each row.used_by as id}<button class="link" onclick={() => select(id)}>{id}</button>{/each}
          </div>
        {/if}
      </li>
    {/each}
  </ul>
  {#if !shown.length}<p class="dim pad">Nothing matches.</p>{/if}
{/if}

{#if menu}
  <div class="menu" style="left: {menu.x}px; top: {menu.y}px" role="menu">
    <button
      role="menuitem"
      onclick={() => {
        add(menu!.row.path);
        menu = null;
      }}>Add to the list…</button
    >
  </div>
{/if}

<style>
  form.link {
    display: flex;
    gap: 4px;
    padding: 8px 12px 0;
  }
  form.link input {
    flex: 1;
    min-width: 0;
  }
  .waiting {
    margin: 6px 12px;
    padding: 6px 8px;
    border: 1px solid var(--warning);
    border-radius: 6px;
    font-size: 12px;
  }
  .nexus {
    margin: 6px 12px;
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 4px;
  }
  .nhead {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
    color: var(--dim);
    padding: 2px 4px 4px;
  }
  .nhead label {
    margin-right: 8px;
  }
  .file {
    display: block;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    border-radius: 4px;
    padding: 3px 6px;
  }
  .file:hover:not(:disabled) {
    background: var(--card-hi);
  }
  .file.named {
    outline: 1px solid var(--accent);
  }
  .fname {
    display: block;
    overflow-wrap: anywhere;
  }
  .here {
    color: var(--good);
  }
  .small {
    font-size: 11px;
  }
  .bar {
    position: sticky;
    top: 0;
    z-index: 1;
    padding: 8px 12px;
    background: var(--bg);
    border-bottom: 1px solid var(--line);
  }
  .bar input {
    width: 100%;
  }
  .opts {
    display: flex;
    justify-content: space-between;
    padding: 6px 12px;
    font-size: 12px;
    color: var(--dim);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0 0 30px;
  }
  li {
    padding: 5px 12px;
    border-bottom: 1px solid var(--line);
    cursor: default;
    -webkit-user-select: none;
    user-select: none;
  }
  li:hover {
    background: var(--card-hi);
  }
  .name {
    overflow-wrap: anywhere;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 8px;
    font-size: 11px;
  }
  .file {
    overflow-wrap: anywhere;
  }
  .tag {
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 0 4px;
  }
  .used {
    font-size: 11px;
    color: var(--dim);
    display: flex;
    flex-wrap: wrap;
    gap: 0 6px;
  }
  .used .link {
    font-size: 11px;
  }
  .pad {
    padding: 0 12px;
  }
  .error {
    color: var(--problem);
    white-space: pre-wrap;
    padding: 0 12px;
    font-family: inherit;
  }
  .menu {
    position: fixed;
    z-index: 60;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 6px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
    padding: 4px;
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
</style>
