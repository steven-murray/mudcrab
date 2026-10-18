<script lang="ts">
  import { onMount } from "svelte";
  import { api, type Answer, type Walk } from "./api";
  import InstallerQuestions from "./InstallerQuestions.svelte";

  // An archive's installer walked again (`mudcrab choose`): each question
  // starts at what the list chooses now, and the new choices are written as
  // the archive's `fomod_selections` or `bain_subpackages`.
  let {
    id,
    number,
    label,
    write,
    close,
  }: {
    id: string;
    number: number;
    label: string;
    /** Write the choices; resolves true when written. */
    write: (answers: Answer[]) => Promise<boolean>;
    close: () => void;
  } = $props();

  let answers = $state<Answer[]>([]);
  let walk = $state<Walk | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);

  async function step() {
    busy = true;
    error = null;
    try {
      walk = await api.chooseWalk(id, number, answers);
    } catch (err) {
      error = String(err);
    } finally {
      busy = false;
    }
  }
  onMount(step);

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
      let next = await api.chooseWalk(id, number, answers);
      while (next.kind === "ask") {
        answers = [...answers, next.asked.kind === "choose" ? { pick: next.asked.question.default } : { yes: false }];
        next = await api.chooseWalk(id, number, answers);
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
    try {
      if (await write(answers)) close();
    } finally {
      busy = false;
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="veil" onclick={close}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="dialog" role="dialog" tabindex="-1" aria-label="Choose the installer's options" onclick={(e) => e.stopPropagation()}>
    <div class="dhead">
      <strong>{id}: {label}</strong>
      <button class="link" onclick={close}>Cancel</button>
    </div>
    <div class="dbody">
      <p class="dim small">Each question starts at what the list chooses now.</p>
      {#if error}<pre class="error">{error}</pre>{/if}
      <InstallerQuestions {walk} answered={answers.length} {busy} {answer} {back} {defaults} />
    </div>
    <div class="dfoot">
      <button onclick={close}>Cancel</button>
      <button class="primary" onclick={save} disabled={busy || walk?.kind !== "done"}>Save the choices</button>
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
