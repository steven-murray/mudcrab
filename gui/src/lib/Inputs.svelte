<script lang="ts">
  import { api, tomlString, type Document, type Entry, type InputDoc, type ModDoc } from "./api";

  // The list's questions: each one's wording, kind, answers and default,
  // editable (`input set`, `input add`, `input remove`), and the mods whose
  // `if` asks it, so it is plain what an answer brings in or leaves out.
  let {
    document,
    select,
    run,
  }: {
    document: Document;
    select: (mod: string) => void;
    /** Run an edit; resolves true when it was written. */
    run: (edit: () => Promise<unknown>) => Promise<boolean>;
  } = $props();

  // Every mod with a condition, by the inputs it names.
  const asking = $derived.by(() => {
    const mods: ModDoc[] = [];
    const walk = (entries: Entry[]) => {
      for (const e of entries) {
        if (e.kind === "mod" && e.condition) mods.push(e);
        else if (e.kind === "section") walk(e.entries);
      }
    };
    walk(document.entries);
    const out = new Map<string, ModDoc[]>();
    for (const input of document.inputs) {
      const word = new RegExp(`(^|[^A-Za-z0-9_])${input.id.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}($|[^A-Za-z0-9_])`);
      out.set(
        input.id,
        mods.filter((m) => word.test(m.condition!.expr)),
      );
    }
    return out;
  });

  // The one being changed, as a draft.
  let editing = $state<string | null>(null);
  // Answers are edited one per line, so an answer may hold a comma; whether
  // there is a default is kept apart from its text, which may be empty.
  let draft = $state({ query: "", choices: "", hasDefault: false, fallback: "", condition: "" });
  const lines = (text: string) => text.split("\n").map((c) => c.trim()).filter(Boolean);
  const listed = (kind: string) => kind === "choice" || kind === "multi";
  // A default as the form edits it: a multi's choices with commas between.
  const shownDefault = (value: unknown) => (Array.isArray(value) ? value.join(", ") : String(value));
  const picked = (text: string) => text.split(",").map((c) => c.trim()).filter(Boolean);

  function start(input: InputDoc) {
    editing = input.id;
    draft = {
      query: input.query,
      choices: (input.choices ?? []).join("\n"),
      hasDefault: input.default !== undefined,
      fallback: input.default === undefined ? (input.type === "bool" ? "false" : "") : shownDefault(input.default),
      condition: input.condition?.expr ?? "",
    };
  }

  async function save(input: InputDoc) {
    // A group of optional mods: only its wording is the list's to change;
    // its answers are its mods, and which start ticked is said on each.
    if (input.optional_group !== undefined) {
      if (draft.query === input.query) editing = null;
      else if (
        await run(() =>
          input.optional_group
            ? api.setInput(input.id, [`query=${tomlString(draft.query)}`], [])
            : api.addInput(input.id, "multi", draft.query, [], null),
        )
      )
        editing = null;
      return;
    }
    const sets: string[] = [];
    const unsets: string[] = [];
    if (draft.query !== input.query) sets.push(`query=${tomlString(draft.query)}`);
    if (draft.condition.trim() !== (input.condition?.expr ?? "")) {
      if (draft.condition.trim()) sets.push(`if=${tomlString(draft.condition.trim())}`);
      else unsets.push("if");
    }
    if (listed(input.type)) {
      const choices = lines(draft.choices);
      if (JSON.stringify(choices) !== JSON.stringify(input.choices ?? []))
        sets.push(`choices=[${choices.map(tomlString).join(", ")}]`);
    }
    const had = input.default !== undefined;
    if (!draft.hasDefault) {
      if (had) unsets.push("default");
    } else if (!had || draft.fallback !== shownDefault(input.default)) {
      if (input.type === "bool") sets.push(`default=${draft.fallback === "true"}`);
      else if (input.type === "multi") sets.push(`default=[${picked(draft.fallback).map(tomlString).join(", ")}]`);
      else sets.push(`default=${tomlString(draft.fallback)}`);
    }
    if (!sets.length && !unsets.length) {
      editing = null;
      return;
    }
    if (await run(() => api.setInput(input.id, sets, unsets))) editing = null;
  }

  // A new question.
  let adding = $state(false);
  const blank = () => ({ name: "", kind: "bool", query: "", choices: "", hasDefault: false, fallback: "" });
  let fresh = $state(blank());
  async function add(event: SubmitEvent) {
    event.preventDefault();
    const choices = lines(fresh.choices);
    const ok = await run(() =>
      api.addInput(
        fresh.name.trim(),
        fresh.kind,
        fresh.query,
        listed(fresh.kind) ? choices : [],
        fresh.hasDefault ? (fresh.kind === "bool" ? (fresh.fallback === "true" ? "true" : "false") : fresh.fallback) : null,
      ),
    );
    if (ok) {
      adding = false;
      fresh = blank();
    }
  }
</script>

<p class="dim small">
  The questions a player answers before installing. A mod's <code>if</code> names them; change which mods
  one brings in from the mod itself.
</p>

{#if adding}
  <form class="new" onsubmit={add}>
    <p class="dim small">Added after the last question: one is asked after those above it, and only they can decide whether it is.</p>
    <label>Name <input type="text" bind:value={fresh.name} placeholder="as conditions use it, e.g. big_textures" /></label>
    <label
      >Kind
      <select bind:value={fresh.kind}
        ><option value="bool">bool (yes or no)</option><option value="choice">choice (one of its answers)</option><option value="multi"
          >multi (any of its answers)</option
        ><option value="text">text</option></select
      >
    </label>
    <label>Question <input type="text" bind:value={fresh.query} /></label>
    {#if listed(fresh.kind)}
      <label>Answers <textarea rows="3" bind:value={fresh.choices} placeholder="one per line"></textarea></label>
    {/if}
    <label
      >Default
      <span class="default">
        <input type="checkbox" bind:checked={fresh.hasDefault} title="Whether the question has a default" />
        {#if fresh.kind === "bool"}
          <select bind:value={fresh.fallback} disabled={!fresh.hasDefault}><option>false</option><option>true</option></select>
        {:else}
          <input type="text" bind:value={fresh.fallback} disabled={!fresh.hasDefault} placeholder={fresh.hasDefault ? "(empty)" : "none"} />
        {/if}
      </span>
    </label>
    <div class="buttons">
      <button type="submit" class="primary" disabled={!fresh.name.trim() || !fresh.query.trim()}>Add</button>
      <button type="button" onclick={() => (adding = false)}>Cancel</button>
    </div>
  </form>
{:else}
  <button class="addq" onclick={() => (adding = true)}>Add a question…</button>
{/if}

{#each document.inputs as input (input.id)}
  {@const mods = asking.get(input.id) ?? []}
  <article>
    <div class="top">
      <code class="name">{input.id}</code>
      <span class="kind">{input.optional_group === undefined ? input.type : "optional mods"}</span>
      {#if editing !== input.id}
        <button class="link" onclick={() => start(input)}>Change</button>
        {#if input.optional_group === undefined}<button
          class="link danger"
          onclick={() => run(() => api.removeInput(input.id))}
          title="Refused while a mod's condition still asks it">Remove</button
        >{/if}
      {/if}
    </div>
    {#if editing === input.id}
      <form
        onsubmit={(e) => {
          e.preventDefault();
          save(input);
        }}
      >
        <label>Question <input type="text" bind:value={draft.query} /></label>
        {#if input.optional_group === undefined}
          <label
            >Asked only if
            <input type="text" bind:value={draft.condition} placeholder="always asked; or a condition on the questions above, e.g. enb" />
          </label>
        {/if}
        {#if input.optional_group !== undefined}
          <p class="dim small">Its answers are the mods marked optional in it; which start ticked is said on each mod.</p>
        {:else}
        {#if listed(input.type)}
          <label>Answers <textarea rows="3" bind:value={draft.choices} placeholder="one per line"></textarea></label>
        {/if}
        <label
          >Default
          <span class="default">
            <input type="checkbox" bind:checked={draft.hasDefault} title="Whether the question has a default" />
            {#if input.type === "bool"}
              <select bind:value={draft.fallback} disabled={!draft.hasDefault}><option>true</option><option>false</option></select>
            {:else}
              <input
                type="text"
                bind:value={draft.fallback}
                disabled={!draft.hasDefault}
                placeholder={!draft.hasDefault ? "none" : input.type === "multi" ? "none picked; or answers, with commas" : "(empty)"}
              />
            {/if}
          </span>
        </label>
        {/if}
        <div class="buttons">
          <button type="submit" class="primary">Save</button>
          <button type="button" onclick={() => (editing = null)}>Cancel</button>
        </div>
      </form>
    {:else}
      <p class="query">{input.query}</p>
      {#if input.condition}<div class="dim small">Asked only if {input.condition.text}</div>{/if}
      {#each input.default_when ?? [] as when}
        <div class="dim small">Default {Array.isArray(when.value) ? when.value.join(", ") || "none" : `“${when.value}”`} if {when.condition.text}</div>
      {/each}
      <div class="dim small">
        {#if input.choices?.length}answers: {input.choices.map((c) => `“${c}”`).join(" ")} · {/if}default: {input.default === undefined
          ? "none"
          : Array.isArray(input.default)
            ? input.default.length
              ? input.default.map((c) => `“${c}”`).join(" ")
              : "none picked"
            : `“${input.default}”`}
      </div>
    {/if}
    {#if mods.length}
      <div class="mods small">
        <span class="dim">Asked by {mods.length} mod(s):</span>
        {#each mods as m}
          <button class="link" onclick={() => select(m.id)} title={m.condition?.text}>{m.title}</button>
        {/each}
      </div>
    {:else}
      <div class="dim small">No mod asks it yet.</div>
    {/if}
  </article>
{/each}


<style>
  .small {
    font-size: 12px;
  }
  article {
    border-bottom: 1px solid var(--line);
    padding: 8px 2px;
  }
  .top {
    display: flex;
    gap: 10px;
    align-items: baseline;
  }
  .name {
    font-size: 13px;
    font-weight: 600;
  }
  .kind {
    font-size: 11px;
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 0 5px;
    color: var(--dim);
  }
  .danger {
    color: var(--problem);
  }
  .query {
    margin: 4px 0 2px;
  }
  .mods {
    display: flex;
    flex-wrap: wrap;
    gap: 2px 10px;
    margin-top: 4px;
  }
  form {
    display: grid;
    gap: 6px;
    margin: 6px 0;
    max-width: 560px;
  }
  form.new {
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 10px;
    margin-bottom: 10px;
  }
  .addq {
    margin-bottom: 10px;
  }
  label {
    display: grid;
    grid-template-columns: 90px minmax(0, 1fr);
    gap: 8px;
    align-items: center;
    font-size: 13px;
  }
  .default {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .default input[type="text"],
  .default select {
    flex: 1;
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
  .buttons {
    display: flex;
    gap: 6px;
  }
</style>
