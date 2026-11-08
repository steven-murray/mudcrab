<script lang="ts">
  import { untrack } from "svelte";
  import { SvelteMap, SvelteSet } from "svelte/reactivity";
  import { api, type Edited, type IniFormat, type IniLine, type IniView, type InputDoc } from "./api";
  import ConditionPicker from "./ConditionPicker.svelte";
  import type { Archive } from "./ModFiles.svelte";

  // A mod's INI shown as written, comments and all, with an input for each
  // setting. What is changed is written as the mod's `ini_set`,
  // `ini_comment` and `ini_append_block` actions on that file, for one
  // condition at a time; the values other conditions give are shown beside.
  let {
    id,
    archives,
    inputs,
    run,
    close,
  }: {
    id: string;
    /** The mod's archives and what each installs, for its INIs. */
    archives: Archive[];
    inputs: InputDoc[];
    /** Write an edit; resolves to why it was not written, or null. */
    run: (edit: () => Promise<Edited | unknown>) => Promise<string | null>;
    close: () => void;
  } = $props();

  type Ini = { key: string; label: string; file: string; scope: "mod" | "game"; number: number | null; source: string | null };
  type Setting = { section: string | null; key: string; value: string; format: IniFormat };

  // The mod's INIs, as installed (a later archive's wins), and the game's.
  const inis = $derived.by(() => {
    const byPath = new Map<string, Ini>();
    for (const archive of archives)
      for (const f of archive.tree?.installed ?? [])
        if (f.taken && /\.ini$/i.test(f.path))
          byPath.set(f.path.toLowerCase(), { key: f.path.toLowerCase(), label: f.path, file: f.path, scope: "mod", number: archive.number, source: f.source });
    const out = [...byPath.values()].sort((a, b) => a.label.localeCompare(b.label));
    out.push({ key: "game", label: "Oblivion.ini (the game's)", file: "Oblivion.ini", scope: "game", number: null, source: null });
    return out;
  });
  let chosen = $state("");
  $effect(() => {
    if (!inis.some((i) => i.key === chosen)) chosen = inis[0]?.key ?? "";
  });
  const ini = $derived(inis.find((i) => i.key === chosen) ?? null);

  let view = $state<IniView | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);
  async function load(which: Ini) {
    loading = true;
    error = null;
    try {
      view = await api.iniView(id, which.file, which.scope, which.number, which.source);
    } catch (err) {
      view = null;
      error = String(err);
    } finally {
      loading = false;
    }
  }
  $effect(() => {
    const which = ini;
    if (which) untrack(() => load(which));
  });

  // The condition being edited, and its settings: changed values by
  // section and key, those commented out, and lines appended.
  let condition = $state<string | null>(null);
  const known = $derived((view?.groups ?? []).map((g) => g.if).filter((c): c is string => !!c));
  const values = new SvelteMap<string, Setting>();
  const commented = new SvelteSet<string>();
  let block = $state("");
  let dirty = $state(false);

  const fileSettings = $derived(
    (view?.lines ?? []).filter((l): l is Extract<IniLine, { kind: "setting" }> => l.kind === "setting"),
  );
  const sk = (section: string | null, key: string) => `${(section ?? "").toLowerCase()}\u0000${key}`;
  // The file's own setting an action names: by key, and its section where
  // it gives one.
  function inFile(section: string | null, key: string) {
    return fileSettings.find((s) => s.key === key && (section === null || (s.section ?? "").toLowerCase() === section.toLowerCase()));
  }
  // Most of a file is written one way; a setting added follows it.
  const usual = $derived<IniFormat>(
    fileSettings.filter((s) => s.format === "set-to").length > fileSettings.length / 2 ? "set-to" : "standard",
  );

  function loadLayer() {
    values.clear();
    commented.clear();
    block = "";
    const group = view?.groups.find((g) => g.if === condition);
    for (const action of group?.actions ?? []) {
      if (action.kind === "set") {
        const own = inFile(action.section, action.key);
        const section = own ? own.section : action.section;
        values.set(sk(section, action.key), { section, key: action.key, value: action.value, format: own?.format ?? action.format });
      } else if (action.kind === "comment") commented.add(action.key);
      else block = action.block;
    }
    dirty = false;
  }
  $effect(() => {
    void view;
    void condition;
    untrack(loadLayer);
  });

  function setValue(s: { section: string | null; key: string; value: string; format: IniFormat }, to: string, file?: string) {
    const k = sk(s.section, s.key);
    if (file !== undefined && to === file) values.delete(k);
    else values.set(k, { section: s.section, key: s.key, value: to, format: s.format });
    dirty = true;
  }
  function toggleComment(key: string) {
    if (commented.has(key)) commented.delete(key);
    else commented.add(key);
    dirty = true;
  }

  // What the other conditions make of a setting.
  function elsewhere(section: string | null, key: string): string[] {
    const out: string[] = [];
    for (const group of view?.groups ?? []) {
      if (group.if === condition) continue;
      const when = group.if ? `if ${group.if}` : "always";
      for (const a of group.actions) {
        if (a.kind === "set" && a.key === key && (a.section === null || section === null || a.section.toLowerCase() === section.toLowerCase()))
          out.push(`${a.value} ${when}`);
        if (a.kind === "comment" && a.key === key) out.push(`commented out ${when}`);
      }
    }
    return out;
  }

  // Shown: everything, or the settings found or changed with the comments
  // just above each and the section headers they sit under.
  let search = $state("");
  let changedOnly = $state(false);
  const touched = $derived(
    new Set([
      ...values.keys(),
      ...fileSettings.filter((s) => commented.has(s.key) || elsewhere(s.section, s.key).length).map((s) => sk(s.section, s.key)),
    ]),
  );
  const shown = $derived.by(() => {
    const lines = view?.lines ?? [];
    const words = search.trim().toLowerCase();
    if (!words && !changedOnly) return lines.map((_, i) => i);
    const keep = new Set<number>();
    let header = -1;
    lines.forEach((line, i) => {
      if (line.kind === "section") header = i;
      if (line.kind !== "setting") return;
      const found = !words || line.key.toLowerCase().includes(words) || line.value.toLowerCase().includes(words);
      if (!found || (changedOnly && !touched.has(sk(line.section, line.key)))) return;
      if (header >= 0) keep.add(header);
      for (let j = i - 1; j >= 0; j--) {
        const above = lines[j];
        if (above.kind !== "text" || !/^\s*[;#]/.test(above.text)) break;
        keep.add(j);
      }
      keep.add(i);
    });
    return [...keep].sort((a, b) => a - b);
  });

  // Settings the condition sets that the file does not have.
  const extras = $derived([...values.entries()].filter(([, s]) => !inFile(s.section, s.key)));
  const sections = $derived([...new Set(fileSettings.map((s) => s.section).filter((s): s is string => !!s))]);
  let newSection = $state("");
  let newKey = $state("");
  let newValue = $state("");
  function addSetting() {
    const key = newKey.trim();
    if (!key) return;
    setValue({ section: newSection.trim() || null, key, value: newValue, format: usual }, newValue);
    newKey = "";
    newValue = "";
  }

  let saving = $state(false);
  async function save() {
    if (!ini) return;
    saving = true;
    const formatOf = (key: string) => fileSettings.find((s) => s.key === key)?.format ?? usual;
    error = await run(() =>
      api.setIni(id, {
        file: ini.file,
        scope: ini.scope,
        if: condition?.trim() || null,
        sets: [...values.values()],
        comments: ini.scope === "mod" ? [...commented].map((key) => ({ key, format: formatOf(key) })) : [],
        block: ini.scope === "mod" && block.trim() ? block : null,
      }),
    );
    saving = false;
    if (!error) {
      dirty = false;
      await load(ini);
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="veil" onclick={close}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="dialog" role="dialog" tabindex="-1" aria-label="INI editor" onclick={(e) => e.stopPropagation()}>
    <div class="dhead">
      <strong>{id}: INI editor</strong>
      <button class="link" onclick={close}>Close</button>
    </div>
    <div class="top" class:held={dirty}>
      <div class="line">
        <select bind:value={chosen} disabled={dirty}>
          {#each inis as i (i.key)}<option value={i.key}>{i.label}</option>{/each}
        </select>
        <input class="search" bind:value={search} placeholder="Find a setting" />
        <label class="small"><input type="checkbox" bind:checked={changedOnly} /> Changed only</label>
      </div>
      <div class="picker" inert={dirty}>
        <ConditionPicker bind:value={condition} {known} {inputs} />
      </div>
      {#if dirty}<p class="small dim">Save or discard these changes to choose another file or condition.</p>{/if}
    </div>
    <div class="dbody">
      {#if error}<pre class="error">{error}</pre>{/if}
      {#if loading && !view}
        <p class="dim">Reading the INI…</p>
      {:else if view}
        {#if ini?.scope === "game"}
          <p class="small dim">
            The game's <code>Oblivion.ini</code>, as it ships (<code>Oblivion_default.ini</code>): written with
            <code>scope = "game"</code>, to the MO2 profile's copy.
          </p>
        {/if}
        <div class="ini">
          {#each shown as i (i)}
            {@const line = view.lines[i]}
            {#if line.kind === "section"}
              <div class="section">[{line.name}]</div>
            {:else if line.kind === "text"}
              <div class="text">{line.text || " "}</div>
            {:else}
              {@const k = sk(line.section, line.key)}
              {@const set = values.get(k)}
              {@const off = commented.has(line.key)}
              {@const others = elsewhere(line.section, line.key)}
              {@const repeated = line.times > 1}
              <div class="setting" class:changed={!!set} class:off>
                <span class="key">{line.format === "set-to" ? "set " : ""}{line.key}</span>
                <span class="eq">{line.format === "set-to" ? "to" : "="}</span>
                <input
                  class="value"
                  value={set?.value ?? line.value}
                  size={Math.min(60, Math.max(8, (set?.value ?? line.value).length + 1))}
                  disabled={off || repeated}
                  title={repeated
                    ? `Set ${line.times} times in this file: a script building a list, not one setting. ini_set can't change one of these lines; Append lines can add to the list.`
                    : ""}
                  spellcheck="false"
                  oninput={(e) => setValue(line, e.currentTarget.value, line.value)}
                />
                {#if line.comment}<span class="trailing">{line.comment}</span>{/if}
                {#if repeated}<span class="tag" title="ini_set can't change one line of a list">×{line.times}</span>{/if}
                {#if set}
                  <button class="link" title="Back to the file's value ({line.value})" onclick={() => ((values.delete(k), (dirty = true)))}>↺</button>
                {/if}
                {#if ini?.scope === "mod"}
                  <button
                    class="link small"
                    class:on={off}
                    title={off
                      ? "Put it back"
                      : repeated
                        ? "Comment out every line setting it (ini_comment)"
                        : "Comment it out (ini_comment), so the mod falls back to its own default"}
                    onclick={() => toggleComment(line.key)}>{off ? "uncomment" : "comment out"}</button
                  >
                {/if}
                {#each others as other (other)}<span class="tag">{other}</span>{/each}
              </div>
            {/if}
          {/each}
        </div>

        <h4>Not in the file</h4>
        {#each extras as [k, s] (k)}
          <div class="setting changed">
            <span class="key">{s.section ? `[${s.section}] ` : ""}{s.key}</span>
            <span class="eq">{s.format === "set-to" ? "to" : "="}</span>
            <input class="value" value={s.value} spellcheck="false" oninput={(e) => setValue(s, e.currentTarget.value)} />
            <button class="link" title="Take it out" onclick={() => ((values.delete(k), (dirty = true)))}>✕</button>
          </div>
        {:else}
          <p class="small dim">Settings this condition adds that the file lacks are listed here.</p>
        {/each}
        <div class="add">
          {#if sections.length || ini?.scope === "game"}
            <input list="ini-sections" bind:value={newSection} placeholder="Section" />
            <datalist id="ini-sections">{#each sections as s (s)}<option value={s}></option>{/each}</datalist>
          {/if}
          <input bind:value={newKey} placeholder="Key" spellcheck="false" />
          <input bind:value={newValue} placeholder="Value" spellcheck="false" />
          <button onclick={addSetting} disabled={!newKey.trim()}>Add a setting</button>
        </div>

        {#if ini?.scope === "mod"}
          <h4>Append lines</h4>
          <p class="small dim">For a guide's "paste these lines at the end": added as written (<code>ini_append_block</code>).</p>
          <textarea rows="4" bind:value={block} oninput={() => (dirty = true)} spellcheck="false"></textarea>
        {/if}
      {/if}
    </div>
    <div class="dfoot">
      <span class="small dim">{view?.from ?? ""}</span>
      {#if dirty}<button onclick={loadLayer}>Discard</button>{/if}
      <button class="primary" onclick={save} disabled={!dirty || saving || (condition !== null && !condition.trim())}>Save</button>
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
    width: min(960px, 96vw);
    height: 90vh;
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
  }
  .dfoot span {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .top {
    padding: 10px 16px;
    border-bottom: 1px solid var(--line);
    display: grid;
    gap: 8px;
  }
  .top .line {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
  }
  .top select {
    max-width: 50%;
  }
  .search {
    flex: 1;
    min-width: 140px;
  }
  .held .picker {
    opacity: 0.5;
  }
  .dbody {
    padding: 10px 16px 16px;
    overflow: auto;
    flex: 1;
  }
  .ini {
    font-family: var(--mono);
    font-size: 12px;
    line-height: 1.5;
  }
  .text {
    white-space: pre-wrap;
    color: var(--dim);
  }
  .section {
    font-weight: 600;
    margin-top: 4px;
  }
  .setting {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    padding: 1px 4px;
    border-left: 2px solid transparent;
    font-family: var(--mono);
    font-size: 12px;
  }
  .setting.changed {
    border-left-color: var(--accent);
    background: var(--accent-soft);
  }
  .setting.off .key,
  .setting.off .value {
    text-decoration: line-through;
    opacity: 0.6;
  }
  .eq {
    color: var(--dim);
  }
  .value {
    font-family: var(--mono);
    font-size: 12px;
    padding: 1px 4px;
    max-width: 100%;
  }
  .trailing {
    color: var(--dim);
    white-space: pre;
  }
  .setting .link.small {
    font-size: 11px;
    opacity: 0.6;
  }
  .setting:hover .link.small,
  .setting .link.small.on {
    opacity: 1;
  }
  .tag {
    font-family: inherit;
    font-size: 11px;
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 0 4px;
    color: var(--dim);
  }
  .link.on {
    color: var(--accent);
  }
  h4 {
    margin: 16px 0 4px;
  }
  .add {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    margin-top: 6px;
  }
  textarea {
    width: 100%;
    font-family: var(--mono);
    font-size: 12px;
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
