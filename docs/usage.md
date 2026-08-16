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
```

Example:

```bash
mudcrab download build/plan.json --cache .mudcrab-cache --retry 3
```

Note: `--parallel` is accepted but currently downloads sequentially.

## validate

Validate a source modlist without generating compiled output.

```bash
mudcrab validate <modlist.toml> [--strict]
```

## install

Install from a personalized plan and cache into a mods directory.

```bash
mudcrab install <plan.json> --cache <dir> --mods-dir <mods_dir> [--dry-run] [--skip-actions]
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
mudcrab check <plan.json> [--cache <dir>]
```

Example:

```bash
mudcrab check build/plan.json --cache .mudcrab-cache
```

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
download_handler = "nexus"
```
