# mudcrab Usage Guide

This guide describes the currently implemented command flow.

## Pipeline

1. Compile source TOML into compiled JSON.
2. Resolve query inputs into a personalized plan JSON.
3. Download archives into a local cache.
4. Install staged mod archive layout from cache, then build any declared merges.

## Commands

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

A large list is built a section at a time, so `download`, `check` and `install`
all accept the same two flags:

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
