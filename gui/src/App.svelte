<script lang="ts">
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type ListView } from "./lib/api";
  import Guide from "./lib/Guide.svelte";
  import Install from "./lib/Install.svelte";

  let list = $state<ListView | null>(null);
  let recent = $state<string[]>([]);
  let error = $state<string | null>(null);
  let opening = $state(false);

  async function openPath(path: string) {
    opening = true;
    error = null;
    try {
      list = await api.openList(path);
    } catch (err) {
      error = String(err);
    } finally {
      opening = false;
      recent = await api.recent();
    }
  }

  async function choose() {
    const path = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "Modlist", extensions: ["toml"] }],
    });
    if (typeof path === "string") await openPath(path);
  }

  // Links in a list's prose open in the browser, not in this window.
  function onclick(event: MouseEvent) {
    const link = (event.target as HTMLElement).closest("a");
    const href = link?.getAttribute("href");
    if (!link || !href || href.startsWith("#")) return;
    event.preventDefault();
    api.openPage(href).catch((err) => (error = String(err)));
  }

  onMount(async () => {
    recent = await api.recent();
    const start = await api.opening();
    if (start) await openPath(start);
  });
</script>

<svelte:window {onclick} />

<div class="app">
  <header>
    <span class="logo">mudcrab</span>
    {#if list}
      <span class="name">{list.document.name}</span>
      <span class="path dim" title={list.path}>{list.path}</span>
    {/if}
    <span class="spacer"></span>
    <button onclick={choose} disabled={opening}>{list ? "Open another list…" : "Open a list…"}</button>
    <div class="modes" title="Author mode, for editing a list, comes later">
      <span class="mode on">Install</span>
      <span class="mode off">Author</span>
    </div>
  </header>

  {#if error}
    <div class="error">
      <pre>{error}</pre>
      <button class="link" onclick={() => (error = null)}>Dismiss</button>
    </div>
  {/if}

  {#if list}
    {#key list.path}
      <main>
        <section class="left"><Guide document={list.document} /></section>
        <aside class="right"><Install {list} /></aside>
      </main>
    {/key}
  {:else}
    <div class="welcome">
      <h1>Install a modlist</h1>
      <p class="dim">
        Open a list's <code>.toml</code> file to read it, answer its questions and install it into a
        Mod Organizer 2 instance. Every step is one of mudcrab's commands, shown as it runs.
      </p>
      <button class="primary" onclick={choose} disabled={opening}>Open a list…</button>
      {#if recent.length}
        <h2>Recent</h2>
        <ul>
          {#each recent as path}
            <li><button class="link" onclick={() => openPath(path)}>{path}</button></li>
          {/each}
        </ul>
      {/if}
    </div>
  {/if}
</div>

<style>
  .app {
    height: 100%;
    display: grid;
    grid-template-rows: auto auto 1fr;
  }
  header {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px 16px;
    background: var(--panel);
    border-bottom: 1px solid var(--line);
    min-width: 0;
  }
  .logo {
    font-weight: 700;
    color: var(--accent);
  }
  .name {
    font-weight: 600;
    white-space: nowrap;
  }
  .path {
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }
  .spacer {
    flex: 1;
  }
  .modes {
    display: flex;
    border: 1px solid var(--line);
    border-radius: 6px;
    overflow: hidden;
    font-size: 13px;
  }
  .mode {
    padding: 4px 10px;
  }
  .mode.on {
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
  }
  .mode.off {
    color: var(--faint);
  }
  .error {
    margin: 10px 16px 0;
    padding: 10px 12px;
    border: 1px solid var(--problem);
    border-radius: 8px;
    background: color-mix(in srgb, var(--problem) 10%, transparent);
  }
  .error pre {
    margin: 0 0 6px;
    white-space: pre-wrap;
    font-family: inherit;
    font-size: 13px;
  }
  main {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(380px, 460px);
    min-height: 0;
  }
  .left {
    overflow: auto;
    min-height: 0;
  }
  .right {
    overflow: auto;
    min-height: 0;
    border-left: 1px solid var(--line);
    background: var(--panel);
  }
  .welcome {
    max-width: 620px;
    margin: 12vh auto 0;
    padding: 0 24px;
  }
  .welcome h1 {
    margin: 0 0 8px;
  }
  .welcome h2 {
    font-size: 14px;
    margin: 28px 0 6px;
    color: var(--dim);
  }
  .welcome ul {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .welcome li {
    padding: 3px 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
