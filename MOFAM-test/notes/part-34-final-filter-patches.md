# Part 34 — Final Filter Patches

7 rows, 7 mods, 8 files. Diff: **7 of 7 identical**, nothing to explain.

The section's premise is that filter patches deliberately name masters they do
not require, so MO2 shows all seven as having missing masters and the guide says
to ignore it — the Bashed Patch consumes only the masters actually in the load
order. mudcrab does not check masters at install time, so no override was
needed; this is recorded because "ignore the warning" is the kind of instruction
that looks like it needs a feature and does not.

Three rows needed a selection rather than a plain extract:

| Row | Selection |
|---|---|
| 3 | `data_folder = "BasicHarvest_FilterPatch_V1.4/00_CoreFilterPatch"` — the same archive Part 11 uses, a different folder out of it, so it is a second mod rather than a second subpackage |
| 5 | `bain_subpackages = ["OCRAFT Patches"]`, less the two esps the row names |
| 7 | `bain_subpackages = ["01 Separate UL"]` |

## Naming

Row 5 asks for the mod to be called "Miscellaneous Patch Collection by
Dispensation - Filter Patches". The Oracle abbreviated it to "Misc Patch
Collection…". The guide's name is used, with `oracle_name` mapping it back.

## Version drift

`Unique Landscapes - OOO Adaptation` is dated 2025-04-25, five weeks after the
guide. The Oracle installed 2.12; the guide says only "this mod". Flagged as
POST-GUIDE, matched to the Oracle's file so the two instances agree.

## `Unique Landscapes Separate  - OOO Adaptation.esp`

Two spaces before the dash. That is how the archive spells it, and how the load
order has to.
