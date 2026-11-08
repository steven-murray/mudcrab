# The GUI

**New.** Everything the GUI does is one of the commands in
[Getting started](getting-started.md), which stay the complete way to use
mudcrab.

## Getting it

Each [release](https://github.com/steven-murray/mudcrab/releases) has the GUI
beside the command line:

- **Linux, and the Steam Deck in desktop mode**, either:
  - `mudcrab-gui-<version>-x86_64.flatpak`, installed like any Flatpak and
    then found among your applications (and updated by installing the next
    one the same way):

    ```bash
    flatpak install --user mudcrab-gui-<version>-x86_64.flatpak
    ```

    It uses the GNOME runtime, which Flatpak fetches from Flathub the first
    time. It sees your home folder, SD cards and other drives, and runs
    Wine or Proton (for xEdit) and LOOT on the system itself, outside its
    sandbox, so they need to be installed there as usual.
  - or `mudcrab-gui-<version>-x86_64.AppImage`: make it executable (in a
    file manager's Properties, or `chmod +x`) and run it; nothing is
    installed. It carries its own WebKitGTK, so it is large, and needs glibc
    2.35 or newer.
- **Windows**: `mudcrab-gui-<version>-windows-x64-setup.exe` installs it for
  your account (no administrator needed) with a Start menu entry and an
  uninstaller, and the command line with it: `mudcrab` then runs from any
  terminal opened afterwards (the installer puts its folder on your `PATH`,
  and the uninstaller takes it off). `mudcrab-gui-<version>-windows-x64.zip`
  is the programs alone, to unpack and run (its `install.ps1` puts the
  command line on your `PATH`, as below). Both use Windows' own WebView2, which Windows 10 and 11
  have; the installer fetches it on the rare system without.

The first time it opens, and the first time a list is open, a short tour
shows what is where; **?** at the top plays it again.

The GUI's install mode reads a list and installs it from one window:

1. **Open a list**: its `.toml` file. Its guide is on the left -- sections,
   each mod with its notes and plugins, and the load order -- read-only.
   **About this list** shows what its author wrote for whoever installs it
   (its `about`), or, for a list without one, its guide's introduction.
2. **Set up its profile**, in **Profile**: a name, which is the MO2 profile
   it installs into (an existing one, or a new one typed in), and the
   answers to the list's questions. The list's defaults are chosen, and each
   profile's answers are remembered, so one MO2 instance can hold the list
   several times with different choices; see
   [Several profiles](usage.md#several-profiles-in-one-instance).
3. **Say where things go**, in **Settings** (the cog), which opens by
   itself for a list with no MO2 instance chosen yet and stays until one is
   -- installing a list and writing one both need it, and the choices are
   shared by both modes. It has two parts:
   - **This list**: first the MO2 instance to install into -- the folder
     with its `ModOrganizer.ini` in it (for a portable instance, the folder
     `ModOrganizer.exe` is in; any other folder is refused, and the folder of
     an instance's mods and profiles is found from it where it can be). The
     mods and profiles go where that ini says (its base directory). The
     downloads folder and the game folder then follow from it: the downloads
     folder from `tools.toml` if it is set there, else the instance's own;
     the game folder from the instance's settings, or found as `game-dir`
     finds it. Change either if they are not right. **Other folders to
     search for archives** are folders that may already hold some of the
     list's archives -- another instance's downloads, a backup -- looked in
     before anything is downloaded; an archive found there is linked into the
     downloads folder (`download --archive-search-path`).
   - **mudcrab, for every list**: what is in `tools.toml`, which the command
     line reads too -- which `tools.toml` (the usual one, unless you choose
     another), your **Nexus API key** (from Nexus's API keys page), the usual
     downloads folder, and where xEdit (and its Quick Auto Clean), LOOT and,
     off Windows, the Wine prefix or Proton are. For a list that cleans
     plugins with xEdit, what is not set up yet is shown, and Install waits
     until it is. These are also here from the opening page, with no list
     open.
4. **Install.** The steps run one after another -- plan, download, check,
   install -- each showing what it is doing, what it writes, and the command
   that does the same from a terminal (with a button to copy it).

   Archives that need a click (a free Nexus account, or a file the list says
   to fetch by hand) are listed with a button to open each page; click the
   download button there, and each file is picked up as it arrives, including
   from your browser's downloads folder, which it is moved out of. The install
   carries on once all have arrived.

   If `check` finds a problem, the run stops there, with the findings listed,
   as the command would. A plan that passed check before -- the same answers,
   list and downloads folder -- is not checked again, unless **Check again**
   is ticked.

   **Cancel** stops a run at its next archive or mod: nothing is left half
   unpacked, and what was installed is kept, so installing again carries on
   from where it stopped.
5. **Still to do by hand**: the list's steps that no tool can take, such as
   building the Bashed Patch in Wrye Bash. Each ticks itself when it is done
   (when the plugin it makes appears); the rest you tick yourself.

As with `install`, close MO2 before installing: it writes its own copy of the
profile back when it exits.

The GUI keeps its own settings -- recent lists, and where each list was
installed -- in `gui.toml` beside `tools.toml`, never in the list.

## Author mode

To start a list of your own, **Create a new list…** on the opening page asks
its name and where to keep it (`mudcrab new`), then the MO2 instance to try
it in (**Settings**, as above), and opens it here, empty, ready for its first
mod from the downloads folder.

**Clone…** copies the open list to a new file, under a new name: every
mod, question and note, its docs file copied beside it (as
`<name>.docs.toml`), and its lock file. Files it includes, and the shims
its entries `use`, are shared rather than copied, named from wherever the
copy is. The copy opens in the list's place, with its MO2 instance,
folders and answers, ready to change -- a variant of a list, say, without
touching the original.

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
  section (or start a new one) and where in it, and an `if` if it is optional. An archive whose
  `.meta` does not say where it came from is looked up on Nexus by its
  checksum, as `mudcrab identify` does, so it goes in as a Nexus file that
  players' installs download for them. That needs a Nexus API key, which
  **Settings** keeps in `tools.toml` (where the command line reads it too).
  Failing that, a file whose name is plainly a Nexus download links its
  mod's page; the dialog says which happened, and the source can be changed
  by hand.
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
  remove it from the list, with its plugins and notes (`remove`). Its
  **Make optional** submenu puts it in one of the list's groups of optional
  mods, or a new group, in one click each; **Custom condition…** writes its
  `if` instead. **+** by
  the search box adds a mod, from an archive or a Nexus link.
- **Mod**: its **Notes** -- title, one-line summary and body, in Markdown,
  with a preview -- written where they already are, in the list or its docs
  file (`mudcrab docs`). Then its settings:
  - **Optional**: **Make this mod optional…** puts it among the list's
    optional mods (`optional`): in the list's own group or another, ticked
    or not to start -- each group is one question in Install's Profile
    panel, a tick per mod. Or it chooses the answers that bring it in -- one
    of the list's questions and which answers, a new question (added to the
    list with it: `input add`), or an `if` written out -- and writes the
    mod's `if`. **Always install it** takes both out.
  - **Inactive**: ticked, the mod is installed but left unticked in MO2,
    its plugins inactive (`disable`; unticked again, `enable`).
  - **Needs**: **Choose…** ticks the mods it needs from every mod in the
    list, found by name (`dependencies`). Once the masters are read
    (**Find from its masters**, or the load order), the mods its plugins'
    masters come from are offered too -- **Add them**, or ticked to start in
    **Choose…** -- and an optional one it may not need is named, with what
    to do. A mod `check` found is marked with why (*a master*, *its files*).
    For every mod at once, **Check** has **Add plugin masters as
    dependencies to all mods**, which sets each mod's
    `dependencies.masters` (see [Dependencies](modlist-format.md#dependencies)),
    taking out those no longer needed; and **Find resource dependencies**
    (`check --masters-only --resources`), which lists the mods that alone
    provide files a mod's plugins name, then **Add resource dependencies to
    all mods**, likewise for `dependencies.resources`. Each is one edit,
    undone at once; a mod's right-click menu has the same for that mod
    alone. They cover the mods the current answers install, and leave out
    an optional mod a mod that stays would need, a mod in another included
    file, and a dependency that would make a loop -- each said.

  Its actions follow (each can be removed: `action remove`), then each of
  its archives: its installer's choices, its filters and its fields (`set`),
  each written as it is changed -- its source, whether it is optional (as
  for a mod), its layout (**Automatic**, the default, finds the data folder
  in the archive), the data folder for a layout that names one, a folder
  in the mod to put it under, its game folder files, and, for one not from
  Nexus, its download page and note. Its file name is what the download
  gave it, so it is shown, not changed. What a BAIN or FOMOD archive installs is chosen as in the
  installer you know:
  - **BAIN**: its packages are ticked to choose them, as in Wrye Bash's
    installer tab (`files --package`); **Choose its BAIN packages…** runs
    its wizard, if it has one.
  - **FOMOD**: **Choose its FOMOD options…** walks its installer's steps
    again, each starting at what the list chooses now, and writes the new
    choices (`mudcrab choose`).

  Each archive's `include` and `exclude` are listed, each with how many
  files it matches (one that matches nothing is marked: filters match a
  file by its path as the archive lays it out, which hovering a file
  shows), a ✕ to take it out, and a box to add one.

  **Files**, below, is the mod's folder as installed, from all its
  archives together (hover a file for the archive and path it comes from).
  Untick a file or folder to leave it out -- an `exclude` on its archive,
  for the path the filters see it as (`files --exclude`); under BAIN it
  applies in every package -- and tick it to bring it back. A file left out
  stays in the tree, greyed, where it would go. Drag a file or folder by its
  handle onto a folder (a closed one opens as you hover), or onto the top of
  the mod, or **move…** to type its new path -- `optional/` for a plugin kept
  but not loaded, say: each adds a `file_move` action, and the tree shows it
  in its new place (moved ones marked, and those a `file_hide` names struck
  through). Files of BAIN packages or FOMOD options not chosen are not
  listed. This needs the downloads folder, as `mudcrab files` does.
- **Load order**: every plugin, the mods that ship it, and its masters,
  read when the tab opens (`check --masters-only`: slow the first time for a
  large solid archive, then moments). Untick a plugin to leave it inactive
  in MO2 (`plugins deactivate`), tick it to make it active (`plugins
  activate`). Click a plugin to see its masters: it is shaded, its masters
  shaded in another colour, and its mods marked in the list on the left. A
  plugin placed above one of its masters, or needing one that is not in the
  list, is inactive, or is merged away, is flagged, with why -- and the flags
  follow as you drag a plugin by its handle to a new place (`plugins move`).
- **Conflicts**: which installed mods provide the same files, who wins (a
  later mod in MO2's order), and, a click on a pair, the files both provide
  -- `mudcrab conflicts`. **Hide these in** a mod adds a `file_hide` with
  `conflicts_with` to it, so the other mod's copies are used whatever the
  order. This reads the instance chosen in Install, so the list needs
  installing there first.
- **Check**: `mudcrab check`, on demand, its findings under the same
  headings; a finding about a mod opens that mod.
- **Questions**: the list's inputs, each with the mods whose `if` asks it.
  A *multi* question (any of its answers) is ticked in Install's Profile
  panel, one box per answer. **Asked only if** gives a question an `if` on
  the questions above it: in the Profile panel it shows only when that
  holds, and a default that follows an earlier answer (`default_when`)
  changes with it until it is answered.
  Change one's wording, answers or default (`input set`), add one (`input
  add`), or remove one no mod asks any more (`input remove`).
- **Changes**: every change made in this session, as the commands that make
  them -- run in order on another copy of the list, they make the same
  changes -- and as one diff.

## Building it

To build it yourself, you need Rust, Node.js, and on Linux WebKitGTK 4.1
(`webkit2gtk-4.1` on Arch, `libwebkit2gtk-4.1-dev` on Debian and Ubuntu).
From the repository:

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

The bundles a release ships are made by Tauri's bundler, from `gui/`:
`npx tauri build --bundles appimage` on Linux, `--bundles nsis` on Windows,
into `target/release/bundle/`. On a distribution newer than the AppImage
tools (Arch and its family), their `strip` cannot read the system's
libraries; `NO_STRIP=true` before the command skips it. The Flatpak is made from the
binary that builds, with `flatpak-builder` and the manifest in
`gui/flatpak/`, which says how.
