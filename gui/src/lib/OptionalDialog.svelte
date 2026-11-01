<script lang="ts" module>
  /** A question to add with the `if`. */
  export type Fresh = { name: string; kind: "bool" | "choice"; query: string; choices: string[]; fallback: string };
</script>

<script lang="ts">
  import { untrack } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import type { InputDoc } from "./api";
  import Dialog from "./Dialog.svelte";

  // Which answers bring a mod in: one of the list's questions and its
  // answers, a new question, or (for anything else) the `if` written out.
  let {
    id,
    inputs,
    current,
    save,
    close,
  }: {
    id: string;
    inputs: InputDoc[];
    /** The mod's `if` now, if any. */
    current: string | null;
    /** Write the `if`, after adding the new question if there is one. */
    save: (condition: string, fresh: Fresh | null) => Promise<boolean>;
    close: () => void;
  } = $props();

  const quoted = (value: string) => (/^[A-Za-z0-9_.-]+$/.test(value) ? value : JSON.stringify(value));
  const unquoted = (value: string) => value.replace(/^"(.*)"$/, "$1");

  // What the mod's `if` is now, read back into the form where it is one
  // question's answers; anything else is shown as written.
  function read(text: string | null) {
    const t = (text ?? "").trim();
    if (!t) return { mode: "existing" as const, question: inputs[0]?.id ?? "", yes: true, picked: [] as string[] };
    const flag = /^(!?)\s*([A-Za-z0-9_.-]+)$/.exec(t);
    if (flag && inputs.some((i) => i.id === flag[2]))
      return { mode: "existing" as const, question: flag[2], yes: !flag[1], picked: [] };
    const terms = t.split("||").map((term) => /^\s*([A-Za-z0-9_.-]+)\s*==\s*("[^"]*"|\S+)\s*$/.exec(term));
    if (terms.every(Boolean) && new Set(terms.map((m) => m![1])).size === 1 && inputs.some((i) => i.id === terms[0]![1]))
      return { mode: "existing" as const, question: terms[0]![1], yes: true, picked: terms.map((m) => unquoted(m![2])) };
    return { mode: "text" as const, question: inputs[0]?.id ?? "", yes: true, picked: [] };
  }
  // The form starts at what the mod has when the dialog opens.
  const start = untrack(() => read(current));

  let mode = $state<"existing" | "new" | "text">(untrack(() => (inputs.length ? start.mode : "new")));
  let question = $state(start.question);
  let yes = $state(start.yes);
  const picked = new SvelteSet<string>(start.picked);
  let text = $state(untrack(() => current ?? ""));
  let fresh = $state<Fresh>({ name: "", kind: "bool", query: "", choices: [], fallback: "false" });
  let choicesText = $state("");
  let busy = $state(false);

  const asked = $derived(inputs.find((i) => i.id === question));
  const freshChoices = $derived(
    choicesText
      .split("\n")
      .map((l) => l.trim())
      .filter(Boolean),
  );

  function forAnswers(name: string, kind: string): string {
    if (kind === "bool") return yes ? name : `!${name}`;
    if (kind === "choice") return [...picked].map((v) => `${name} == ${quoted(v)}`).join(" || ");
    return name;
  }
  const condition = $derived(
    mode === "text"
      ? text.trim()
      : mode === "existing"
        ? asked
          ? forAnswers(asked.id, asked.type)
          : ""
        : fresh.name.trim()
          ? forAnswers(fresh.name.trim(), fresh.kind)
          : "",
  );
  const ready = $derived(
    !!condition &&
      (mode !== "new" ||
        (/^[A-Za-z0-9_.-]+$/.test(fresh.name.trim()) &&
          !inputs.some((i) => i.id === fresh.name.trim()) &&
          !!fresh.query.trim() &&
          (fresh.kind === "bool" || freshChoices.length >= 2))),
  );

  // A different question's answers start empty.
  function ask(next: string) {
    question = next;
    picked.clear();
    yes = true;
  }

  async function ok() {
    busy = true;
    try {
      const made =
        mode === "new"
          ? {
              ...fresh,
              name: fresh.name.trim(),
              choices: fresh.kind === "choice" ? freshChoices : [],
              fallback: fresh.kind === "choice" ? (freshChoices.includes(fresh.fallback) ? fresh.fallback : freshChoices[0]) : fresh.fallback,
            }
          : null;
      if (await save(condition, made)) close();
    } finally {
      busy = false;
    }
  }
</script>

<Dialog title="When is {id} installed?" {close}>
  <nav class="modes">
    <button class:on={mode === "existing"} onclick={() => (mode = "existing")} disabled={!inputs.length}>A question the list asks</button>
    <button class:on={mode === "new"} onclick={() => (mode = "new")}>A new question</button>
    <button class:on={mode === "text"} onclick={() => (mode = "text")}>Written out</button>
  </nav>

  {#if mode === "existing"}
    <label class="field">
      <span>Question</span>
      <select value={question} onchange={(e) => ask(e.currentTarget.value)}>
        {#each inputs as input}<option value={input.id}>{input.query || input.id}</option>{/each}
      </select>
    </label>
    {#if asked}
      {@render answers(asked.type, asked.choices ?? [])}
    {/if}
  {:else if mode === "new"}
    <label class="field">
      <span>Asked as</span>
      <input type="text" bind:value={fresh.query} placeholder="e.g. Install the big texture packs?" />
    </label>
    <label class="field">
      <span>Its name</span>
      <input type="text" bind:value={fresh.name} placeholder="e.g. big_textures" />
    </label>
    {#if fresh.name.trim() && inputs.some((i) => i.id === fresh.name.trim())}
      <p class="bad">The list already has a question called {fresh.name.trim()}.</p>
    {/if}
    <div class="field">
      <span>Answered</span>
      <span class="radios">
        <label><input type="radio" bind:group={fresh.kind} value="bool" /> yes or no</label>
        <label><input type="radio" bind:group={fresh.kind} value="choice" /> one of several answers</label>
      </span>
    </div>
    {#if fresh.kind === "choice"}
      <label class="field">
        <span>Answers</span>
        <textarea rows="3" bind:value={choicesText} placeholder="one per line"></textarea>
      </label>
      {#if freshChoices.length}
        <label class="field">
          <span>Unless chosen</span>
          <select bind:value={fresh.fallback}>{#each freshChoices as c}<option>{c}</option>{/each}</select>
        </label>
      {/if}
    {:else}
      <label class="field">
        <span>Unless chosen</span>
        <select bind:value={fresh.fallback}><option value="false">no</option><option value="true">yes</option></select>
      </label>
    {/if}
    {@render answers(fresh.kind, freshChoices)}
  {:else}
    <label class="field">
      <span>Only if</span>
      <input type="text" bind:value={text} placeholder="e.g. big_textures && !mod:Other" />
    </label>
    <p class="dim">
      Questions by name, <code>name == answer</code>, <code>mod:id</code> for another mod, joined with
      <code>&&</code> and <code>||</code>, each negated with <code>!</code>.
    </p>
  {/if}

  {#if condition}<p class="dim">Written as <code>if = {JSON.stringify(condition)}</code></p>{/if}

  {#snippet foot()}
    <button onclick={close}>Cancel</button>
    <button class="primary" onclick={ok} disabled={busy || !ready}>Make it optional</button>
  {/snippet}
</Dialog>

{#snippet answers(kind: string, choices: string[])}
  <div class="field">
    <span>Installed when</span>
    {#if kind === "bool"}
      <span class="radios">
        <label><input type="radio" bind:group={yes} value={true} /> the answer is yes</label>
        <label><input type="radio" bind:group={yes} value={false} /> the answer is no</label>
      </span>
    {:else if kind === "choice"}
      <span class="radios">
        {#each choices as choice}
          <label
            ><input
              type="checkbox"
              checked={picked.has(choice)}
              onchange={(e) => (e.currentTarget.checked ? picked.add(choice) : picked.delete(choice))}
            />
            {choice}</label
          >
        {/each}
        {#if !choices.length}<span class="dim">Its answers, above, first.</span>{/if}
      </span>
    {:else}
      <span class="dim">it is answered at all</span>
    {/if}
  </div>
{/snippet}

<style>
  .modes {
    display: flex;
    gap: 4px;
    margin-bottom: 12px;
  }
  .modes button.on {
    background: var(--accent-soft);
    border-color: var(--accent);
    color: var(--accent);
  }
  .field {
    display: grid;
    grid-template-columns: 110px minmax(0, 1fr);
    gap: 8px;
    align-items: baseline;
    margin: 6px 0;
  }
  .field > span:first-child {
    color: var(--dim);
  }
  .radios {
    display: flex;
    flex-direction: column;
    gap: 3px;
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
  .bad {
    color: var(--problem);
  }
  code {
    overflow-wrap: anywhere;
  }
</style>
