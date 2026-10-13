# A GUI, alongside the CLI

**Status: phases 1 and 2 done** (the library API and CLI parity). Phase 3,
the framework spike, is built and measured on Linux ([spikes/](https://github.com/steven-murray/mudcrab/tree/main/spikes));
the decision waits on trying it by hand on Windows and the Steam Deck.

## Why, and the rule it lives under

The GUI is mainly for **modlist authors**. They come from MO2, where some
things are simply faster by hand: dragging a mod up the list, ticking which of
an archive's files to keep, dragging a folder somewhere else in the mod,
seeing a conflict and fixing it on the spot. A text file of 700 entries is
not the place for those. For **players** it can offer one button that goes from
a list to an installed game, showing what each step writes as it goes.

**The rule: the CLI is complete, always.** Nothing will ever need the GUI.
Everything the GUI does, it does by calling the same library code a CLI
command calls, and each edit it makes is one a CLI command can make. Three
consequences:

- **CLI parity comes first.** Where the GUI would edit something the CLI
  cannot (set a field on a mod, reorder the load order, add an action), the
  command is written first and the GUI calls it. That work makes the CLI
  better for everyone, GUI or not.
- **The modlist file is the only state.** The GUI keeps nothing of its own
  that the CLI cannot read. It edits the TOML through the same line-splicing
  editor `add` and `move` use, so comments and formatting survive, and it
  watches the file, reloading when it changes on disk.
- **Every GUI edit shows its command.** An edit panel shows the CLI command
  that does the same thing (and the diff it makes), so the GUI teaches the CLI
  instead of hiding it, and an author can script what they just did.

## Two modes

### Install mode (players)

Opening a list -- a file, or a URL once [composability](composability.md)
lands -- shows it **read-only**:

- the list's sections and mods, with the prose `export` renders (the export
  JSON document is exactly this view's data), conditions and choices shown
  plainly;
- its questions (`query`), with the author's defaults selected;
- where things go: the downloads folder, the MO2 instance and the game folder,
  found as `setup-tools` and `game-dir` find them, each changeable.

One **Install** button runs the pipeline -- compile, query, download, check,
install -- as a timeline. Each step says what it is doing and what it writes
(`compiled.json`, `plan.json`, archives into the downloads folder, mods into
the instance, the profile), with the CLI command for that step beside it.
Downloads that need a click open the browser and tick off as files arrive, as
`download` does today. `check`'s findings appear grouped, as on the terminal.
The **steps by hand** (`[[manual-steps]]`) end it as a checklist that ticks
itself when, say, the Bashed Patch appears.

### Author mode

The same window, with editing switched on: a switch, not a second window or a
tab, so there is one set of views to build and learn. Views, roughly in the order they
would be built:

- **The list**: sections as a tree and mods as a table (MO2's layout, so it
  is familiar). Drag a mod to reorder it or move it to another section
  (`move`). Add a mod from an archive, walking its FOMOD or BAIN choices with
  the same question model `add` uses on the terminal.
- **The load order**: a list of plugins to drag into order, showing each
  plugin's masters and flagging one placed above a master (the `check` rule,
  live).
- **A mod's files**: the archive's contents as a tree, with what `install`
  would take from it highlighted (the layout planner `check` already runs).
  The tree updates from the planner, so what is shown is what will be
  installed. What a tick means follows the archive's layout, so an author
  sees the installer they already know:
  - **BAIN**: ticks are packages, at the top level only, as in Wrye Bash's
    installer tab, writing `bain_subpackages`. The files inside are shown
    read-only, marked installed or not. A `wizard.txt` can be run, as `add`
    runs it.
  - **FOMOD**: the installer's steps and options, writing
    `fomod_selections`, not a file tree to tick.
  - **Simple, data folder, auto**: ticks on files and folders, writing
    `exclude` (and dropping it to tick back).

  Separately, for any layout, an expert override: **Exclude from install** on
  a file or folder's context menu writes an `exclude` for the path the
  filters see it as (inside the data folder, BAIN package or FOMOD folder;
  the planner reports it as `filtered_as`). It is visibly an override, not a
  tick, and says what it covers -- under BAIN one exclude applies to every
  package, so `Docs/readme.txt` hits each package's copy. (The spike tried
  letting a file inside an unchosen BAIN package be ticked, choosing the
  package and excluding the rest of it; it worked, but it is not how BAIN
  thinks, and the shared excludes made it refuse often. A filter per package
  in the format would be the cleaner way, if authors ask for it.)

  Also here: drag a folder to a new place to write a `file_move`; mark files
  for the game folder (`game_root_files`).
- **Conflicts**: the `conflicts` sweep as a matrix or list -- who wins, who
  loses -- with a mod's overwritten files one click away, and "hide these in
  that mod" writing the `file_hide` with `conflicts_with`.
- **Check**: `check` on demand (it takes under a minute), findings grouped as
  on the terminal, each one linking to the mod and field it is about.
- **Inputs and conditions**: the list's questions, and which mods each
  answer brings in or leaves out.

Everything is undoable as file edits are: the GUI keeps the file's history for
the session, and the file is plain text in git anyway. Each edit is written
at once (as the CLI writes it), so the history is a list of `Edit`s, each
with its before and after text, its command and its diff:

- **Undo** (and redo) steps back through it, writing the earlier text -- and,
  like any edit, only if the file has not changed on disk since; an edit made
  outside the GUI ends the history there.
- **This session's changes**: one view listing every edit made since the
  list was opened, each with its command and diff, and the session's diff as
  a whole -- what a commit message would describe. The commands together are
  a script that makes the same changes to another copy.

(The spike showed only the last edit's diff, which was enough to try the
framework but not to work in.)

## Architecture

```
mudcrab (library)   compile, query, check, install, editing, discovery ...
  ├─ mudcrab        the CLI: argument parsing and terminal output
  └─ mudcrab-gui    the GUI: windows, views, and the same library calls
```

- **A workspace with two binaries.** The CLI stays a small, separate download
  with no GUI dependencies; the GUI is its own release artifact.
- **The library speaks in events, not prints.** Long operations report
  progress and findings through a sink the caller provides. The CLI's sink is
  today's `ui` module (bars, styled lines); the GUI's feeds its views. Most of
  this is moving output that commands print today behind that interface --
  `check`'s `Findings` and `download`'s progress are already nearly there.
- **Long work on background threads, cancellable.** Install and download
  already run as resumable passes; the GUI adds a cancel that stops between
  mods, as Ctrl-C does now.
- **Editing is a library API.** `add`, `move`, `downloads record` and the new
  parity commands become functions that take a file's text and return the new
  text (they nearly are: `plan_insertion`, `plan_move`), so the GUI and CLI share
  one implementation, validation and restore-on-failure included.

## The framework

Requirements: Linux and Windows first-class (Linux includes the Steam Deck's
desktop mode, where many Oblivion players under Proton are) and macOS
reasonable; looks good -- this is for gamers -- with theming; and specific
widgets: a virtualised table of ~700 rows, trees, drag-and-drop reordering
within and between lists, and file trees with checkboxes.

What the Rust options look like, as of October 2026:

| Framework | How it draws | Fit |
| --- | --- | --- |
| **Tauri 2** | The OS web view (WebView2 on Windows, WebKitGTK on Linux), a web front end, a Rust back end | Richest widgets by far (virtual tables, trees, drag-and-drop libraries), CSS theming, small binaries, mature, actively released. Costs: two languages and a JS build step; WebKitGTK has Linux quirks -- blank windows on some NVIDIA setups (documented workarounds) and HTML5 drag-and-drop events that do not fire, so drag-and-drop must use a pointer-event library, which the good ones are. |
| **Slint 1.17** | Its own renderer, a declarative `.slint` language plus Rust | One Rust binary, no web view, live preview, looks native or custom. Drag-and-drop arrived only in 1.17 (June 2026); no stock tree view, so trees are built by hand. Licence: GPL, or a royalty-free community licence that asks for an "About Slint" credit. |
| **Iced 0.14** | wgpu, Elm-style, pure Rust | Used by System76's COSMIC desktop, so maintained; reactive rendering and IME arrived in 0.14 (December 2025). Drag-and-drop and rich widgets are thinner, and the docs have gaps. |
| **egui** | Immediate mode, pure Rust | Quickest to a working window, widely used -- but looks like a debug tool, and repaints constantly. Fails "slick". |
| **Dioxus 0.7** | React-style Rust; a web view, or the experimental Blitz renderer | Rust-only front end code, but on desktop it is a web view (as Tauri) with a smaller widget ecosystem; the native renderer is still experimental. |
| **GPUI** | Zed's GPU framework | Fast and modern, but it lives with Zed: its standalone releases are forks and community kits, Windows support is the least proven, and the docs are thin. Too risky to build on. |

**Recommendation: Tauri 2**, with a typed front end (TypeScript and Svelte or
SolidJS). The GUI's hard parts are exactly the widgets the web has solved:
virtualised tables, trees and drag-and-drop. Getting them good in Slint or Iced
means building them, and "slick" is cheapest with CSS. The Rust core stays the
whole of mudcrab's logic; the front end only draws and calls it.

**Runner-up: Slint**, if the web view proves troublesome where it matters most
-- the Steam Deck and Linux under NVIDIA. It keeps everything in one Rust
binary at the cost of building the tree and table widgets ourselves.

**Decide with a spike, not this table.** Build the same two views in Tauri and
Slint -- the 244-plugin load order with drag-and-drop, and an archive's file
tree with checkboxes over a real MOFAM archive -- and run them on Linux
(Wayland and X11, AMD and NVIDIA), the Steam Deck and Windows. A week, and the
choice is then evidence.

## CLI parity

Every edit the GUI makes is one of these commands (documented in
[Editing a list in Commands](../usage.md#editing-a-list)):

| Command | Does |
| --- | --- |
| `set <list> --mod <id> [--archive N] key=value... [--unset key]` | Set or remove fields of a mod or one of its archives. |
| `remove <list> --mod <id>` | Remove a mod and its plugins from the load order; refused while anything refers to it. |
| `plugins list <list>` / `plugins move <list> <plugin> --after/--before/--first/--last` | Read and reorder the load order. |
| `files <list> --mod <id> [--include/--exclude/--drop-... <glob>]` | What an archive installs, from the planner; edit its filters and see what changes. |
| `action list/add/remove <list> --mod <id> ...` | A mod's actions. |
| `input list/add/set/remove <list> ...` | The list's questions. |

Each plans its change as text, validates it in memory, and writes it only if
valid and only if the file has not changed on disk since it was read; with
`--dry-run` it prints the diff instead. Fields and arrays are edited with
`toml_edit`, which leaves every byte it is not asked to change alone (MOFAM's
10,000 lines round-trip identically); whole mod blocks are still moved and
removed by the line splicer `add` and `move` use, which knows which comments
belong to a block.

## The library API (phase 1)

- **`mudcrab::events`**: `Event` (a status line, a titled group of items, a
  step by hand), `Text` with names marked rather than styled, `Item` with the
  mod it is about, progress bars, and `set_sink` for the front end's own
  `Sink`. Events serialise to JSON for a web front end.
- **`config::check::check_all`** returns its `findings` (kind, text, mod) and
  counts in the report; the CLI, not the library, fails on problems.
- **`config::query::build_plan`** takes the answers and an optional `ask`
  callback; the GUI collects answers in a form and passes none.
- **`config::edit`**: `plan(path, change) -> Edit`, `Edit::diff`,
  `Edit::write`, and the operations in `config::edit::doc` and
  `config::add` (`plan_add`, `plan_move`, `plan_removal`).
- **`config::pipeline`**: `compile`, `compile_text`, `read_compiled`,
  `read_plan`, `write_json`.

Still printing directly, for later phases: `conflicts`' sweep and `assets`
(their logic sits in the command; it moves to the library with phase 5), and
`download`'s browser mode.

## Phases

1. **Library API and events.** Done: see above. MOFAM's `check` output is
   byte-identical before and after.
2. **CLI parity.** Done: the commands above, documented and tested.
3. **Framework spike.** The two views in Tauri and Slint, on the four
   platforms; decide. Built: see [spikes/README.md](https://github.com/steven-murray/mudcrab/blob/main/spikes/README.md)
   for what was measured. On Linux both work; Tauri needs a workaround to
   start on NVIDIA under Wayland and uses about five times the memory, Slint
   needs its tree and tri-state widgets built by hand.
4. **Install mode.** Read-only list view from the export document, questions,
   locations, the Install timeline, manual-step checklist. Ship it: it is
   useful to players on its own.
5. **Author mode, reading.** Load order, a mod's files with the planner's
   highlighting, conflicts, check results.
6. **Author mode, editing.** Drag-and-drop through the parity commands, the
   command-and-diff panel, undo.
7. **Packaging.** AppImage (and Flatpak) for Linux, an installer or zip for
   Windows, the Deck checked; the GUI as a separate release artifact.

## Settled

- **One window, with a switch** that turns author mode's editing on in the
  same views.
- **The GUI's own settings** (recent lists, window layout) live beside
  `tools.toml`, never in the list.
- **`check` runs on demand.** Live checking as the author edits is a later
  feature, its own project.

## Sources

Framework facts above are from: [Slint 1.17 release](https://slint.dev/blog/slint-1.17-released),
[Slint royalty-free licence](https://slint.dev/agreements/slint-royalty-free-license.pdf),
[Tauri: Linux graphics issues](https://v2.tauri.app/develop/debug/linux-graphics/),
[WebKitGTK drag-and-drop bug 265857](https://bugs.webkit.org/show_bug.cgi?id=265857),
[Dioxus 0.7 release](https://dioxuslabs.com/blog/release-070),
[Iced 0.14 report](https://habr.com/ru/post/974728),
[The Rust GUI landscape in 2026](https://wrenlearnsrust.com/posts/2026-03-11-rust-gui-landscape-2026.html),
[Tritium: Rust GUI observations](https://tritium.legal/blog/desktop),
[gpui-component](https://github.com/longbridge/gpui-component).
