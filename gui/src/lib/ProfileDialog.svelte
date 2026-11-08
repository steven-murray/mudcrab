<script lang="ts">
  import { untrack } from "svelte";
  import { SvelteSet } from "svelte/reactivity";
  import { api, type AuthorProfile, type InputDoc } from "./api";

  // A profile's answers, for Author mode: the set of answers the list is
  // looked at by. A question whose `if` does not hold is hidden; a default
  // that follows an earlier answer moves with it until the question is
  // answered.
  let {
    profile,
    inputs,
    saved,
    close,
  }: {
    profile: AuthorProfile;
    inputs: InputDoc[];
    /** The profile as saved. */
    saved: (profile: AuthorProfile) => void;
    close: () => void;
  } = $props();

  const asText = (value: unknown) => (Array.isArray(value) ? value.join(",") : value === undefined ? "" : String(value));
  // Read once: the dialog is made for one profile.
  const start = untrack(() => profile);
  let answers = $state<Record<string, string>>(
    Object.fromEntries(untrack(() => inputs).map((i) => [i.id, start.answers[i.id] ?? asText(i.default)])),
  );
  const given = new SvelteSet<string>(Object.keys(start.answers));
  let asked = $state<Record<string, boolean>>({});
  let reading = 0;
  $effect(() => {
    const now = JSON.stringify(answers);
    const mine = ++reading;
    api
      .questionStates(JSON.parse(now))
      .then((states) => {
        if (mine !== reading) return;
        asked = Object.fromEntries(states.map((q) => [q.id, q.asked]));
        untrack(() => {
          for (const q of states)
            if (!given.has(q.id) && q.default !== undefined && answers[q.id] !== asText(q.default)) answers[q.id] = asText(q.default);
        });
      })
      .catch(() => {});
  });
  const pickedFor = (id: string) => answers[id].split(",").filter(Boolean);
  function tick(id: string, choices: string[], choice: string, on: boolean) {
    const now = new Set(pickedFor(id));
    if (on) now.add(choice);
    else now.delete(choice);
    answers[id] = choices.filter((c) => now.has(c)).join(",");
    given.add(id);
  }

  let error = $state<string | null>(null);
  let saving = $state(false);
  async function save() {
    saving = true;
    error = null;
    try {
      // Only the answers given: the rest keep following their defaults.
      const kept = Object.fromEntries(Object.entries(answers).filter(([id]) => given.has(id)));
      saved(await api.setAuthorAnswers(kept));
      close();
    } catch (err) {
      error = String(err);
    } finally {
      saving = false;
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="veil" onclick={close}>
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="dialog" role="dialog" tabindex="-1" aria-label="Profile" onclick={(e) => e.stopPropagation()}>
    <div class="dhead">
      <strong>Profile: {start.name}</strong>
      <button class="link" onclick={close}>Cancel</button>
    </div>
    <div class="dbody">
      <p class="dim small">
        The answers Author mode plans the list with: which mods it installs, and what a mod's archives, installer
        choices and actions with an <code>if</code> do. Mods these answers leave out are still shown, as they would
        install. Install mode offers the same profiles.
      </p>
      {#each inputs.filter((i) => asked[i.id] !== false) as input (input.id)}
        <div class="question" onchange={() => given.add(input.id)} oninput={() => given.add(input.id)}>
          <div class="query">{input.query} <code class="dim">{input.id}</code></div>
          {#if input.type === "bool"}
            <div class="choices">
              {#each ["true", "false"] as value}
                <label><input type="radio" name={input.id} {value} bind:group={answers[input.id]} /> {value === "true" ? "Yes" : "No"}</label>
              {/each}
            </div>
          {:else if input.type === "multi"}
            <div class="choices">
              {#each input.choices ?? [] as choice}
                <label
                  ><input
                    type="checkbox"
                    checked={pickedFor(input.id).includes(choice)}
                    onchange={(e) => tick(input.id, input.choices ?? [], choice, e.currentTarget.checked)}
                  />
                  {choice}</label
                >
              {/each}
            </div>
          {:else if input.type === "choice"}
            <select bind:value={answers[input.id]}>
              {#if answers[input.id] === ""}<option value="">Choose…</option>{/if}
              {#each input.choices ?? [] as choice}<option value={choice}>{choice}</option>{/each}
            </select>
          {:else}
            <input type="text" bind:value={answers[input.id]} />
          {/if}
        </div>
      {:else}
        <p class="dim">The list asks no questions.</p>
      {/each}
      {#if error}<pre class="error">{error}</pre>{/if}
    </div>
    <div class="dfoot">
      <button onclick={close}>Cancel</button>
      <button class="primary" onclick={save} disabled={saving}>Save</button>
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
    width: min(600px, 94vw);
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
  .dfoot {
    border-top: 1px solid var(--line);
    justify-content: flex-end;
  }
  .dbody {
    padding: 10px 18px 16px;
    overflow: auto;
  }
  .question {
    margin: 12px 0;
  }
  .query {
    margin-bottom: 4px;
  }
  .choices {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
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
