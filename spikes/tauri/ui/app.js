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
function startDrag(down) {
  if (down.button !== 0) return;
  const row = down.currentTarget;
  const list = $("lo-list");
  const from = Number(row.dataset.index);
  const height = row.getBoundingClientRect().height;
  const top0 = row.offsetTop;
  let moved = false;
  let target = from;

  const gap = el("div", "row gap");
  row.setPointerCapture(down.pointerId);

  const onMove = (move) => {
    const dy = move.clientY - down.clientY;
    if (!moved) {
      if (Math.abs(dy) < 4) return;
      moved = true;
      row.after(gap);
      row.classList.add("dragging");
      list.append(row);
    }
    row.style.top = `${top0 + dy}px`;
    // Where the dragged row's middle is, as a row index.
    target = Math.max(0, Math.min(plugins.length - 1, Math.round((top0 + dy) / height)));
    const rows = [...list.children].filter((r) => r !== row && r !== gap);
    const before = rows[target];
    if (before) before.before(gap); else list.append(gap);
    // Keep the dragged row in view near the edges.
    const box = list.getBoundingClientRect();
    if (move.clientY < box.top + 30) list.scrollTop -= 12;
    if (move.clientY > box.bottom - 30) list.scrollTop += 12;
  };
  const onUp = async () => {
    row.removeEventListener("pointermove", onMove);
    row.removeEventListener("pointerup", onUp);
    if (!moved || target === from) {
      row.classList.remove("dragging");
      row.style.top = "";
      gap.remove();
      if (moved) await loadOrder();
      return;
    }
    const scroll = list.scrollTop;
    try {
      const edited = await invoke("move_plugin", { from, to: target });
      showEdit(edited);
      await loadOrder();
      status(`Moved ${plugins[target].name} in ${edited.millis} ms (validated and written).`);
    } catch (err) {
      await loadOrder();
      status(String(err), true);
    }
    list.scrollTop = scroll;
  };
  row.addEventListener("pointermove", onMove);
  row.addEventListener("pointerup", onUp);
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
  const row = el("div", `row ${r.folder ? "folder" : "file"} ${r.state === "none" ? "off" : ""}`);
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
  name.append(twisty, box, el("span", "label", r.name));
  row.append(name);
  row.append(r.folder
    ? el("span", "count", `${r.taken} of ${r.files}`)
    : el("span", "dest", r.destination ? `→ ${r.destination}` : ""));
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
