# The GUI

**Preview.** The GUI is new and not yet in a release; build it from source
(below) to try it. Everything it does is one of the commands in
[Getting started](getting-started.md), which stay the complete way to use
mudcrab.

The GUI's install mode reads a list and installs it from one window:

1. **Open a list**: its `.toml` file. Its guide is on the left -- sections,
   each mod with its notes and plugins, and the load order -- read-only.
   **About this list** shows what its author wrote for whoever installs it
   (its `about`), or, for a list without one, its guide's introduction.
2. **Answer its questions**, if it asks any. The list's defaults are chosen;
   your answers are remembered for next time.
3. **Say where things go**: first the MO2 instance to install into -- a
   portable instance, the folder with `ModOrganizer.exe` in it (any other
   folder is refused). The downloads folder and the game folder then follow
   from it: the downloads folder from `tools.toml` if it is set there, else
   the instance's own; the game folder from the instance's settings, or found
   as `game-dir` finds it. Change either if they are not right. **Tools**
   shows which `tools.toml` is used (the usual one, unless you choose
   another), and, for a list that cleans plugins with xEdit, anything in it
   that is not set up; Install waits until it is. **Other folders to look in** are folders
   that may already hold some of the list's archives -- another instance's
   downloads, a backup -- looked in before anything is downloaded; an archive
   found there is linked into the downloads folder (`download
   --archive-search-path`).
4. **Install.** The steps run one after another -- plan, download, check,
   install -- each showing what it is doing, what it writes, and the command
   that does the same from a terminal (with a button to copy it).

   Archives that need a click (a free Nexus account, or a file the list says
   to fetch by hand) are listed with a button to open each page; click the
   download button there, and each file is picked up as it arrives, including
   from your browser's downloads folder, which it is moved out of. The install
   carries on once all have arrived.

   If `check` finds a problem, the run stops there, with the findings listed,
   as the command would.
5. **Still to do by hand**: the list's steps that no tool can take, such as
   building the Bashed Patch in Wrye Bash. Each ticks itself when it is done
   (when the plugin it makes appears); the rest you tick yourself.

As with `install`, close MO2 before installing: it writes its own copy of the
profile back when it exits.

The GUI keeps its own settings -- recent lists, and where each list was
installed -- in `gui.toml` beside `tools.toml`, never in the list.

## Author mode

**Author** in the top right switches to working on the list as its author.
It reads the list as planned with the answers given in Install, and plans it
again after each change.

Every change is one of the [editing commands](usage.md#editing-a-list),
made the way the command makes it: planned, checked to leave a valid list,
and written at once -- only if the file has not changed on disk since it was
read. The bar at the bottom shows the last change's command, and a click
shows the diff it made. **Undo** and **Redo** (Ctrl+Z, Ctrl+Shift+Z) step
back and forward through this session's changes, as long as the file is
still what this window last wrote; a change made in an editor ends the
history there, rather than be undone by it.

- **Archives** on the left, beside the list: every archive in the downloads
  folder and the other folders chosen in Install, newest first, with what
  its MO2 `.meta` says (the mod's name, its version, whether it came from
  Nexus) and the mods that already use it (hidden unless asked for). Paste a
  Nexus link above it to choose one of that mod's files: one already
  downloaded is used as it is; otherwise it is downloaded (with a Premium
  key), or its page is opened and it is picked up when the browser has
  downloaded it. A Nexus API key is needed to list a mod's files.
- **Adding a mod**: double-click an archive (or right-click, **Add to the
  list**). Name it (the mod's name on Nexus, or the file's), choose its
  section and where in it, and an `if` if it is optional. An archive whose
  `.meta` does not say where it came from is looked up on Nexus by its
  checksum, as `mudcrab identify` does; the source can be changed by hand.
  Most archives install as they are; one with an installer asks its
  questions, as `add` does in a terminal -- a FOMOD's steps in order (a step
  hidden by an earlier answer is never asked), a BAIN wizard's questions, or
  which BAIN packages -- with the installer's own choice ticked, and
  **Defaults for the rest** to take it everywhere. The plugins it then
  installs are listed to declare, with where they load (after the last
  plugin of the mod before it, unless you say). The entry is shown as the
  diff it makes, with the `mudcrab add` command that writes the same; **Add
  to the list** writes it, undoably.
- **The list** on the left: sections as a tree, each mod with a note of
  whether it is optional (`if`), left unticked in MO2 (`off`), or not made
  from archives. Choose one to see it on the right. Drag a mod by its handle
  onto another mod to put it before or after that one, or onto a section to
  put it at the end of it (`move`). Right-click a mod to untick it in MO2 (its
  plugins leave the load order: `disable`), tick it again (`enable`), or
  remove it from the list, with its plugins and notes (`remove`). **+** by
  the search box adds a mod, from an archive or a Nexus link.
- **Mod**: its **Notes** -- title, one-line summary and body, in Markdown,
  with a preview -- written where they already are, in the list or its docs
  file (`mudcrab docs`); its actions (each can be removed: `action remove`),
  and each of its archives as a tree of files, marked with what `install`
  takes. What a tick does follows the archive's layout, so it works as the
  installer you know:
  - **BAIN**: the packages at the top are ticked to choose them, as in Wrye
    Bash's installer tab (`files --package`); the files inside show what
    each brings, read-only.
  - **FOMOD**: its options are chosen in its installer's steps, not here:
    **Choose its FOMOD options…** walks them again, each step starting at
    what the list chooses now, and writes the new choices (`mudcrab
    choose`); the tree shows what they install. A BAIN archive has
    **Choose its BAIN packages…** too, which runs its wizard if it has one.
  - **Anything else**: untick a file or folder to leave it out (an
    `exclude`, `files --exclude`), tick it to bring it back.

  **Mod folder**, beside **Archive**, shows the same files where they end up
  in the mod, after its `file_move` actions (moved ones marked, and those a
  `file_hide` names struck through). Move a file or folder there: drag it
  by its handle onto a folder (a closed one opens as you hover), or onto the
  top of the mod, or **move…** to type its new path -- `optional/` for a
  plugin kept but not loaded, say. Each adds a `file_move` action. Below the notes, **the mod's fields** and each
  **archive's fields** are a form for what `set` changes: the `if`,
  whether it is ticked in MO2, what it needs, an archive's source, layout,
  data folder, game folder files, download page and note.

  For any layout, **exclude** beside a file or folder (on hover) is the
  expert override: an `exclude` for that path as the filters see it -- under
  BAIN, for that path in every package. The archive's `include` and
  `exclude` are listed above the tree, each with a ✕ to take it out, and a
  box to add one. Greyed out is what the layout does not choose; ○ is what a
  filter leaves out. This needs the downloads folder, as `mudcrab files`
  does.
- **Load order**: every plugin, the mods that ship it, and once a check has
  read them, its masters. A plugin placed above one of its masters, or
  needing one that is not in the list, is inactive, or is merged away, is
  flagged, with why -- and the flags follow as you drag a plugin by its
  handle to a new place (`plugins move`).
- **Conflicts**: which installed mods provide the same files, who wins (a
  later mod in MO2's order), and, a click on a pair, the files both provide
  -- `mudcrab conflicts`. **Hide these in** a mod adds a `file_hide` with
  `conflicts_with` to it, so the other mod's copies are used whatever the
  order. This reads the instance chosen in Install, so the list needs
  installing there first.
- **Check**: `mudcrab check`, on demand, its findings under the same
  headings; a finding about a mod opens that mod.
- **Questions**: the list's inputs, each with the mods whose `if` asks it.
  Change one's wording, answers or default (`input set`), add one (`input
  add`), or remove one no mod asks any more (`input remove`).
- **Changes**: every change made in this session, as the commands that make
  them -- run in order on another copy of the list, they make the same
  changes -- and as one diff.

## Building it

You need Rust, Node.js, and on Linux WebKitGTK 4.1 (`webkit2gtk-4.1` on Arch,
`libwebkit2gtk-4.1-dev` on Debian and Ubuntu). From the repository:

```bash
cd gui
npm install
npm run build
cd ..
cargo build --release -p mudcrab-gui --features custom-protocol
```

```bash
target/release/mudcrab-gui path/to/list.toml
```

The list is optional; without it the GUI starts on its own page.
