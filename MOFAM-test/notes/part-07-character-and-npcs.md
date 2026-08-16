# Part 7 (Character & NPCs)

The guide's 33 numbered rows are 38 mod folders: seven rows say "install
separately" and become two or three entries each. Built 2026-08-16 with
`./MOFAM-test/scripts/run-full.sh --section "7 - CHARACTER AND NPCS"`.

**Result: 38 compared, 26 identical, 12 differing — all 12 explained below.
Nothing missing, nothing extra. No defects on our side: ten differences clear
at Part 36, one is a known Oracle error, one is a readme.**

Ten of the twelve are one expected thing repeated. The real content is the
other two.

## 1. Ten plugins hidden in the Oracle, active here (expected)

Each of these mods is identical except that its plugin is `.mohidden` in the
Oracle and live in ours:

| mod | plugin |
|---|---|
| Oblivion Character Overhaul - Advanced Edition | `Oblivion_Character_Overhaul_Faces.esp` |
| NPC Hair Matches Beard - Updated | `NPC Hair Matches Beard.esp` |
| OCOv2 Uses Merged Teeth | `OCO uses merged teeth.esp` |
| Claws whiskers and Seamless tails only | `OCOv2 Beast Races Enhanced.esp` |
| Improved NPC Faces for OCOv2 | `Improved NPC Faces for OCOv2.esp` |
| Oblivion Character Overhaul v2 - DLC Addon | `OCOv2 - DLC Addon.esp` |
| Unused OCOv2 Eyes and DLC Characters Incorporated | `OCO Unused Eyes...esp` |
| OCOv2 Baurus tweak | `Baurus tweak.esp` |
| Sirens Deception Beautified | `Siren's Deception Beautified.esp` |
| Oblivion Character Overhaul Version 2 Patches | `DispMiscPatch_OCOv2 - Adoring Fan No Beard.esp` |

**Every one is a Part 36 merge source.** The Oracle hides them because their
merged replacement exists there; ours has no merge yet, so they must stay
active or the mods do nothing. Part 36 hides them and removes them from the
top-level `plugins` array, and these ten differences disappear.

That correspondence is exact and worth stating as a check: of the thirteen
plugins Part 7 hides in the Oracle, **exactly three are not merge sources**, and
those three are precisely the three the guide gives an explicit instruction for
(`EVE_ShiveringIslesEasterEggs.esp`, `EVE_StockEquipmentReplacer.esp`,
`DispMiscPatch_OCOv2 - VKVII Argonian and Khajiit Patch.esp`). Guide and Oracle
agree completely on which plugins Part 7 itself should hide.

## 2. A known Oracle error — `OCOv2 Enhanced Beast Races patch`

**Resolved 2026-08-16: the guide is right and the Oracle is wrong.** Confirmed
by the user, who built the Oracle: the standard Khajiit head was removed instead
of the Nuska variant.

Guide row 21 says to remove, among other things:

> Textures > Characters > Nuska > Khajiit > headkhajiit (x2 files)

The Oracle instead hid `textures/characters/khajiit/headkhajiit.dds` and
`headkhajiit_n.dds`, leaving the `nuska/khajiit` pair visible. Both pairs exist
in the archive. The tell was already in the guide: it lists the neighbouring
`khajiit/earkhajiit.dds (x2)` separately and *without* the Nuska prefix, so it
distinguishes the two folders deliberately.

**This build is correct. These four lines in `diff` are permanent** until the
Oracle is repaired, and are not a defect on our side:

```
textures/characters/khajiit/headkhajiit.dds         (hidden in the Oracle)
textures/characters/khajiit/headkhajiit_n.dds       (hidden in the Oracle)
textures/characters/nuska/khajiit/headkhajiit.dds   (hidden in ours)
textures/characters/nuska/khajiit/headkhajiit_n.dds (hidden in ours)
```

To repair the Oracle by hand in MO2: un-hide
`textures/characters/khajiit/headkhajiit.dds` and `headkhajiit_n.dds`, and hide
`textures/characters/nuska/khajiit/headkhajiit.dds` and `headkhajiit_n.dds`.
Doing so makes the row match exactly.

This is the first case where following the guide over the Oracle was tested and
came out right. It is the reason for the practice, and it is worth remembering
that the reading which found it was noticing the guide's own internal
consistency, not anything in the installed files.

## 3. `Oblivion Character Overhaul version 2` — a readme

The archive is `Data/` plus one top-level file,
`Oblivion Character Overhaul Readme.txt`. Setting `Data` as the root discards
its siblings, so we do not install the readme; the Oracle has it. A text file
with no game effect, and the alternative is a layout override that exists only
to carry a readme. Left as is.

(This also corrected an arithmetic coincidence: archive 5484 files vs Oracle
5483 looked like "exactly the one excluded plugin", but it is the readme in and
the plugin out, netting the same count. The diff caught what the counting did
not.)

## Rows that needed more than an archive reference

| # | mod | what it needed |
|---|---|---|
| 1a | Oblivion Character Overhaul version 2 | `exclude` the plugin; 1b ships the copy used |
| 2 | AI Enhanced | `data_folder` (archive top folder is misspelled "Enahced"), `file_hide` of `textures/characters/nuska/hair` |
| 4 | Light compatible Skeleton | `target_subdir = "meshes/characters"` — the whole "create two folders and drag" instruction |
| 5 | Seamless - OCOv2 | `file_hide` of two EVE plugins and `meshes/characters/argonian` |
| 18 | Warpaints ×2 patches | `data_folder` — `Data/` nested inside a folder not named after the mod |
| 19 | Reposition Teeth | BAIN, `001 core` only |
| 21 | Enhanced Beast Races patch | `data_folder` two levels down, `exclude` the plugin, `file_hide` ×9 |
| 22 | Claws whiskers | `data_folder`, `exclude` the female Argonian tail |
| 30 | Patch Collection | one folder of sixteen, `file_hide` of one plugin |

**[MI] mostly needs nothing.** The guide's "manual install" marker means the
archive wraps its content in `Data/`, which the auto layout already unwraps.
Only where `Data/` is nested *deeper* (rows 18, 21) or under a differently named
folder does the entry need `data_folder`.

## Two tooling gaps this section closed

- **`file_hide`.** The guide says "hide or delete" constantly, and the Oracle
  hides — MO2 renames the file, or the whole directory, to `<name>.mohidden`.
  There was no way to express that. `file_prune` is now specifically for the
  cases where the instruction is to delete.
- **`diff` was blind to hiding.** It strips `.mohidden` so a hidden plugin
  matches its unhidden twin, which is right for "are these the same files" and
  wrong for "does the game see the same files". It stripped only the *filename*,
  too, so MO2's folder-level hide slipped past entirely. Both fixed: the suffix
  comes off every path segment, and a file hidden on one side only is now its
  own reported difference.

  Without that second fix the guide/Oracle conflict above would have shown as
  zero differences — the section would have reported 37 of 38 identical and been
  wrong about it.

## Section naming

This section is filed as `7 - CHARACTER AND NPCS`, matching the Oracle's MO2
separator, as Part 5 does. Parts 1-4 and 6 were authored earlier with bare names
(`TWEAKS AND FIXES`). Cosmetic and inconsistent; worth one pass to normalise,
not worth churning 105 rows mid-build.
