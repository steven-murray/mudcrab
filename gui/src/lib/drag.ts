// Dragging rows with pointer events. WebKitGTK (Tauri on Linux) does not
// deliver HTML5 drag-and-drop reliably, and pointer events work everywhere
// (the phase 3 spike, spikes/README.md). Rows stay where Svelte put them; a
// copy follows the pointer, the row it would land by is marked, and the
// scrolling container scrolls when the pointer nears its edge. The listeners
// are on the window, so the pointer is never lost.
//
// A row that can be dropped on carries `data-drop="<key>"`; with
// `data-drop-into`, a drop on its middle means "into it" (a section), not
// before or after it, and with `data-drop-into="only"` anywhere on it does
// (a folder).

export type Place = "before" | "after" | "into";

export interface DragOptions {
  /** The element that scrolls. */
  scroller: HTMLElement;
  /** Whether a drop on this key, at this place, would be accepted. */
  accepts?: (key: string, place: Place) => boolean;
  onDrop: (key: string, place: Place) => void;
  /** The row the pointer is over changed (null: none), e.g. to open a folder. */
  hover?: (key: string | null) => void;
}

export function startDrag(down: PointerEvent, options: DragOptions) {
  if (down.button !== 0) return;
  // Pressed on a handle: the row is what holds it.
  const handle = down.currentTarget as HTMLElement;
  const row = handle.closest<HTMLElement>("[data-drop]") ?? handle;
  down.preventDefault(); // no text selection
  const startY = down.clientY;
  const grab = startY - row.getBoundingClientRect().top;
  let ghost: HTMLElement | null = null;
  let marked: HTMLElement | null = null;
  let target: { key: string; place: Place } | null = null;
  let lastX = down.clientX;
  let lastY = startY;
  let timer: ReturnType<typeof setInterval> | undefined;
  let hovered: string | null = null;
  const hoverOn = (key: string | null) => {
    if (key !== hovered) {
      hovered = key;
      options.hover?.(key);
    }
  };

  const unmark = () => {
    marked?.classList.remove("drop-before", "drop-after", "drop-into");
    marked = null;
    target = null;
  };
  const place = () => {
    if (!ghost) return;
    ghost.style.top = `${lastY - grab}px`;
    unmark();
    const under = document.elementFromPoint(lastX, lastY)?.closest<HTMLElement>("[data-drop]");
    if (!under || under === row) return hoverOn(null);
    const box = under.getBoundingClientRect();
    const at = (lastY - box.top) / box.height;
    const into = under.dataset.dropInto;
    const place: Place =
      into === "only" || (into !== undefined && at > 0.25 && at < 0.75) ? "into" : at < 0.5 ? "before" : "after";
    const key = under.dataset.drop!;
    hoverOn(key);
    if (options.accepts && !options.accepts(key, place)) return;
    marked = under;
    target = { key, place };
    under.classList.add(`drop-${place}`);
  };
  const scroll = () => {
    const box = options.scroller.getBoundingClientRect();
    const edge = 40;
    let step = 0;
    if (lastY < box.top + edge) step = -Math.ceil((box.top + edge - lastY) / 3);
    if (lastY > box.bottom - edge) step = Math.ceil((lastY - box.bottom + edge) / 3);
    if (step) {
      options.scroller.scrollTop += step;
      place();
    }
  };
  const move = (event: PointerEvent) => {
    lastX = event.clientX;
    lastY = event.clientY;
    if (!ghost) {
      if (Math.abs(lastY - startY) < 4) return;
      ghost = row.cloneNode(true) as HTMLElement;
      const box = row.getBoundingClientRect();
      Object.assign(ghost.style, {
        position: "fixed",
        left: `${box.left}px`,
        width: `${box.width}px`,
        pointerEvents: "none",
        zIndex: "50",
        opacity: "0.85",
      });
      ghost.classList.add("dragging");
      document.body.append(ghost);
      row.classList.add("dragged");
      timer = setInterval(scroll, 16);
    }
    place();
  };
  const up = () => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    clearInterval(timer);
    ghost?.remove();
    row.classList.remove("dragged");
    const dropped = target;
    unmark();
    hoverOn(null);
    if (ghost && dropped) options.onDrop(dropped.key, dropped.place);
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
}
