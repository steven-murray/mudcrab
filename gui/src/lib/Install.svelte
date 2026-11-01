<script lang="ts">
  import { onDestroy, onMount, untrack } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import {
    api,
    type Bar,
    type ByHand,
    type Done,
    type LibEvent,
    type ListView,
    type Locations,
    type StepByHand,
  } from "./api";
  import Text from "./Text.svelte";

  let {
    list,
    locations = $bindable(),
    openFolders,
    running = $bindable(false),
  }: {
    list: ListView;
    /** Where things go, set in the Settings dialog; the profile is chosen here. */
    locations: Locations | null;
    openFolders: () => void;
    /** Whether an install is running, for the Settings dialog to wait on. */
    running?: boolean;
  } = $props();

  // --- Questions ------------------------------------------------------------
  // Answers as text, as `query --set` takes them: the last ones given here,
  // else the list's defaults.
  // Read once: this panel is made afresh for each list opened.
  const initial: Record<string, string> = untrack(() => {
    const out: Record<string, string> = {};
    for (const input of list.document.inputs) {
      const saved = list.answers[input.id];
      out[input.id] = saved ?? (input.default === undefined ? "" : String(input.default));
    }
    return out;
  });
  let answers = $state<Record<string, string>>(initial);
  // Each profile has its own answers: choosing another brings its
  // answers back (or the last given anywhere, for a new one).
  let shownProfile = untrack(() => locations?.profile);
  $effect(() => {
    if (!locations || locations.profile === shownProfile) return;
    shownProfile = locations.profile;
    // What a check said about another profile's steps is not this one's.
    notes = {};
    const saved = locations.answers;
    untrack(() => {
      for (const input of list.document.inputs) {
        answers[input.id] = saved[input.id] ?? (input.default === undefined ? "" : String(input.default));
      }
    });
  });
  const unanswered = $derived(list.document.inputs.filter((i) => answers[i.id] === ""));

  // --- The run ----------------------------------------------------------------
  type State = "idle" | "running" | "waiting" | "done" | "failed";
  interface Step {
    key: string;
    title: string;
    what: string;
    state: State;
    done?: Done;
    error?: string;
    log: LibEvent[];
    bars: Bar[];
  }
  const fresh = (): Step[] => [
    { key: "plan", title: "Plan", what: "Compile the list and apply your answers", state: "idle", log: [], bars: [] },
    { key: "download", title: "Download", what: "Fetch every archive the plan needs", state: "idle", log: [], bars: [] },
    { key: "check", title: "Check", what: "Archives, layouts, actions, masters and load order", state: "idle", log: [], bars: [] },
    { key: "install", title: "Install", what: "Into the MO2 instance, and write its profile", state: "idle", log: [], bars: [] },
  ];
  let steps = $state<Step[]>(fresh());
  let current = $state(-1);
  let byHand = $state<ByHand[]>([]);
  let byHandTotal = $state(0);
  let browserFolder = $state<string | null>(null);
  let manual = $state<StepByHand[] | null>(null);
  let notes = $state<Record<string, string>>({});
  let ticked = $state<Record<string, boolean>>({});
  let stopWaiting: (() => void) | null = null;
  let manualTimer: ReturnType<typeof setInterval> | null = null;

  const ready = $derived(
    !!locations?.instance.path &&
      !!locations?.downloads.path &&
      !locations?.tools.problems.length &&
      unanswered.length === 0,
  );

  async function run() {
    running = true;
    cancelling = false;
    steps = fresh();
    manual = null;
    byHand = [];
    try {
      await step(0, async () => api.plan($state.snapshot(answers)));
      await step(1, async () => {
        const result = await api.download();
        byHand = result.by_hand;
        byHandTotal = byHand.length;
        browserFolder = result.browser_folder;
        if (byHand.length) {
          steps[1].done = result.done;
          steps[1].state = "waiting";
          await waitForDownloads();
          // The download's own line said what was missing; it is all here now.
          const log = steps[1].log;
          let stale = -1;
          log.forEach((e, i) => {
            if (e.event === "status" && e.tone === "warning") stale = i;
          });
          const settled: LibEvent = {
            event: "status",
            tone: "good",
            text: [{ kind: "plain", text: `Every archive is in the downloads folder (${byHandTotal} downloaded by hand).` }],
          };
          if (stale >= 0) log[stale] = settled;
          else log.push(settled);
          return { ...result.done, summary: `every archive is in the downloads folder, ${byHandTotal} by hand` };
        }
        return result.done;
      });
      const checked = await step(2, async () => {
        const result = await api.check(recheck);
        if (result.problems > 0) throw new Error(`${result.done.summary}. Fix these before installing.`);
        return result.done;
      });
      if (!checked) return;
      await step(3, () => api.install());
      await refreshManual();
      manualTimer = setInterval(refreshManual, 5000);
    } catch {
      // The failed step says why.
    } finally {
      running = false;
      current = -1;
    }
  }

  async function step(index: number, work: () => Promise<Done>): Promise<boolean> {
    current = index;
    steps[index].state = "running";
    try {
      steps[index].done = await work();
      steps[index].state = "done";
      return true;
    } catch (err) {
      steps[index].state = "failed";
      const message = String(err instanceof Error ? err.message : err);
      steps[index].error = cancelling
        ? "Stopped. What was finished is kept: installing again carries on from here."
        : message;
      throw err;
    }
  }

  // Stop the run at its next archive or mod: nothing is left half done.
  let cancelling = $state(false);
  async function cancel() {
    cancelling = true;
    stopWaiting?.();
    await api.cancelRun();
  }

  // Check again a plan that already passed, rather than skipping it.
  let recheck = $state(false);

  // The profile: its name (the MO2 profile it installs into) and its answers.
  let newProfile = $state("");
  let profileError = $state<string | null>(null);
  async function chooseProfile(name: string) {
    try {
      locations = await api.setLocation("profile", name);
      newProfile = "";
      profileError = null;
    } catch (err) {
      profileError = String(err);
    }
  }

  // Archives that need a click: their pages open on request, and the folders
  // are looked at every two seconds until all have arrived.
  function waitForDownloads(): Promise<void> {
    return new Promise((resolve, reject) => {
      const timer = setInterval(async () => {
        try {
          byHand = await api.lookForDownloads();
          if (!byHand.length) {
            clearInterval(timer);
            stopWaiting = null;
            resolve();
          }
        } catch (err) {
          clearInterval(timer);
          reject(err);
        }
      }, 2000);
      stopWaiting = () => {
        clearInterval(timer);
        stopWaiting = null;
        reject(new Error("stopped while waiting for downloads"));
      };
    });
  }

  let opened = $state<Record<string, boolean>>({});
  async function openPage(item: ByHand) {
    if (!item.page) return;
    await api.openPage(item.page);
    opened[item.label] = true;
  }
  async function openNext(count: number) {
    for (const item of byHand.filter((i) => i.page && !opened[i.label]).slice(0, count)) {
      await openPage(item);
      // A browser handed many tabs at once drops some.
      await new Promise((r) => setTimeout(r, 300));
    }
  }

  async function refreshManual() {
    try {
      manual = await api.manualSteps();
      // A move is said once, by the check that made it: keep it in view.
      for (const m of manual) if (m.note) notes[m.title] = m.note;
    } catch {
      // Shown again on the next look.
    }
  }

  // --- Events from the library, to the step running now ------------------------
  const unlisten: UnlistenFn[] = [];
  onMount(async () => {
    unlisten.push(
      await listen<LibEvent>("mudcrab://event", ({ payload }) => {
        if (current >= 0 && payload.event !== "manual_step") steps[current].log.push(payload);
      }),
      await listen<Bar>("mudcrab://progress", ({ payload }) => {
        if (current < 0) return;
        const bars = steps[current].bars;
        const at = bars.findIndex((b) => b.id === payload.id);
        if (payload.done) {
          if (at >= 0) bars.splice(at, 1);
        } else if (at >= 0) bars[at] = payload;
        else bars.push(payload);
      }),
    );
  });
  onDestroy(() => {
    unlisten.forEach((u) => u());
    stopWaiting?.();
    if (manualTimer) clearInterval(manualTimer);
  });

  const icon: Record<State, string> = { idle: "○", running: "◌", waiting: "⏸", done: "✔", failed: "✖" };
  let copied = $state<string | null>(null);
  async function copy(text: string) {
    await navigator.clipboard.writeText(text);
    copied = text;
    setTimeout(() => (copied = null), 1500);
  }
</script>

<div class="install">
  <section data-tour="profile">
    <h3>Profile</h3>
    <p class="dim small">
      A profile is a set of answers to the list's questions, installed into the MO2 profile of the same
      name. One MO2 instance can hold several, with different answers; a mod they install alike is shared.
    </p>
    <div class="question">
      <div class="query">Name</div>
      {#if locations}
        <div class="profile">
          <select
            value={locations.profiles.includes(locations.profile) ? locations.profile : ""}
            onchange={(e) => {
              const value = (e.target as HTMLSelectElement).value;
              if (value) chooseProfile(value);
            }}
            disabled={running}
          >
            {#if !locations.profiles.includes(locations.profile)}
              <option value="">{locations.profile} (new)</option>
            {/if}
            {#each locations.profiles as name}<option value={name}>{name}</option>{/each}
          </select>
          <form
            onsubmit={(e) => {
              e.preventDefault();
              if (newProfile.trim()) chooseProfile(newProfile.trim());
            }}
          >
            <input type="text" bind:value={newProfile} placeholder="a new profile" disabled={running} />
            <button type="submit" disabled={running || !newProfile.trim()}>Use</button>
          </form>
        </div>
        {#if profileError}<div class="small err">{profileError}</div>{/if}
      {/if}
    </div>
    {#each list.document.inputs as input}
      <div class="question">
        <div class="query">{input.query}</div>
        {#if input.type === "bool"}
          <div class="choices">
            {#each ["true", "false"] as value}
              <label><input type="radio" name={input.id} {value} bind:group={answers[input.id]} disabled={running} /> {value === "true" ? "Yes" : "No"}</label>
            {/each}
          </div>
        {:else if input.type === "choice"}
          <select bind:value={answers[input.id]} disabled={running}>
            {#if answers[input.id] === ""}<option value="">Choose…</option>{/if}
            {#each input.choices ?? [] as choice}<option value={choice}>{choice}</option>{/each}
          </select>
        {:else}
          <input type="text" bind:value={answers[input.id]} disabled={running} />
        {/if}
      </div>
    {/each}
  </section>

  <section>
    <div class="row" data-tour="install">
      <button class="primary go" onclick={run} disabled={running || !ready}>
        {running ? "Installing…" : steps[3].state === "done" ? "Install again" : "Install"}
      </button>
      {#if running}
        <button onclick={cancel} disabled={cancelling} title="Stop at the next archive or mod; what is finished is kept">
          {cancelling ? "Stopping…" : "Cancel"}
        </button>
      {/if}
    </div>
    <label class="small dim recheck">
      <input type="checkbox" bind:checked={recheck} disabled={running} />
      Check again, even a plan that passed check before
    </label>
    {#if !ready && !running}
      <p class="dim small">
        {#if !locations?.instance.path}Choose the MO2 instance. {/if}
        {#if !locations?.downloads.path}Choose the downloads folder. {/if}
        {#if locations?.tools.problems.length}Set up xEdit in tools.toml. {/if}
        {#if unanswered.length}Answer every question.{/if}
        {#if !locations?.instance.path || !locations?.downloads.path || locations?.tools.problems.length}
          <button class="link" onclick={openFolders}>Open Settings</button>
        {/if}
      </p>
    {/if}
  </section>

  <section class="timeline" data-tour="steps">
    {#each steps as s, i}
      <div class="step {s.state}">
        <div class="shead">
          <span class="icon">{icon[s.state]}</span>
          <span class="stitle">{s.title}</span>
          <span class="dim swhat">
            {#if s.state === "waiting"}{byHand.length} of {byHandTotal} still to download by hand{:else}{s.done?.summary ?? s.what}{/if}
          </span>
        </div>
        {#if s.state !== "idle"}
          <div class="sbody">
            {#each s.bars as bar (bar.id)}
              <div class="bar">
                <div class="blabel"><span>{bar.label}</span><span class="dim">{bar.pos}/{bar.len} {bar.message}</span></div>
                <progress max={bar.len} value={bar.pos}></progress>
              </div>
            {/each}
            {#each s.log as event}
              {#if event.event === "status"}
                <div class="status {event.tone}"><Text spans={event.text} /></div>
              {:else if event.event === "section"}
                <details class="group {event.tone}" open={event.tone === "problem"}>
                  <summary>{event.heading} <span class="dim">({event.items.length})</span></summary>
                  <ul>{#each event.items as item}<li><Text spans={item.text} /></li>{/each}</ul>
                </details>
              {/if}
            {/each}
            {#if i === 1 && s.state === "waiting"}
              <div class="byhand">
                <div class="bar">
                  <div class="blabel"><span>downloaded by hand</span><span class="dim">{byHandTotal - byHand.length}/{byHandTotal}</span></div>
                  <progress max={byHandTotal} value={byHandTotal - byHand.length}></progress>
                </div>
                <p>
                  {byHand.length} archive(s) need a click. Open each page and use its download button;
                  each is picked up as it arrives{browserFolder ? `, including from ${browserFolder}` : ""}.
                </p>
                <div class="row">
                  <button onclick={() => openNext(5)}>Open the next 5 pages</button>
                  <button onclick={() => stopWaiting?.()}>Stop</button>
                </div>
                <ul>
                  {#each byHand as item (item.label)}
                    <li>
                      <div><strong>{item.mod_id}</strong> <span class="dim">{item.label}</span></div>
                      {#if item.note}<div class="dim small">{item.note}</div>{/if}
                      {#if item.page}
                        <button class="link" onclick={() => openPage(item)}>{opened[item.label] ? "Open again" : "Open page"}</button>
                      {:else}
                        <div class="small warn">No page to open: get it by hand and put it in the downloads folder.</div>
                      {/if}
                    </li>
                  {/each}
                </ul>
              </div>
            {/if}
            {#if s.error}<pre class="err">{s.error}</pre>{/if}
            {#if s.done}
              {#if s.done.writes.length}
                <div class="small dim">Writes: {#each s.done.writes as w}<code>{w}</code> {/each}</div>
              {/if}
              <div class="cmd">
                <pre>{s.done.command}</pre>
                <button class="link small" onclick={() => copy(s.done!.command)}>{copied === s.done.command ? "Copied" : "Copy"}</button>
              </div>
            {/if}
          </div>
        {/if}
      </div>
    {/each}
  </section>

  {#if manual}
    <section>
      <h3>Still to do by hand</h3>
      {#if !manual.length}<p class="dim">Nothing: the list is ready to play.</p>{/if}
      {#each manual as m}
        {@const done = m.done ?? ticked[m.title] ?? false}
        <div class="manual" class:done>
          <label>
            <input type="checkbox" checked={done} disabled={m.done !== null} onchange={(e) => (ticked[m.title] = (e.target as HTMLInputElement).checked)} />
            <strong>{m.title}</strong>
            {#if m.done !== null}<span class="dim small">· ticks itself when done</span>{/if}
          </label>
          {#if notes[m.title]}<p class="dim small">{notes[m.title]}</p>{/if}
          {#if !done}<ol>{#each m.steps as line}<li>{line.replace(/^\d+\.\s*/, "")}</li>{/each}</ol>{/if}
        </div>
      {/each}
    </section>
  {/if}
</div>

<style>
  .install {
    padding: 4px 16px 32px;
  }
  section {
    padding: 12px 0;
    border-bottom: 1px solid var(--line);
  }
  section:last-child {
    border-bottom: none;
  }
  h3 {
    margin: 0 0 8px;
    font-size: 13px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--dim);
  }
  .small {
    font-size: 12px;
  }
  .profile {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .profile form {
    display: flex;
    gap: 6px;
  }
  .recheck {
    display: flex;
    gap: 6px;
    align-items: center;
    margin-top: 6px;
  }
  .question {
    margin: 8px 0 12px;
  }
  .query {
    margin-bottom: 4px;
  }
  .choices {
    display: flex;
    gap: 16px;
  }
  .question select,
  .question input[type="text"] {
    width: 100%;
  }
  .go {
    width: 100%;
    padding: 10px;
    font-size: 15px;
  }
  .step {
    padding: 8px 0;
  }
  .shead {
    display: flex;
    gap: 10px;
    align-items: baseline;
  }
  .icon {
    width: 16px;
    text-align: center;
    color: var(--faint);
  }
  .running .icon {
    color: var(--accent);
    animation: spin 1.2s linear infinite;
    display: inline-block;
  }
  .waiting .icon {
    color: var(--warning);
  }
  .done .icon {
    color: var(--good);
  }
  .failed .icon {
    color: var(--problem);
  }
  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
  .stitle {
    font-weight: 600;
  }
  .swhat {
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sbody {
    margin: 6px 0 0 26px;
  }
  .bar {
    margin: 4px 0;
  }
  .blabel {
    display: flex;
    justify-content: space-between;
    font-size: 12px;
    gap: 8px;
  }
  .blabel .dim {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  progress {
    width: 100%;
    height: 6px;
    accent-color: var(--accent);
  }
  .status {
    font-size: 13px;
    margin: 3px 0;
  }
  .status.good {
    color: var(--good);
  }
  .status.warning,
  .warn {
    color: var(--warning);
  }
  .status.problem {
    color: var(--problem);
  }
  .group {
    font-size: 13px;
    margin: 4px 0;
  }
  .group summary {
    cursor: pointer;
  }
  .group.problem summary {
    color: var(--problem);
  }
  .group.warning summary {
    color: var(--warning);
  }
  .group ul {
    margin: 4px 0;
    padding-left: 18px;
  }
  .err {
    white-space: pre-wrap;
    color: var(--problem);
    font-family: inherit;
    font-size: 13px;
    margin: 4px 0;
  }
  .cmd {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    margin-top: 6px;
    background: var(--bg);
    border-radius: 6px;
    padding: 6px 8px;
  }
  .cmd pre {
    margin: 0;
    flex: 1;
    white-space: pre-wrap;
    word-break: break-all;
    color: var(--dim);
  }
  .byhand ul {
    list-style: none;
    padding: 0;
    margin: 8px 0 0;
    max-height: 320px;
    overflow: auto;
  }
  .byhand li {
    padding: 6px 0;
    border-top: 1px solid var(--line);
  }
  .row {
    display: flex;
    gap: 8px;
  }
  .manual {
    margin: 8px 0;
  }
  .manual.done strong {
    color: var(--dim);
    text-decoration: line-through;
  }
  .manual ol {
    margin: 4px 0 0;
    padding-left: 26px;
    font-size: 13px;
  }
</style>
