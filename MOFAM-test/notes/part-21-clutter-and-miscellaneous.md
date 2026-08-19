# Part 21 — Clutter & Miscellaneous Retextures

35 guide rows, 44 mods. **37 of 44 identical**; six differences are Part 36
merge-source plugins hidden in the Oracle, and the seventh is a guide step the
Oracle did not perform.

## The second `conflicts_with` call site, and its count matches again

Guide row 3:

> *Once installed, open the Conflicts tab & hide the 5 winning mesh conflicts
> over Katkat's Vegetable Garden.*

```toml
[[mods.actions]]
action = "file_hide"
conflicts_with = ["KatKat's Vegetable Garden"]
under = "meshes"
```

Hid **exactly 5**, the number the guide states — the second time the mechanism
has been checked against a count the guide wrote rather than a list read off a
finished install:

```
meshes/plants/CropLettuce01.nif
meshes/plants/CropPotato01.NIF
meshes/plants/CropPumpkin01.NIF
meshes/plants/CropWaterMelon01.NIF
meshes/plants/PlantTomato01.NIF
```

`under = "meshes"` is load-bearing. The guide says *mesh* conflicts, and the two
mods also share textures; without it the selection would have been larger than
the guide's count, and the count is the only check available.

**The Oracle did not do this step** — all five are unhidden there. Same shape as
Part 18's ships row, which Steven then applied by hand. Flagged for him.

The partner mod is `KatKat's Vegetable Garden`, which is the Part 18 row this
build found missing from the Oracle in the first place. Two sections later it is
load-bearing for a different row's conflict list.

## A whole class of false version warnings, removed

Three of this section's rows (the TIBs Compact Quivers files) were reported
POST-GUIDE — "the Oracle's archive is newer than the March 2025 guide". They are
2018 uploads. What misled `diff` was `nexusLastModified=2026-01-26`: the date
they were downloaded to this machine.

The Part 19 fix caught the version of this with *legacy* file ids. These have
modern ids, so it did not help. The real observation is stronger:

**Nexus allocates file ids in ascending order, so an id is an upload date.**

Checked against every archive in this list carrying both a file id and a Unix
timestamp in its filename — **406** of them, of which **405** were usable. Across
those 405 the orderings agree exactly; the 406th is the exception described
below. The boundary is clean:

| | file id | date |
|---|---|---|
| largest pre-guide | 1_000_040_927 | 2025-02-23 |
| smallest post-guide | 1_000_040_999 | 2025-03-01 |

Nothing in between. `classify_guide_age` now uses the file id as its second
source, after the filename timestamp.

**The constant is baked into the binary, so a user who has never seen a
reference instance gets the same answers.** That is the real gain over
`nexusLastModified`, which was only ever as good as one install's metadata.

Worth stating the limit honestly: this is one game's corpus over one year, not a
verified site-wide invariant. It is a strong, clean signal and it can be
re-derived from any corpus of Nexus downloads — but "Nexus allocates ids in
ascending order, globally" is an inference from 405 archives, not something this
repo can prove.

Part 21's POST-GUIDE count went from 4 to 1, and the survivor is real.

### The one exception, and it is the row already flagged

The single archive that broke monotonicity is Part 20 row 8's VGR patch, whose
two versions' MO2 sidecars both claim file id `1000038748`. That row was already
written up as unresolved for exactly this reason. Excluded from the calibration
rather than allowed to widen the boundary.

### What this removed

`nexusLastModified` is no longer consulted at all. It was reachable only when a
mod had a real Nexus file id — the same condition under which the file id now
answers — so the branch became unreachable the moment the id rule landed. It and
its date parser are deleted rather than left as dead code. The field is still
read, because its *presence* is what distinguishes "no date recorded" from "a
date we decline to trust" in the message.

## Row 1 is unresolved

The guide names **CLUTTER_BETA1**; the only archive on disk is
`T4UT - CLUTTER_BETA1-54904-CLUTTER-BETA2-1748437804.7z` — mod page BETA1, file
BETA2. Steven is downloading the real BETA1.

**The diff is clean on this row either way**, because the Oracle installed the
same BETA2-named file. That is precisely why it is written down: a clean diff
here means "both sides did the same thing", not "both sides did the right
thing". `diff` does flag it POST-GUIDE, which is the only signal there is.

The guide also asks for the mod to be named `T4UT - CLUTTER_BETA1 - Farmhouse &
Vinyard`; the Oracle called it `T4UTXL - CLUTTER_BETA1`, and ours matches the
Oracle so the two can be paired.

## Small things

- Row 1 uses `include` rather than a post-install prune. Same result, and it
  never writes the ~3 GB it would then delete.
- Row 29 is *"Kat's Actually Decent Enviroment Map"* in the guide; the mod page
  and the Oracle folder both say *"Kinda Actually Decent Environment Map"*.
- Rows 9, 15 (mediafire) and 18 (moddb) are `manual:`. Three more hosts, same
  caveat as `oracle-dependence.md` records.
- Rows 22 and 30 had no Nexus file id anywhere on disk until Steven ran
  `mudcrab identify --write-meta`. Worth remembering as the standing answer to
  that shape.
