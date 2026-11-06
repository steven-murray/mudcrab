<script lang="ts">
  import type { Answer, InputDoc, Question, Row, Walk } from "./api";

  // An installer's next question, as the library's walk asks it: a FOMOD
  // group, a BAIN wizard's question, a BAIN package list, or a wizard asking
  // whether a file is installed. What install (or the list) would pick is
  // ticked to start with. Shared by adding a mod and choosing again; given
  // the list's questions, a FOMOD group's options can depend on them, a row
  // of options per answer.
  let {
    walk,
    answered,
    busy,
    answer,
    back,
    defaults,
    inputs = [],
    current = {},
  }: {
    walk: Walk | null;
    /** How many questions are answered, for Back. */
    answered: number;
    busy: boolean;
    answer: (given: Answer) => void;
    back: () => void;
    /** Take every remaining question's default. */
    defaults: () => void;
    /** The list's questions, for a FOMOD group's options to depend on. */
    inputs?: InputDoc[];
    /** The answers now, to tell which row the walk goes on with. */
    current?: Record<string, string>;
  } = $props();

  const question = $derived(walk?.kind === "ask" && walk.asked.kind === "choose" ? walk.asked.question : null);
  // Its rows, when it is a FOMOD group (`null` otherwise).
  const prior = $derived(walk?.kind === "ask" && walk.asked.kind === "choose" ? (walk.asked.rows ?? null) : null);
  let picked = $state<number[]>([]);
  // The group's options per condition, when they depend on the questions.
  let rows = $state<Row[] | null>(null);
  let asking = $state(false);
  $effect(() => {
    picked = question ? [...question.default] : [];
    rows = prior?.length ? prior.map((r) => ({ if: r.if, pick: [...r.pick] })) : null;
    asking = false;
  });
  // A question's rows can only follow its answers: yes or no, or one choice.
  const askable = $derived(inputs.filter((i) => !i.optional_group && (i.type === "bool" || i.type === "choice")));

  function dependOn(input: InputDoc) {
    const start = question ? [...question.default] : [];
    rows =
      input.type === "bool"
        ? [
            { if: input.id, pick: [...start] },
            { if: `!${input.id}`, pick: [...start] },
          ]
        : // A condition's quotes hold no quotes.
          (input.choices ?? [])
            .filter((c) => !c.includes('"'))
            .map((c) => ({ if: `${input.id} == "${c}"`, pick: [...start] }));
    asking = false;
  }

  // Whether `condition` holds on the answers now, where it is one term this
  // can read (`flag`, `!flag`, `key == value`, `key != value`); else null.
  function holds(condition: string): boolean | null {
    const term = condition.trim();
    const compared = /^([A-Za-z0-9_]+)\s*(==|!=)\s*(?:"([^"]*)"|([^\s"]+))$/.exec(term);
    if (compared) {
      const [, key, op, inQuotes, bare] = compared;
      if (!(key in current)) return null;
      const value = inQuotes ?? bare;
      return (current[key] === value) === (op === "==");
    }
    const flag = /^(!?)([A-Za-z0-9_]+)$/.exec(term);
    if (flag) {
      const [, not, key] = flag;
      if (!(key in current)) return null;
      return (current[key] === "true") !== (not === "!");
    }
    return null;
  }
  // What the walk goes on with: the first row that holds now, or the
  // group's default (what the list chooses now) when none can be told.
  const now = $derived(rows?.find((r) => holds(r.if) === true)?.pick ?? question?.default ?? []);

  function next() {
    if (rows) answer({ depends: { now, rows: rows.map((r) => ({ if: r.if, pick: r.pick })) } });
    else answer({ pick: picked });
  }

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

  function toggled(q: Question, from: number[], index: number): number[] {
    if (from.includes(index)) return q.rule === "exactly_one" ? from : from.filter((i) => i !== index);
    return single ? [index] : [...from, index];
  }
</script>

{#snippet options(q: Question, from: number[], set: (next: number[]) => void)}
  {#each q.choices as choice, i}
    <label class="choice" class:off={choice.availability === "unavailable"} title={choice.description}>
      <input
        type={single ? "radio" : "checkbox"}
        checked={choice.availability === "required" || from.includes(i)}
        disabled={choice.availability === "required" || choice.availability === "unavailable"}
        onclick={(e) => {
          e.preventDefault();
          set(toggled(q, from, i));
        }}
      />
      <span>
        {choice.name}
        {#if choice.availability === "recommended"}<span class="tag good">recommended</span>{/if}
        {#if choice.availability === "required"}<span class="tag">always</span>{/if}
        {#if q.default.includes(i)}<span class="tag">default</span>{/if}
        {#if choice.description}<span class="desc dim">{choice.description.split("\n")[0]}</span>{/if}
      </span>
    </label>
  {/each}
{/snippet}

{#if busy && !walk}
  <p class="dim">Reading the installer…</p>
{:else if question}
  <div class="question">
    <div class="qtitle">{question.title} <span class="dim small">{rule}</span></div>
    {#if rows}
      <p class="dim small">
        Its options depend on the list's questions: a row for each condition, written with its <code>if</code>.
        Where none holds, the installer's own default is taken.
      </p>
      {#each rows as row, r (r)}
        {@const holdsNow = holds(row.if)}
        <div class="row">
          <div class="rowhead">
            <span class="dim small">if</span>
            <input class="cond" bind:value={row.if} placeholder="a condition, as in an if" spellcheck="false" />
            {#if holdsNow === true}<span class="tag good" title="It holds on the answers now: the walk goes on with it">now</span>{/if}
            <button class="link" onclick={() => (rows = rows!.filter((_, i) => i !== r))} title="Take this row out">✕</button>
          </div>
          {@render options(question, row.pick, (next) => (row.pick = next))}
        </div>
      {/each}
      <div class="buttons">
        <button class="link" onclick={() => (rows = [...rows!, { if: "", pick: [...question.default] }])}>Add a row</button>
        <button class="link" onclick={() => (rows = null)}>Doesn't depend on a question</button>
      </div>
    {:else}
      {@render options(question, picked, (next) => (picked = next))}
      {#if prior && askable.length}
        {#if asking}
          <div class="ask small">
            Depends on:
            {#each askable as input (input.id)}
              <button class="link" onclick={() => dependOn(input)} title={input.query}>{input.id}</button>
            {/each}
            <button class="link dim" onclick={() => (asking = false)}>never mind</button>
          </div>
        {:else}
          <button class="link small" onclick={() => (asking = true)} title="Choose this group's options once per answer to one of the list's questions"
            >Depends on a question…</button
          >
        {/if}
      {/if}
    {/if}
    <div class="buttons">
      {#if answered}<button onclick={back} disabled={busy}>Back</button>{/if}
      <button class="primary" onclick={next} disabled={busy || (!!rows && (!rows.length || rows.some((r) => !r.if.trim())))}
        >Next</button
      >
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
  .row {
    border-left: 2px solid var(--line);
    padding: 2px 0 2px 10px;
    margin: 8px 0;
  }
  .rowhead {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .cond {
    flex: 1;
    font-family: var(--mono, monospace);
    font-size: 12px;
  }
  .ask {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 6px;
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
