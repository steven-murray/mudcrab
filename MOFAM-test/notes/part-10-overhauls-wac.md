# Part 10 (Overhauls: WAC)

Five guide rows, five Oracle folders, five archives already on disk. The
cleanest section so far.

**Status: 5 mods, every file byte-for-byte identical. The 3 mods reported as
differing differ only by Part 36 merge-source plugins being hidden in the
Oracle — the known permanent difference, unchanged since Part 7.**

| difference | mods | why |
|---|---|---|
| plugin hidden in the Oracle | 3 | Part 36 merge sources, as in Parts 7-9 |

`WACIntegration - MOO Patch.esp`, `WAC - HGEC Equipment Replacer.esp` and
`WAC Integration HGEC Gauntlets Patch.esp`. All three appear in
`mofam.merges.toml`.

## Two latent bugs this section surfaced

### `layout = "simple"` was declared but never honoured

`ArchiveLayout::Simple` has existed in the schema since the beginning —
*"archive root is the mod's data folder; copied as-is"* — and `extract_archive`
never checked for it. It matched `Fomod`, then `Bain`, then tested for
`data_folder`/`target_subdir`, and otherwise fell through to auto-detection. So
declaring `simple` did nothing at all, and two entries in `mofam.minimal.toml`
have been silently taking the detected path.

Inert would have been survivable. What made it a bug is that *declaring the
answer still got you the guess*, and for WAC the guess is a hard failure: the
archive holds 18 plugins, two of them under `WAC_Natural_Habitat_by_Max_Tael/`,
which matches none of the four roots detection knows, so the whole archive is
rejected. The two files the guide actually wants are sitting at the root.

Now honoured, with a test that asserts both halves — `simple` extracts, and the
same archive with no declared layout still errors. Without the second assertion
the test would have passed before the fix.

### A false POST-GUIDE flag on any non-Nexus mod

`diff` flagged WAC as *"the Oracle's archive is newer than the March 2025
guide"*, dated 2026-01-25. It is a v1 beta from around 2010.

WAC is hosted on TES Alliance, so `modid=0`. MO2 writes `nexusLastModified`
anyway, and for a mod that never came from Nexus that value is simply when the
entry was written here — the archive's own mtime is 2026-01-24, the day it was
downloaded. `classify_guide_age` now ignores `nexusLastModified` when
`modid=0` and reports UNKNOWN AGE with the reason.

This is the **second** time this exact value has produced a confident wrong
answer; Part 7 restricted the fallback to mods that have an `installationFile`,
and WAC has one. The pattern worth remembering: `nexusLastModified` is only
evidence about a file when the file came from Nexus. A filename timestamp still
counts whoever hosted it, so that path is unchanged.

## `manual:` — the list's first archive nothing can fetch

Row 1 is the first non-Nexus archive in the whole modlist. Every prior archive
resolves as `nexus:oblivion/<mod>/<file>`.

`download` already resolved cache → search paths → network, so the file was
found; the question was what to write in `path`, which is mandatory. An absolute
local path works but bakes this machine into a list that is otherwise
host-agnostic. `manual:WACv_1beta.7z` says what is true — *no automated source
exists* — resolves from `--archive-search-path` like anything else, and when the
archive is absent fails with a message naming the file and the directories
searched, without retrying three times for a file no retry can produce.

`mudcrab add` now suggests exactly this in the TODO it writes for a `modid=0`
mod, rather than the vaguer advice it used to give.

## The BSA rename is the whole mod

> *"Once installed, rename the WAC BSA file to WACIntegration. This ensures the
> handshake with the following mod's plugin."*

Oblivion loads a BSA by matching its stem to a plugin's. WAC ships `WAC.bsa`
with no `WAC.esp` in this build — row 2's `WACIntegration.esp` is the plugin
that survives — so without the rename the game loads none of WAC's 7627 assets
and the whole section is inert. `file_move` covers it; no new action was needed,
which retires the plan's backlog entry *"rename after extraction — Part 10 #1"*.

## The guide's subpackage name is not the subpackage's name

Row 2 asks for *"01 Maskar's Oblivion Overhaul INI Files"*. The archive calls it
`01 Maskar's Oblivion Overhaul patch and INI files`, and it ships a **plugin**
as well as the two INIs — content the guide's shortened name hides. Same trap as
Part 9's `Colourful`/`Colorful`, and the reason `inspect` is run on every BAIN
archive before its selections are written down.

## Re-checking Part 9's conflict list — the point of doing this early

WAC is what makes Part 9's conflict files visible, so this section was the first
chance to check that list against the mod it names. It did not survive contact.

The recorded list claimed 1024 paths, held 247, and described textures only. The
true figure is **1738**, of which 701 are meshes. All three errors and the
corrected accounting are written up in `part-09-overhauls-oscuro.md`.

The valuable half is that the WAC portion could be **derived** rather than
looked up. Intersecting OOO Enhanced's 8427 files with WAC's BSA contents yields
**577 files** — exactly the set the Oracle removed under "Winning File conflicts
→ Overwritten mods". The three clothing mods account for **1148** more, and the
residue is 13 `thumbs.db`.

That is the `conflicts_with` algorithm from `conflict-resolution-design.md`, run
by hand, against a mod whose assets live entirely inside a BSA — the case the
design said would be the hard one. It works.

## Numbers

| mod | files | layout |
|---|---|---|
| WAC Waalx Animals & Creatures | 2 (+7627 in the BSA) | `simple` + `include` + `file_move` |
| WAC - Integration | 4 | BAIN, 2 of 4 subpackages |
| HGEC Equipment Replacer for WAC | 212 | BAIN, `00 Data` only |
| WAC - Integration - Roberts Conversion | 94 | nested data folder |
| WAC - Integration - HGEC Gauntlets Conversion | 26 | nested data folder |
