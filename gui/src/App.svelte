<script lang="ts">
  import { onMount } from "svelte";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { dirname, join } from "@tauri-apps/api/path";
  import { api, type ListView, type Locations } from "./lib/api";
  import Places from "./lib/Places.svelte";
  import ToolSettings from "./lib/ToolSettings.svelte";
  import Author from "./lib/Author.svelte";
  import Guide from "./lib/Guide.svelte";
  import Install from "./lib/Install.svelte";
  import Tour from "./lib/Tour.svelte";
  import { tours } from "./lib/tours";
  import { inline, markdown } from "./lib/markdown";

  // Raw: the list is only ever replaced whole (each edit reads it again),
  // and a deep proxy of a big list made every edit re-track its every field
  // -- about 400 ms for a 700-mod list, with nothing on screen changing.
  let list = $state.raw<ListView | null>(null);
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

  // The guided tour: its opening part plays on first launch, its list part
  // the first time a list is open (once its instance is chosen); `?` plays
  // the part for what is on screen again.
  let tour = $state<"start" | "list" | null>(null);
  let toured = $state<string[] | null>(null);
  // Parts finished before the record of those already shown has loaded.
  let finished: string[] = [];
  let started = $state(false);
  onMount(async () => {
    let shown: string[] = [];
    try {
      shown = await api.toured();
    } catch {
      // Played again; no harm.
    }
    toured = [...new Set([...shown, ...finished])];
  });
  $effect(() => {
    // Not over the Settings dialog, nor before a list given at start has
    // opened.
    if (!toured || tour || !started || folders) return;
    const part = list ? (!locations || needsInstance ? null : "list") : "start";
    if (part && !toured.includes(part)) tour = part;
  });
  function startTour() {
    folders = false;
    about = false;
    tour = list ? "list" : "start";
  }
  async function endTour() {
    const part = tour;
    tour = null;
    if (!part) return;
    if (toured) {
      if (!toured.includes(part)) toured = [...toured, part];
    } else {
      finished.push(part);
    }
    try {
      await api.tourShown(part);
    } catch {
      // Shown again next time; no harm.
    }
  }
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

  // A copy of the open list, to a new file, opened in its place.
  let cloning = $state(false);
  let cloneName = $state("");
  async function cloneTo(event: SubmitEvent) {
    event.preventDefault();
    const name = cloneName.trim();
    if (!name || !list) return;
    const file = name.replace(/[^A-Za-z0-9 _-]+/g, "").trim().replace(/\s+/g, "-").toLowerCase() || "modlist";
    // Beside the list it copies.
    const path = await save({
      defaultPath: await join(await dirname(list.path), `${file}.toml`),
      filters: [{ name: "Modlist", extensions: ["toml"] }],
    });
    if (!path) return;
    error = null;
    try {
      const made = await api.cloneList(path.endsWith(".toml") ? path : `${path}.toml`, name);
      cloning = false;
      await openPath(made);
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
    try {
      recent = await api.recent();
      const start = await api.opening();
      if (start) await openPath(start);
    } finally {
      // Only now is it known whether this starts on a list or the opening
      // page, and so which part of the tour is for it.
      started = true;
    }
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
    <button class="help" data-tour="help" onclick={startTour} title="A tour of the window: what is where, and what to do">?</button>
    {#if list}<button data-tour="about" onclick={() => (about = true)}>About this list</button>{/if}
    <button data-tour="settings" onclick={() => (folders = true)} title="This list's MO2 instance and folders, and mudcrab's own: tools.toml, the Nexus key, xEdit, LOOT"
        ><svg class="cog" viewBox="0 0 24 24" aria-hidden="true"
          ><path
            d="M19.14 12.94a7.5 7.5 0 0 0 .05-.94 7.5 7.5 0 0 0-.05-.94l2.03-1.58a.5.5 0 0 0 .12-.64l-1.92-3.32a.5.5 0 0 0-.6-.22l-2.39.96a7 7 0 0 0-1.63-.94l-.36-2.54a.5.5 0 0 0-.5-.42h-3.84a.5.5 0 0 0-.49.42l-.36 2.54a7 7 0 0 0-1.63.94l-2.39-.96a.5.5 0 0 0-.6.22L2.66 8.84a.5.5 0 0 0 .12.64l2.03 1.58a7.5 7.5 0 0 0 0 1.88l-2.03 1.58a.5.5 0 0 0-.12.64l1.92 3.32a.5.5 0 0 0 .6.22l2.39-.96c.5.39 1.05.7 1.63.94l.36 2.54a.5.5 0 0 0 .49.42h3.84a.5.5 0 0 0 .5-.42l.36-2.54a7 7 0 0 0 1.63-.94l2.39.96a.5.5 0 0 0 .6-.22l1.92-3.32a.5.5 0 0 0-.12-.64zM12 15.5A3.5 3.5 0 1 1 12 8.5a3.5 3.5 0 0 1 0 7z"
          /></svg
        >Settings</button
      >
    {#if list}
      <button
        onclick={() => {
          cloneName = `${list!.document.name} (copy)`;
          cloning = true;
        }}
        disabled={opening || installing}
        title="Copy this list to a new file, with its docs, and open the copy">Clone…</button
      >
    {/if}
    <button onclick={choose} disabled={opening}>{list ? "Open another list…" : "Open a list…"}</button>
    {#if list}
      <div class="modes" role="group" aria-label="Mode" data-tour="modes">
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
      <div class="dialog" role="dialog" tabindex="-1" aria-label="Settings" onclick={(e) => e.stopPropagation()}>
        <div class="dhead">
          <strong>Settings{list ? `: ${list.document.name}` : ""}</strong>
          {#if !needsInstance}<button class="link" onclick={() => (folders = false)}>Done</button>{/if}
        </div>
        <div class="dbody">
          {#if needsInstance}
            <p class="dim small">
              Choose the Mod Organizer 2 instance this list installs into -- and, writing a list, where you try it as
              you go. Its downloads and game folders follow from it; change them after if they are not right.
            </p>
          {/if}
          <h4 class="part">This list</h4>
          <Places bind:locations disabled={installing} />
          {#if !needsInstance}
            <h4 class="part">mudcrab, for every list</h4>
            <ToolSettings disabled={installing} changed={async () => (locations = await api.locations())} />
          {/if}
          {#if needsInstance}
            <p class="dim small">
              Not this list? <button class="link" onclick={choose}>Open another</button>
            </p>
          {/if}
        </div>
      </div>
    </div>
  {/if}

  {#if !list && folders}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="veil" onclick={() => (folders = false)}>
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <div class="dialog" role="dialog" tabindex="-1" aria-label="Settings" onclick={(e) => e.stopPropagation()}>
        <div class="dhead">
          <strong>Settings</strong>
          <button class="link" onclick={() => (folders = false)}>Done</button>
        </div>
        <div class="dbody">
          <p class="dim small">mudcrab's own, for every list. A list's MO2 instance and folders are set once it is open.</p>
          <ToolSettings changed={() => {}} />
        </div>
      </div>
    </div>
  {/if}

  {#if list && cloning}
    <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div class="veil" onclick={() => (cloning = false)}>
      <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
      <div class="dialog" role="dialog" tabindex="-1" aria-label="Clone this list" onclick={(e) => e.stopPropagation()}>
        <div class="dhead">
          <strong>Clone {list.document.name}</strong>
          <button class="link" onclick={() => (cloning = false)}>Cancel</button>
        </div>
        <form class="dbody create" onsubmit={cloneTo}>
          <label for="clone-name">The copy's name, as players will see it</label>
          <div>
            <!-- svelte-ignore a11y_autofocus -->
            <input id="clone-name" type="text" bind:value={cloneName} autofocus />
            <button type="submit" class="primary" disabled={!cloneName.trim()}>Choose where to save it…</button>
          </div>
          <p class="dim small">
            A new file with every mod, question and note of this one, and its docs file copied beside it. Files it
            includes are shared, not copied. The copy opens here, with this list's MO2 instance and answers.
          </p>
          {#if error}<p class="error small">{error}</p>{/if}
        </form>
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
        <section class="left" data-tour="guide"><Guide document={list.document} /></section>
        <aside class="right"><Install {list} bind:locations openFolders={() => (folders = true)} bind:running={installing} /></aside>
      </main>
      <div class="authoring" class:hidden={mode !== "author"}><Author bind:list active={mode === "author"} bind:locations /></div>
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
        <button class="primary" data-tour="open" onclick={choose} disabled={opening}>Open a list…</button>
        <button data-tour="new" onclick={() => (creating = !creating)} disabled={opening}>Create a new list…</button>
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

  {#if tour}
    <Tour stops={tours[tour]} setMode={(m) => (mode = m)} onclose={endTour} />
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
  .part {
    margin: 4px 0 0;
    padding-bottom: 4px;
    border-bottom: 1px solid var(--line);
    font-size: 13px;
  }
  .part + :global(*) {
    margin-top: 6px;
  }
  .cog {
    width: 14px;
    height: 14px;
    fill: currentColor;
    vertical-align: -2px;
    margin-right: 5px;
  }
  .help {
    width: 28px;
    padding: 4px 0;
    border-radius: 50%;
    font-weight: 600;
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
  /* Author fills it by growing, not by `height: 100%`, which WebKitGTK does
     not resolve against a grid row. */
  .authoring {
    display: flex;
    flex-direction: column;
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
