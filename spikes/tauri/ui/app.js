// The spike's page: two views over spike-core, through Tauri commands.
"use strict";

const invoke = window.__TAURI__.core.invoke;
const $ = (id) => document.getElementById(id);
const el = (tag, cls, text) => {
  const node = document.createElement(tag);
  if (cls) node.className = cls;
  if (text !== undefined) node.textContent = text;
  return node;
};

function status(text, bad) {
  $("status").textContent = text;
  $("status").className = bad ? "status bad" : "status";
}

// An edit's command and diff, as the CLI would print them.
function showEdit(edited) {
  const box = $("edit");
  box.replaceChildren(el("div", "cmd", edited.command));
  const pre = el("pre", "diff");
  for (const line of edited.diff.split("\n")) {
    const cls = line.startsWith("@@") ? "hunk"
      : line.startsWith("+") && !line.startsWith("+++") ? "add"
      : line.startsWith("-") && !line.startsWith("---") ? "del" : "";
    pre.append(el("span", cls, line + "\n"));
  }
  box.append(pre);
}

// --- Tabs --------------------------------------------------------------------

for (const button of document.querySelectorAll(".tabs button")) {
  button.addEventListener("click", () => {
    for (const b of document.querySelectorAll(".tabs button")) b.classList.toggle("on", b === button);
    for (const v of document.querySelectorAll(".view")) v.classList.toggle("on", v.id === button.dataset.view);
  });
}

// --- The load order ------------------------------------------------------------

let plugins = [];

async function loadOrder() {
  const started = performance.now();
  plugins = await invoke("load_order");
  const list = $("lo-list");
  list.replaceChildren(...plugins.map((p, i) => {
    const row = el("div", "row");
    row.dataset.index = i;
    row.append(el("span", "grip", "⠿"), el("span", "n", String(i + 1)), el("span", "", p.name),
      el("span", "mods", p.mods.join(", ") || "—"));
    row.append(p.inactive ? el("span", "badge", "inactive") : el("span"));
    row.addEventListener("pointerdown", startDrag);
    return row;
  }));
  $("lo-count").textContent = `${plugins.length} plugins`;
  status(`Load order read in ${Math.round(performance.now() - started)} ms.`);
}

// Dragging with pointer events: WebKitGTK (Tauri on Linux) does not deliver
// HTML5 drag-and-drop events reliably, and pointer events work everywhere.
// The row itself stays in the list as the placeholder, moving to where it
// would land, while a copy follows the pointer. The listeners are on the
// window, so the pointer is never lost when rows move under it.
function startDrag(down) {
  if (down.button !== 0) return;
  down.preventDefault(); // no text selection
  const row = down.currentTarget;
  const list = $("lo-list");
  const from = Number(row.dataset.index);
  const startY = down.clientY;
  const grabOffset = startY - row.getBoundingClientRect().top;
  let ghost = null;
  let scrollTimer = null;
  let lastY = startY;

  const place = () => {
    const box = list.getBoundingClientRect();
    ghost.style.top = `${lastY - box.top + list.scrollTop - grabOffset}px`;
    // The placeholder goes before the first row whose middle is below the
    // pointer.
    const rows = [...list.children].filter((r) => r !== ghost && r !== row);
    const before = rows.find((r) => {
      const b = r.getBoundingClientRect();
      return lastY < b.top + b.height / 2;
    });
    if (before) {
      if (row.nextSibling !== before) list.insertBefore(row, before);
    } else if (list.lastElementChild !== row) {
      list.insertBefore(row, ghost);
    }
  };
  const autoScroll = () => {
    const box = list.getBoundingClientRect();
    const edge = 40;
    let step = 0;
    if (lastY < box.top + edge) step = -Math.ceil((box.top + edge - lastY) / 3);
    if (lastY > box.bottom - edge) step = Math.ceil((lastY - box.bottom + edge) / 3);
    if (step) {
      list.scrollTop += step;
      place();
    }
  };
  const onMove = (move) => {
    lastY = move.clientY;
    if (!ghost) {
      if (Math.abs(lastY - startY) < 4) return;
      ghost = row.cloneNode(true);
      ghost.classList.add("dragging");
      list.append(ghost);
      row.classList.add("gap");
      scrollTimer = setInterval(autoScroll, 16);
    }
    place();
  };
  const onUp = async () => {
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", onUp);
    if (!ghost) return;
    clearInterval(scrollTimer);
    ghost.remove();
    row.classList.remove("gap");
    const to = [...list.children].indexOf(row);
    const scroll = list.scrollTop;
    if (to === from) {
      await loadOrder();
      list.scrollTop = scroll;
      return;
    }
    try {
      const edited = await invoke("move_plugin", { from, to });
      showEdit(edited);
      await loadOrder();
      status(`Moved ${plugins[to].name} in ${edited.millis} ms (validated and written).`);
    } catch (err) {
      await loadOrder();
      status(String(err), true);
    }
    list.scrollTop = scroll;
  };
  window.addEventListener("pointermove", onMove);
  window.addEventListener("pointerup", onUp);
}

// --- A mod's files ---------------------------------------------------------------

let currentMod = null;
const open = new Set();

async function loadMods() {
  const ids = await invoke("mods");
  $("mod-ids").replaceChildren(...ids.map((id) => {
    const option = el("option");
    option.value = id;
    return option;
  }));
}

async function loadTree() {
  if (!currentMod) return;
  status(`Planning ${currentMod}…`);
  try {
    const tree = await invoke("tree", { id: currentMod, open: [...open] });
    $("tree-label").textContent =
      `${tree.label}: ${tree.taken} of ${tree.entries} files taken · planned in ${tree.millis} ms`;
    $("tree").replaceChildren(...tree.rows.map(treeRow));
    status(`Planned ${currentMod} in ${tree.millis} ms.`);
  } catch (err) {
    $("tree").replaceChildren();
    status(String(err), true);
  }
}

function treeRow(r) {
  // Three ways a row reads: taken, left out by a filter (struck through, can
  // be ticked back), or not chosen by the layout (dim; only a package can be
  // ticked).
  const chosen = r.package ? r.files > 0 : r.offered;
  const cls = r.state === "none" ? (chosen ? "excluded" : "unchosen") : "";
  const row = el("div", `row ${r.folder ? "folder" : "file"} ${cls}`);
  const name = el("span", "name");
  name.style.paddingLeft = `${r.depth * 18}px`;
  const twisty = el("span", "twisty", r.folder ? (open.has(r.path) ? "▾" : "▸") : "");
  if (r.folder) {
    twisty.addEventListener("click", () => {
      if (open.has(r.path)) open.delete(r.path); else open.add(r.path);
      loadTree();
    });
  }
  const box = el("input");
  box.type = "checkbox";
  box.checked = r.state === "all";
  box.indeterminate = r.state === "some";
  box.disabled = !r.tickable;
  if (box.disabled) box.title = "Not chosen by the archive's layout (a package or option)";
  box.addEventListener("change", async () => {
    const keep = r.state !== "all";
    try {
      const edited = await invoke("toggle", { id: currentMod, path: r.path, folder: r.folder, keep });
      showEdit(edited);
      await loadTree();
    } catch (err) {
      box.checked = r.state === "all";
      box.indeterminate = r.state === "some";
      status(String(err), true);
    }
  });
  name.append(twisty, box, el("span", "label", r.name + (r.package ? "  · package" : "")));
  row.append(name);
  let detail = "";
  if (r.folder) detail = chosen && r.files ? `${r.taken} of ${r.files}` : "not chosen";
  else if (r.destination) detail = `→ ${r.destination}`;
  else if (r.offered) detail = "excluded";
  row.append(el("span", r.folder ? "count" : "dest", detail));
  return row;
}

$("mod").addEventListener("change", (event) => {
  currentMod = event.target.value;
  open.clear();
  loadTree();
});

// --- Start -------------------------------------------------------------------------

(async () => {
  try {
    $("path").textContent = await invoke("list_path");
    await loadOrder();
    await loadMods();
    const start = await invoke("start_mod");
    if (start) {
      document.querySelector('.tabs button[data-view="files"]').click();
      $("mod").value = currentMod = start;
      await loadTree();
    }
  } catch (err) {
    status(String(err), true);
  }
})();
