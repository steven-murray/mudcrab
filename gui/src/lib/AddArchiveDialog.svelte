<script lang="ts">
  import { onMount } from "svelte";
  import { api, type Answer, type Started, type Walk } from "./api";
  import InstallerQuestions from "./InstallerQuestions.svelte";

  // Adding an archive to a mod already in the list: its source, and its
  // installer answered (a FOMOD's steps, a BAIN wizard's questions, or
  // which BAIN packages); written after the mod's own archives.
  let {
    path,
    modId,
    add,
    close,
  }: {
    path: string;
    modId: string;
    /** Write it; resolves to why it was not written, or null. */
    add: (request: { path: string; source: string | null; answers: Answer[] }) => Promise<string | null>;
    close: () => void;
  } = $props();

  let started = $state<Started | null>(null);
  let source = $state("");
  let answers = $state<Answer[]>([]);
  let walk = $state<Walk | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);

  onMount(async () => {
    try {
      started = await api.addStart(path);
      source = started.source;
      await step();
    } catch (err) {
      error = String(err);
    }
  });

  async function step() {
    error = null;
    busy = true;
    try {
      walk = await api.addWalk(path, answers);
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
    await step();
  }
  async function defaults() {
    busy = true;
    try {
      let next = await api.addWalk(path, answers);
      while (next.kind === "ask") {
        answers = [...answers, next.asked.kind === "choose" ? { pick: next.asked.question.default } : { yes: false }];
        next = await api.addWalk(path, answers);
      }
      walk = next;
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }
  async function save() {
    busy = true;
    error = await add({ path, source: source.trim() || null, answers: $state.snapshot(answers) as Answer[] });
    busy = false;
    if (!error) close();
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="veil" onclick={close}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="dialog" role="dialog" tabindex="-1" aria-label="Add an archive to a mod" onclick={(e) => e.stopPropagation()}>
    <div class="dhead">
      <strong>Add {started?.file_name ?? "an archive"} to {modId}</strong>
      <button class="link" onclick={close}>Cancel</button>
    </div>
    <div class="dbody">
      {#if started}
        <p class="dim small">{started.layout_label}</p>
        <label class="field">
          <span>Source</span>
          <input bind:value={source} spellcheck="false" />
        </label>
        {#if started.why_manual}<p class="dim small">{started.why_manual}</p>{/if}
      {/if}
      {#if error}<pre class="error">{error}</pre>{/if}
      <InstallerQuestions {walk} answered={answers.length} {busy} {answer} {back} {defaults} />
    </div>
    <div class="dfoot">
      <button onclick={close}>Cancel</button>
      <button class="primary" onclick={save} disabled={busy || walk?.kind !== "done"}>Add to {modId}</button>
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
    width: min(720px, 94vw);
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
  .field {
    display: grid;
    grid-template-columns: 70px 1fr;
    align-items: center;
    gap: 8px;
    margin: 8px 0;
  }
  .field input {
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
