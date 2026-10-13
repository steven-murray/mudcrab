# The GUI framework spike

Phase 3 of [the GUI plan](../docs/design/gui.md): the same two views built in
**Tauri 2** (a web page in the system web view) and **Slint 1.18** (native
drawing, one Rust binary), over the same core, so the frameworks can be
compared on what the GUI will actually need. Not part of mudcrab's build or
releases.

- **The load order**: the list's plugins, with the mods that declare each.
  Drag one to move it.
- **A mod's files**: one archive as a tree, ticked where `install` takes a
  file (MO2-style, from the same planner `check` uses), folders showing all,
  some or none. Untick a file or folder to exclude it; tick it back to drop
  that exclude.

Every edit goes through the CLI's own code (`plugins move`, `files
--exclude`), is validated and written to the list, and is shown with the
command that does the same and the diff it made.

```
spikes/
  core/    what both show and do (spike-core), over mudcrab's library
  tauri/   Rust commands + ui/index.html and app.js (no npm, no build step)
  slint/   ui/app.slint + Rust
```

## Running them

**They edit the list: run them on a copy.**

```bash
cp mylist.toml /tmp/spike-list.toml
cd spikes
cargo run --release -p spike-tauri -- /tmp/spike-list.toml --mod "Some Mod"
cargo run --release -p spike-slint -- /tmp/spike-list.toml --mod "Some Mod"
```

`--downloads <dir>` overrides `downloads_dir` from tools.toml; `--mod` opens
on that mod's files. Linux needs WebKitGTK 4.1 to build and run the Tauri one
(`webkit2gtk-4.1` on Arch, `libwebkit2gtk-4.1-dev` on Debian/Ubuntu). The
[GUI spike workflow](../.github/workflows/spike.yml) builds both for Linux and
Windows on every push that touches `spikes/`; the binaries are its artifacts.

## What was measured (Linux, KDE Plasma, Intel UHD + NVIDIA RTX A2000)

On MOFAM: 244 plugins; Cobl's archive, 2331 entries with a BAIN layout.

| | Tauri 2.11 | Slint 1.18 |
| --- | --- | --- |
| Wayland, Intel | works | works |
| Wayland, NVIDIA (PRIME offload) | **dies at start** (`Error 71 (Protocol error) dispatching to Wayland display`); works with `WEBKIT_DISABLE_DMABUF_RENDERER=1`, which the spike now sets itself | works |
| X11, Intel / NVIDIA | works / works | works / works |
| Memory once showing the files view | 460-510 MB, in 3 processes (app, WebKit web and network processes) | 90-125 MB (Intel), 200-270 MB (NVIDIA) |
| Binary, stripped | 12 MB, plus the system's WebKitGTK | 29 MB, needing only fontconfig and freetype |
| Crates | 454 | 586 |
| Load order read / Cobl planned | 96 ms / 129 ms | 76 ms / 38 ms |

The time to plan is mudcrab's (reading the archive), so the difference there
is noise between runs, not the framework.

What building them showed:

- **Tauri** needed no workarounds besides the NVIDIA one, and the page is
  ordinary HTML and CSS: the tree, tri-state boxes (`indeterminate`) and the
  diff panel took minutes. Drag-and-drop is written with pointer events (40
  lines), since HTML5 drag-and-drop is unreliable under WebKitGTK; a library
  would do it in production. The native checkboxes looked wrong under
  WebKitGTK and needed CSS.
- **Slint** needed more care for layout: text in a stretching cell is centred
  and sized by its content unless told otherwise, so columns wandered until
  every cell got `horizontal-alignment: left` and a zero preferred width.
  There is no tree widget or tri-state checkbox in the standard set: both were
  built (a flattened list with indentation, and a 30-line `Tick`). Dragging is
  a `TouchArea` tracking the pointer, which was straightforward. Compile errors
  in `.slint` are good; the live preview was not tried.
- Both run planning off the UI thread, and both edit through the same
  library calls, so the CLI-parity rule held without effort.

What the first hands-on run (Tauri, Linux) found, now fixed:

- **The drag selected text** instead of moving smoothly: WebKitGTK needs
  `-webkit-user-select: none` and a `preventDefault` on the press, and moving
  the dragged row in the page dropped its pointer capture. The drag now
  listens on the window, leaves the row in the list as the placeholder, and
  scrolls the list when the pointer nears its edge.
- **Unticking wrote an exclude that did nothing.** `include` and `exclude`
  match paths inside the data folder, BAIN package or FOMOD folder a file
  comes from, not its path in the archive; the spike was writing archive
  paths. The planner now records, for each file, the path it tested against
  the filters (`filtered_as`), and `archive_contents` also reports what the
  layout offers before any filter, so the tree tells three things apart:
  taken, left out by a filter (struck through; ticking it back drops the
  exclude), and not chosen by the layout (dim). A BAIN package's folder ticks
  by editing `bain_subpackages`, as MO2's installer does. The GUI proper will
  need exactly this, so it lives in mudcrab's library.

## What is left to try (by hand)

On each machine, with each spike, on a copy of a real list:

1. **Drag** a plugin a long way, up and down, past the window's edge (does the
   list scroll?), and drop it. Does the diff show just that line moving? Does
   it feel smooth, or does it lag?
2. **Open folders** in a mod's files, untick a file, untick a folder, tick
   them back. Does it keep up? Is the tree readable?
3. **Resize** the window, use a 4K or scaled display, and switch the desktop
   between light and dark: does either break?
4. **Keyboard**: Tab through the views. Neither spike adds keyboard
   reordering yet; note how far each gets on its own.

Machines: Linux on Wayland and X11 (done above, except by hand); **Windows**
(the workflow's artifact); **the Steam Deck** in desktop mode (the Linux
artifact; the Tauri one needs WebKitGTK 4.1, which SteamOS may not have
outside a Flatpak -- whether it starts is itself the finding).

## The decision so far

Both are viable. The trade is:

- **Tauri**: the fastest to build views in, and the views look the most
  finished for the effort; but five times the memory, a dependency on the
  system web view (WebKitGTK on Linux: the NVIDIA crash above, and a question
  mark on the Steam Deck), and its packaging (AppImage or Flatpak) to ship
  WebKitGTK where it is missing.
- **Slint**: one self-contained binary, a fifth of the memory, no GPU or
  web-view surprises seen; but the widgets the author mode needs (tree, table,
  tri-state boxes) are ours to build, and layout takes more care.

The Windows and Steam Deck runs decide it: if Tauri starts and drags well on
both, its speed of building views wins; if it does not start on the Deck, or
the drag is poor anywhere, Slint does.
