# Part 36 — zMerged Plugins

All six merges now exist. **The load order is 240 active plugins**, under
Oblivion's 255 for the first time in this build — `mudcrab validate` reports zero
warnings.

13 mods compared (6 consistency patches, 6 merges, 1 already-built): 7 identical,
6 differing. All six differences are in the merge outputs, and all six are
accounted for.

## The engine is exact; the sources are not

The decisive measurement is `tests/merge_oracle.rs`, run against the real Oracle:

```
Late Loaders Merged: tier 2 passed -- 4361 records, reference graphs identical
NPC Merge:           tier 2 passed -- 2278 records, reference graphs identical
Prebash Merge:       tier 2 passed -- 4505 records, reference graphs identical
```

Fed the Oracle's own source plugins, mudcrab reproduces every merge's record
count and reference graph exactly, TACE at 8533 included. So the differences in
the *installed* merges come from the inputs, not the merge.

Checking every source of every merge, byte for byte, against the Oracle's copy:

| merge | sources | differing |
|---|---|---|
| Unique Forts Merged | 11 | 0 |
| OOO Patches Merged | 19 | 0 |
| Late Loaders Merged | 20 | 0 |
| NPC Merge | 14 | 0 |
| TACE Merge | 21 | **7** |
| Prebash Merge | 85 | **2** |

**All nine are `[QAC]` rows**, and QAC is deferred list-wide (see
`incomplete-rows.md` B1). The record counts confirm it rather than merely fitting
it — for six of the seven TACE sources, ours holds *more* records than the
Oracle's and **not one record's contents differ**:

```
Anvil_MorningGlory_Mixed.esp     1 record only in ours;   76 vs 75
Anvil the city of Dibella.esp    6 records only in ours; 777 vs 771
Chorrol Castle Courtyard.esp     2 records only in ours; 241 vs 239
Cheydinhal Peach Tree Island.esp 6 records only in ours; 439 vs 433
Chorrol Park.esp                10 records only in ours; 233 vs 223
Thorn Lodge.esp                  4 records only in ours; 281 vs 277
```

That is the exact signature of Quick Auto Clean: it removes identical-to-master
records and leaves the rest untouched. The seventh, `SkingradDeuglified.esp`,
also shows renumbering (500 vs 497 records unique to each side), which is what
undeleting UDRs does.

The merged totals do not move by the same amount — ours is 8512 against 8533 —
because a clobber merge with FormID remapping does not sum its sources: a changed
source shifts allocation for everything after it. Running the deferred QAC pass
is what settles the final numbers, not more analysis.

## The other differences, already understood

- **Masters**: the Oracle's merges carry masters no source requires — `knights.esp`
  in Late Loaders, three in NPC Merge, two each in OOO Patches Merged and Prebash.
  The oracle test names these explicitly ("zMerge carries N master(s) no source
  requires"). zMerge's stale `merges.json` load orders, harmless, and ours is the
  tighter list. See `zmerge-non-canonical-refs.md`.
- **Every record's bytes differ in Unique Forts and TACE** while the reference
  graphs are identical: mudcrab writes canonical mod indices, zMerge writes the
  source's load-order index. Same graph, different high byte. Closed, cosmetic —
  same note.
- **Bookkeeping files**: ours writes `mudcrab-merge.json`, zEdit writes
  `fidCache.json`, `merge.json` and a timestamped log. Not game content.

## Prebash still has no ORC.esp

The guide's Prebash list has 86 entries; ours has 85. `ORC.esp` is deliberately
absent — this build uses ORC 315F, which ships no plugin. Decided long before
this section; recorded here because a future reader counting the list will notice.

## New in mudcrab: `diff` explains plugin differences

`diff` could already say what two differing BSAs differ *in*. It could say
nothing about plugins, which is the most common difference in this whole build —
every merge and every `[QAC]` row lands there — so each one cost a manual xEdit
session to reach an answer already in the bytes.

`describe_plugin_difference` now reports records only on one side, records whose
contents differ, both totals, and master-list differences by name. Everything in
the two tables above came out of it.

One detail is load-bearing: **the `.mohidden` suffix comes off before the
extension is read.** Once a merge exists, every plugin it consumed is hidden on
both sides, so the files whose difference most needs explaining are exactly the
ones no longer named `.esp`. Reading the extension straight skipped all of them.

## Naming is a contract

Each merge's `output` matches the Bash Tags in the Conflict Resolution mod
exactly. A renamed merge does not fail — the bashed patch just quietly degrades.
Do not tidy these names.
