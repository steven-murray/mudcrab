<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api, type Locations } from "./api";

  // Where things go, for the open list: the MO2 instance first, then the
  // folders that follow from it (downloads, game), other folders to look in
  // for archives, and the tools.toml in use. Installing and authoring both
  // need them, so they are set here, in the Folders dialog, for both.
  let {
    locations = $bindable(),
    disabled = false,
  }: {
    locations: Locations;
    /** While an install runs, nothing here may change under it. */
    disabled?: boolean;
  } = $props();

  const places: { kind: "downloads" | "game"; title: string; hint: string }[] = [
    { kind: "downloads", title: "Downloads folder", hint: "Where the archives are kept" },
    { kind: "game", title: "Game folder", hint: "Oblivion's own folder" },
  ];

  const instance = $derived(locations.instance);

  let placeError = $state<string | null>(null);
  async function choosePlace(kind: string) {
    const path = await open({ directory: true, multiple: false });
    if (typeof path !== "string") return;
    try {
      locations = await api.setLocation(kind, path);
      placeError = null;
    } catch (err) {
      placeError = String(err);
    }
  }
  async function resetPlace(kind: string) {
    locations = await api.setLocation(kind, null);
  }
  async function chooseTools() {
    const path = await open({ multiple: false, filters: [{ name: "tools.toml", extensions: ["toml"] }] });
    if (typeof path !== "string") return;
    try {
      locations = await api.setToolsConfig(path);
      placeError = null;
    } catch (err) {
      placeError = String(err);
    }
  }
  async function resetTools() {
    locations = await api.setToolsConfig(null);
  }
  async function addSearch() {
    const path = await open({ directory: true, multiple: false });
    if (typeof path === "string") locations = await api.addSearchFolder(path);
  }
  async function removeSearch(path: string) {
    locations = await api.removeSearchFolder(path);
  }
</script>

<!-- The instance first: the other folders follow from it. -->
  <div class="place">
    <div class="ptitle">MO2 instance</div>
    {#if instance.path}
      <div class="ppath" class:missing={!instance.exists}>{instance.path}</div>
    {:else}
      <div class="pfrom dim">
        The portable Mod Organizer 2 instance to install into and test with: the folder with
        ModOrganizer.exe in it.
      </div>
    {/if}
    <button class={instance.path ? "" : "primary"} onclick={() => choosePlace("instance")} {disabled}>
      {instance.path ? "Change…" : "Choose…"}
    </button>
  </div>
  {#if placeError}<div class="perror">{placeError}</div>{/if}

  {#if instance.path}
    {#each places as place}
      {@const loc = locations[place.kind]}
      <div class="place sub">
        <div class="ptitle">{place.title}</div>
        {#if loc.path}
          <div class="ppath" class:missing={!loc.exists}>{loc.path}</div>
          <div class="pfrom dim">
            {loc.exists ? "" : "not there yet · "}from {loc.from}
            {#if loc.from === "chosen"}· <button class="link" onclick={() => resetPlace(place.kind)} {disabled}>reset</button>{/if}
          </div>
        {:else}
          <div class="pfrom dim">{place.hint}</div>
        {/if}
        <button class="link" onclick={() => choosePlace(place.kind)} {disabled}>Change…</button>
      </div>
    {/each}
    <div class="place sub">
      <div class="ptitle">Other folders to look in</div>
      <div class="pfrom dim">Archives already in these are linked into the downloads folder, not downloaded again.</div>
      <button class="link" onclick={addSearch} {disabled}>Add…</button>
    </div>
    {#each locations.search as folder}
      <div class="search">
        <span class="ppath" class:missing={!folder.exists}>{folder.path}</span>
        <button class="link small" onclick={() => removeSearch(folder.path!)} {disabled}>Remove</button>
      </div>
    {/each}
    {@const tools = locations.tools}
    <div class="place sub">
      <div class="ptitle">Tools (tools.toml)</div>
      {#if tools.path}<div class="ppath" class:missing={!tools.exists}>{tools.path}</div>{/if}
      <div class="pfrom dim">
        {tools.exists ? "" : "not there · "}{tools.from === "chosen" ? "chosen here" : tools.from === "usual" ? "the usual one" : `from ${tools.from}`}
        {#if tools.from === "chosen"}· <button class="link" onclick={resetTools} {disabled}>reset</button>{/if}
      </div>
      <button class="link" onclick={chooseTools} {disabled}>Change…</button>
    </div>
    {#if tools.problems.length}
      <div class="toolsbad">
        <div>This list cleans plugins with xEdit, and this tools.toml is not set up for it:</div>
        <ul>{#each tools.problems as problem}<li>{problem}</li>{/each}</ul>
        <div class="dim">Run <code>mudcrab setup-tools</code>, edit the file, or choose another.</div>
      </div>
    {/if}
    <div class="dim small">Profile: {locations.profile}</div>
  {/if}

<style>
  .small {
    font-size: 12px;
  }
  .place {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 0 10px;
    margin: 8px 0;
  }
  .ptitle {
    font-weight: 600;
    grid-column: 1;
  }
  .ppath {
    grid-column: 1;
    font-family: var(--mono);
    font-size: 12px;
    /* Whole, wrapped wherever it must be: a path cut short is no use. */
    overflow-wrap: anywhere;
    user-select: text;
  }
  .place.sub {
    margin-left: 12px;
  }
  .toolsbad {
    border: 1px solid var(--problem);
    border-radius: 6px;
    padding: 6px 10px;
    margin: 4px 0 8px 12px;
    font-size: 12px;
  }
  .toolsbad ul {
    margin: 4px 0;
    padding-left: 18px;
    color: var(--problem);
    overflow-wrap: anywhere;
  }
  .perror {
    color: var(--problem);
    font-size: 12px;
    margin: -4px 0 8px;
  }
  .ppath.missing {
    color: var(--warning);
  }
  .pfrom {
    grid-column: 1;
    font-size: 12px;
  }
  .place button {
    grid-column: 2;
    grid-row: 1 / span 3;
    align-self: center;
  }
  .search {
    display: flex;
    gap: 10px;
    align-items: baseline;
    margin: 2px 0 4px 12px;
  }
  .search .ppath {
    flex: 1;
    min-width: 0;
  }
</style>
