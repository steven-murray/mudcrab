# Still open in this build

Everything else about the MOFAM list is done: it installs, the load order
matches the guide's published one, and it has been played.

## In-game problems found by playing

Four were reported. Three are closed:

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

- **T4UT - Menus Repolished installed 2195 textures where nothing read them.**
  Its archive ships `menus/hud/...` with no `textures/` root, so a literal
  install drops .dds files into `Data/Menus/`, the XML directory. Menu XML
  resolves `<filename>` under `Data/Textures/Menus/`. Fixed with
  `target_subdir = "textures"`; the mod now does something for the first time.
  **The reference instance has the same defect** -- MO2's installer was just as
  literal -- so this was invisible to `diff`.

  It is *not* the black compass: vanilla ships
  `textures\menus\hud\hud_compass_face.dds`, so the fallback was always there.

- **~~Black compass~~** — DXT1 punch-through alpha draws as opaque black in
  menus on DXVK/NVIDIA under Proton; DarNified's compass frame covered the face.
  Fixed by the optional `decompress_dxt1_alpha` input (`dds_decompress`). See
  `docs/known-issues.md`, Linux / Proton.

- **~~Invisible water~~** — on an NVIDIA vendor id ORC reads the depth buffer
  through NVAPI, which returns nothing under Proton. Fixed by the optional
  `dxvk_report_amd_vendor` input: a `dxvk.conf` deployed by Root Builder.

Still open: dying when a gargoyle dies.

- **~~DarNified UI was missing two menus~~** — `menus/main/stats_menu.xml` and
  `menus/prefabs/darn/stats_config.xml`. Not a selection error: the FOMOD
  answers already matched the guide exactly (all Components, Normal, Default
  Kingthings, no Custom Options). The files install through
  `conditionalFileInstalls`, gated on `Oblivion XP.esp` being absent, and
  **mudcrab never read that block at all**. Fixed; DarNified UI is now
  file-for-file identical to the reference instance, and AWLS gained a file by
  the same fix. Steven's "fonts messed up" report is what led here.

- **Two `Oblivion.ini` files in one profile, differing only by case.** MO2 reads
  and writes `oblivion.ini`; BethINI created `Oblivion.ini` on 2026-08-22.
  mudcrab's `ini_set` resolves case-insensitively and picked the capital one, so
  **all eighteen game-scoped settings have been landing in a file the game never
  opens** -- wrong fonts, `bUseRefractionShader=1` where the guide says 0, grass
  at 3000 instead of 8000, and the game running 3840x2400 instead of 2560x1600.
  `ini_set` now refuses when a case twin exists rather than guessing. The
  duplicate still has to be removed by hand in the instance.

### Dying when a gargoyle dies (parked)

Reported once, with both spells and weapons, and only at the gargoyle's death.
What is known: OOO's `AbGargoyle`, `GargoyleCorrosion1` and
`GargoyleMagicResist1` carry Reflect Damage (REDG) and Reflect Spell (RFLC),
and OOO's plugins are byte-identical to the reference instance, so if it is
anything it is OOO as designed rather than an install difference. Not yet
reproduced deliberately; it may be a reflect killing a low-health character
rather than a bug. Next step if it recurs: reproduce at full health, and check
the death against the gargoyle's last hit.

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
