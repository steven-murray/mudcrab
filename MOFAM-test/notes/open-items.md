# Still open in this build

Everything else about the MOFAM list is done: it installs, the load order
matches the guide's published one, and it has been played.

## In-game problems found by playing

Four were reported. One is closed:

- **~~Missing meshes (OOO's Fine Wood Desk and others)~~** — the modlist pinned
  **OOO BSA v1.7** (Dec 2025) where the guide meant **v1.5.15**, the newest file
  on the page in March 2025. v1.7 drops **1009 files** relative to v1.5.15,
  among them 138 `meshes\harvest` and 124 `meshes\armor`, so this was costing
  far more than the one desk. Now pinned to file id `1000038495`.

  `diff` **did** flag the row POST-GUIDE. The flag was read as version drift and
  waved through, which is the failure mode that warning exists to prevent: a
  newer file is not only newer, it can be *smaller*. Comparing a referenced
  asset path against what the install actually provides would have caught it
  outright — see roadmap Phase F.

Still open: the black compass, water not rendering, and dying when a gargoyle
dies. None is explained by the OOO version: the 1009 dropped files include
**no** compass or HUD texture, and OOO's plugins are byte-identical between
instances.

## Deliberately different from the reference instance

- **`Ultimate Leveling`, `ini_xp_kill_other`** — 25 here (the archive's default),
  10 in the reference. **The guide never mentions this setting**, so the build
  keeps the archive value. Change it only as a decision, not to close a diff.
- **`Swearing Rats.esp`** is omitted, which the guide permits. It is the natural
  first test case for optional mods — see
  [roadmap Phase B](../../docs/roadmap.md#b5-prove-it-on-mofam).

## Cosmetic, recorded so they are not re-investigated

- **`ImpeREAL City ... Merged.esp`** — same size, same 8845 records, same
  contents, different bytes. A full xEdit load-and-save sorts records within a
  group; QuickAutoClean does not. Record order inside a group carries no
  meaning.
- **Repacked BSAs** — same payload, different internal ordering. The BSA format
  does not require the payload region to follow record order.
- **`MigTraining.ini` line 65** — split across two lines in the reference by a
  hand edit that broke at a tab. The value the game reads is unchanged.

## On the reference side only

- **Its `Prebash Merge.esp` predates its own sources.** Three Harvest Flora DLC
  plugins were re-cleaned after that merge was built, and the zMerge GUI cannot
  be re-run there. Not a problem for this list — mudcrab rebuilds a merge
  whenever a source changes — but it means the reference is no longer a witness
  for that one file.
- **`Bashed Patch, 0.esp` sits seven positions early by timestamp.** Wrye Bash
  wrote it after MO2 last saved the load order, so it kept its own mtime.
  Whether that reaches the game depends on whether MO2 restamps at launch or
  only when the list changes — unverified. Restart MO2 after building a patch.
