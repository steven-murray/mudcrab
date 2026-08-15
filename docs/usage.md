# mudcrab Usage Guide

This guide describes the currently implemented command flow.

## Pipeline

1. Compile source TOML into compiled JSON.
2. Resolve query inputs into a personalized plan JSON.
3. Download archives into a local cache.
4. Install staged mod archive layout from cache.

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
5. Runs post-install actions (e.g. `loot-sort`, per-mod actions declared in the
   modlist) by default; pass `--skip-actions` to skip them.

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
