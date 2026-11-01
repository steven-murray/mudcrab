<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import { api, type ToolSettings } from "./api";

  // mudcrab's own settings, shared by every list: the tools.toml in use and
  // what is in it -- the usual downloads folder, the Nexus API key, and
  // where xEdit, LOOT and (off Windows) Wine or Proton are. The command line
  // reads the same file.
  let {
    disabled = false,
    changed,
  }: {
    /** While an install runs, nothing here may change under it. */
    disabled?: boolean;
    /** After a change: what follows from tools.toml (a list's downloads folder) is read again. */
    changed: () => void;
  } = $props();

  let tools = $state<ToolSettings | null>(null);
  let error = $state<string | null>(null);
  onMount(async () => {
    try {
      tools = await api.toolSettings();
    } catch (err) {
      error = String(err);
    }
  });

  type Setting = { key: string; title: string; hint: string; pick: "folder" | "file" | "none"; wine?: boolean };
  const groups: { title: string; settings: Setting[] }[] = [
    {
      title: "Downloads",
      settings: [
        {
          key: "downloads_dir",
          title: "Usual downloads folder",
          hint: "Where every list keeps its archives, unless a list says otherwise; else each MO2 instance's own",
          pick: "folder",
        },
      ],
    },
    {
      title: "xEdit, for cleaning plugins",
      settings: [
        { key: "tes4edit.exe", title: "TES4Edit", hint: "TES4Edit.exe (xEdit for Oblivion)", pick: "file" },
        {
          key: "tes4edit.qac_exe",
          title: "Quick Auto Clean",
          hint: "TES4EditQuickAutoClean.exe; unset, the one beside TES4Edit.exe is used",
          pick: "file",
        },
      ],
    },
    {
      title: "LOOT",
      settings: [
        { key: "loot.exe", title: "LOOT", hint: "Unset, LOOT is run from the PATH", pick: "file" },
        {
          key: "loot.game_appdata_path",
          title: "The game's AppData",
          hint: "Where LOOT reads Oblivion's plugins.txt (under the Proton prefix, off Windows)",
          pick: "folder",
        },
      ],
    },
    {
      title: "Wine or Proton, for Windows tools",
      settings: [
        { key: "wine.prefix", title: "Prefix", hint: "The Wine prefix: the folder with drive_c in it", pick: "folder", wine: true },
        { key: "wine.proton", title: "Proton", hint: "A proton executable, to run tools with instead of wine", pick: "file", wine: true },
      ],
    },
  ];

  async function set(key: string, value: string | null) {
    try {
      tools = await api.setTool(key, value);
      error = null;
      changed();
    } catch (err) {
      error = String(err);
    }
  }
  async function choose(setting: Setting) {
    const path = await open({ directory: setting.pick === "folder", multiple: false });
    if (typeof path === "string") await set(setting.key, path);
  }
  async function chooseFile() {
    const path = await open({ multiple: false, filters: [{ name: "tools.toml", extensions: ["toml"] }] });
    if (typeof path !== "string") return;
    try {
      tools = await api.setToolsConfig(path);
      error = null;
      changed();
    } catch (err) {
      error = String(err);
    }
  }
  async function resetFile() {
    tools = await api.setToolsConfig(null);
    changed();
  }

  let key = $state("");
</script>

{#if error}<div class="perror">{error}</div>{/if}
{#if tools}
  <div class="place">
    <div class="ptitle">tools.toml</div>
    {#if tools.path}<div class="ppath" class:missing={!tools.exists}>{tools.path}</div>{/if}
    <div class="pfrom dim">
      {tools.exists ? "" : "not there yet · "}{tools.from === "chosen" ? "chosen here" : tools.from === "usual" ? "the usual one" : `from ${tools.from}`}
      {#if tools.from === "chosen"}· <button class="link" onclick={resetFile} {disabled}>reset</button>{/if}
    </div>
    <button class="link" onclick={chooseFile} {disabled}>Change…</button>
  </div>
  {#if tools.error}<div class="perror">{tools.error}</div>{/if}

  <div class="place sub">
    <div class="ptitle">Nexus API key</div>
    <div class="pfrom dim">
      {#if tools.key_from_env}
        Set, from NEXUS_API_KEY
      {:else if tools.values["nexus_api_key"]}
        Set · <button class="link" onclick={() => set("nexus_api_key", null)} {disabled}>remove</button>
      {:else}
        Not set. With one, mods are added from Nexus by name rather than as hand downloads, and archives are
        downloaded for you. Yours is on <a href="https://www.nexusmods.com/settings/api-keys">Nexus's API keys page</a>.
      {/if}
    </div>
  </div>
  {#if !tools.key_from_env}
    <form
      class="key"
      onsubmit={async (e) => {
        e.preventDefault();
        if (key.trim()) {
          await set("nexus_api_key", key.trim());
          key = "";
        }
      }}
    >
      <input type="password" bind:value={key} placeholder={tools.values["nexus_api_key"] ? "a new key" : "paste your key"} autocomplete="off" {disabled} />
      <button type="submit" disabled={disabled || !key.trim()}>Save</button>
    </form>
  {/if}

  {#each groups as group}
    {@const shown = group.settings.filter((s) => !s.wine || tools!.wine)}
    {#if shown.length}
      <div class="group">{group.title}</div>
      {#each shown as setting}
        {@const value = tools.values[setting.key]}
        <div class="place sub" title={setting.hint}>
          <div class="ptitle">{setting.title}</div>
          {#if value}
            <div class="ppath">{value}</div>
            <div class="pfrom dim"><button class="link" onclick={() => set(setting.key, null)} {disabled}>unset</button></div>
          {:else}
            <div class="pfrom dim">{setting.hint}</div>
          {/if}
          <button class="link" onclick={() => choose(setting)} {disabled}>{value ? "Change…" : "Choose…"}</button>
        </div>
      {/each}
      {#if group.settings.some((s) => s.key.startsWith("tes4edit")) && tools.xedit_problems.length}
        <ul class="problems">{#each tools.xedit_problems as problem}<li>{problem}</li>{/each}</ul>
      {/if}
    {/if}
  {/each}
{:else if !error}
  <p class="dim">Reading tools.toml…</p>
{/if}

<style>
  .place {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    gap: 0 10px;
    margin: 8px 0;
  }
  .place.sub {
    margin-left: 12px;
  }
  .ptitle {
    font-weight: 600;
    grid-column: 1;
  }
  .ppath {
    grid-column: 1;
    font-family: var(--mono);
    font-size: 12px;
    overflow-wrap: anywhere;
    user-select: text;
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
  .pfrom button {
    grid-column: auto;
    grid-row: auto;
  }
  .group {
    margin: 14px 0 2px;
    font-size: 12px;
    color: var(--dim);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .key {
    display: flex;
    gap: 6px;
    margin: 2px 0 8px 12px;
  }
  .key input {
    flex: 1;
    min-width: 0;
    font-size: 12px;
  }
  .key button {
    font-size: 12px;
  }
  .problems {
    margin: 2px 0 8px 12px;
    padding-left: 18px;
    font-size: 12px;
    color: var(--problem);
    overflow-wrap: anywhere;
  }
  .perror {
    color: var(--problem);
    font-size: 12px;
    margin: 4px 0 8px;
  }
</style>
