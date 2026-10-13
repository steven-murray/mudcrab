# A GUI, alongside the CLI

**Status: plan.** Nothing here is implemented yet.

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

The same window, with editing switched on. Views, roughly in the order they
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
  Tick and untick files and folders to write `include`/`exclude`; drag a
  folder to a new place to write a `file_move`; mark files for the game
  folder (`game_root_files`). The tree updates from the planner, so what is
  shown is what will be installed.
- **Conflicts**: the `conflicts` sweep as a matrix or list -- who wins, who
  loses -- with a mod's overwritten files one click away, and "hide these in
  that mod" writing the `file_hide` with `conflicts_with`.
- **Check**: `check` on demand (it takes under a minute), findings grouped as
  on the terminal, each one linking to the mod and field it is about.
- **Inputs and conditions**: the list's questions, and which mods each
  answer brings in or leaves out.

Everything is undoable as file edits are: the GUI keeps the file's history for
the session, and the file is plain text in git anyway.

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

## CLI parity: commands to write first

Edits an author would make in the GUI that the CLI cannot make today:

| Command (sketch) | Does |
| --- | --- |
| `mudcrab set <list> <mod> <key>=<value>` | Set or replace a field on a mod (`layout`, `if`, `data_folder`, ...). |
| `mudcrab remove <list> <mod>` | Remove a mod, its plugins from the load order, and references to it. |
| `mudcrab plugins <list> move <plugin> --after <plugin>` | Reorder the load order. |
| `mudcrab files <list> <mod> include\|exclude <glob>` | Edit a mod's file filters, printing what the planner then takes. |
| `mudcrab action <list> <mod> add <action> ...` | Add an action (`file_move`, `file_hide`, ...). |
| `mudcrab input <list> add\|set ...` | Declare or change a question. |

Each one splices lines (comments survive), validates the result, and restores
the file if it would not validate -- as `add` and `move` do now.

## Phases

1. **Library API and events.** Command implementations become library
   functions reporting through an event sink; the CLI renders the events as
   today. Nothing visible changes.
2. **CLI parity.** The commands above, each documented and tested.
3. **Framework spike.** The two views in Tauri and Slint, on the four
   platforms; decide.
4. **Install mode.** Read-only list view from the export document, questions,
   locations, the Install timeline, manual-step checklist. Ship it: it is
   useful to players on its own.
5. **Author mode, reading.** Load order, a mod's files with the planner's
   highlighting, conflicts, check results.
6. **Author mode, editing.** Drag-and-drop through the parity commands, the
   command-and-diff panel, undo.
7. **Packaging.** AppImage (and Flatpak) for Linux, an installer or zip for
   Windows, the Deck checked; the GUI as a separate release artifact.

## Questions to settle

- **One window or two?** Install and author modes as tabs of one window, or
  author mode as a switch that unlocks editing in the same views. The second
  keeps one set of views to build and learn.
- **Where the GUI keeps its settings** (recent lists, window layout): beside
  `tools.toml`, never in the list.
- **Live `check`.** Re-run affected parts of `check` as the author edits, or
  only on demand? On demand first: it is under a minute, and incremental
  checking is its own project.

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
