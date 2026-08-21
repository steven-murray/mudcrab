# Part 32 — Common Oblivion (COBL)

8 rows, 8 mods, 2737 files. Diff: **6 of 8 identical**; the two that differ do so
only because their plugin is hidden in the Oracle by a Part 36 merge.

## The wizard answers in row 1 cancel themselves

Row 1 gives BAIN Wizard answers — Stable / Tweaks / `Cobl Tweaks - SI` — and then
says "once installed, delete every plugin except" four names. Reading the
archive's own `Wizard.txt` settles what the first half does:

- `SelectOne "Stable or Development"` → `Case "Stable"` selects `01 StableCore`
  (`Cobl Main.esm`, `Cobl Glue.esp`, `Cobl Si.esp`). `00 Cobl Core` is selected
  unconditionally before that.
- `SelectMany` → `Tweaks` selects `02 Tweaks (Only install one)`, and the
  `Cobl Tweaks - SI` case then `DeSelectEspm`s all of that subpackage bar
  `Cobl Tweaks - SI.esp`.

`Cobl Tweaks - SI.esp` is *not* one of the four names the row keeps, so the
second half of the row deletes the only thing the Tweaks answer contributed. The
end state is `00 Cobl Core` + `01 StableCore` and nothing else. That is what the
entry declares, and it reproduces the Oracle's file set exactly — 2266 files,
byte-identical.

The tweaks plugin the Oracle *does* end up with, `Cobl Tweaks - OOO.esp`, comes
from Part 35 row 3 (`COBL Tweaks - MOFAM Patch`), not from here.

## Row 3: which of two near-identical folders

The Complete Bundle ships 34 mini-mods including both `KMM High-Res Welkynd
Textures…` and `KMM Higher-Res Welkynd Textures…`. The guide names Higher-Res.
Confirmed independently against the Oracle's three files (349672 / 349672 /
699216 bytes = Higher-Res; High-Res is 87528 / 87528 / 174928), so the guide and
the Oracle agree and a guide-only user would also get it right from the name.

## Row 8: the guide asks for a rename the Oracle did not do

Row 8 says to rename the mod to `Legacy of the Champion (Cobl Porridge DDS)`.
The Oracle's folder is plain `Legacy of the Champion`. Following the guide, the
mod id carries the rename and `oracle_name` maps it back for `diff`. Cosmetic —
MO2 folder names carry no load-order meaning.

## The two expected differences

| Mod | Plugin | Hidden in the Oracle because |
|---|---|---|
| Cobl Unofficial Patch | `Cobl Glue - Bravil Barrel Fix.esp` | Late Loaders merge source (Part 36 row 8) |
| Wrye Bash Collection of Mergeable Mods - Pekkas COBL Books Jackets | `PekCOBLBookJackets.esp` | Prebash merge source (Part 36 row 6) |

Both stay active here until Part 36 builds those merges, as with every earlier
section's merge sources.

## Deactivated-but-present in the Oracle

`Cobl Filter Late MERGE ONLY.esp` and
`Cobl - Buffet Plate and Hibernation Potion Disabler.esp` are in the Oracle's
`loadorder.txt` (lines 221 and 222) but absent from its `plugins.txt` — installed
and unticked. The guide's row 1 explicitly keeps the first, and row 4 says
nothing about disabling the second. Both are active here. This is a
plugins.txt-level distinction that `diff` does not compare and the merge/bashed
patch steps in Parts 36–38 are the natural place to settle it; see report 4.
