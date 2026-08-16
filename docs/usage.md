# mudcrab Usage Guide

This guide describes the currently implemented command flow.

## Pipeline

1. Compile source TOML into compiled JSON.
2. Resolve query inputs into a personalized plan JSON.
3. Download archives into a local cache.
4. Install staged mod archive layout from cache, then build any declared merges.
5. Diff the result against a reference instance to verify the section.

`inspect` sits before all of this: it reads an archive and prints the modlist
entry it wants, so step 1 can be written without extracting anything by hand.

## Commands

## add

Add one `[[mods]]` block to a source modlist, in place.

```bash
mudcrab add <modlist.toml> --from-oracle <ORACLE_MODS_DIR> --mod "<mod folder>" [--id "<mod id>"] [--section "<name>"] [--file-name "<archive.7z>"] [--dry-run]
mudcrab add <modlist.toml> --nexus <modid>/<fileid> --id "<mod id>" --section "<name>" [--file-name "<archive.7z>"] [--dry-run]
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
mudcrab download <plan.json> [--cache <dir>] [--retry <n>] [--parallel <n>]
                 [--section <name>]... [--only <mod id>]...
                 [--archive-search-path <dir>]...
```

Example:

```bash
mudcrab download build/plan.json --cache .mudcrab-cache --retry 3
```

Note: `--parallel` is accepted but currently downloads sequentially.

A failed source does not stop the run. Every archive is attempted, the failures
are reported together at the end and the exit code is non-zero -- the same shape
as `check`. A section routinely contains more than one dead Nexus link, and
aborting on the first one turned finding them into one round trip each.

## Using archives you already have

Most of a large modlist is usually already on the machine, in an MO2 or
Wabbajack downloads folder from an earlier build. `--archive-search-path` points
`download`, `check` and `install` at those folders so the archives are reused
instead of fetched again.

```bash
mudcrab install build/plan.json --cache .mudcrab-cache \
  --mo2-instance-dir ~/mo2/MOFAM \
  --archive-search-path ~/Games/mod-organizer-2-oblivion/modorganizer2/downloads \
  --archive-search-path ~/Games/Wabbajack/Oblivion/MudCrab/downloads
```

Each archive is resolved in this order:

1. Already in the cache -- used as is.
2. Otherwise, if the archive declares `file_name`, each search path is scanned
   for that exact filename, case-insensitively, in the order given. The first
   hit is hard-linked into the cache (falling back to a copy across
   filesystems) and recorded there under its cache name, so no download runs.
3. Otherwise the archive is downloaded as usual.

Search paths are read-only sources: nothing is ever written, renamed or deleted
inside them. An archive with no `file_name` has nothing to match against, so it
always takes the download path.

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
mudcrab download build/plan.json --cache .mudcrab-cache --section "5 - LOD"
mudcrab install  build/plan.json --cache .mudcrab-cache \
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

Install from a personalized plan and cache into a mods directory.

```bash
mudcrab install <plan.json> --cache <dir> --mods-dir <mods_dir> [--dry-run] [--skip-actions]
                [--force-merges]
                [--section <name>]... [--only <mod id>]...
                [--archive-search-path <dir>]...
```

Example:

```bash
mudcrab install build/plan.json --cache .mudcrab-cache --mods-dir build/mods
```

Current behavior:

1. Verifies required cached archives exist.
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
mudcrab merge MOFAM-test/input/mofam.merges.toml \
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

Validate cached archives and archive-backed file references without installing.

```bash
mudcrab check <plan.json> [--cache <dir>] [--section <name>]... [--only <mod id>]...
              [--archive-search-path <dir>]...
```

Example:

```bash
mudcrab check build/plan.json --cache .mudcrab-cache \
  --archive-search-path ~/Games/Wabbajack/Oblivion/MudCrab/downloads
```

Every archive is reported as one of `cached`, `resolvable locally` (present in a
search path) or `MUST BE DOWNLOADED`, followed by a summary count. The summary is
printed even when the check fails, since a failing run is the one whose report
matters -- it is how a dead link is found before a later section depends on it.
`check` only reports: it never adopts a locally resolvable archive into the
cache.

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
```

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
find a copy already sitting in an `--archive-search-path`. It is also the name
the archive is exported under into MO2's `downloads/`, in preference to whatever
the server happened to call it.
