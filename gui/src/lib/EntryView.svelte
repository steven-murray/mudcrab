<script lang="ts">
  import type { Entry } from "./api";
  import { inline, markdown } from "./markdown";
  import Self from "./EntryView.svelte";

  let {
    entry,
    words,
    matches,
    prose = true,
  }: {
    entry: Entry;
    words: string;
    matches: (entry: Entry, words: string) => boolean;
    /** Show a mod's summary and notes (author mode shows its own). */
    prose?: boolean;
  } = $props();

  // Sections start closed, so a 700-mod list opens fast; a filter opens them.
  let opened = $state(false);
  let notes = $state(false);
  const open = $derived(opened || !!words);

  // Where the mod lives: its Nexus page(s), links its docs give, and for a
  // mod hosted elsewhere, the page its archive comes from. Shown on every
  // mod, to send people to the authors' own pages.
  function pagesOf(e: Entry & { kind: "mod" }): { url: string; label: string; title: string }[] {
    const out = e.pages.map((p) => ({
      url: p.mod_url,
      label: "Nexus",
      title: "The mod's page on Nexus: read about it, and endorse it if you enjoy it",
    }));
    for (const link of e.docs.links ?? [])
      out.push({ url: link.url, label: "Page", title: link.label ?? "The mod's page" });
    if (!out.length)
      for (const archive of e.archives) {
        const url = archive.url ?? archive.raw?.download_page;
        if (url) out.push({ url, label: "Page", title: "Where the mod is hosted" });
      }
    return out.filter((p, i) => out.findIndex((q) => q.url === p.url) === i);
  }

  function count(e: Entry): number {
    if (e.kind === "mod") return 1;
    if (e.kind === "section") return e.entries.reduce((n, c) => n + count(c), 0);
    return 0;
  }
</script>

{#if entry.kind === "section"}
  <section class="section" style="--depth: {entry.depth}">
    <button class="head" onclick={() => (opened = !opened)}>
      <span class="twisty">{open ? "▾" : "▸"}</span>
      <span class="title">{entry.title}</span>
      <span class="dim count">{count(entry)} mod(s)</span>
    </button>
    {#if open}
      {#if !words && entry.docs.summary}<p class="summary">{@html inline(entry.docs.summary)}</p>{/if}
      {#if !words && entry.docs.body}<div class="prose sbody">{@html markdown(entry.docs.body)}</div>{/if}
      <div class="children">
        {#each entry.entries.filter((e) => matches(e, words)) as child}
          <Self entry={child} {words} {matches} />
        {/each}
      </div>
      {#if !words && entry.docs.footer}<div class="prose sbody">{@html markdown(entry.docs.footer)}</div>{/if}
    {/if}
  </section>
{:else if entry.kind === "mod"}
  <article class="mod">
    <div class="top">
      <span class="title">{entry.title}</span>
      {#each pagesOf(entry) as page}
        <a class="page" href={page.url} title={page.title}>{page.label} ↗</a>
      {/each}
      {#if entry.condition}<span class="cond" title={entry.condition.expr}>{entry.condition.text}</span>{/if}
      {#if entry.mod_type}<span class="kind">{entry.mod_type}</span>{/if}
      {#if entry.enabled === false}
        <span class="kind" title="Installed, but left unticked in MO2: tick it there when you need it">off in MO2</span>
      {/if}
    </div>
    {#if prose && entry.docs.summary}<p class="summary">{@html inline(entry.docs.summary)}</p>{/if}
    {#if entry.plugins.length}
      <div class="plugins">{#each entry.plugins as plugin}<code>{plugin}</code>{/each}</div>
    {/if}
    {#if (prose && entry.docs.body) || entry.steps.length}
      <button class="link more" onclick={() => (notes = !notes)}>{notes ? "Less" : "More"}</button>
      {#if notes}
        {#if prose && entry.docs.body}<div class="prose">{@html markdown(entry.docs.body)}</div>{/if}
        {#if entry.steps.length}
          <ul class="steps">
            {#each entry.steps as step}<li><span class="dim">{step.kind}:</span> {step.text}</li>{/each}
          </ul>
        {/if}
      {/if}
    {/if}
  </article>
{:else}
  <article class="mod revisit">
    <div class="top"><span class="title">{entry.title}</span><span class="kind">revisit</span></div>
    <ul class="steps">{#each entry.steps as step}<li>{step.text}</li>{/each}</ul>
  </article>
{/if}

<style>
  .section {
    margin: 6px 0;
  }
  .head {
    display: flex;
    align-items: baseline;
    gap: 8px;
    width: 100%;
    text-align: left;
    background: none;
    border: none;
    padding: 6px 0;
    border-bottom: 1px solid var(--line);
    border-radius: 0;
  }
  .head .title {
    font-weight: 650;
    font-size: calc(17px - var(--depth) * 1.5px);
  }
  .twisty {
    color: var(--dim);
    width: 12px;
  }
  .count {
    margin-left: auto;
    font-size: 12px;
  }
  .summary {
    margin: 6px 0;
    color: var(--dim);
  }
  .sbody {
    margin: 6px 0 10px;
  }
  .children {
    padding-left: 14px;
  }
  .mod {
    background: var(--card);
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 9px 12px;
    margin: 8px 0;
  }
  .mod .top {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    align-items: baseline;
  }
  .mod .title {
    font-weight: 600;
  }
  .page {
    font-size: 11px;
    padding: 1px 6px;
    border-radius: 4px;
    border: 1px solid var(--accent);
    text-decoration: none;
    white-space: nowrap;
  }
  .page:hover {
    background: var(--accent-soft);
  }
  .cond,
  .kind {
    font-size: 11px;
    padding: 1px 6px;
    border-radius: 4px;
    border: 1px solid var(--line);
    color: var(--dim);
  }
  .cond {
    border-color: var(--note);
    color: var(--note);
  }
  .mod .summary {
    margin: 4px 0 0;
  }
  .plugins {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 6px;
  }
  .plugins code {
    background: var(--card-hi);
    border-radius: 4px;
    padding: 1px 6px;
  }
  .more {
    font-size: 12px;
    margin-top: 4px;
  }
  .steps {
    margin: 6px 0 0;
    padding-left: 18px;
    font-size: 13px;
  }
</style>
