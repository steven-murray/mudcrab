<script lang="ts">
  import { onMount } from "svelte";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { api, type ListView, type Locations } from "./lib/api";
  import Places from "./lib/Places.svelte";
  import Author from "./lib/Author.svelte";
  import Guide from "./lib/Guide.svelte";
  import Install from "./lib/Install.svelte";
  import { inline, markdown } from "./lib/markdown";

  let list = $state<ListView | null>(null);
  let recent = $state<string[]>([]);
  let error = $state<string | null>(null);
  let opening = $state(false);
  let about = $state(false);
  let mode = $state<"install" | "author">("install");

  // Where things go -- the MO2 instance, downloads, game, tools -- shared by
  // both modes. Set in the Folders dialog, which opens by itself for a list
  // with no MO2 instance chosen yet and stays until one is: installing and
  // authoring both need it.
  let locations = $state<Locations | null>(null);
  let folders = $state(false);
  let installing = $state(false);
  const needsInstance = $derived(!!list && !!locations && !locations.instance.path);

  async function openPath(path: string) {
    opening = true;
    error = null;
    try {
      list = await api.openList(path);
      locations = await api.locations();
      folders = !locations.instance.path;
    } catch (err) {
      error = String(err);
    } finally {
      opening = false;
      recent = await api.recent();
    }
  }

  // A new list: a name, then where to keep it; opened in author mode, where
  // its mods are added.
  let creating = $state(false);
  let newName = $state("");
  async function create(event: SubmitEvent) {
    event.preventDefault();
    const name = newName.trim();
    if (!name) return;
    const file = name.replace(/[^A-Za-z0-9 _-]+/g, "").trim().replace(/\s+/g, "-").toLowerCase() || "modlist";
    const path = await save({
      defaultPath: `${file}.toml`,
      filters: [{ name: "Modlist", extensions: ["toml"] }],
    });
    if (!path) return;
    error = null;
    try {
      const made = await api.newList(path.endsWith(".toml") ? path : `${path}.toml`, name);
      creating = false;
      newName = "";
      await openPath(made);
      mode = "author";
    } catch (err) {
      error = String(err);
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
    {#if list}<button onclick={() => (about = true)}>About this list</button>{/if}
    {#if list}
      <button onclick={() => (folders = true)} title="The MO2 instance, downloads and game folders, and tools.toml, for this list"
        >Folders</button
      >
    {/if}
    <button onclick={choose} disabled={opening}>{list ? "Open another list…" : "Open a list…"}</button>
    {#if list}
      <div class="modes" role="group" aria-label="Mode">
        <button class="mode" class:on={mode === "install"} onclick={() => (mode = "install")}>Install</button>
        <button
          class="mode"
          class:on={mode === "author"}
          onclick={() => (mode = "author")}
          title="Look into the list as its author: each mod's files, the load order with masters, conflicts, check"
          >Author</button
        >
      </div>
    {/if}
  </header>

  {#if error}
    <div class="error">
      <pre>{error}</pre>
      <button class="link" onclick={() => (error = null)}>Dismiss</button>
    </div>
  {/if}

  {#if list && locations && (folders || needsInstance)}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="veil" onclick={() => !needsInstance && (folders = false)}>
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <div class="dialog" role="dialog" tabindex="-1" aria-label="Folders" onclick={(e) => e.stopPropagation()}>
        <div class="dhead">
          <strong>Where things go{list ? `: ${list.document.name}` : ""}</strong>
          {#if !needsInstance}<button class="link" onclick={() => (folders = false)}>Done</button>{/if}
        </div>
        <div class="dbody">
          {#if needsInstance}
            <p class="dim small">
              Choose the Mod Organizer 2 instance this list installs into -- and, writing a list, where you try it as
              you go. Its downloads and game folders follow from it; change them after if they are not right.
            </p>
          {/if}
          <Places bind:locations disabled={installing} />
          {#if needsInstance}
            <p class="dim small">
              Not this list? <button class="link" onclick={choose}>Open another</button>
            </p>
          {/if}
        </div>
      </div>
    </div>
  {/if}

  {#if list && about}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="veil" onclick={() => (about = false)}>
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <div class="dialog" role="dialog" tabindex="-1" aria-label="About this list" onclick={(e) => e.stopPropagation()}>
        <div class="dhead">
          <strong>{list.document.name}</strong>
          <button class="link" onclick={() => (about = false)}>Close</button>
        </div>
        <div class="dbody">
          {#if list.about}
            <div class="prose">{@html markdown(list.about)}</div>
          {:else}
            <p class="dim small">
              This list has no introduction for installing it. This is its guide's, written for
              someone following it by hand, so some of it is done for you here.
            </p>
            {#if list.document.docs.summary}<p>{@html inline(list.document.docs.summary)}</p>{/if}
            <div class="prose">{@html markdown(list.document.docs.body)}</div>
          {/if}
        </div>
      </div>
    </div>
  {/if}

  {#if list}
    {#key list.path}
      <!-- Both stay mounted, so switching modes keeps an install's progress. -->
      <main class:hidden={mode !== "install"}>
        <section class="left"><Guide document={list.document} /></section>
        <aside class="right"><Install {list} {locations} openFolders={() => (folders = true)} bind:running={installing} /></aside>
      </main>
      <div class="authoring" class:hidden={mode !== "author"}><Author bind:list active={mode === "author"} {locations} /></div>
    {/key}
  {:else}
    <div class="welcome">
      <h1>Install a modlist</h1>
      <p class="dim">
        Open a list's <code>.toml</code> file to read it, answer its questions and install it into a
        Mod Organizer 2 instance -- or start a list of your own. Every step is one of mudcrab's
        commands, shown as it runs.
      </p>
      <div class="starts">
        <button class="primary" onclick={choose} disabled={opening}>Open a list…</button>
        <button onclick={() => (creating = !creating)} disabled={opening}>Create a new list…</button>
      </div>
      {#if creating}
        <form class="create" onsubmit={create}>
          <label for="new-name">Its name, as players will see it</label>
          <div>
            <!-- svelte-ignore a11y_autofocus -->
            <input id="new-name" type="text" bind:value={newName} placeholder="My Oblivion list" autofocus />
            <button type="submit" class="primary" disabled={!newName.trim()}>Choose where to save it…</button>
          </div>
          <p class="dim small">It starts empty, and opens in author mode, where mods are added from your downloads or a Nexus link.</p>
        </form>
      {/if}
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
    border: none;
    border-radius: 0;
    background: none;
    color: var(--dim);
  }
  .mode.on {
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
  }
  /* Both views in the last row, the one that fills the window, whether or
     not an error is shown above: placed by order, a view would land in the
     error's row when there is none, and be only as tall as its content. */
  main,
  .authoring {
    grid-row: 3;
    min-height: 0;
  }
  .hidden {
    display: none !important;
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
  .veil {
    position: fixed;
    inset: 0;
    z-index: 10;
    background: rgba(0, 0, 0, 0.45);
    display: grid;
    place-items: center;
  }
  .dialog {
    width: min(760px, 92vw);
    max-height: 82vh;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 10px;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.45);
  }
  .dhead {
    display: flex;
    justify-content: space-between;
    padding: 12px 16px;
    border-bottom: 1px solid var(--line);
  }
  .dbody {
    padding: 14px 18px 18px;
    overflow: auto;
  }
  .small {
    font-size: 12px;
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
  .starts {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .create {
    margin-top: 14px;
    padding: 12px;
    border: 1px solid var(--line);
    border-radius: 8px;
    background: var(--panel);
  }
  .create label {
    display: block;
    font-size: 13px;
    margin-bottom: 6px;
  }
  .create div {
    display: flex;
    gap: 6px;
  }
  .create input {
    flex: 1;
    min-width: 0;
  }
  .create p {
    margin: 8px 0 0;
  }
  .welcome li {
    padding: 3px 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
