# 1. Disagreements with the Oracle

Where our build and yours differ deliberately. Ordered by how much I think you
need to look at them.

## Needs your decision

### Part 23 — two combat numbers

Guide row 1 gives four INI edits for `Dynamic Oblivion Combat.ini`. Two do not
match what your Oracle contains:

| setting | guide | Oracle |
|---|---|---|
| `dcvars.ini_DodgeKeyCode` | 42 | 42 |
| `dcvars.ini_NPCdodgePercent` | **50** | **70** |
| `dcvars.ini_NPCflankPercent` | **50** | **70** |
| `dcvars.ini_NPCDisarmToKOratio` | 10 | 10 |

Ours follows the guide, so that file differs by exactly those two lines and
nothing else. Either the guide's numbers are what you meant and the Oracle
drifted, or you tuned them deliberately and the guide is stale. **Only you know
which.**

### Part 21 row 1 — CLUTTER_BETA1 vs BETA2, unresolved

The guide names **CLUTTER_BETA1**. The only archive on disk is
`T4UT - CLUTTER_BETA1-54904-CLUTTER-BETA2-1748437804.7z` — the mod *page* is
BETA1, the *file* is CLUTTER-BETA2. You were downloading the real BETA1 when you
turned in; it had not appeared by the end of the run.

**The diff is clean on this row either way**, because your Oracle installed the
same BETA2-named file. That is exactly why it is written down: a clean diff here
means "both sides did the same thing", not "both sides did the right thing".
`diff` does flag it POST-GUIDE, which is the only signal there is.

## Guide steps your Oracle did not perform

Three in a row, all conflict-hides, all now done on our side and not yours. Each
one is a real behavioural difference in game, not cosmetic.

| part | row | what the guide asks | files |
|---|---|---|---|
| 18 | 7 | hide what wins over `VWD Ships` | 4 |
| 21 | 3 | hide the mesh conflicts over `KatKat's Vegetable Garden` | 5 |
| 22 | 2 | hide the texture conflicts over Katkat's Ayleid Ruins HD | 2 |

You fixed Part 18's by hand during the run. Parts 21 and 22 are still open.

Worth noting the guide states a count for all three — 4, 5 and 2 — and
`conflicts_with` produced exactly those numbers each time, from the modlist
alone. That is the strongest evidence so far that the mechanism is right.

## Rows the guide asks for that your Oracle lacks

- **Part 19 row 9**, *Beautiful Creatures - Spider Daedra*. You fetched it and
  ran `identify` mid-run, so it builds now — but there is still no folder for it
  in the Oracle, so it shows as extra in ours.
- **Part 18 row 14**, the vegetable garden, was the same shape until you
  installed it. Worth mentioning because it then became **load-bearing for Part
  21 row 3's conflict list** two sections later.

## Resolved during the run

- Part 18 is now **16 of 16** after you installed the garden and hid the ships
  files.
- Part 20 row 8's POST-GUIDE archive: you said 3.4.7 is fine. Kept.
- `meshes/realswords/nord/chainmailm1.nif`: you said not a defect. The Part 24
  row will need an explicit exception when it goes live, and that is recorded on
  the row itself.
