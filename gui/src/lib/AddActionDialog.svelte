<script lang="ts">
  import { api, type Edited, type InputDoc } from "./api";
  import ConditionPicker from "./ConditionPicker.svelte";
  import type { Archive } from "./ModFiles.svelte";

  // An action added to a mod (`action add`): its fields as a form, and when
  // it applies. The INI and file actions with editors of their own (the INI
  // editor, and the Files and Conflicts views) are not offered. The list's checks run on the result, so a field left wrong
  // is refused with the reason, and nothing is written.
  let {
    id,
    archives,
    inputs,
    known,
    run,
    close,
  }: {
    id: string;
    /** The mod's archives and what each installs, to suggest paths. */
    archives: Archive[];
    inputs: InputDoc[];
    /** Conditions the list's actions already use. */
    known: string[];
    /** Write an edit; resolves to why it was not written, or null. */
    run: (edit: () => Promise<Edited | unknown>) => Promise<string | null>;
    close: () => void;
  } = $props();

  type Kind = "text" | "lines" | "list" | "bool" | "choice";
  type Field = { name: string; kind: Kind; required?: boolean; hint?: string; choices?: string[]; suggest?: "files" | "plugins" | "bsas" };
  type Spec = { action: string; what: string; fields: Field[] };

  const specs: Spec[] = [
    {
      action: "qac",
      what: "Clean plugins with xEdit's Quick Auto Clean (needs xEdit in tools.toml).",
      fields: [{ name: "plugins", kind: "list", required: true, hint: "Plugins to clean, one per line (globs allowed)", suggest: "plugins" }],
    },
    {
      action: "delete_records",
      what: 'Remove records with xEdit, as a guide\'s "open this in xEdit and remove that".',
      fields: [
        { name: "plugin", kind: "text", required: true, suggest: "plugins" },
        { name: "groups", kind: "list", hint: "Whole groups, one per line (WRLD)" },
        { name: "form_ids", kind: "list", hint: "Records, one per line (xx010F43)" },
      ],
    },
    {
      action: "write_file",
      what: "Add a file no archive has, such as a dxvk.conf; never overwrites one.",
      fields: [
        { name: "path", kind: "text", required: true },
        { name: "content", kind: "lines", required: true },
      ],
    },
    {
      action: "pack_bsa",
      what: "Pack loose files into a BSA.",
      fields: [
        { name: "output", kind: "text", required: true, hint: "Example Mod.bsa" },
        { name: "include", kind: "list", hint: "Globs to pack, one per line (all, if none)" },
        { name: "exclude", kind: "list", hint: "Globs to leave loose, one per line" },
        { name: "keep_loose", kind: "bool", hint: "Keep the loose files it packs (they are deleted otherwise)" },
      ],
    },
    {
      action: "create_dummy_plugin",
      what: "An empty plugin for a BSA with none of the same name, so Oblivion loads it.",
      fields: [{ name: "output", kind: "text", required: true, hint: "Example Mod.esp" }],
    },
    {
      action: "extract_bsa",
      what: "Unpack a BSA the mod ships.",
      fields: [
        { name: "archive", kind: "text", required: true, suggest: "bsas" },
        { name: "keep_archive", kind: "bool", hint: "Keep the BSA after unpacking it" },
      ],
    },
    {
      action: "ini_tweak",
      what: "A Wrye Bash INI Tweak, as a BAIN wizard's EditINI writes: a fragment under ini tweaks/, applied from Wrye Bash.",
      fields: [
        { name: "file", kind: "text", required: true },
        { name: "key", kind: "text", required: true },
        { name: "value", kind: "text", required: true },
        { name: "format", kind: "choice", choices: ["standard", "set-to"] },
      ],
    },
    {
      action: "dds_decompress",
      what: "Decode DXT1 textures with transparency, which DXVK draws black (Linux/Proton).",
      fields: [{ name: "paths", kind: "list", required: true, hint: "Globs, one per line", suggest: "files" }],
    },
  ];

  let action = $state(specs[0].action);
  const spec = $derived(specs.find((s) => s.action === action)!);
  let values = $state<Record<string, string>>({});
  let flags = $state<Record<string, boolean>>({});
  let condition = $state<string | null>(null);
  let error = $state<string | null>(null);
  let saving = $state(false);
  $effect(() => {
    void action;
    values = {};
    flags = {};
    error = null;
  });

  const files = $derived([
    ...new Set(archives.flatMap((a) => (a.tree?.installed ?? []).filter((f) => f.taken).map((f) => f.path))),
  ].sort());
  const suggestions = (kind: Field["suggest"]) =>
    kind === "plugins"
      ? files.filter((f) => /\.es[pm]$/i.test(f))
      : kind === "bsas"
        ? files.filter((f) => /\.bsa$/i.test(f))
        : kind === "files"
          ? files
          : [];

  const lines = (text: string) =>
    text
      .split("\n")
      .map((l) => l.trim())
      .filter(Boolean);
  // Each field given, as `key=value` with a TOML value.
  function fields(): string[] {
    const out: string[] = [];
    for (const f of spec.fields) {
      if (f.kind === "bool") {
        // Kept loose files are `prune_packed = false`: pruning is the default.
        if (flags[f.name]) out.push(f.name === "keep_loose" ? "prune_packed=false" : `${f.name}=true`);
        continue;
      }
      const raw = values[f.name] ?? "";
      if (f.kind === "list") {
        const items = lines(raw);
        if (items.length) out.push(`${f.name}=[${items.map((i) => JSON.stringify(i)).join(", ")}]`);
      } else if (f.kind === "lines" ? raw.length : raw.trim()) {
        out.push(`${f.name}=${JSON.stringify(f.kind === "lines" ? raw : raw.trim())}`);
      }
    }
    return out;
  }
  const missing = $derived(
    spec.fields.filter((f) => f.required && (f.kind === "list" ? !lines(values[f.name] ?? "").length : !(values[f.name] ?? "").trim())),
  );

  async function add() {
    saving = true;
    error = null;
    error = await run(() => api.addAction(id, action, fields(), condition?.trim() || null));
    saving = false;
    if (!error) close();
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="veil" onclick={close}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="dialog" role="dialog" tabindex="-1" aria-label="Add an action" onclick={(e) => e.stopPropagation()}>
    <div class="dhead">
      <strong>{id}: add an action</strong>
      <button class="link" onclick={close}>Cancel</button>
    </div>
    <div class="dbody">
      <label class="field">
        <span>Action</span>
        <select bind:value={action}>
          {#each specs as s (s.action)}<option value={s.action}>{s.action}</option>{/each}
        </select>
      </label>
      <p class="small dim">{spec.what} It runs after the mod's other actions.</p>
      {#each spec.fields as f (spec.action + f.name)}
        {@const list = suggestions(f.suggest)}
        <label class="field" class:check={f.kind === "bool"}>
          {#if f.kind === "bool"}
            <input type="checkbox" bind:checked={flags[f.name]} />
            <span><code>{f.name}</code>{#if f.hint}<span class="dim small"> — {f.hint}</span>{/if}</span>
          {:else}
            <span><code>{f.name}</code>{#if f.required}<span class="req" title="Required">*</span>{/if}</span>
            {#if f.kind === "choice"}
              <select bind:value={values[f.name]}>
                <option value="">(default)</option>
                {#each f.choices ?? [] as c (c)}<option value={c}>{c}</option>{/each}
              </select>
            {:else if f.kind === "list" || f.kind === "lines"}
              <textarea rows={f.kind === "lines" ? 6 : 3} bind:value={values[f.name]} placeholder={f.hint ?? ""} spellcheck="false"></textarea>
              {#if list.length && f.kind === "list"}
                <select
                  class="pick"
                  onchange={(e) => {
                    const v = e.currentTarget.value;
                    if (v) values[f.name] = [...lines(values[f.name] ?? ""), v].join("\n");
                    e.currentTarget.value = "";
                  }}
                >
                  <option value="">add one of the mod's files…</option>
                  {#each list as p (p)}<option value={p}>{p}</option>{/each}
                </select>
              {/if}
            {:else}
              <input bind:value={values[f.name]} placeholder={f.hint ?? ""} spellcheck="false" list={list.length ? `sugg-${f.name}` : undefined} />
              {#if list.length}
                <datalist id={`sugg-${f.name}`}>{#each list as p (p)}<option value={p}></option>{/each}</datalist>
              {/if}
            {/if}
          {/if}
        </label>
      {/each}
      <div class="when">
        <ConditionPicker bind:value={condition} {known} {inputs} />
      </div>
      {#if error}<pre class="error">{error}</pre>{/if}
    </div>
    <div class="dfoot">
      <button onclick={close}>Cancel</button>
      <button
        class="primary"
        onclick={add}
        disabled={saving || missing.length > 0 || (condition !== null && !condition.trim())}
        title={missing.length ? `Fill in ${missing.map((f) => f.name).join(", ")}` : ""}>Add the action</button
      >
    </div>
  </div>
</div>

<style>
  .veil {
    position: fixed;
    inset: 0;
    z-index: 40;
    background: rgba(0, 0, 0, 0.45);
    display: grid;
    place-items: center;
  }
  .dialog {
    width: min(640px, 94vw);
    max-height: 90vh;
    display: flex;
    flex-direction: column;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 10px;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.45);
  }
  .dhead,
  .dfoot {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
    padding: 10px 16px;
  }
  .dhead {
    border-bottom: 1px solid var(--line);
  }
  .dfoot {
    border-top: 1px solid var(--line);
    justify-content: flex-end;
  }
  .dbody {
    padding: 10px 18px 16px;
    overflow: auto;
  }
  .field {
    display: grid;
    gap: 4px;
    margin: 10px 0;
  }
  .field.check {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  textarea,
  input:not([type="checkbox"]) {
    font-family: var(--mono);
    font-size: 12px;
  }
  .pick {
    justify-self: start;
    font-size: 12px;
  }
  .req {
    color: var(--accent);
    margin-left: 2px;
  }
  .when {
    margin-top: 14px;
  }
  .small {
    font-size: 12px;
  }
  .error {
    color: var(--problem);
    white-space: pre-wrap;
    font-family: inherit;
    font-size: 13px;
  }
</style>
