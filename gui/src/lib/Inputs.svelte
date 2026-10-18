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
  let draft = $state({ query: "", choices: "", fallback: "" });

  function start(input: InputDoc) {
    editing = input.id;
    draft = {
      query: input.query,
      choices: (input.choices ?? []).join(", "),
      fallback: input.default === undefined ? "" : String(input.default),
    };
  }

  async function save(input: InputDoc) {
    const sets: string[] = [];
    const unsets: string[] = [];
    if (draft.query !== input.query) sets.push(`query=${tomlString(draft.query)}`);
    if (input.type === "choice") {
      const choices = draft.choices.split(",").map((c) => c.trim()).filter(Boolean);
      if (choices.join(",") !== (input.choices ?? []).join(","))
        sets.push(`choices=[${choices.map(tomlString).join(", ")}]`);
    }
    const was = input.default === undefined ? "" : String(input.default);
    if (draft.fallback !== was) {
      if (!draft.fallback) unsets.push("default");
      else if (input.type === "bool") sets.push(`default=${draft.fallback === "true"}`);
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
  let fresh = $state({ name: "", kind: "bool", query: "", choices: "", fallback: "" });
  async function add(event: SubmitEvent) {
    event.preventDefault();
    const choices = fresh.choices.split(",").map((c) => c.trim()).filter(Boolean);
    const ok = await run(() =>
      api.addInput(fresh.name.trim(), fresh.kind, fresh.query, fresh.kind === "choice" ? choices : [], fresh.fallback || null),
    );
    if (ok) {
      adding = false;
      fresh = { name: "", kind: "bool", query: "", choices: "", fallback: "" };
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
          <label>Answers <input type="text" bind:value={draft.choices} placeholder="comma-separated" /></label>
        {/if}
        <label
          >Default
          {#if input.type === "bool"}
            <select bind:value={draft.fallback}><option value="">none</option><option>true</option><option>false</option></select>
          {:else}
            <input type="text" bind:value={draft.fallback} />
          {/if}
        </label>
        <div class="buttons">
          <button type="submit" class="primary">Save</button>
          <button type="button" onclick={() => (editing = null)}>Cancel</button>
        </div>
      </form>
    {:else}
      <p class="query">{input.query}</p>
      <div class="dim small">
        {#if input.choices?.length}answers: {input.choices.join(", ")} · {/if}default: {input.default ?? "none"}
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
      <label>Answers <input type="text" bind:value={fresh.choices} placeholder="comma-separated" /></label>
    {/if}
    <label>Default <input type="text" bind:value={fresh.fallback} placeholder={fresh.kind === "bool" ? "true or false" : ""} /></label>
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
  .buttons {
    display: flex;
    gap: 6px;
  }
</style>
