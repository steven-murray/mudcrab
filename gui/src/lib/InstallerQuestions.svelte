<script lang="ts">
  import type { Answer, Question, Walk } from "./api";

  // An installer's next question, as the library's walk asks it: a FOMOD
  // group, a BAIN wizard's question, a BAIN package list, or a wizard asking
  // whether a file is installed. What install (or the list) would pick is
  // ticked to start with. Shared by adding a mod and choosing again.
  let {
    walk,
    answered,
    busy,
    answer,
    back,
    defaults,
  }: {
    walk: Walk | null;
    /** How many questions are answered, for Back. */
    answered: number;
    busy: boolean;
    answer: (given: Answer) => void;
    back: () => void;
    /** Take every remaining question's default. */
    defaults: () => void;
  } = $props();

  const question = $derived(walk?.kind === "ask" && walk.asked.kind === "choose" ? walk.asked.question : null);
  let picked = $state<number[]>([]);
  $effect(() => {
    picked = question ? [...question.default] : [];
  });

  const single = $derived(question?.rule === "exactly_one" || question?.rule === "at_most_one");
  const rule = $derived(
    question
      ? {
          exactly_one: "Choose one.",
          at_most_one: "Choose one, or none.",
          at_least_one: "Choose one or more.",
          any: "Choose any, or none.",
          all: "All are installed.",
        }[question.rule]
      : "",
  );

  function toggle(q: Question, index: number) {
    if (picked.includes(index)) picked = q.rule === "exactly_one" ? picked : picked.filter((i) => i !== index);
    else picked = single ? [index] : [...picked, index];
  }
</script>

{#if busy && !walk}
  <p class="dim">Reading the installer…</p>
{:else if question}
  <div class="question">
    <div class="qtitle">{question.title} <span class="dim small">{rule}</span></div>
    {#each question.choices as choice, i}
      <label class="choice" class:off={choice.availability === "unavailable"} title={choice.description}>
        <input
          type={single ? "radio" : "checkbox"}
          checked={choice.availability === "required" || picked.includes(i)}
          disabled={choice.availability === "required" || choice.availability === "unavailable"}
          onclick={(e) => {
            e.preventDefault();
            toggle(question, i);
          }}
        />
        <span>
          {choice.name}
          {#if choice.availability === "recommended"}<span class="tag good">recommended</span>{/if}
          {#if choice.availability === "required"}<span class="tag">always</span>{/if}
          {#if question.default.includes(i)}<span class="tag">default</span>{/if}
          {#if choice.description}<span class="desc dim">{choice.description.split("\n")[0]}</span>{/if}
        </span>
      </label>
    {/each}
    <div class="buttons">
      {#if answered}<button onclick={back} disabled={busy}>Back</button>{/if}
      <button class="primary" onclick={() => answer({ pick: picked })} disabled={busy}>Next</button>
      <button onclick={defaults} disabled={busy} title="Take the default for every question left">Defaults for the rest</button>
    </div>
  </div>
{:else if walk?.kind === "ask" && walk.asked.kind === "yes_no"}
  <div class="question">
    <div class="qtitle">
      The wizard asks whether <code>{walk.asked.path}</code> is already installed, by a mod earlier in the list. Is it?
    </div>
    <div class="buttons">
      {#if answered}<button onclick={back} disabled={busy}>Back</button>{/if}
      <button onclick={() => answer({ yes: true })} disabled={busy}>Yes</button>
      <button onclick={() => answer({ yes: false })} disabled={busy}>No</button>
    </div>
  </div>
{:else if walk?.kind === "done"}
  <p class="small">
    Answered{#if answered} ({answered} question(s)){/if}.
    {#if answered}<button class="link" onclick={back}>Back</button>{/if}
  </p>
  {#each walk.installed.notes as note}<p class="note small">{note}</p>{/each}
{/if}

<style>
  .question {
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 8px 12px;
  }
  .qtitle {
    font-weight: 600;
    margin-bottom: 6px;
  }
  label.choice {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin: 4px 0;
    font-size: 13px;
  }
  .choice.off {
    color: var(--faint);
  }
  .desc {
    display: block;
    font-size: 12px;
  }
  .tag {
    font-size: 10px;
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 0 4px;
    margin-left: 4px;
    color: var(--dim);
  }
  .tag.good {
    color: var(--good);
    border-color: var(--good);
  }
  .buttons {
    display: flex;
    gap: 6px;
    margin-top: 8px;
  }
  .small {
    font-size: 12px;
  }
  .note {
    color: var(--note);
    margin: 2px 0;
  }
</style>
