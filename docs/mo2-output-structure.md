# ModOrganizer2 Output Structure

This document defines the first launcher-specific export target for mudcrab.

## Goals

- Keep mudcrab's internal install pipeline launcher-agnostic.
- Add a first concrete export target for ModOrganizer2.
- Preserve enough metadata during compile/query/install to support future launcher exports.

## Target Directory Layout

The initial ModOrganizer2-compatible output structure is:

```text
<instance>/
  mods/
    Mod-1/
      extracted_files...
    Mod-2/
      extracted_files...
  downloads/
    archive-1.zip
    archive-2.zip
  profiles/
    <profile-name>/
      modlist.txt
      loadorder.txt
      archives.txt
      oblivion.ini
      plugins.txt
```

## File Semantics

### mods/

- Contains one directory per installed mod.
- Mod directory names are the mod IDs/names from the source TOML.
- Mod order is significant and must match source TOML order.

### downloads/

- Contains downloaded mod archives.
- Initial implementation may copy cached archives into this directory.
- This is launcher-facing output, distinct from mudcrab's internal cache.

### profiles/<profile-name>/modlist.txt

- One mod per line.
- Prefix with `+` if the mod is enabled for the personalized plan.
- Prefix with `-` if the mod is present in the source order but not selected.
- Order must match the source TOML mod order exactly.

Example:

```text
+Core Fixes
-Optional HD Pack
+UI Overhaul
```

### profiles/<profile-name>/loadorder.txt

- One plugin per line.
- Order is the explicit plugin load order declared in the source TOML.
- This does not need to match mod order.

### profiles/<profile-name>/plugins.txt

- For mudcrab's initial MO2 export, this is identical to `loadorder.txt`.
- Future launcher targets may diverge here.

### profiles/<profile-name>/archives.txt

- One `.bsa` archive per line.
- Order follows mod order, not plugin order.
- Since remote archive contents are not available at compile time, the initial implementation detects `.bsa` files after install by scanning installed mod directories.

### profiles/<profile-name>/oblivion.ini

- This is copied from the game directory.
- mudcrab must never modify the original `Oblivion.ini` in the game folder.
- Any game-scoped INI edits are applied to the copied profile-local file only.

## Compile Responsibilities

- Preserve source mod order.
- Preserve explicit plugin load order from source TOML.
- Validate that every declared mod plugin is present in the global load order.
- Validate plugin filename shape (`.esp` or `.esm`).

## Install Responsibilities

- Install extracted mod contents into `mods/`.
- Export cached archives into `downloads/`.
- Generate MO2 profile files under `profiles/<profile-name>/`.
- Copy `Oblivion.ini` from the game directory into the profile.
- Apply game-scoped INI edits to the copied profile-local `oblivion.ini`.
- Detect installed `.bsa` files and write `archives.txt` in mod order.

## Deferred Generalization

- The internal mudcrab model should remain generic enough to support future launcher/export targets.
- ModOrganizer2 is the first concrete export format, not the final internal abstraction.