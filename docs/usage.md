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
mudcrab add <modlist.toml> --from-oracle <ORACLE_MODS_DIR> --mod "<mod folder>" [--id "<mod id>"] [--section "<name>"] [--file-name "<archive.7z>"] [--dry-run]
mudcrab add <modlist.toml> --nexus <modid>/<fileid> --id "<mod id>" --section "<name>" [--file-name "<archive.7z>"] [--plugin <FILE>]... [--if <CONDITION>] [--dry-run]
```

The edit is a line-based splice, not a parse-and-rewrite: every byte of the file
outside the inserted block -- comments, blank lines, tab indentation, key order --
is preserved exactly. The block goes after the last existing mod in `--section`,
so the file stays grouped; if that section has no mods yet, it is appended at the
end of the file. Repeat `--section` for a nested section path, outermost first.

`--from-oracle` reads `<ORACLE_MODS_DIR>/<mod folder>/meta.ini` as written by Mod
Organizer 2 and takes `modid` + `[installedFiles] 1\fileid` for the
`nexus:oblivion/<modid>/<fileid>` path, `installationFile` for `file_name`, and
`version` for a trailing `# oracle version ...` comment. The mod id defaults to
the folder name. The directory is only ever read from.

A folder with `modid=0` was not installed from Nexus: the block is written with
`file_name` but no `path`, marked with a `# TODO: non-Nexus source` comment, and a
warning goes to stderr. No URL is invented.

If the Oracle folder ships `.esp`/`.esm` files, they are listed on stderr. They
are **not** added to the top-level `plugins` array and the block declares no
`plugins` key -- putting a plugin at the wrong point in load order fails silently
in game, so the position is yours to choose.

`--plugin <FILE>` (repeatable) writes the block's `plugins` key, and `--if
<CONDITION>` its `if`, for an optional mod. Neither touches the load order: the
plugin still has to be placed in `plugins`, and the input declared, before the
result validates -- which `add` checks, restoring the file if it does not.

Safety:

- an id that already exists in the file is refused;
- the file is written via a temp file next to it, then renamed;
- after writing, the modlist is re-parsed and validated, and the original is
  restored if either fails;
- `--dry-run` prints the block and its destination and changes nothing.

Example:

```bash
mudcrab add mofam.toml \
  --from-oracle ~/Games/Wabbajack/Oblivion/MOFAM-03.25/mods \
  --mod "Blockhead" --section "OBSE PLUGINS" --dry-run
```

## move

Move a mod's block to another section, or before another mod, keeping its
comments, archives and actions; only its `section` line changes.

```bash
mudcrab move <modlist.toml> --mod "<mod id>" [--section "<name>"]... [--before-mod "<mod id>"] [--dry-run]
```

As with `add`, the file is validated after the move and restored if it no
longer validates.

## compile

Validate and compile a source modlist.

```bash
mudcrab compile <modlist.toml> --output <compiled.json> [--strict] [--offline]
```

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

An archive is found by what it is, never by which mod uses it:

- a Nexus archive by the `modID`/`fileID` in its `.meta` (so MO2's `(1)`
  suffixes and hand renames do not matter);
- anything else by its `file_name`, case-insensitively, or for a URL source by
  the `url` in its `.meta`.

So renaming a mod, reordering its archives or using one archive in several mods
changes nothing on disk. A download mudcrab makes is saved under the server's
name with a `.meta` written as MO2 would.

## Using archives you already have

`--archive-search-path` points `download`, `check` and `install` at other
folders that may hold archives -- another instance's downloads, a Wabbajack
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
mudcrab install build/plan.json --downloads ~/Games/OblivionModDownloads \
  --mo2-instance-dir ~/mo2/MOFAM \
  --archive-search-path ~/Games/Wabbajack/Oblivion/MudCrab/downloads
```

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

Install from a personalized plan and the downloads folder into a mods directory.

```bash
mudcrab install <plan.json> [--downloads <dir>] --mods-dir <mods_dir> [--dry-run] [--skip-actions]
                [--force-merges]
                [--section <name>]... [--only <mod id>]...
                [--archive-search-path <dir>]...
```

Example:

```bash
mudcrab install build/plan.json --downloads ~/Games/OblivionModDownloads --mods-dir build/mods
```

Current behavior:

1. Finds every archive in the downloads folder (or a search path).
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

Validate downloaded archives and archive-backed file references without installing.

```bash
mudcrab check <plan.json> [--downloads <dir>] [--section <name>]... [--only <mod id>]...
              [--archive-search-path <dir>]...
```

Example:

```bash
mudcrab check build/plan.json --downloads ~/Games/OblivionModDownloads \
  --archive-search-path ~/Games/Wabbajack/Oblivion/MudCrab/downloads
```

Every archive is reported as one of `cached`, `resolvable locally` (present in a
search path) or `MUST BE DOWNLOADED`, followed by a summary count. The summary is
printed even when the check fails, since a failing run is the one whose report
matters -- it is how a dead link is found before a later section depends on it.
`check` only reports: it never links a search-path archive into the downloads
folder.

## inspect

Read an archive and print what its `[[mods.archives]]` block has to say. Takes
an archive path rather than a plan, so it can be run on a download before the
modlist mentions it. Nothing is written and the archive is only ever read from.

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

Compare the mods directory we produced against a reference ("Oracle") MO2
instance, so each section can be verified as it is built rather than at the end.
Where `check` validates the archives we are about to install from, `diff`
validates what actually landed on disk.

```bash
mudcrab diff --mods-dir <OURS> --oracle <ORACLE_MODS_DIR> [--plan <plan.json>]
             [--section <name>]... [--only <mod id>]... [--format text|json]
             [--details] [--fail-on-differences]
```

By default the terminal gets the totals and one line per section that is not
identical; `--details` prints every difference, mod by mod, and the full report
is in the run's log either way. Differences are the report, not an error, so the
exit code is 0 unless `--fail-on-differences` is given.

Example:

```bash
mudcrab diff --mods-dir ~/Games/Wabbajack/Oblivion/MudCrab/mods \
  --oracle ~/Games/Wabbajack/Oblivion/MOFAM-03.25/mods \
  --plan build/plan.json --section "OBSE PLUGINS"
```

For every mod folder in scope the report gives presence (ours only, Oracle only,
or both) and, for the ones in both, the files present on only one side and the
files whose contents differ. The exit code is non-zero when anything differs, so
a section can be gated on it.

Both trees come from Windows-authored archives, so paths are matched
case-insensitively and `\` is folded to `/`. Three things are deliberately not
differences: MO2's own `meta.ini` at a mod root (ours will never have one), a
`.mohidden` suffix (a plugin hidden for a merge is the same file), and MO2's
`_separator` folders (list furniture, not mods).

`--plan` supplies each mod's section and the archive it was meant to come from.
Without it every directory in either tree is compared and `--section` is refused,
because a mod folder on disk does not record which section it belongs to.
`--only` works either way, since it matches the folder name.

Mods are matched between the trees by folder name, so an id we deliberately
spell differently from the Oracle's folder would otherwise report as one missing
mod plus one extra, twice over, and bury the real differences. A mod may
therefore declare `oracle_name` -- the folder name to look for in the reference
instance. Our side always uses `id`; only the Oracle side follows `oracle_name`,
and the mod is still reported under our `id`, with the Oracle's folder named
alongside it. It is read by `diff` alone and has no effect on installation:

```toml
[[mods]]
id = "Cleaned DLC Masters"
oracle_name = "Clean ESM"  # ours is build-from-files
```

Version drift is reported separately, under `version notes`. The MOFAM guide was
published in March 2025 and often says only "use the top file on the page", so an
Oracle archive stamped later than that is a file the guide never named. The
timestamp comes from the `installationFile=` recorded in the Oracle's `meta.ini`,
which usually ends in a Unix timestamp (`Better Fort Aurus-50682-1-1-1647873144.7z`
-> 2022-03-21); such mods are flagged `POST-GUIDE`. Where no timestamp can be
read the mod is listed under `UNKNOWN AGE` rather than assumed to be fine.
Neither flag affects the exit code -- both describe the reference's own archive,
not a fault in our copy of it. A plan that names an archive the Oracle did not
install *is* a difference, and does gate the run.

Content is established lazily, because the Oracle is 40GB: nothing is read for a
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
[modlist-format.md](modlist-format.md#docs-prose-for-export)); the rest --
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
writes a Jekyll site into the `-o` directory -- an index, a page per top-level
section, the load order -- with local images copied into `images/`; put it in a
repository's `docs/` folder and turn on GitHub Pages.

To preview a site before publishing it, with Jekyll installed
(`gem install --user-install jekyll minima jekyll-relative-links`):

```bash
jekyll serve -s <site> --config <site>/_config.yml,<extra>.yml
```

where `<extra>.yml` turns on what GitHub Pages enables by default:
`plugins: [jekyll-relative-links]` (links between pages are written as `.md`).

Exporters take options (`--opt key=value`, or `[export.<exporter>]` in the
modlist; `--list-options` shows them) and can be customised or replaced by your
own: see [exporters.md](exporters.md).

## setup-tools

Scan a source modlist and generate a `tools.toml` configuration template for the
machine-local tools (e.g. LOOT, xEdit) that the modlist's actions require.

```bash
mudcrab setup-tools <modlist.toml> [--output <tools.toml>] [--force]
```

`--output` defaults to `tools.toml` in the same directory as the input file.
`--force` overwrites an existing `tools.toml`.

Example:

```bash
mudcrab setup-tools path/to/modlist.toml --output tools.toml
```

## Nexus Sources

Nexus downloads support:

1. Direct URL with `nexusmods.com`.
2. API descriptor format: `nexus:<game>/<mod_id>/<file_id>`.

For API descriptor sources, export a key:

```bash
export NEXUS_API_KEY="your-api-key"
```

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
