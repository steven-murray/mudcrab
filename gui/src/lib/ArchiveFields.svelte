<script lang="ts">
  import { api, tomlString, type InputDoc } from "./api";
  import OptionalDialog, { type Fresh } from "./OptionalDialog.svelte";

  // One of a mod's archives' settings, each written as it is changed (`set
  // --mod <id> --archive N`): where it comes from, whether it is optional,
  // its layout and what that layout needs. Its file name is what the
  // download gave it, so it is shown, not changed. Filters and installer
  // choices have their own controls.
  let {
    id,
    number,
    label,
    inputs,
    run,
  }: {
    id: string;
    number: number;
    label: string;
    inputs: InputDoc[];
    /** Run an edit; resolves true when written. */
    run: (edit: () => Promise<unknown>) => Promise<boolean>;
  } = $props();

  type Value = string | number | boolean | (string | number | boolean)[];
  let fields = $state<Map<string, Value>>(new Map());
  let error = $state<string | null>(null);
  async function load() {
    try {
      fields = new Map(await api.fields(id, number));
      error = null;
    } catch (err) {
      error = String(err);
    }
  }
  $effect(() => {
    void id;
    void number;
    load();
  });

  const text = (key: string) => {
    const value = fields.get(key);
    return value === undefined ? "" : Array.isArray(value) ? value.join("\n") : String(value);
  };
  // A custom data folder is written with its folder, once that is given.
  let wanted = $state<string | null>(null);
  const layout = $derived(wanted ?? text("layout"));
  const source = $derived(text("path"));
  const byHand = $derived(!source.startsWith("nexus:"));

  // A value written, or (empty) the field taken out; with `also`, other
  // values in the same edit. Refused, the form shows what is written again.
  let shown = $state(0);
  async function save(key: string, value: string | string[], also: string[] = []) {
    const empty = Array.isArray(value) ? !value.length : !value.trim();
    if (empty && !fields.has(key) && !also.length) return;
    const written = Array.isArray(value) ? `[${value.map(tomlString).join(", ")}]` : tomlString(value.trim());
    const sets = [...(empty ? [] : [`${key}=${written}`]), ...also];
    const done = await run(() => api.setFields(id, number, sets, empty ? [key] : []));
    await load();
    if (done) wanted = null;
    else shown += 1;
  }
  function chooseLayout(value: string) {
    if (value === "custom-data-folder" && !fields.has("data_folder")) wanted = value;
    else {
      wanted = null;
      save("layout", value);
    }
  }
  function saveDataFolder(value: string) {
    if (wanted && value.trim()) save("data_folder", value, [`layout=${tomlString(wanted)}`]);
    else if (!wanted) save("data_folder", value);
  }
  const lines = (value: string) =>
    value
      .split("\n")
      .map((l) => l.trim())
      .filter(Boolean);

  let optional = $state(false);
  async function setCondition(condition: string, fresh: Fresh | null): Promise<boolean> {
    if (fresh && !(await run(() => api.addInput(fresh.name, fresh.kind, fresh.query, fresh.choices, fresh.fallback)))) return false;
    const done = await run(() => api.setFields(id, number, [`if=${tomlString(condition)}`], []));
    if (done) await load();
    return done;
  }

  const layouts: [string, string][] = [
    ["", "Automatic (the default): finds the data folder in the archive"],
    ["simple", "Simple: the archive's top is the data folder"],
    ["custom-data-folder", "A folder in the archive is the data folder"],
    ["fomod", "FOMOD installer"],
    ["bain", "BAIN packages"],
  ];
</script>

{#if error}<pre class="error">{error}</pre>{/if}
{#key shown}
<div class="fields">
  <label class="row" title="nexus:oblivion/<mod>/<file>, a URL, or manual:<file name>">
    <span class="name">Source</span>
    <input type="text" value={source} onchange={(e) => save("path", e.currentTarget.value)} />
  </label>
  <div class="row" title="The archive's own name, as downloaded">
    <span class="name">File name</span>
    <span class="value">{text("file_name") || label}</span>
  </div>
  <div class="row">
    <span class="name">Optional</span>
    <span>
      {#if fields.has("if")}
        Installed only if <code>{text("if")}</code>
        <button class="link" onclick={() => (optional = true)}>Change…</button>
        · <button class="link" onclick={() => save("if", "")}>Always install it</button>
      {:else}
        <button class="small" onclick={() => (optional = true)}>Make this archive optional…</button>
      {/if}
    </span>
  </div>
  <label class="row">
    <span class="name">Layout</span>
    <select value={layout} onchange={(e) => chooseLayout(e.currentTarget.value)}>
      {#each layouts as [value, title]}<option {value}>{title}</option>{/each}
    </select>
  </label>
  {#if layout === "custom-data-folder" || fields.has("data_folder")}
    <label class="row" title="The folder in the archive that is the data folder">
      <span class="name">Data folder</span>
      <!-- svelte-ignore a11y_autofocus -->
      <input
        type="text"
        value={text("data_folder")}
        placeholder="e.g. Data"
        autofocus={!!wanted}
        onchange={(e) => saveDataFolder(e.currentTarget.value)}
      />
    </label>
    {#if wanted}<p class="dim small hint">Name the folder to use this layout.</p>{/if}
  {/if}
  <label class="row" title="A folder in the mod to put everything from this archive under">
    <span class="name">Into folder</span>
    <input type="text" value={text("target_subdir")} placeholder="the top of the mod" onchange={(e) => save("target_subdir", e.currentTarget.value)} />
  </label>
  <label class="row" title="Files for the game's own folder, beside Oblivion.exe, one per line">
    <span class="name">Game folder files</span>
    <textarea rows="2" value={text("game_root_files")} placeholder="none; one per line" onchange={(e) => save("game_root_files", lines(e.currentTarget.value))}
    ></textarea>
  </label>
  {#if byHand || fields.has("download_page") || fields.has("download_note")}
    <label class="row" title="Where to get it by hand, printed by download and check">
      <span class="name">Download page</span>
      <input type="text" value={text("download_page")} onchange={(e) => save("download_page", e.currentTarget.value)} />
    </label>
    <label class="row" title="Which file on the page, and how">
      <span class="name">Download note</span>
      <input type="text" value={text("download_note")} onchange={(e) => save("download_note", e.currentTarget.value)} />
    </label>
  {/if}
  <label class="row" title="An archive inside this one to install instead">
    <span class="name">Inner archive</span>
    <input type="text" value={text("inner_archive")} placeholder="none" onchange={(e) => save("inner_archive", e.currentTarget.value)} />
  </label>
</div>
{/key}

{#if optional}
  <OptionalDialog
    {id}
    title="When is {label} installed?"
    {inputs}
    current={fields.has("if") ? text("if") : null}
    save={setCondition}
    close={() => (optional = false)}
  />
{/if}

<style>
  .fields {
    font-size: 13px;
    margin: 4px 0;
  }
  .row {
    display: grid;
    grid-template-columns: 120px minmax(0, 1fr);
    gap: 8px;
    align-items: baseline;
    margin: 4px 0;
  }
  .name {
    color: var(--dim);
  }
  .value {
    overflow-wrap: anywhere;
  }
  input[type="text"],
  select,
  textarea {
    font-size: 12px;
    min-width: 0;
  }
  textarea {
    font: inherit;
    font-size: 12px;
    color: inherit;
    background: var(--card);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 4px 8px;
    resize: vertical;
  }
  code {
    font-size: 11px;
    overflow-wrap: anywhere;
  }
  .small {
    font-size: 12px;
  }
  .hint {
    margin: 0 0 4px 128px;
  }
  .error {
    color: var(--problem);
    white-space: pre-wrap;
    font-family: inherit;
    font-size: 12px;
  }
</style>
