# Part 35 — Oscuro's Patches

12 rows, 12 mods. Diff: 3 identical, 9 differing — and all nine differ in exactly
one way, their plugin being hidden in the Oracle because it is an **OOO Patches
Merged** source (Part 36 row 2). Every file is otherwise byte-identical.

The three that are not merge sources — Improved Chests, `Cobl Tweaks - OOO.esp`
and the Bounty Quests OOO patch — are unhidden in both instances, which is the
right cross-check that the hiding is the merge and not something else.

## A bug this section found in mudcrab

Row 7 says "Install manually & deselect everything except Bounty Quests OOO
Patch.esp". The archive holds a top-level `Data/` folder, so the first attempt
was `include = ["Data/Bounty Quests OOO Patch.esp"]`.

That matched nothing — auto-detection strips the `Data/` wrapper *before* filters
run, so the path the filter sees is `Bounty Quests OOO Patch.esp`. The install
**reported success and created an empty mod folder.**

That is the failure mode worth fixing, not the pattern. An `include` that matches
no entry is always a mistake, and every version of mudcrab up to now rendered it
as a healthy-looking install; only a diff notices, and only if someone reads it.
An archive that contributes no files is now an error naming the archive and the
patterns, with the one legitimate exception — an archive whose whole content is
`game_root_files` and so belongs outside the mod folder.

A scan of all 730 installed folders confirms no other mod in the list was
silently empty; the only empty folders are MO2's separators, which mudcrab
generates rather than extracts.

## Selections

| Row | Guide | Encoded as |
|---|---|---|
| 1 | "Within the BAIN Installer, select just Improved Chests (OOO Compatible)" | `data_folder` — the download is **not** a BAIN package. It holds two plain folders, the wanted one and `OLDChests`. The guide calls it a BAIN installer; it is not one. |
| 5 | "select just 01 OOO Patch" | `bain_subpackages = ["01 OOO Patch"]` — a real BAIN package |
| 7 | "deselect everything except Bounty Quests OOO Patch.esp" | `include`, see above |
| 9 | "right click … > Oscuro's Oblivion Overhaul Patch and Set as \ directory" | `data_folder = "Local Guards Features/Oscuro's Oblivion Overhaul Patch"` — the same archive Part 27 installs, a different folder out of it |

Row 9 also asks for the mod to be renamed `Local Guards Features - OOO Patch`,
which the Oracle did; the name is used directly, no `oracle_name` needed.

## Two filenames with a doubled space

`Weapons Of Morrowind - OOO  (Extended) Patch.esp` — two spaces before the
bracket, as the archive spells it. So does Part 34's `Unique Landscapes Separate
 - OOO Adaptation.esp`. Both had to be copied exactly into the load order; a
single space names a plugin that does not exist.

## Version drift

`Various OOO Adaptations (Arthmoor mods MOFAM Edit)` is dated 2025-03-30, on the
guide author's own mod page — the same twelve-day-later pattern as Part 33's two
guide-page downloads. Not drift.
