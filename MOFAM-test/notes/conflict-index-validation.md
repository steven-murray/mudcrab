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

`meshes/realswords/nord/chainmailm1.nif`. Steven kept it because MO2 lists it as
overwritten by **WAC - Integration - Roberts Conversion**, not by WAC. That is
right, and it explains itself once the priorities are laid out:

| mod | modlist.txt line | ships it as |
|---|---|---|
| WAC - Integration - Roberts Conversion | 566 | loose |
| WAC Waalx Animals & Creatures | 569 | inside `WACIntegration.bsa` |
| OOO Enhanced 5.3b - Resources | 571 | loose |

WAC ships **no loose files at all** — its folder is an esm and a BSA. So for the
other 577 files the only competitor is a packed one, MO2 files them under archive
conflicts, and it names WAC. `chainmailm1.nif` is the single file in the 578 that
*also* has a loose competitor, and MO2's conflict list names the **winner**:
loose beats packed regardless of priority, and Roberts outranks everything here
anyway. So this one file appears under Roberts and vanishes from the WAC list.

**Which means the computation and the Oracle are both right about the world, and
the disagreement is only about what the guide's "conflicts with WAC" covers.**
The claim in the first draft of this note — that it "meets the guide's condition
the same way its 270 neighbours do" — is true of the file and misleading about
the consequence. It does not behave the same way, because it is the only one with
a loose competitor.

Functionally it does not matter either way. Roberts wins that path whatever OOO
Enhanced Resources does with its copy, so the file is already dead weight; the
choice is between two spellings of the same in-game result.

The argument for removing it is only consistency: `conflicts_with = ["WAC ..."]`
is a statement about mods, and WAC does provide this path. Keeping it would mean
carrying a one-file exception in the modlist for a file that has no effect.
**Decided by Steven: this is not a defect.** The Oracle keeps the file, and any
`conflicts_with` written for this row has to reproduce that. Since the guide's
instruction names WAC and WAC does provide the path, the eventual Part 24 row
will select it, so it needs an explicit exception when that row goes live —
recorded on the row itself and in task #25.

Worth keeping regardless: MO2's conflict tab names the winner, not every
provider, so a file with two competitors is filed under one of them and drops out
of the other's list. That is a property of the UI, not of the conflict, and it is
a good reason not to derive these lists by reading them off a finished install.

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
