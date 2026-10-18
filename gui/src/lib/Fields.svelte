<script lang="ts">
  import { api, tomlString } from "./api";

  // A mod's fields, or one of its archives', as a form: each one `set`
  // changes (`mudcrab set --mod <id> [--archive N] key=value --unset key`).
  // What has a command of its own -- the id, section, plugins, filters,
  // actions, installer choices -- is changed there instead.
  let {
    id,
    archive,
    save,
  }: {
    id: string;
    /** An archive's number (from 1), or null for the mod itself. */
    archive: number | null;
    /** Write the changes; resolves true when written. */
    save: (sets: string[], unsets: string[]) => Promise<boolean>;
  } = $props();

  type Kind = "text" | "bool" | "list" | { choose: string[] };
  type Field = { key: string; label: string; kind: Kind; hint: string };

  const MOD: Field[] = [
    { key: "if", label: "Only if", kind: "text", hint: "A condition on the list's questions; the mod is left out when it is false." },
    { key: "enabled", label: "Ticked in MO2", kind: "bool", hint: "Unticked: installed, but switched off in MO2 until the player ticks it." },
    { key: "dependencies", label: "Needs", kind: "list", hint: "Mods it needs, by id, one per line." },
    { key: "oracle_name", label: "Reference name", kind: "text", hint: "Its folder name in a reference instance, for `diff`, when that differs from the id." },
  ];
  const ARCHIVE: Field[] = [
    { key: "path", label: "Source", kind: "text", hint: "nexus:oblivion/<mod>/<file>, a URL, or manual:<file name>." },
    { key: "file_name", label: "File name", kind: "text", hint: "The archive's own name, as downloaded." },
    { key: "if", label: "Only if", kind: "text", hint: "A condition; the archive is left out when it is false." },
    {
      key: "layout",
      label: "Layout",
      kind: { choose: ["simple", "fomod", "bain", "custom-data-folder"] },
      hint: "How it is laid out; unset, it is worked out from the archive.",
    },
    { key: "data_folder", label: "Data folder", kind: "text", hint: "With custom-data-folder: the folder in the archive that is the data folder." },
    { key: "target_subdir", label: "Into folder", kind: "text", hint: "A folder in the mod to put everything under." },
    { key: "inner_archive", label: "Inner archive", kind: "text", hint: "An archive inside this one to install instead." },
    { key: "game_root_files", label: "Game folder files", kind: "list", hint: "Files for the game's own folder, beside Oblivion.exe, one per line." },
    { key: "download_page", label: "Download page", kind: "text", hint: "Where to get it by hand, printed by download and check." },
    { key: "download_note", label: "Download note", kind: "text", hint: "Which file on the page, and how." },
  ];
  const known = $derived(archive === null ? MOD : ARCHIVE);

  type Value = string | number | boolean | (string | number | boolean)[];
  let current = $state<Map<string, Value>>(new Map());
  let draft = $state<Record<string, string>>({});
  let present = $state<Record<string, boolean>>({});
  let error = $state<string | null>(null);

  // The field's value as the form edits it: text, "true"/"false", or lines.
  function shown(value: Value | undefined, kind: Kind): string {
    if (value === undefined) return kind === "bool" ? "true" : "";
    if (Array.isArray(value)) return value.join("\n");
    return String(value);
  }

  async function load() {
    error = null;
    try {
      current = new Map(await api.fields(id, archive));
      draft = Object.fromEntries(known.map((f) => [f.key, shown(current.get(f.key), f.kind)]));
      present = Object.fromEntries(known.map((f) => [f.key, current.has(f.key)]));
    } catch (err) {
      error = String(err);
    }
  }
  $effect(() => {
    void id;
    void archive;
    load();
  });

  function encode(field: Field, text: string): string {
    if (field.kind === "bool") return text === "true" ? "true" : "false";
    if (field.kind === "list")
      return `[${text
        .split("\n")
        .map((l) => l.trim())
        .filter(Boolean)
        .map(tomlString)
        .join(", ")}]`;
    return tomlString(text);
  }

  // What changed: a field set where it was absent or different, and one
  // taken out where it was there.
  const changes = $derived.by(() => {
    const sets: string[] = [];
    const unsets: string[] = [];
    for (const field of known) {
      const had = current.has(field.key);
      if (!present[field.key]) {
        if (had) unsets.push(field.key);
        continue;
      }
      if (!had || draft[field.key] !== shown(current.get(field.key), field.kind))
        sets.push(`${field.key}=${encode(field, draft[field.key])}`);
    }
    return { sets, unsets };
  });
  const changed = $derived(changes.sets.length + changes.unsets.length > 0);

  async function write() {
    if (await save(changes.sets, changes.unsets)) await load();
  }

  // The fields this form does not know, shown as they are.
  const others = $derived([...current.entries()].filter(([key]) => key !== "id" && !known.some((f) => f.key === key)));
</script>

<div class="fields">
  {#if error}<pre class="error">{error}</pre>{/if}
  {#each known as field (field.key)}
    <div class="row" title={field.hint}>
      <label class="name"><input type="checkbox" bind:checked={present[field.key]} title="Whether the field is set" /> {field.label}</label>
      {#if field.kind === "bool"}
        <select bind:value={draft[field.key]} disabled={!present[field.key]}><option>true</option><option>false</option></select>
      {:else if field.kind === "list"}
        <textarea rows="2" bind:value={draft[field.key]} disabled={!present[field.key]} placeholder="one per line"></textarea>
      {:else if typeof field.kind === "object"}
        <select bind:value={draft[field.key]} disabled={!present[field.key]}>
          {#each field.kind.choose as option}<option>{option}</option>{/each}
        </select>
      {:else}
        <input type="text" bind:value={draft[field.key]} disabled={!present[field.key]} />
      {/if}
    </div>
  {/each}
  {#if others.length}
    <div class="others dim">
      Also set, changed by their own commands or in the file:
      {#each others as [key, value]}<code title={JSON.stringify(value)}>{key}</code>{/each}
    </div>
  {/if}
  <div class="buttons">
    <button class="primary" onclick={write} disabled={!changed}>Save</button>
    <button onclick={load} disabled={!changed}>Revert</button>
    {#if changed}<code class="dim small">{[...changes.sets, ...changes.unsets.map((k) => `--unset ${k}`)].join(" ")}</code>{/if}
  </div>
</div>

<style>
  .fields {
    font-size: 13px;
  }
  .row {
    display: grid;
    grid-template-columns: 150px minmax(0, 1fr);
    gap: 8px;
    align-items: center;
    margin: 3px 0;
  }
  .name {
    display: flex;
    gap: 6px;
    align-items: center;
    color: var(--dim);
  }
  textarea {
    font: inherit;
    color: inherit;
    background: var(--card);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 4px 8px;
    resize: vertical;
  }
  input:disabled,
  select:disabled,
  textarea:disabled {
    opacity: 0.5;
  }
  .others {
    font-size: 12px;
    margin: 6px 0;
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .buttons {
    display: flex;
    gap: 6px;
    align-items: center;
    margin-top: 6px;
    flex-wrap: wrap;
  }
  .small {
    font-size: 11px;
    overflow-wrap: anywhere;
  }
  .error {
    color: var(--problem);
    white-space: pre-wrap;
    font-family: inherit;
  }
</style>
