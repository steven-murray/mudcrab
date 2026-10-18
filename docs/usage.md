# mudcrab Usage Guide

This guide describes the currently implemented command flow.

## Pipeline

1. Compile source TOML into compiled JSON.
2. Resolve query inputs into a personalized plan JSON.
3. Download archives into the downloads folder.
4. Install staged mod archive layout from those archives, then build any
   declared merges.
5. Diff the result against a reference instance to verify the section.

`inspect` sits before all of this: it reads an archive and prints the modlist
entry it wants, so step 1 can be written without extracting anything by hand.

## Output and logs

Each run writes its full log -- a line per mod and per archive -- to
`~/.local/state/mudcrab/logs/<command>-<time>.log` (`%LOCALAPPDATA%\mudcrab\logs`
on Windows; the newest 30 are kept). The terminal shows progress bars for the
long phases, any warnings, and a short summary on stdout. A failed run prints
the error and its causes, then `full log: <path>`.

- `-v` / `--verbose` mirrors the full log to stderr instead of drawing bars.
- `--log-file <FILE>` writes the log somewhere else.
- `RUST_LOG` (e.g. `RUST_LOG=mudcrab=debug`) sets how much the log records.

## Commands

## add

Add one `[[mods]]` block to a source modlist, in place.

```bash
mudcrab add <modlist.toml> --archive <archive> --id "<mod id>" --section "<name>" [--nexus <modid>/<fileid> | --manual] [--plugin <FILE>]... [--load-after <PLUGIN> | --load-last] [--if <CONDITION>] [--dry-run]
mudcrab add <modlist.toml> --nexus <modid>/<fileid> --id "<mod id>" --section "<name>" [--file-name "<archive.7z>"] [--plugin <FILE>]... [--load-after <PLUGIN> | --load-last] [--if <CONDITION>] [--dry-run]
mudcrab add <modlist.toml> --from-mo2 <MODS_DIR> --mod "<mod folder>" [--id "<mod id>"] [--section "<name>"] [--file-name "<archive.7z>"] [--dry-run]
```

[Writing a modlist](authoring.md) walks through using it on a new list.

`add` only writes the mod's declaration into the list; nothing is installed or
unpacked until `install`.

`--archive` writes the archive entry from the archive itself, as `inspect`
reads it: the source from the MO2 `.meta` beside it (or `--nexus`, or
`--manual` for `manual:<file name>`, with the file's `size` and `md5` so a
renamed copy is still found), the file name and the layout. An archive with
no `.meta` saying where it came from is looked up on Nexus by its MD5, as
`identify` does, when there is an API key to ask with. One that stays a
`manual:` archive, but whose name is plainly a Nexus download's
(`<title>-<mod id>-<version>...`), gets that mod's page as its
`download_page`, so whoever installs the list knows where to fetch it. In a
terminal, an installer's choices are asked: a FOMOD step by step, through the
same walk install makes (so a step shown only after certain answers is shown
here only then), and a BAIN package's subpackages as one list. A BAIN package
with a `wizard.txt` has its wizard run instead, as Wrye Bash runs it --
questions, conditions, variables, loops -- and what it does becomes the entry:
the subpackages it selects, the plugins in them it leaves unselected (as
`exclude`), plugins it renames (`file_move` actions) and its `EditINI` calls
(`ini_tweak` actions). When the wizard asks whether a file is already
installed, a plugin is answered from the list's load order and the game's own
files are taken as present; anything else is put to you, since only you know
what the rest of your list installs. A wizard that cancels, or uses something
not followed (`Exec`), falls back to the subpackage list. Enter takes the
installer's own choice; the answers are written as `fomod_selections` or
`bain_subpackages`. With `--defaults`, or when input is not a terminal, nothing
is asked: each FOMOD group gets its default, a BAIN wizard runs on its own
defaults (a file it asks about counts as absent), other BAIN packages list
every subpackage,
and `add` notes what may need changing, as well as plugins in the archive not
given with `--plugin`. `--nexus` alone writes the source, and, with
`NEXUS_API_KEY` set, the file name from Nexus's list of the mod's files.

The edit is a line-based splice, not a parse-and-rewrite: every byte of the file
outside the inserted block -- comments, blank lines, tab indentation, key order --
is preserved exactly. The block goes after the last existing mod in `--section`,
so the file stays grouped; if that section has no mods yet, it is appended at the
end of the file. Repeat `--section` for a nested section path, outermost first.

`--from-mo2` takes a mod from an MO2 instance where it is already installed --
the way to turn a list built by hand in MO2 into a modlist, one mod at a time.
It reads `<MODS_DIR>/<mod folder>/meta.ini` as MO2 writes it and takes `modid`
+ `[installedFiles] 1\fileid` for the `nexus:oblivion/<modid>/<fileid>` path,
`installationFile` for `file_name`, and `version` for a trailing
`# version ..., as installed in MO2` comment. The mod id defaults to the folder
name. The directory is only ever read from. (`--from-oracle` is the same flag's
old name.)

A folder with `modid=0` was not installed from Nexus: the block is written with
`file_name` but no `path`, marked with a `# TODO: non-Nexus source` comment, and a
warning goes to stderr. No URL is invented.

If the MO2 folder holds `.esp`/`.esm` files, they are listed on stderr. They
are **not** added to the top-level `plugins` array and the block declares no
`plugins` key -- putting a plugin at the wrong point in load order fails silently
in game, so the position is yours to choose (`--plugin` with `--load-after`).

`--plugin <FILE>` (repeatable) writes the block's `plugins` key, and `--if
<CONDITION>` its `if`, for an optional mod (the input has to be declared
first). A plugin the top-level `plugins` load order does not list yet goes
where `--load-after <PLUGIN>` or `--load-last` says; with neither, `add` stops
before writing, rather than guess a position. A new list with no `plugins`
array gets one.

Safety:

- an id that already exists in the file is refused;
- the new text is parsed and validated before anything is written; an edit
  that would leave the list invalid is refused, and the file is untouched;
- the file is written via a temp file next to it, then renamed, and only if
  nobody changed it since it was read;
- `--dry-run` prints the block and its destination and changes nothing.

Example:

```bash
mudcrab add mylist.toml \
  --from-mo2 ~/mo2/MyHandBuiltList/mods \
  --mod "Blockhead" --section "OBSE PLUGINS" --dry-run
```

## move

Move a mod's block to another section, or before another mod, keeping its
comments, archives and actions; only its `section` line changes.

```bash
mudcrab move <modlist.toml> --mod "<mod id>" [--section "<name>"]... [--before-mod "<mod id>"] [--dry-run]
```

As with `add`, the result is validated before it is written.

## Editing a list

`add` and `move` have siblings for the rest of what an author changes by hand.
All of them edit the file in place the way `add` does: only the lines that
change are touched, comments stay where they are (a replaced value keeps its
end-of-line comment), the result is validated before anything is written, and
`--dry-run` prints the change as a diff instead. A GUI built on mudcrab makes
its edits through these same commands, so anything it can do, they can.

### new

```bash
mudcrab new <modlist.toml> --name "<name>"
```

Starts a list: its name, and a load order holding only the game's master,
ready for `add`. Never written over a file that is already there.

### set

```bash
mudcrab set <modlist.toml> --mod "<mod id>" [--archive <n>] [key=value]... [--unset <key>]... [--dry-run]
```

Sets or removes fields of a mod, or with `--archive` (counting from 1) of one
of its archives: `if=butterflies`, `layout=bain`, `data_folder="Data Files"`,
`merge.output=Merged.esp`. A value is read as TOML when it is one (`true`,
`3`, `["a.esp", "b.esp"]`) and as text otherwise; quote it to force text
(`version='"1.0"'`). `id`, `section`, `archives` and `actions` are refused,
naming the command that changes them.

### choose

```bash
mudcrab choose <modlist.toml> --mod "<mod id>" [--archive <n>] [--downloads <dir>]
               [--set key=value]... [--defaults] [--dry-run]
```

Walks an archive's installer again, as `add` walks it for a new entry -- a
FOMOD step by step, a BAIN wizard, or a BAIN package list -- with what the
list chooses now as each question's default, and writes the new choices:
`fomod_selections` (one table per group asked, each keeping its comments) or
`bain_subpackages` (with the plugins a wizard leaves out added to
`exclude`). The archive is found in the downloads folder. Selections or
packages that depend on the list's questions (an `if`) are refused, to be
changed in the file. With `--defaults` (or no terminal) nothing is asked: the
list's choices are kept, and a group it does not name gets the installer's
own.

### docs

```bash
mudcrab docs <modlist.toml> --mod "<mod id>" [--title <text>] [--summary <text>]
             [--body <text> | --body-file <file>] [--footer <text>] [--unset <field>]... [--dry-run]
```

Writes a mod's notes (see [Docs](modlist-format.md#docs-prose-for-export))
where they already are: inline in the list as `docs.<field>`, or in the docs
file the list names as `[mods."<id>"]`. A mod with no notes yet gets them in
the docs file when the list has one. Text over several lines is written as a
`'''` block, as a person would write it; `--body-file -` reads the body from
standard input. A mod whose notes are all removed loses its table.

### remove

```bash
mudcrab remove <modlist.toml> --mod "<mod id>" [--dry-run]
```

Removes the mod's block -- its comments, archives and actions -- and its
plugins from `plugins` and `inactive-plugins` (unless another mod declares
them too). Refused while anything still refers to the mod, such as another
mod's `conflicts_with`; `check` afterwards finds any plugin that needed one of
its plugins as a master. Its notes in the list's docs file go too.

### disable, enable

```bash
mudcrab disable <modlist.toml> --mod "<mod id>" [--dry-run]
mudcrab enable <modlist.toml> --mod "<mod id>" [--load-after <plugin> | --load-last] [--dry-run]
```

`disable` leaves a mod installed but unticked in MO2 (`enabled = false`), and
takes its plugins out of the load order, where MO2 could not see them.
`enable` ticks it again and puts its plugins back: after `--load-after`, at
the end with `--load-last`, or by default after the last plugin of the
nearest mod before it that has any.

### plugins

```bash
mudcrab plugins list <modlist.toml>
mudcrab plugins move <modlist.toml> <plugin> (--after <plugin> | --before <plugin> | --first | --last) [--dry-run]
```

`list` prints the load order, a plugin a line, tab-separated: the plugin, the
mods that declare it (or make it, for a merge; `-` when none does, `(the
game)` for the game's own), and `inactive` for one the list leaves unticked.

`move` moves one plugin. Comment lines in the array stay put, so a heading
such as `# Part 3` stays above whatever followed it; `--before` puts the
plugin directly above the named one, below any heading over it, and the
plugin's own end-of-line comment goes with it.

### files

```bash
mudcrab files <modlist.toml> --mod "<mod id>" [--archive <n>] [--downloads <dir>] [--set key=value]...
mudcrab files <modlist.toml> --mod "<mod id>" [--include <glob>]... [--exclude <glob>]...
                             [--drop-include <glob>]... [--drop-exclude <glob>]...
                             [--package <name>]... [--drop-package <name>]... [--dry-run]
```

Without filters to change, prints the files `install` would take from the
archive (the first, unless `--archive`), before the mod's actions run: the
same plan `check` makes, from the archive in the downloads folder. With
filters, edits the archive's `include` and `exclude`, and shows which files
that adds and drops. `--package` and `--drop-package` choose a BAIN package
or stop choosing it (`bain_subpackages`), as ticking one in Wrye Bash's
installer tab does; a package chosen only under a condition is changed in the
file itself. `--set` answers the list's questions where they decide
which archives a mod has; the rest take their defaults.

### action

```bash
mudcrab action list <modlist.toml> --mod "<mod id>"
mudcrab action add <modlist.toml> --mod "<mod id>" <action> [key=value]... [--if <condition>] [--dry-run]
mudcrab action remove <modlist.toml> --mod "<mod id>" <n> [--dry-run]
```

`list` numbers a mod's actions, one a line. `add` appends one as a
`[[mods.actions]]` table, with its fields as `key=value` read as `set` reads
them -- `mudcrab action add list.toml --mod "WAC" file_hide 'paths=["meshes/x.nif"]'`.
A field the action does not have is refused. See [Actions](#actions) for each
action's fields.

### input

```bash
mudcrab input list <modlist.toml>
mudcrab input add <modlist.toml> <name> --type bool|choice|text --query "<question>" [--choice <c>]... [--default <answer>] [--dry-run]
mudcrab input set <modlist.toml> <name> [key=value]... [--unset <key>]... [--dry-run]
mudcrab input remove <modlist.toml> <name> [--dry-run]
```

The list's questions. `add` writes them the way the list already does (dotted
`inputs.<name>.<key>` lines at the top, if it has none yet). `remove` is
refused while a condition still uses the question.

## compile

Check a source modlist and write it out compiled: sections flattened, the
top-level INI tables turned into actions, each archive's fields checked.

```bash
mudcrab compile <modlist.toml> --output <compiled.json> [--strict]
```

What it checks is the list's own consistency, from the TOML alone:

- each input has a question, and its default fits its type;
- each condition (`if`) asks about a declared input or an existing mod;
- dependencies and `conflicts_with` name mods that exist;
- each plugin a mod declares is in the load order, once, with a valid name;
- merges name their sources, and nothing depends on itself in a cycle;
- each archive has a `path` (or `build`), the fields its `layout` needs, and
  `size` with `md5`.

It downloads and opens nothing, so it cannot see inside archives: whether an
archive is there and opens is for `check`, whether a FOMOD option or BAIN
subpackage exists for `install`, and which mods overwrite each other's files
for `conflicts`. (Whether each plugin's masters are present is not checked;
MO2 flags a missing master.) Nor does it answer the list's questions; that is `query`. A list running over
Oblivion's 255-plugin limit is a warning; `--strict` makes warnings errors.
The docs file (`docs_file`) is not read: only `export` and `validate` need it.

Example:

```bash
mudcrab compile tests/fixtures/modlists/simple.toml --output build/compiled.json
```

## query

Resolve user inputs and produce a personalized plan.

```bash
mudcrab query <compiled.json> --output <plan.json> [--headless]
```

Example:

```bash
mudcrab query build/compiled.json --output build/plan.json --headless
```

## download

Download archives required by a personalized plan.

```bash
mudcrab download <plan.json> [--downloads <dir>] [--retry <n>] [--parallel <n>]
                 [--section <name>]... [--only <mod id>]...
                 [--archive-search-path <dir>]...
                 [--no-browser | [--batch <n>] [--watch <dir>] [--browser-downloads leave|move|link]]
```

Example:

```bash
mudcrab download build/plan.json --downloads ~/Games/OblivionModDownloads --retry 3
```

`--parallel` (default 4) is how many archives are fetched at once.

A failed source does not stop the run. Every archive is attempted, the failures
are reported together at the end and the exit code is non-zero -- the same shape
as `check`. A section routinely contains more than one dead Nexus link, and
aborting on the first one turned finding them into one round trip each.

### Downloading by hand, quickly: the browser

Without a premium Nexus account (or without an API key, or for a `manual:`
archive) a person has to click to download each file. `download` makes that
as quick as clicking allows, whenever it runs in a terminal and a browser can
be opened (on Linux, a desktop session with `xdg-open`); `--no-browser` turns
it off, reporting what is missing and failing instead. After the direct downloads, it opens the page of
each archive still missing -- 10 at a time, `--batch` to change that -- and
waits. Each file is picked up as it arrives, and its page is replaced by the
next one:

- **"Mod Manager Download"** (not on Linux) hands the file to MO2, which
  saves it into its downloads folder with a `.meta`. When that is mudcrab's
  downloads folder (`install` sets the instance up that way), the file is
  simply there. MO2 has to be open for this; the command says whether it is.
  On Linux the pages open on the browser download instead: there, a Nexus
  link goes to whichever MO2 instance owns `nxm:` links (often not this
  list's when you have several), and the usual handler, which starts the
  game's Steam entry, does nothing while that is already running.
- **"Slow download"** (or "Manual Download") has the browser save it, normally to your Downloads
  folder (`--watch` names another). The first time a file arrives there, you
  are asked whether to leave such files where they are (you will move them
  yourself), move them into the downloads folder (recommended), or hard-link
  them there. `--browser-downloads leave|move|link` answers in advance.

A file counts as arrived when it has the archive's name (or, for a `manual:`
archive that records them, its size and MD5) and has stopped growing; a
browser's partial `.part` or `.crdownload` file is ignored. A `manual:`
archive's page is the list's `download_page`, opened like the rest, with its
`download_note` printed beside it; one with no page is listed, with its note,
for you to fetch, and waited for too.

Ctrl-C stops; running the command again carries on with what is still
missing. The click is always yours: nothing presses a download button for you.

On Linux the browser takes its profile from `XDG_CONFIG_HOME`. If that is set
somewhere unusual in the shell you run `download` from, the pages open in a
fresh browser profile -- a second window without your Nexus login -- and the
command says so. To use a different `tools.toml`, set `MUDCRAB_TOOLS_CONFIG`
instead (see [setup-tools](#setup-tools)).

## The downloads folder

Archives live in one **downloads folder**, under their real names, each with the
`.meta` file MO2 writes beside a download. It is an ordinary MO2 downloads
folder, so MO2 can be pointed straight at it, and the recommended setup is one
central folder -- `~/Games/OblivionModDownloads`, say -- that every MO2 instance
and every modlist share.

It is chosen by, in order: `--downloads <dir>` (`--cache` is an alias);
`downloads_dir` in tools.toml; and, for an MO2 install, the instance's own
`downloads/`. `install --mo2-instance-dir` sets `download_directory` in the
instance's `ModOrganizer.ini` to the folder, so MO2 lists everything there --
including archives you may want to install by hand afterwards -- rather than
being given copies.

An archive is found by what it is, never by which mod uses it -- and, when
the list records an archive's `size` and `md5` and nothing has its name, by
content, so a hand-downloaded file its host has renamed is still found. Only
files of exactly that size are hashed.

The lookups, in order:

- by the `file_name` the list gives, when a file of that name is there;
- a Nexus archive by the `modID`/`fileID` in its `.meta` (so MO2's `(1)`
  suffixes and hand renames do not matter);
- anything else by its `file_name`, case-insensitively, or for a URL source by
  the `url` in its `.meta`;
- failing those, by `size` and `md5`, when the list records them.

So renaming a mod, reordering its archives or using one archive in several mods
changes nothing on disk. A download mudcrab makes is saved under the server's
name with a `.meta` written as MO2 would.

## Using archives you already have

`--archive-search-path` points `download` at other folders that may hold
archives -- another instance's downloads, a Wabbajack
folder. Each archive is resolved in this order:

1. In the downloads folder -- used as is.
2. In a search path, matched the same way -- hard-linked into the downloads
   folder under its own name, with its `.meta` (a copy across filesystems), so
   no download runs.
3. Otherwise downloaded.

Search paths are read-only sources: nothing is ever written, renamed or deleted
inside them. If a search path holds a newer, different-sized copy of an archive
already in the downloads folder, it replaces it -- a re-downloaded fix there is
not silently ignored.

```bash
mudcrab download build/plan.json --downloads ~/Games/OblivionModDownloads \
  --archive-search-path ~/Games/Wabbajack/Oblivion/MudCrab/downloads
```

Every later step -- `check`, `install`, `conflicts` -- then looks only in the
downloads folder.

## downloads tidy

Convert a downloads folder written by mudcrab before 0.2, when archives were
kept under derived names (`{mod}_{index}_{source}` with a `.orig-name`
sidecar), to real names.

```bash
mudcrab downloads tidy <dir> [--plan <plan.json>] [--apply]
```

A derived name that is the same file as a real-named one is deleted; one that
is the only copy is renamed; one whose real name holds a different file is left
and reported. With `--plan`, archives the modlist names only by Nexus or URL
source get the `.meta` lookup needs. Without `--apply` it only prints the plan.

Because resolution happens during `install` too, a list whose archives are all
already on disk can be installed without running `download` at all.

## downloads record

Write into a list what lets each of its archives be recognised wherever it
turns up.

```bash
mudcrab downloads record <modlist.toml> [--downloads <dir>] [--archive-search-path <dir>]... [--dry-run]
```

For every archive found on disk:

- a Nexus or URL archive (or build layer) with no `file_name` gets one, the
  name of the file it was found as. Without it, a copy with no MO2 `.meta` --
  saved by a browser, brought from another machine -- is not recognised;
- a `manual:` archive gets `size` and `md5`, so it is found after its host
  renames it.

The lines go after the archive's `file_name` (or `path`); nothing else in the
file changes, and the list is validated afterwards. Archives not on disk are
named and skipped. `add --archive` writes all of these when it writes an
entry, so this is for lists written before it did.

## downloads locate

Where each archive a plan needs is, one tab-separated line per archive: mod
id, archive number, source, and its path or `MISSING`. Only reads.

```bash
mudcrab downloads locate <plan.json> [--downloads <dir>] [--archive-search-path <dir>]...
```

For scripts, and for seeing at a glance what a downloads folder lacks.

## Working on part of a modlist

A large list is built a section at a time, so `download`, `check`, `install` and
`diff` all accept the same two flags:

- `--section <name>` -- match any level of a mod's section path,
  case-insensitively. `--section "5 - LOD"` selects `section = ["5 - LOD"]` and
  everything nested under it, such as `section = ["5 - LOD", "Meshes"]`.
- `--only <mod id>` -- match one mod id exactly.

Both are repeatable and they union: a mod is in scope if it matches *either*.
With neither flag the command processes the whole list, exactly as before.

`compile` and `query` have no such flags on purpose. They are cheap, and a
partial compiled artifact or plan would leave every later stage working from a
document that no longer describes the modlist.

```bash
mudcrab download build/plan.json --downloads ~/Games/OblivionModDownloads --section "5 - LOD"
mudcrab install  build/plan.json --downloads ~/Games/OblivionModDownloads \
  --mo2-instance-dir ~/mo2/MOFAM --section "5 - LOD"
```

A filtered install narrows what is installed; it never uninstalls the rest.
Mods skipped by the filter keep their existing `install_manifest.json` entries,
so installing section B after section A leaves both recorded as installed. The
MO2 profile is rewritten from that manifest, so `modlist.txt` lists the mods
installed so far plus the separators for the sections they belong to, and
`plugins.txt` lists only plugins that are actually on disk (in an installed mod
or in the game's own `Data` folder). Sections you have not reached yet simply do
not appear yet.

## validate

Validate a source modlist without generating compiled output.

```bash
mudcrab validate <modlist.toml> [--strict]
```

## install

Install from a personalized plan and the downloads folder into an MO2 instance
(`--mo2-instance-dir`) or a plain mods directory (`--mods-dir`).

```bash
mudcrab install <plan.json> [--downloads <dir>] (--mo2-instance-dir <dir> | --mods-dir <mods_dir>)
                [--profile-name <name>] [--game-dir <dir>] [--dry-run] [--skip-actions]
                [--force-merges] [--force] [--redo-action <action>]...
                [--section <name>]... [--only <mod id>]...
```

Example:

```bash
mudcrab install build/plan.json --downloads ~/Games/OblivionModDownloads --mods-dir build/mods
```

Into an MO2 instance, `install` writes the mods, the profile (`--profile-name`,
default `Default`, which it then owns: its `modlist.txt` and `plugins.txt` are
rewritten from the plan every run) and the load order, and points MO2 at the
downloads folder. Game INI edits go into the profile's own copy of
`Oblivion.ini`, seeded once from the game's, and `install` turns on MO2's
"profile-specific game INI files" for the profile so they take effect. The game
folder is `--game-dir`, else `GAME_DIR`, else the one the MO2 instance was set
up with.

When the list declares steps to take by hand (`[[manual-steps]]`, such as
building a Bashed Patch in Wrye Bash), `install` ends by listing those not yet
done; see [Steps by hand in The modlist format](modlist-format.md#steps-by-hand). For a list
with a Bashed Patch, it also adds Wrye Bash to the MO2 instance's executables,
when the game folder has `Mopy/Wrye Bash.exe` and MO2 does not list it yet.
A list's `[mo2] theme` is set while the instance has no theme chosen; one you
pick in MO2 afterwards is kept.

`--redo-action <action>` reinstalls just the mods that have that action
(`--redo-action qac`, say), when something outside the list changed what it
does -- a tool that was misconfigured, or mudcrab itself.

Mods are installed in the plan's order, except that a mod whose `file_prune` or
`file_hide` yields to another (`conflicts_with`) is installed after that one,
since its selection reads the other mod's installed files. MO2's priority order
is the plan's either way.

Current behavior:

1. Finds every archive in the downloads folder, where `download` put it
   (archives in a search path are linked in by `download`, not here).
2. Unpacks archives under per-mod directories.
3. Writes `install_manifest.json` in the mods directory.
4. Supports extraction for `.zip`, `.tar`, `.tar.gz`, `.tgz`, `.7z`, and `.rar`. The
   last two shell out to the system `bsdtar` (tried first) and `7z` (fallback)
   binaries, so those tools must be available on `PATH` -- see the readme's
   Requirements section.
5. Builds any `type = "merge"` mods, writing the merged plugin plus a
   `merge - <id>/` sidecar containing `map.json` (in zMerge's shape, so it diffs
   directly against a real zMerge run) and `mudcrab-merge.json`.
6. Hides each merged source plugin as `<name>.esp.mohidden`, recording it in the
   manifest. Sources stay **enabled** so their assets and BSAs keep loading.
7. Runs post-install actions (e.g. `loot-sort`, per-mod actions declared in the
   modlist) by default; pass `--skip-actions` to skip them.

Merges are built after all mods are installed and before LOOT sorts, so LOOT sees
the merged plugin rather than the sources it replaced. Re-running `install` is safe:
merges are rebuilt deterministically and hiding is idempotent.

### How the load order is applied

Oblivion has no load-order file. **The order is the plugin files' modification
times**, oldest first; `plugins.txt` says only which plugins are *active*.
Writing it in the declared order therefore looks right and applies nothing — and
MO2, finding a `loadorder.txt` that does not list everything, places the rest by
timestamp, which is whatever order the archives were extracted in.

`install` writes `loadorder.txt` next to `plugins.txt`, from the same list.
Applying it to the plugin timestamps is MO2's job and mudcrab leaves it alone:
MO2 restamps the files from `loadorder.txt` when it loads the profile.

**MO2 must be restarted** for a new order to take, since it reads the file at
profile load. Part 37 of the MOFAM guide says the same thing about pasting in
its own copy: "close & restart MO2 to have the plugins in order".

### What a re-run does not redo

Building a list section by section means running `install` dozens of times over
a plan that has barely changed, so the run is built to pay only for what moved.

**Mods.** A mod is skipped when its definition hash still matches and its folder
is on disk, as recorded in `install_manifest.json`.

**Merges.** A merge is skipped when the recorded fingerprint of its inputs still
matches *and* the plugin it produced is still there. The fingerprint covers the
`[mods.merge]` spec (output name, method, and the ordered source list), the
plan's `plugins` load order -- which decides the order of the merged plugin's
master table, so a merge cached across a load-order change would be a
plausible-looking file with the wrong header -- and, for each source plugin, its
path, size and modification time. Source paths are recorded without any
`.mohidden` suffix: a merge hides its own sources as its last step, and without
that a merge could never be skipped, since building it would change its own
inputs. Skipping the rebuild does not skip the hiding, which is idempotent and
is what makes the merge take effect.

`--force-merges` rebuilds every merge in scope regardless. Reach for it when a
source plugin was changed in a way that left its size and timestamp alone.

**The manifest** is written after each mod, not once at the end. A run that
fails on mod 250 of 300 still records the 249 that succeeded, so the next run
resumes rather than re-extracting all of them. A mod that is cleared and then
fails to extract is dropped from the manifest instead, so it is retried rather
than skipped over an empty folder. A filtered run still carries forward the
entries of the mods it skipped, so installing section B after section A does not
make A look uninstalled.

## Actions

Each mod may declare an ordered `actions` list, applied to its staged folder
after extraction. Entries run **in declaration order**, which is the only
sequencing mechanism involved. The `action` key selects the action; an
unrecognised name is a parse error naming the supported values.

All paths and globs are resolved relative to the mod's staged data folder and
are rejected if they escape it. Every action honours `--dry-run`, logging what
it would do and touching nothing.

### ini_set

```toml
{ action = "ini_set", file = "Ini/Mod.ini", key = "bEnable", value = 0 }
```

`scope` is `"mod"` (default) or `"game"`; a game-scoped write goes to the MO2
profile's copy, never the original `Oblivion.ini`. `format` is `"standard"`
(`key = value`) or `"set-to"` (`set key to value`). `value` accepts any TOML
scalar; booleans become `1`/`0`.

### ini_append_block

```toml
[[mods.actions]]
action = "ini_append_block"
file = "ini/Mod - Custom Trainers.ini"
block = """

set migFeaQ.customTrain to (GetFormFromMod "Oblivion.esm" 02D025);Uurwen
set migFeaQ.customLevl to 65
SetStage migFeaQ 1
"""
```

The guide's "paste the following into `<file>`" rows. `ini_set` cannot express
them: the same key is set several times and what runs between the settings is
part of the meaning, so order and repetition are the content rather than a list
of key/value pairs.

The block is appended verbatim to the end of the file, with line endings
rewritten to whatever that file already uses, and is idempotent — a second
install finds the block already present and appends nothing. Leading blank
lines are kept, which is how the gap before the pasted lines is spelled. One
trailing newline is dropped, since a TOML multi-line string always carries one
nobody typed; a file that ended without a final newline still ends without one.

The file must already exist. Appending to a file the mod does not ship means
the path is wrong, and a file created this way would never be read.

### ini_comment

Comment out a setting in an INI the mod ships, so the script falls back to its
own default -- what a guide means by "comment out `set X` by adding a
semi-colon". Every active line setting `key` gets a leading `;`; finding none
is an error.

```toml
[[mods.actions]]
action = "ini_comment"
file = "QZ Easy Menus.ini"
format = "set-to"
key = "EasyMenu.iAltExitKey"
```

### file_prune: keep

`keep` is the other way round: delete everything in the mod *except* the named
files and folders -- a guide's "keep only X and Y". Each must exist.

```toml
[[mods.actions]]
action = "file_prune"
keep = ["Dagger_Data.esm", "meshes/dag/architecture", "textures/dag"]
```

### ini_tweak

```toml
{ action = "ini_tweak", file = "Oscuro's_Oblivion_Overhaul.ini", key = "OOOOptions.bHK", value = 0, format = "set-to" }
```

A BAIN wizard's `EditINI`, which despite the name does not edit the INI it
names. It writes a *fragment* into the package's `INI Tweaks/` folder, and Wrye
Bash's INI Tweaks tab applies it later, on request. Use this for a wizard
question whose answer produces a tweak; use `ini_set` to change a value in the
mod's own INI directly.

`file` names the INI the tweak is for, not a path — the fragment lands at
`ini tweaks/<file>` in the staged mod, and giving a path is an error. `key`,
`value` and `format` are as `ini_set`. A second `ini_tweak` naming the same
file adds its setting to the existing fragment, which is what a wizard calling
`EditINI` twice produces.

### qac

```toml
{ action = "qac", plugins = ["*.esp"] }
```

Runs xEdit's Quick Auto Clean over the matching plugins. Requires `tes4edit` in
`tools.toml`.

### pack_bsa

```toml
{ action = "pack_bsa", output = "Example Mod.bsa", include = ["meshes/**"], exclude = ["*.esp"] }
```

Packs the staged files into a BSA using mudcrab's native writer -- no BSArch.exe
under Wine. `include` defaults to everything; `exclude` is applied after it. The
output archive always excludes itself, so re-running is idempotent rather than
nesting the previous archive inside the new one.

Payloads are stored uncompressed, which is what BSArch produces for Oblivion by
default and what Oblivion requires for voice files.

A BSA cannot address a file outside a folder, so files at the top level of the
staged mod are left loose and logged. Fails if nothing matched, rather than
writing an empty archive.

### create_dummy_plugin

```toml
{ action = "create_dummy_plugin", output = "Example Mod.esp" }
```

Writes an empty plugin: a TES4 header with no masters, records or groups, built
through the same writer that produces merged plugins. Oblivion loads `Foo.bsa`
only when a plugin named `Foo.esp` is active, so a mod shipped as a bare archive
needs one of these; give it the same stem as the BSA.

### file_prune

```toml
{ action = "file_prune", paths = ["meshes/**", "textures/**"] }
```

Deletes staged files matching the globs, then removes any folders left empty.
`paths` is required -- an empty list would match everything.

This is a separate action rather than a `pack_bsa` option because it has to run
*after* the archive is written: the loose files must still exist to be packed.
The three compose in exactly that order:

```toml
actions = [
  { action = "pack_bsa", output = "Example Mod.bsa", exclude = ["*.esp"] },
  { action = "create_dummy_plugin", output = "Example Mod.esp" },
  { action = "file_prune", paths = ["meshes/**", "textures/**", "sound/**"] },
]
```

leaving the mod folder holding just the `.bsa` and its `.esp`.

## merge

Build merged plugins from an already-installed mods directory, without
installing anything. Reads the source mod folders and writes elsewhere: it
never renames a source plugin, never writes into the mods directory, and never
touches a profile -- so a merge can be built and inspected before anything in a
real instance changes.

```bash
mudcrab merge <modlist.toml> --mods-dir <mods> --output <dir> [--only <merge id>]
```

Example:

```bash
mudcrab merge mofam.merges.toml \
  --mods-dir ~/Games/Wabbajack/Oblivion/MOFAM-03.25/mods \
  --output /tmp/mudcrab-merges --only "Unique Forts Merged"
```

Each merge is written to `<output>/<merge id>/`, alongside a
`merge - <id>/map.json` in zMerge's shape.

## unhide-merges

Restore source plugins that `install` hid on behalf of a merge, reading the install
manifest so it undoes what was actually done rather than what the modlist currently
says.

```bash
mudcrab unhide-merges --mo2-instance-dir <dir> [--profile-name <name>]
mudcrab unhide-merges --mods-dir <dir>
```

Example:

```bash
mudcrab unhide-merges --mo2-instance-dir ~/Games/MO2 --profile-name Default
```

## check

Everything about the plan that can be known before installing, with every
archive in the downloads folder. Changes nothing.

```bash
mudcrab check <plan.json> [--downloads <dir>] [--section <name>]... [--only <mod id>]...
```

It looks only in the downloads folder: `download` puts every archive there,
linking in those it finds in a search path, so that is where they are. Run
`download` first; an archive still missing is reported with the list's
`download_page` and `download_note`.

For each archive:

- **present and readable**: found by name (or by `size` and `md5`), opened to
  read its file list, which fails for a truncated or mislabelled download, and,
  where the list records an `md5`, hashed and compared;
- **installable**: the layout `install` would use is worked out from that
  list, exactly as `install` does it, so a FOMOD option or BAIN subpackage the
  archive lacks, or an archive whose layout cannot be told, fails here;
  `game_root_files` patterns must each match a file; an `include` that matches
  nothing, or an `exclude` that leaves nothing out, is a warning.

For each mod with actions that change its files (`file_prune`, `file_hide`,
`file_move`, `pack_bsa`, `extract_bsa`, `create_dummy_plugin`, `write_file`),
those actions are **rehearsed**: run, by the same code `install` uses, on a
stand-in of the mod's folder made of placeholder files (under
`~/.cache/mudcrab`, removed afterwards). A prune pattern that matches nothing,
a path to hide or move that is not there, an `except` nothing picks, a
`conflicts_with` whose mods share no file: each fails here as it would at
install. A mod whose action compares with a mod whose own files are only known
once installed is noted, not rehearsed.

Then, across the finished folders:

- **masters**: each plugin's header is read (only plugins are unpacked), and
  every master must be in the load order, active, and earlier. The game's own
  plugins count as present unless the list orders them. A master merged away
  by a `hide_sources` merge is named with the merge.
- **load order**: a plugin in `plugins` that no mod installs is an error (a
  merge's output and Wrye Bash's `Bashed Patch, 0.esp` are made later), as is
  a plugin a mod declares but does not install. A plugin a mod installs that
  `plugins` does not list is a warning: MO2 shows it unticked.
- **BSAs**: Oblivion loads a BSA only when an active plugin's name begins its
  own (`Foo.esp` loads `Foo.bsa` and `Foo - Textures.bsa`). A BSA no active
  plugin loads, and no INI action names, is an error: its contents would never
  reach the game.
- **duplicates**: the same plugin from two mods is a warning. It is allowed --
  the copy from the mod lowest in MO2's list wins -- but worth knowing.

Plugins made during the install (merge outputs) and mods built from layers,
inside an `inner_archive`, or installed into MO2 itself are checked as far as
they can be; the summary says how many plugins it could not read. Every
problem is reported together, and the summary and warnings are printed even
when the check fails. Mods are checked in parallel, a thread per CPU; a
700-mod list takes under a minute.

## steps

What a list leaves to do by hand after installing -- building a Bashed
Patch, settings only the game can load -- and what of it is done:

```bash
mudcrab steps <plan.json> --mo2-instance-dir <instance> [--game-dir <dir>]
```

A step the list says how to recognise (`done_when_plugin`: the plugin it
makes has appeared in a mod or in MO2's `overwrite/`, built since the last
install that changed the mods or load order) shows ✔ once done;
the rest show ○ with their instructions, every time. `install` prints the
ones not done at the end of each run; the GUI shows them as a checklist.

## assets

After an install: the meshes and sounds the active plugins name that the game
would not find.

```bash
mudcrab assets --mo2-instance-dir <dir> [--profile-name <name>] [--game-dir <dir>] [--all]
```

Everything the game would see is gathered first: the game's own `Data` and
its BSAs, each mod the profile enables (with the BSAs inside it), and
`overwrite/` -- reading only each BSA's file table. Then every active plugin is
read, and each model it names (`MODL`, and armour and clothing's `MOD2`-`MOD4`;
a tree's SpeedTree file, under `Trees`) and each sound file is looked for.

It is a report, not a check that fails: many well-known mods name files that
were never shipped. What the game's own plugins already lack is left out, so
what remains is what the list added. Each plugin's first few missing files are
shown with the record that names them (`--all` for every one); a file several
plugins name, often one mod left out, is listed at the end. Textures are named
by meshes rather than plugins, and are not checked. The profile defaults to
`Default`, and the game folder to the instance's. On a 700-mod instance it
takes a few seconds.

## conflicts

Which installed mods provide the same files, and which wins. Reads the mod
folders (and the contents of their BSAs, from each BSA's directory alone), in
the plan's order: a later mod wins, as in MO2.

```bash
mudcrab conflicts --plan <plan.json> --mods-dir <mods> [--section <name>]... [--only <mod id>]...
mudcrab conflicts --plan <plan.json> --mods-dir <mods> --mod <mod id> [--with <mod id>]... [--under <folder>]
```

Without `--mod`, the whole list is swept. Every mod sharing files with another
is listed with how many it wins and loses, and to which mods (the five with
most in common first):

```text
Unofficial Oblivion DLC Patches - UODP  [UNOFFICIAL PATCHES]
  wins 1 file over: Unofficial Oblivion Patch - UOP (1)
  loses 48 files to: T4UT - Menus Repolished (27), DLC Upscaled Textures (DLCUT) (12), ...
```

That is the overview to start from: it shows which pairs to look at.
`--section` and `--only` narrow the report, not the comparison.

With `--mod`, one mod's files are listed -- all of them, or with `--with` only
those the named mods also provide, optionally `--under` one folder. That is
exactly the set `file_prune` with `conflicts_with` acts on, so a row can be
checked before it is written. A mod not yet installed is read from its
archives, which needs the downloads folder.

## inspect

Read an archive and print what its `[[mods.archives]]` block has to say. Takes
an archive path rather than a plan, so it can be run on a download before the
modlist mentions it. Nothing is written and the archive is only ever read from.
When MO2 (or mudcrab) downloaded the archive, the `.meta` beside it supplies
the `path`; otherwise the snippet leaves the Nexus ids as placeholders.
`add --archive` writes the same entry into a list directly.

```bash
mudcrab inspect <ARCHIVE_PATH> [--files] [--format text|json]
```

Example:

```bash
mudcrab inspect ~/Games/MO2/downloads/AWLS-19628-5-6-3.7z
```

Writing a mod's entry otherwise means downloading the archive, extracting it by
hand, opening `fomod/ModuleConfig.xml` in a text editor and transcribing step,
group and option names into TOML -- where a typo only surfaces at install time,
part way through a run. `inspect` prints the same names the installer will look
up, so they can be copied rather than retyped.

The report covers, as applicable:

- **Layout guess** -- FOMOD (has `fomod/ModuleConfig.xml`), BAIN (numbered
  top-level directories like `00 Core`, `01 Option`), a plain data folder that
  `install` finds on its own, or one nested somewhere that needs `data_folder`
  -- followed by the TOML snippet to paste.
- **FOMOD** -- every install step, its groups, each group's type
  (`SelectExactlyOne`, `SelectAny`, ...) and its options with their types.
  A `*` marks the option `install` picks for a group with no `fomod_selections`
  entry, and those defaults are pre-filled into the snippet. Steps behind a
  `<visible>` condition are shown either way and flagged, since whether they run
  depends on the rest of the list.
- **BAIN** -- the top-level subpackage directory names, for `bain_subpackages`.
  All of them are listed: `01a`/`01b` style subpackages are alternatives to each
  other and the ones you are not taking have to be deleted from the snippet.
- **Plugins** -- every `.esp`/`.esm` in the archive, since those go into the
  modlist's `plugins` load order by hand.

The default report is a summary: a 4000-file texture pack prints its layout and
a count, not 4000 lines. `--files` lists every path. `--format json` emits the
same findings structured, for scripting.

Only the FOMOD case unpacks anything, and then only `ModuleConfig.xml`; the
layout guess, subpackages, plugins and file listing all come from the archive's
entry headers.

## diff

Compare the mods mudcrab installed against a **reference** MO2 instance: the
same list built some other way -- by hand in MO2, say, or by Wabbajack -- that
you know to be right. `diff` calls that instance the *Oracle*, as in "the
answer to check against", in its options and its report. It is how a list is
verified while it is being written, a section at a time, rather than by playing
the result. Where `check` validates the archives about to be installed from,
`diff` validates what actually landed on disk.

```bash
mudcrab diff --mods-dir <OURS> --oracle <REFERENCE_MODS_DIR> [--plan <plan.json>]
             [--section <name>]... [--only <mod id>]... [--format text|json]
             [--details] [--fail-on-differences]
```

By default the terminal gets the totals and one line per section that is not
identical; `--details` prints every difference, mod by mod, and the full report
is in the run's log either way. Differences are the report, not an error, so the
exit code is 0 unless `--fail-on-differences` is given.

Example:

```bash
mudcrab diff --mods-dir ~/mo2/MyList/mods \
  --oracle ~/mo2/MyHandBuiltList/mods \
  --plan build/plan.json --section "OBSE PLUGINS"
```

For every mod folder in scope the report gives presence (ours only, Oracle only,
or both) and, for the ones in both, the files present on only one side and the
files whose contents differ.

Both trees come from Windows-authored archives, so paths are matched
case-insensitively and `\` is folded to `/`. Three things are deliberately not
differences: MO2's own `meta.ini` at a mod root (ours will never have one), a
`.mohidden` suffix (a plugin hidden for a merge is the same file), and MO2's
`_separator` folders (list furniture, not mods).

`--plan` supplies each mod's section and the archive it was meant to come from.
Without it every directory in either tree is compared and `--section` is refused,
because a mod folder on disk does not record which section it belongs to.
`--only` works either way, since it matches the folder name.

Mods are matched between the trees by folder name, so an id spelled differently
from the reference's folder would otherwise report as one missing mod plus one
extra, twice over, and bury the real differences. A mod may therefore declare
`oracle_name` -- the folder name to look for in the reference instance. Our
side always uses `id`; only the reference side follows `oracle_name`, and the
mod is still reported under our `id`, with the reference's folder named
alongside it. It is read by `diff` alone and has no effect on installation:

```toml
[[mods]]
id = "Cleaned DLC Masters"
oracle_name = "Clean ESM"  # ours is build-from-files
```

Version drift is reported separately, under `version notes`, when the list says
when the guide it follows was published (`[guide] published = "YYYY-MM-DD"`).
A guide that says "use the top file on the page" means the top file *then*, so
a reference archive stamped later than that is a file the guide never named.
The timestamp comes from the `installationFile=` recorded in the reference's
`meta.ini`, which usually ends in a Unix timestamp
(`Better Fort Aurus-50682-1-1-1647873144.7z` -> 2022-03-21); such mods are
flagged `POST-GUIDE`. Where no timestamp can be read the mod is listed under
`UNKNOWN AGE` rather than assumed to be fine. Neither flag affects the exit code
-- both describe the reference's own archive, not a fault in our copy of it. A
plan that names an archive the reference did not install *is* a difference.

Content is established lazily, because a reference instance can be 40GB: nothing is read for a
mod that exists on only one side, differing sizes settle a file without reading
a byte, equal sizes are compared chunk by chunk with an early exit, and digests
are computed only for the handful of files already known to differ, to name the
difference in the report. Mods are compared in parallel. Diffing a 704-mod, 40GB
instance against itself -- the worst case, where every byte on both sides has to
be read -- takes about 20 seconds.

## export

Render a modlist as a guide.

```bash
mudcrab export <modlist.toml> [--to json] [-o <file>] [--plan <plan.json>] [--downloads <dir>]
```

With `--downloads` (and `--archive-search-path`), export reads each archive's
file list, so instructions name real files -- "leave out
`OBSE/Plugins/x.dll`" rather than the pattern `**/x.dll` -- and a data folder
that Mod Organizer 2 unwraps by itself gets no instruction at all. Without it,
patterns are put into words instead.

The prose comes from the modlist's `docs` tables (see
[Docs: prose for export in The modlist format](modlist-format.md#docs-prose-for-export)); the rest --
sections, Nexus links, installer choices, INI edits, conditions, the load order
-- comes from the install data, put into words.

Without `--plan` the export describes the whole list: an optional mod appears
with its condition as a reader would say it ("only if you answer yes to
“Install Swearing Rats?”"). With `--plan` it describes one build: only the mods
that plan selected, and the answers that chose them.

`--to json` (the default) writes the document every exporter reads, to `-o` or
stdout. It is the contract for rendering a list with your own tools:
`document_version` changes only when a field changes meaning or disappears.

`--to nexus` writes BBCode for a Nexus mod page or article: paste it into the
editor's source (BBCode) mode. Nexus pages cannot hold files, so it links images
by their `url`, and warns about any that only have a local `path`.

`--to markdown` writes one GitHub-flavoured Markdown file. `--to github-pages`
writes a Jekyll site into the `-o` directory -- a landing page (the guide's
`summary` and the contents), the key, the setup (the guide's `body`, tools and
game settings), a page per top-level section, the closing text and the load
order -- with local images copied into `images/`; put it in a repository's
`docs/` folder and turn on GitHub Pages. Its default theme,
[Just the Docs](https://just-the-docs.com/), keeps every page in a sidebar and
adds a search box; `--opt theme=minima` gives a plainer site navigated from
the landing page.

To preview a site before publishing it, with Jekyll installed
(`gem install --user-install jekyll just-the-docs jekyll-relative-links jekyll-seo-tag`):

```bash
jekyll serve -s <site> --config <site>/_config.yml,<extra>.yml
```

where `<extra>.yml` stands in for what GitHub Pages does itself -- fetching the
theme, and turning links between pages (written as `.md`) into links that work:

```yaml
theme: just-the-docs
plugins: [jekyll-relative-links, jekyll-seo-tag, jekyll-include-cache]
relative_links:
  enabled: true
```

Exporters take options (`--opt key=value`, or `[export.<exporter>]` in the
modlist; `--list-options` shows them) and can be customised or replaced by your
own: see [Turning a list into a guide](exporters.md).

## game-dir

Find Oblivion's game folder: Steam's copy, in any Steam library, then any
`Oblivion.exe` under `~/Games`.

```bash
mudcrab game-dir [--plain]
```

On Linux each folder is also printed as MO2 sees it under Proton, on drive
`Z:` (`Z:\home\you\.local\share\Steam\steamapps\common\Oblivion`), ready
to paste into the file window MO2's first-run setup opens. `--plain` prints
only the first folder found, in that form.

## setup-tools

Set up `tools.toml`, the machine-local settings: the downloads folder and the
tools the list's actions need.

```bash
mudcrab setup-tools <modlist.toml> [--output <tools.toml>] [--force] [--defaults]
```

It asks for each value the list needs, offering what it finds where such
things usually are:

- **the downloads folder**: those of the MO2 instances under `~/Games`, and of
  the instances Steam runs;
- **the Wine/Proton prefix** to run xEdit in (Linux, for a list with `qac`
  actions): Steam's prefixes, each named after its Steam shortcut, those
  running MO2 first;
- **Proton**: the build that prefix was last run with, then the others Steam
  has;
- **xEdit**: `TES4Edit.exe` or `xEdit.exe` under `~/Games`, `~/Downloads` and
  the prefix. Its Quick Auto Clean executable is found beside it;
- **your Nexus API key**, kept as `nexus_api_key`. Enter keeps the one in
  `NEXUS_API_KEY`, if that is set; it is never written without being asked.
  The file is made readable by you alone.

Enter takes the first suggestion, a number another, `-` leaves a placeholder
to fill in later, and anything else is a path. With `--defaults`, or when
input is not a terminal, the first suggestion is taken for each.

The file goes where every command looks: `$XDG_CONFIG_HOME/mudcrab/tools.toml`,
else `~/.config/mudcrab/tools.toml` (`%APPDATA%\mudcrab\tools.toml` on
Windows); the command says which, and why. `--force` overwrites an existing
one.

To keep a second `tools.toml` -- for a test install, say -- name it with
`MUDCRAB_TOOLS_CONFIG`, which every mudcrab command then uses (setup-tools
included) while it is set, or with `--tools-config <file>`, which any command
takes for one run. Commands print which file they used when it is not the
usual one. Prefer these to changing `XDG_CONFIG_HOME`, which moves every
program's settings, your browser's included.

## Nexus Sources

Nexus downloads support:

1. Direct URL with `nexusmods.com`.
2. API descriptor format: `nexus:<game>/<mod_id>/<file_id>`.

For API descriptor sources, mudcrab needs your API key: `nexus_api_key` in
`tools.toml` (`setup-tools` asks for it), or the environment, which takes
precedence:

```bash
export NEXUS_API_KEY="your-api-key"
```

Only premium accounts are given download links; a free account's key still
names and recognises files (`add --nexus`, `identify`, `nexus-files`). A
refusal is not retried: asking again gets the same answer.

Optional API base override:

```bash
export NEXUS_API_BASE="https://api.nexusmods.com/v1"
```

## Example Source Modlist

```toml
name = "Simple"

[inputs.use_hd_textures]
type = "bool"
query = "Install HD textures?"

[[mods]]
id = "base"
dependencies = []

[[mods.archives]]
path = "https://example.com/base.zip"
download_handler = "http"

[[mods]]
id = "hd"
dependencies = ["base"]
if = "use_hd_textures"

[[mods.archives]]
path = "nexus:skyrimspecialedition/1234/5678"
file_name = "HD Textures-1234-1-0.7z"
download_handler = "nexus"
```

`path` says where to get an archive; `file_name` says what it is called. A nexus
descriptor carries no filename, so declaring `file_name` is what lets an install
find a copy without a `.meta` (a manual download, say), and it is the name a
download is saved under, in preference to whatever the server calls it.

To find the file id for a descriptor, list the mod's files:

```bash
mudcrab nexus-files 52661
```

Each current file is printed with its category, version and size, then the
`path` and `file_name` lines to paste into `[[mods.archives]]`. Old and
archived files are counted but hidden; `--all` lists them. Listing works for
any account, unlike downloading, which Nexus reserves for premium accounts.
