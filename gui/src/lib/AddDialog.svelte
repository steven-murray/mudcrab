<script lang="ts">
  import { onMount } from "svelte";
  import { api, type AddPlan, type AddRequest, type Answer, type Document, type Entry, type Started, type Walk } from "./api";
  import InstallerQuestions from "./InstallerQuestions.svelte";

  // Adding a mod from an archive, as `add --archive` does: name it and say
  // where it goes, answer its installer (a FOMOD's steps, a BAIN wizard's
  // questions, or which BAIN packages), declare the plugins it installs and
  // place them in the load order, and see the entry before it is written.
  let {
    path,
    document,
    section: startSection,
    add,
    close,
  }: {
    path: string;
    document: Document;
    /** Where to suggest it goes: the section of the mod chosen now. */
    section: string[];
    /** Write it; resolves true when written. */
    add: (request: AddRequest) => Promise<boolean>;
    close: () => void;
  } = $props();

  let started = $state<Started | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);

  // Name and place.
  let id = $state("");
  let sectionKey = $state("");
  // A section the list does not have yet, when "New section…" is chosen.
  const NEW = "\u0001new";
  let newSection = $state("");
  let beforeMod = $state("");
  let condition = $state("");
  let source = $state("");

  // The installer: answers so far, and where the walk stands.
  let answers = $state<Answer[]>([]);
  let walk = $state<Walk | null>(null);

  // Plugins and the entry.
  let plan = $state<AddPlan | null>(null);
  let declared = $state<string[]>([]);
  let after = $state("");
  let preview = $state<AddPlan | null>(null);

  // Every section path in the list, and the mods in each, in order.
  const sections = $derived.by(() => {
    const out: { key: string; path: string[]; title: string; mods: { id: string; title: string }[] }[] = [];
    const walkEntries = (entries: Entry[]) => {
      for (const e of entries)
        if (e.kind === "section") {
          let s = out.find((x) => x.key === e.path.join("\u0000"));
          if (!s) {
            s = { key: e.path.join("\u0000"), path: e.path, title: e.path.join(" › "), mods: [] };
            out.push(s);
          }
          for (const m of e.entries) if (m.kind === "mod") s.mods.push({ id: m.id, title: m.title });
          walkEntries(e.entries);
        }
    };
    walkEntries(document.entries);
    return out;
  });
  const chosenSection = $derived(sections.find((s) => s.key === sectionKey));
  const inputs = $derived(document.inputs.map((i) => i.id));

  onMount(async () => {
    sectionKey = startSection.join("\u0000") || (sections.at(-1)?.key ?? NEW);
    try {
      started = await api.addStart(path);
      id = started.suggested_id;
      source = started.source;
      await step();
    } catch (err) {
      error = String(err);
    }
  });

  // Walk on with the answers so far.
  async function step() {
    error = null;
    busy = true;
    try {
      walk = await api.addWalk(path, answers);
      if (walk.kind === "done") await planIt([]);
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  async function answer(given: Answer) {
    answers = [...answers, given];
    await step();
  }

  async function back() {
    answers = answers.slice(0, -1);
    plan = null;
    preview = null;
    await step();
  }

  // Every remaining question on its default.
  async function defaults() {
    busy = true;
    try {
      let next = await api.addWalk(path, answers);
      while (next.kind === "ask") {
        answers = [...answers, next.asked.kind === "choose" ? { pick: next.asked.question.default } : { yes: false }];
        next = await api.addWalk(path, answers);
      }
      walk = next;
      await planIt([]);
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }

  function request(plugins: string[]): AddRequest {
    return {
      path,
      id: id.trim(),
      section: sectionKey === NEW ? (newSection.trim() ? [newSection.trim()] : []) : (chosenSection?.path ?? []),
      before_mod: beforeMod || null,
      condition: condition.trim() || null,
      source: source.trim() || null,
      answers,
      plugins,
      load_after: after || null,
    };
  }

  // What the entry installs, and so which plugins to declare.
  async function planIt(plugins: string[]) {
    if (!id.trim()) return;
    error = null;
    try {
      const planned = await api.addPlan(request(plugins));
      if (!plugins.length) {
        plan = planned;
        declared = [...planned.installs_plugins];
        after = planned.suggested_after ?? "";
      }
      preview = declared.length ? await api.addPlan(request(declared)) : planned;
    } catch (err) {
      error = String(err);
      preview = null;
    }
  }

  // Name, place or plugins changed: plan the preview again.
  async function replan() {
    if (walk?.kind !== "done") return;
    await planIt(plan ? declared : []);
  }

  async function write() {
    busy = true;
    try {
      if (await add(request(declared))) close();
    } finally {
      busy = false;
    }
  }

</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="veil" onclick={close}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="dialog" role="dialog" tabindex="-1" aria-label="Add a mod" onclick={(e) => e.stopPropagation()}>
    <div class="dhead">
      <strong>Add {started?.file_name ?? "an archive"}</strong>
      <button class="link" onclick={close}>Cancel</button>
    </div>
    <div class="dbody">
      {#if error}<pre class="error">{error}</pre>{/if}
      {#if !started}
        <p class="dim">Reading the archive…</p>
      {:else}
        <p class="dim small">
          {started.layout_label}{#if started.has_installer}; its installer's choices are asked below{:else}; it installs as it is{/if}.
          From <code>{started.source}</code>{#if started.identified} (found on Nexus by its checksum){/if}.
        </p>

        <section>
          <h4>Name and place</h4>
          <label>Mod id <input type="text" bind:value={id} onchange={replan} /></label>
          <label
            >Section
            <select bind:value={sectionKey} onchange={() => ((beforeMod = ""), replan())}>
              {#each sections as s}<option value={s.key}>{s.title}</option>{/each}
              <option value={NEW}>New section…</option>
            </select>
          </label>
          {#if sectionKey === NEW}
            <label
              >New section <input type="text" bind:value={newSection} onchange={replan} placeholder="its name, as MO2's separator shows it" />
            </label>
          {/if}
          <label
            >Position
            <select bind:value={beforeMod} onchange={replan}>
              <option value="">at the end of the section</option>
              {#each chosenSection?.mods ?? [] as m}<option value={m.id}>before {m.title}</option>{/each}
            </select>
          </label>
          <label
            >Only if
            <input type="text" bind:value={condition} onchange={replan} placeholder={inputs.length ? `a condition, e.g. ${inputs[0]}` : "a condition (optional)"} />
          </label>
          <label>Source <input type="text" bind:value={source} onchange={replan} title="nexus:oblivion/<mod>/<file>, a URL, or manual:<file name>" /></label>
        </section>

        {#if started.has_installer}
          <section>
            <h4>Installer</h4>
            <InstallerQuestions {walk} answered={answers.length} {busy} {answer} {back} {defaults} />
          </section>
        {/if}

        {#if walk?.kind === "done" && plan}
          <section>
            <h4>Plugins</h4>
            {#if !plan.installs_plugins.length}
              <p class="dim small">It installs no plugin{plan.files ? ` (${plan.files} file(s))` : ""}.</p>
            {:else}
              <p class="dim small">It installs {plan.files} file(s), these plugins among them. Tick the ones to declare; they go into the load order:</p>
              {#each plan.installs_plugins as plugin}
                <label class="choice">
                  <input
                    type="checkbox"
                    checked={declared.includes(plugin)}
                    onchange={(e) => {
                      declared = (e.currentTarget as HTMLInputElement).checked ? [...declared, plugin] : declared.filter((p) => p !== plugin);
                      replan();
                    }}
                  />
                  <code>{plugin}</code>
                </label>
              {/each}
              {#if declared.length}
                <label
                  >Load after
                  <select bind:value={after} onchange={replan}>
                    <option value="">the end of the load order</option>
                    {#each document.load_order as p}<option value={p.name}>{p.name}</option>{/each}
                  </select>
                </label>
              {/if}
            {/if}
          </section>
        {/if}

        {#if preview}
          <section>
            <h4>The entry</h4>
            {#each preview.notes as note}<p class="note small">{note}</p>{/each}
            <pre class="diff">{#each preview.diff.split("\n") as line}<span class:add={line.startsWith("+")} class:del={line.startsWith("-")}>{line}
</span>{/each}</pre>
            <p class="dim small">Same from a terminal: <code>{preview.command}</code></p>
          </section>
        {/if}
      {/if}
    </div>
    <div class="dfoot">
      <button onclick={close}>Cancel</button>
      <button class="primary" onclick={write} disabled={busy || !preview || !id.trim() || walk?.kind !== "done"}>Add to the list</button>
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
    width: min(820px, 94vw);
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
  .dhead strong {
    overflow-wrap: anywhere;
  }
  .dfoot {
    border-top: 1px solid var(--line);
    justify-content: flex-end;
  }
  .dbody {
    padding: 10px 18px 16px;
    overflow: auto;
  }
  section {
    margin: 10px 0 14px;
  }
  h4 {
    font-size: 13px;
    margin: 0 0 6px;
  }
  label {
    display: grid;
    grid-template-columns: 90px minmax(0, 1fr);
    gap: 8px;
    align-items: center;
    font-size: 13px;
    margin: 4px 0;
  }
  label.choice {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin: 4px 0;
  }
  .small {
    font-size: 12px;
  }
  .note {
    color: var(--note);
    margin: 2px 0;
  }
  .error {
    color: var(--problem);
    white-space: pre-wrap;
    font-family: inherit;
    font-size: 13px;
  }
  .diff {
    background: var(--card);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 8px 10px;
    overflow: auto;
    max-height: 260px;
  }
  .diff .add {
    color: var(--good);
  }
  .diff .del {
    color: var(--problem);
  }
  code {
    overflow-wrap: anywhere;
  }
</style>
