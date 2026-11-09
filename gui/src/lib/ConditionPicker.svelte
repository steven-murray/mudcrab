<script lang="ts">
  import type { InputDoc } from "./api";

  // When something applies: always, under a condition already used, or
  // under a new one -- typed, or made from one of the list's questions and
  // an answer to it. With `required`, only a condition: typed, or made from
  // a question, an answer already `taken` left out.
  let {
    value = $bindable(),
    known = [],
    inputs,
    required = false,
    taken = [],
  }: {
    /** The condition, or null for always. */
    value: string | null;
    /** Conditions already in use, offered first. */
    known?: string[];
    inputs: InputDoc[];
    /** A condition must be given: no always, nothing already used offered. */
    required?: boolean;
    /** Conditions already used, their answers not offered again. */
    taken?: string[];
  } = $props();

  let other = $state(false);
  const same = (a: string, b: string) => a.replace(/["\s]/g, "") === b.replace(/["\s]/g, "");
  let question = $state("");
  const askable = $derived(inputs.filter((i) => !i.optional_group && (i.type === "bool" || i.type === "choice")));
  const asked = $derived(askable.find((i) => i.id === question) ?? null);
  // A condition's quotes hold no quotes.
  const answers = $derived(
    asked?.type === "bool"
      ? [
          { label: "yes", condition: asked.id },
          { label: "no", condition: `!${asked.id}` },
        ]
      : (asked?.choices ?? []).filter((c) => !c.includes('"')).map((c) => ({ label: c, condition: `${asked!.id} == "${c}"` })),
  );
  const offered = $derived(answers.filter((a) => !taken.some((t) => same(t, a.condition))));
</script>

{#if !required}<div class="picker">
  <span class="dim small">Applies</span>
  <button class="chip" class:on={value === null && !other} onclick={() => ((value = null), (other = false))}>always</button>
  {#each known as condition (condition)}
    <button class="chip" class:on={value === condition && !other} onclick={() => ((value = condition), (other = false))}
      >if <code>{condition}</code></button
    >
  {/each}
  <button class="chip" class:on={other} onclick={() => ((other = true), (value = value ?? ""))}>another condition…</button>
</div>{/if}
{#if other || required}
  <div class="other">
    <span class="dim small">if</span>
    <input class="cond" bind:value placeholder="a condition, as in an if" spellcheck="false" />
    {#if askable.length}
      <select bind:value={question} title="Make it from one of the list's questions">
        <option value="">from a question…</option>
        {#each askable as input (input.id)}<option value={input.id}>{input.id}</option>{/each}
      </select>
      {#if asked}
        {#each offered as answer (answer.condition)}
          <button class="chip" class:on={value === answer.condition} onclick={() => (value = answer.condition)}>{answer.label}</button>
        {:else}
          <span class="dim small">every answer has a set</span>
        {/each}
      {/if}
    {/if}
  </div>
{/if}

<style>
  .picker,
  .other {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .other {
    margin-top: 6px;
  }
  .chip {
    border: 1px solid var(--line);
    border-radius: 12px;
    padding: 1px 10px;
    font-size: 12px;
    background: none;
  }
  .chip.on {
    border-color: var(--accent);
    color: var(--accent);
  }
  .cond {
    flex: 1;
    min-width: 160px;
    font-family: var(--mono, monospace);
    font-size: 12px;
  }
  .small {
    font-size: 12px;
  }
</style>
