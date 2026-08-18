# Validating the conflict index against the Oracle

The design (`conflict-resolution-design.md`) noted that this feature is unusual
in having a known-correct expected output: `ooo-enhanced-conflict-hidden-files.txt`,
the 1725 paths read off the Oracle when Part 9 was built by hand. That list was
always meant to stop being the mechanism and become the test. This is that test.

## What was run

```bash
mudcrab conflicts --plan MOFAM-test/output/plan.json \
  --mods-dir "$MO2/mods" \
  --mod "OOO Enhanced 5.3 (03.25) - Resources" \
  --with "WAC Waalx Animals & Creatures" \
  --with "WAC - Integration" \
  --with "HGEC Equipment Replacer for WAC" \
  --with "WAC - Integration - Roberts Conversion" \
  --with "WAC - Integration - HGEC Gauntlets Conversion"
```

Only the WAC half can run: the guide's instruction names Colourful Clothing too,
and that mod arrives in Part 24, which is not authored yet.

## Result

**578 files computed. 577 of them are in the Oracle-derived list.**

The 1147 paths in the list that the computation does not produce are exactly the
subtrees Colourful Clothing owns:

| subtree | in the list, not computed |
|---|---|
| `textures/clothes` | 472 |
| `meshes/clothes` | 425 |
| `textures/menus` | 223 |
| `textures/menus80`, `textures/menus50` | 28 |

And what *is* computed is exactly WAC's territory: 271 `meshes/realswords`,
196 `textures/realswords`, 98 `textures/menus`, and 13 stragglers across
creatures, clutter, armor and idleobjects. The two halves partition cleanly,
which is a stronger result than the headline number: the mechanism is not
matching by luck, it is matching by mod.

## The one disagreement

`meshes/realswords/nord/chainmailm1.nif`.

The computation says WAC provides it and so OOO Enhanced Resources should yield
it. The Oracle's hidden list does not contain it, and the file is still sitting
there unhidden:

```
MOFAM-03.25/mods/OOO Enhanced 5.3b - Resources/meshes/RealSwords/Nord/chainmailM1.nif
```

Both `WAC Waalx Animals & Creatures` (inside `WACIntegration.bsa`) and
`WAC - Integration - Roberts Conversion` (loose) ship that path, so it meets the
guide's condition the same way its 270 neighbours in `meshes/realswords` do.

**This looks like a miss in the hand-built Oracle, not an error here** — one file
out of 578 selected by eye through MO2's conflict tab. Worth confirming with
Steven before treating it as settled; it is the sort of thing the mechanism
exists to stop happening, so it would be a shame to "fix" it the wrong way.

## Two bugs this found

Neither would have shown up in a test with a hand-written fixture, because both
are about real archives.

1. **BSA paths are stored `folder\file`.** Comparing them to filesystem paths
   without normalising the separator gave *zero* overlap with a 7628-file mod --
   and zero overlap reads exactly like "these mods do not conflict". The mods
   this silently breaks are the ones that pack their assets, which is the whole
   category the feature exists for. Now normalised, and tested.
2. **`WACIntegration.bsa` has a folder named ` animals & creatures.addon`**, with
   a leading space and no `waalx`. 91 of its 7628 entries sit under it. The
   reader's offset and name-length checks all pass, and our copy of the file is
   byte-identical to the Oracle's, so these are the mod author's own bytes rather
   than anything mudcrab did. Noted, not chased.
