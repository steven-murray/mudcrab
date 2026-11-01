<script lang="ts" module>
  /** One stop of the tour: the element marked `data-tour="<target>"`, or
   *  none for a note in the middle of the window, and the mode to be in. */
  export interface Stop {
    target?: string;
    title: string;
    text: string;
    mode?: "install" | "author";
  }
</script>

<script lang="ts">
  import { onMount, tick } from "svelte";

  // A guided tour: the window dimmed but for one part at a time, with a note
  // beside it. The parts are found by `data-tour` attributes, so the tour
  // follows the layout rather than coordinates; a part not on screen (a panel
  // not open) is skipped.
  let {
    stops,
    setMode,
    onclose,
  }: {
    stops: Stop[];
    setMode: (mode: "install" | "author") => void;
    /** Closed: seen through, or skipped. */
    onclose: () => void;
  } = $props();

  let at = $state(0);
  let rect = $state<DOMRect | null>(null);
  let card = $state<HTMLDivElement | null>(null);
  let size = $state({ width: window.innerWidth, height: window.innerHeight });

  function find(stop: Stop): HTMLElement | null {
    if (!stop.target) return null;
    const el = document.querySelector<HTMLElement>(`[data-tour="${stop.target}"]`);
    if (!el) return null;
    const box = el.getBoundingClientRect();
    return box.width > 0 && box.height > 0 ? el : null;
  }

  // One move at a time: a key held down, or pressed again before the last
  // move has found its part, would race it.
  let moving = false;
  async function show(index: number, step: 1 | -1) {
    if (moving) return;
    moving = true;
    try {
      await move(index, step);
    } finally {
      moving = false;
    }
  }

  async function move(index: number, step: 1 | -1) {
    let i = index;
    // Past a stop whose part is not on screen, in the direction of travel.
    while (i >= 0 && i < stops.length) {
      const stop = stops[i];
      if (stop.mode) setMode(stop.mode);
      await tick();
      await new Promise((r) => requestAnimationFrame(() => r(null)));
      if (!stop.target || find(stop)) break;
      i += step;
    }
    if (i < 0 || i >= stops.length) {
      if (step === 1) onclose();
      return;
    }
    at = i;
    measure(true);
    // The keyboard in the tour, on its way on.
    await tick();
    card?.querySelector<HTMLButtonElement>("button.primary")?.focus();
  }

  function measure(scroll = false) {
    const el = find(stops[at]);
    if (el && scroll) el.scrollIntoView({ block: "nearest", inline: "nearest" });
    rect = el ? el.getBoundingClientRect() : null;
    size = { width: window.innerWidth, height: window.innerHeight };
  }

  const pad = 6;
  // The note below the part if it fits, else above, else beside it (a tall
  // panel), else over it; kept in the window.
  const place = $derived.by(() => {
    const w = card?.offsetWidth ?? 340;
    const h = card?.offsetHeight ?? 160;
    const gap = 12;
    const clampLeft = (left: number) => Math.min(Math.max(8, left), size.width - w - 8);
    const clampTop = (top: number) => Math.min(Math.max(8, top), size.height - h - 8);
    if (!rect) return { left: (size.width - w) / 2, top: (size.height - h) / 2 };
    if (rect.bottom + pad + gap + h < size.height) return { left: clampLeft(rect.left), top: rect.bottom + pad + gap };
    if (rect.top - pad - gap - h > 0) return { left: clampLeft(rect.left), top: rect.top - pad - gap - h };
    const middle = clampTop(rect.top + rect.height / 2 - h / 2);
    if (rect.left - pad - gap - w > 8) return { left: rect.left - pad - gap - w, top: middle };
    if (rect.right + pad + gap + w < size.width - 8) return { left: rect.right + pad + gap, top: middle };
    return { left: clampLeft(rect.left + rect.width / 2 - w / 2), top: middle };
  });

  // Every key is the tour's while it is open: its own, and nothing for the
  // window behind it (Ctrl+Z would undo an edit there). Tab moves between
  // its buttons only.
  function key(e: KeyboardEvent) {
    const inCard = card?.contains(e.target as Node) ?? false;
    if (e.key === "Escape") onclose();
    else if (e.key === "ArrowRight" || (e.key === "Enter" && !inCard)) show(at + 1, 1);
    else if (e.key === "ArrowLeft") {
      if (at > 0) show(at - 1, -1);
    } else if (e.key === "Tab") {
      const buttons = [...(card?.querySelectorAll<HTMLButtonElement>("button") ?? [])];
      if (!buttons.length) return;
      const i = buttons.indexOf(document.activeElement as HTMLButtonElement);
      const next = buttons[(i + (e.shiftKey ? -1 : 1) + buttons.length) % buttons.length];
      next.focus();
    } else if (inCard && (e.key === "Enter" || e.key === " ")) {
      // A button of the tour's own, pressed from the keyboard.
      e.stopImmediatePropagation();
      return;
    }
    e.preventDefault();
    e.stopImmediatePropagation();
  }

  let root = $state<HTMLDivElement | null>(null);
  onMount(() => {
    // The window behind is out of reach -- of the keyboard as of the mouse --
    // until the tour closes.
    const behind = [...(root?.parentElement?.children ?? [])].filter(
      (el): el is HTMLElement => el instanceof HTMLElement && el !== root && !el.inert,
    );
    behind.forEach((el) => (el.inert = true));
    show(0, 1);
    const again = () => measure();
    window.addEventListener("resize", again);
    window.addEventListener("scroll", again, true);
    return () => {
      behind.forEach((el) => (el.inert = false));
      window.removeEventListener("resize", again);
      window.removeEventListener("scroll", again, true);
    };
  });
</script>

<svelte:window onkeydowncapture={key} />

<div class="tour" bind:this={root}>
<!-- Catches clicks, so the window is looked at rather than used meanwhile. -->
<div class="block" aria-hidden="true"></div>
{#if rect}
  <div
    class="spot"
    style:left="{rect.left - pad}px"
    style:top="{rect.top - pad}px"
    style:width="{rect.width + 2 * pad}px"
    style:height="{rect.height + 2 * pad}px"
  ></div>
{:else}
  <div class="dim"></div>
{/if}
<div
  class="card"
  bind:this={card}
  role="dialog"
  aria-label="A tour of mudcrab"
  style:left="{place.left}px"
  style:top="{place.top}px"
>
  <div class="title">{stops[at].title}</div>
  <p>{stops[at].text}</p>
  <div class="foot">
    <span class="count">{at + 1} of {stops.length}</span>
    <button class="link" onclick={onclose}>Skip the tour</button>
    {#if at > 0}<button onclick={() => show(at - 1, -1)}>Back</button>{/if}
    <button class="primary" onclick={() => show(at + 1, 1)}>{at + 1 === stops.length ? "Done" : "Next"}</button>
  </div>
</div>
</div>

<style>
  .block {
    position: fixed;
    inset: 0;
    z-index: 900;
  }
  /* The part shown: a hole in the dimming, the rest of the window darkened
     by its shadow. */
  .spot {
    position: fixed;
    z-index: 901;
    border-radius: 8px;
    box-shadow:
      0 0 0 2px var(--accent),
      0 0 0 9999px rgba(0, 0, 0, 0.55);
    pointer-events: none;
  }
  .dim {
    position: fixed;
    inset: 0;
    z-index: 901;
    background: rgba(0, 0, 0, 0.55);
  }
  .card {
    position: fixed;
    z-index: 902;
    width: min(340px, calc(100vw - 16px));
    padding: 12px 14px;
    background: var(--panel);
    border: 1px solid var(--line);
    border-radius: 10px;
    box-shadow: 0 8px 30px rgba(0, 0, 0, 0.35);
  }
  .title {
    font-weight: 600;
    margin-bottom: 4px;
  }
  .card p {
    margin: 0 0 10px;
    font-size: 13px;
    line-height: 1.45;
  }
  .foot {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .count {
    color: var(--dim);
    font-size: 12px;
    margin-right: auto;
  }
  .foot .link {
    font-size: 12px;
  }
</style>
