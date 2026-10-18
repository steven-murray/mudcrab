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
  let draft = $state({ query: "", choices: "", hasDefault: false, fallback: "" });
  const lines = (text: string) => text.split("\n").map((c) => c.trim()).filter(Boolean);

  function start(input: InputDoc) {
    editing = input.id;
    draft = {
      query: input.query,
      choices: (input.choices ?? []).join("\n"),
      hasDefault: input.default !== undefined,
      fallback: input.default === undefined ? (input.type === "bool" ? "false" : "") : String(input.default),
    };
  }

  async function save(input: InputDoc) {
    const sets: string[] = [];
    const unsets: string[] = [];
    if (draft.query !== input.query) sets.push(`query=${tomlString(draft.query)}`);
    if (input.type === "choice") {
      const choices = lines(draft.choices);
      if (JSON.stringify(choices) !== JSON.stringify(input.choices ?? []))
        sets.push(`choices=[${choices.map(tomlString).join(", ")}]`);
    }
    const had = input.default !== undefined;
    if (!draft.hasDefault) {
      if (had) unsets.push("default");
    } else if (!had || draft.fallback !== String(input.default)) {
      if (input.type === "bool") sets.push(`default=${draft.fallback === "true"}`);
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
        fresh.kind === "choice" ? choices : [],
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

{#each document.inputs as input (input.id)}
  {@const mods = asking.get(input.id) ?? []}
  <article>
    <div class="top">
      <code class="name">{input.id}</code>
      <span class="kind">{input.type}</span>
      {#if editing !== input.id}
        <button class="link" onclick={() => start(input)}>Change</button>
        <button
          class="link danger"
          onclick={() => run(() => api.removeInput(input.id))}
          title="Refused while a mod's condition still asks it">Remove</button
        >
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
        {#if input.type === "choice"}
          <label>Answers <textarea rows="3" bind:value={draft.choices} placeholder="one per line"></textarea></label>
        {/if}
        <label
          >Default
          <span class="default">
            <input type="checkbox" bind:checked={draft.hasDefault} title="Whether the question has a default" />
            {#if input.type === "bool"}
              <select bind:value={draft.fallback} disabled={!draft.hasDefault}><option>true</option><option>false</option></select>
            {:else}
              <input type="text" bind:value={draft.fallback} disabled={!draft.hasDefault} placeholder={draft.hasDefault ? "(empty)" : "none"} />
            {/if}
          </span>
        </label>
        <div class="buttons">
          <button type="submit" class="primary">Save</button>
          <button type="button" onclick={() => (editing = null)}>Cancel</button>
        </div>
      </form>
    {:else}
      <p class="query">{input.query}</p>
      <div class="dim small">
        {#if input.choices?.length}answers: {input.choices.map((c) => `“${c}”`).join(" ")} · {/if}default: {input.default === undefined
          ? "none"
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

{#if adding}
  <form class="new" onsubmit={add}>
    <label>Name <input type="text" bind:value={fresh.name} placeholder="as conditions use it, e.g. big_textures" /></label>
    <label
      >Kind
      <select bind:value={fresh.kind}><option>bool</option><option>choice</option><option>text</option></select>
    </label>
    <label>Question <input type="text" bind:value={fresh.query} /></label>
    {#if fresh.kind === "choice"}
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
  <button onclick={() => (adding = true)}>Add a question…</button>
{/if}

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
    margin-top: 10px;
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
