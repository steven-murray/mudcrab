<script lang="ts">
  import type { Docs } from "./api";
  import { inline, markdown } from "./markdown";

  // A mod's notes -- its title, one-line summary and body, all Markdown --
  // read as the guide shows them, and written (`docs`) where they already
  // are: inline in the list, or in its docs file.
  let {
    id,
    docs,
    save,
  }: {
    id: string;
    docs: Docs;
    /** Write the changed fields; resolves true when written. */
    save: (fields: [string, string | null][]) => Promise<boolean>;
  } = $props();

  let editing = $state(false);
  let preview = $state(false);
  let draft = $state({ title: "", summary: "", body: "" });

  function start() {
    draft = { title: docs.title ?? "", summary: docs.summary ?? "", body: docs.body ?? "" };
    preview = false;
    editing = true;
  }

  // Only what changed is written.
  const changed = $derived(
    (["title", "summary", "body"] as const).filter((key) => draft[key].trim() !== (docs[key] ?? "").trim()),
  );

  async function write() {
    const fields: [string, string | null][] = changed.map((key) => [key, draft[key].trim() ? draft[key] : null]);
    if (await save(fields)) editing = false;
  }

  // A different mod chosen while editing: leave the draft behind.
  $effect(() => {
    void id;
    editing = false;
  });
</script>

<section class="notes">
  <div class="head">
    <h4>Notes</h4>
    {#if !editing}<button class="link" onclick={start}>{docs.summary || docs.body ? "Edit" : "Write notes"}</button>{/if}
  </div>
  {#if editing}
    <label>Title <input type="text" bind:value={draft.title} placeholder="shown instead of the id, when set" /></label>
    <label>Summary <input type="text" bind:value={draft.summary} placeholder="one line: what it is, or why it is in the list" /></label>
    <div class="bodyhead">
      <span>Body <span class="dim">(Markdown)</span></span>
      <button class="link" onclick={() => (preview = !preview)}>{preview ? "Write" : "Preview"}</button>
    </div>
    {#if preview}
      <div class="prose preview">{@html markdown(draft.body || "*Nothing yet.*")}</div>
    {:else}
      <textarea bind:value={draft.body} rows="10" placeholder="instructions, caveats, why it is set up this way"></textarea>
    {/if}
    <div class="buttons">
      <button class="primary" onclick={write} disabled={!changed.length}>Save</button>
      <button onclick={() => (editing = false)}>Cancel</button>
      {#if changed.length}<span class="dim small">changes: {changed.join(", ")}</span>{/if}
    </div>
  {:else if docs.summary || docs.body}
    {#if docs.summary}<p class="summary">{@html inline(docs.summary)}</p>{/if}
    {#if docs.body}<div class="prose">{@html markdown(docs.body)}</div>{/if}
  {:else}
    <p class="dim small">No notes for this mod yet.</p>
  {/if}
</section>

<style>
  .notes {
    margin: 10px 0;
  }
  .head {
    display: flex;
    gap: 10px;
    align-items: baseline;
  }
  h4 {
    font-size: 13px;
    margin: 0 0 4px;
  }
  label {
    display: grid;
    grid-template-columns: 70px minmax(0, 1fr);
    gap: 8px;
    align-items: center;
    font-size: 13px;
    margin: 4px 0;
  }
  .bodyhead {
    display: flex;
    justify-content: space-between;
    font-size: 13px;
    margin-top: 6px;
  }
  textarea {
    width: 100%;
    font: 13px/1.45 var(--mono);
    color: inherit;
    background: var(--card);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 6px 8px;
    resize: vertical;
  }
  .preview {
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 6px 10px;
    min-height: 80px;
  }
  .buttons {
    display: flex;
    gap: 6px;
    align-items: center;
    margin-top: 6px;
  }
  .summary {
    margin: 2px 0 6px;
    color: var(--dim);
  }
  .small {
    font-size: 12px;
  }
</style>
