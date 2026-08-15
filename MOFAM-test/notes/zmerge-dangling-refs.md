# Open: zMerge's Unique Forts output has 718 dangling references

**Status: pinned, not concluded.** One merge, one machine, one zMerge run. Do
not treat this as "zMerge is broken" until M7 has checked the other five.

## What was measured

`Unique Forts Merged.esp`, as built by zMerge and currently installed in
MOFAM-03.25, contains **718 references whose mod index exceeds the plugin's own
mod index**. A TES4 FormID's high byte indexes the *owning plugin's* master
list; the merged plugin has 2 masters, so its own index is 2 and any index > 2
addresses nothing. Those references are dangling in the shipped file.

The pattern is consistent and diagnostic: the bad mod index equals the source
plugin's **position in the load order**, not its position in the merged
plugin's master list.

Worked example, verified end to end:

| | |
|---|---|
| record | an ACHR inside Fort Irony, referencing another of Fort Irony's own records |
| correct merged FormID | `01001F8D` (own index 1... see below) |
| zMerge wrote | `06001F8D` |
| Fort Irony's load order position | **6** |

So the high byte is a load-order index that leaked into a field that is
supposed to hold a master-list index.

## Why this is not our bug

- mudcrab's output for the same merge has **zero** references with an
  out-of-range mod index (asserted directly in `tests/merge_unique_forts.rs`).
- Record count matches zMerge exactly (7912), clobber count matches the recon
  measurement exactly (56), and every other reference edge agrees.
- The 718 are excluded from the tier-2 comparison *by count*, and the test
  asserts our surplus edges are exactly the repaired ones — so the exclusion
  cannot mask a genuine disagreement elsewhere.

## Consequences

1. **Tier 3 (near-byte-exact) is unreachable for this merge.** The oracle is
   wrong in 718 places; matching it byte-for-byte would mean reproducing the
   defect. Tier 2 is the real gate.
2. **The installed merge is probably faulty in game** — dangling references
   typically surface as missing NPCs/objects or scripts failing to resolve
   their targets, localised to the affected forts.

## Alternative explanations not yet ruled out

The user built this merge through zEdit's GUI, which was misbehaving badly on
this machine (see the Wine/zEdit debugging session). Plausible causes, in
rough order of likelihood:

1. **Operator error / GUI misfire** — plugins added to the merge in a state
   zMerge did not expect, or a merge re-run over stale output.
2. **A zMerge bug** specific to some property of this merge (it is the only one
   of the six with a large CELL/worldspace remap load: 2004 object-index
   remaps, 84 CELLs, PGRDs, worldspace children).
3. **A genuine zMerge bug affecting everything**, which would make all six
   installed merges suspect.

## How to discriminate — do this in M7

Run each remaining merge and count dangling references in **zMerge's** output
for each:

| merge | sources | remaps | zMerge dangling refs |
|---|---|---|---|
| Unique Forts Merged | 11 | 2004 | **718** |
| TACE Merge | 21 | 1170 | ? |
| NPC Merge | 14 | 0 | ? |
| Late Loaders Merged | 20 | 0 | ? |
| OOO Patches Merged | 19 | 0 | ? |
| Prebash Merge | 86 | 0 | ? |

Reading the result:

- **Only Unique Forts affected** → most likely a one-off (cause 1 or 2). Rebuild
  that merge and move on; do not generalise.
- **Only the two merges with remaps affected** (Unique Forts, TACE) → points at
  a real zMerge bug in the renumbering path, since the four zero-remap merges
  exercise only master-index remapping. This is the most informative outcome.
- **All six affected** → systemic; every installed merge needs replacing, and
  the claim deserves an upstream report with a reproducer.

Before concluding zMerge is wrong in *any* of these cases, confirm the
opposite direction too: pick 3 of the flagged references per merge, resolve
them by hand against the source plugin, and check that mudcrab's value is the
one that resolves to the intended record. Being different from a suspect oracle
is not the same as being right.
