<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { api, type Answer, type InputDoc, type Walk } from "./api";
  import ConditionPicker from "./ConditionPicker.svelte";
  import InstallerQuestions from "./InstallerQuestions.svelte";

  // An archive's installer walked again (`mudcrab choose`): each question
  // starts at what the list chooses now, and the new choices are written as
  // the archive's `fomod_selections` or `bain_subpackages`. The choices are
  // sets, one per condition: the base set, when none holds, and each
  // condition's, overriding it where it holds -- walked one at a time.
  let {
    id,
    number,
    label,
    inputs,
    current,
    write,
    close,
  }: {
    id: string;
    number: number;
    label: string;
    /** The list's questions, and the answers now. */
    inputs: InputDoc[];
    current: Record<string, string>;
    /** Write the choices, as set `set`'s; resolves true when written. */
    write: (answers: Answer[], set: string | null) => Promise<boolean>;
    close: () => void;
  } = $props();

  let answers = $state<Answer[]>([]);
  let walk = $state<Walk | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);

  // The set walked: its condition, null for the base; and the conditions
  // the archive's choices already name.
  let condition = $state<string | null>(null);
  const set = $derived(condition?.trim() || null);
  let known = $state<string[]>([]);
  onMount(async () => {
    try {
      known = await api.installerSets(id, number);
    } catch {
      known = [];
    }
  });
  // Another set chosen: walked again from its choices (once typing stops).
  let walked: string | null | undefined = undefined;
  let timer: ReturnType<typeof setTimeout> | undefined;
  $effect(() => {
    const now = set;
    if (now === walked) return;
    clearTimeout(timer);
    timer = setTimeout(
      () =>
        untrack(() => {
          walked = now;
          answers = [];
          walk = null;
          step();
        }),
      walked === undefined ? 0 : 400,
    );
  });

  async function step() {
    busy = true;
    error = null;
    try {
      walk = await api.chooseWalk(id, number, answers, set);
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
      let next = await api.chooseWalk(id, number, answers, set);
      while (next.kind === "ask") {
        const asked = next.asked;
        // A group's rows that depend on the questions are kept as they are.
        const given: Answer =
          asked.kind !== "choose"
            ? { yes: false }
            : asked.rows?.length
              ? { depends: { now: asked.question.default, rows: asked.rows } }
              : { pick: asked.question.default };
        answers = [...answers, given];
        next = await api.chooseWalk(id, number, answers, set);
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
      if (await write(answers, set)) close();
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
      <div class="sets">
        <ConditionPicker bind:value={condition} {known} {inputs} />
        <p class="dim small">
          {#if set}
            The choices <b>if {set}</b>: where it holds, they take the place of the base's, group by group (BAIN
            packages, all of them). Each question starts at what this set chooses now.
          {:else}
            The base choices: those installed when no condition holds. Each question starts at what the list chooses
            now.
          {/if}
        </p>
      </div>
      {#if error}<pre class="error">{error}</pre>{/if}
      <InstallerQuestions {walk} answered={answers.length} {busy} {answer} {back} {defaults} {inputs} {current} />
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
  .sets {
    margin-bottom: 8px;
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
