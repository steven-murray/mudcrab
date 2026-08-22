# Rows the build knowingly does not complete

**This is the list to work through before calling the build finished.** Every
entry is a place where mudcrab installs the mod but does *not* do everything the
guide's row asks. Nothing here fails; each one succeeds and leaves the instance
slightly short of the guide, which is exactly why it needs writing down rather
than trusting to a diff that has other things to say.

Kept current as sections are built. Last updated after Part 40 — the
list is now built end to end, so this is the finish-line checklist.

---

## A. Steps mudcrab has no action for

These need a feature before they can be automated, or a hand pass at the end.

### A1. `ini_append_block` — Part 30 row 7 (FEA - Fundament Enchanting Addons)

The guide asks for nine lines to be pasted into `Custom Trainers.ini`:

```
set migFeaQ.customTrain to (GetFormFromMod "Oblivion.esm" 02D025)   ;Uurwen
set migFeaQ.customLevl to 65
SetStage migFeaQ 1
set migFeaQ.customTrain to (GetFormFromMod "Oblivion.esm" 015EA9)   ;Calindil
set migFeaQ.customLevl to 40
SetStage migFeaQ 1
set migFeaQ.customTrain to (GetFormFromMod "Oblivion.esm" 0222B7)   ;Contumeliorus Florius
set migFeaQ.customLevl to 65
SetStage migFeaQ 1
```

`ini_set` cannot express this. It is not three key/value edits — the same two
keys are set three times each, and `SetStage` between them is what commits each
triple. Order and repetition are the content, so an append is the only correct
shape.

**Effect if left**: the three custom enchanting trainers (Uurwen, Calindil,
Contumeliorus Florius) are never registered. The mod works; those three NPCs
just do not offer the service.

**Fix**: an `ini_append_block` action taking a raw multi-line string, appended
verbatim if not already present. Idempotence is the only hard part — match on
the whole block, not a key.

Target file, as installed: `ini/MigFEA - Custom Trainers.ini`.
Our copy is 1788 bytes; a correct one is 2181.

### A2. ~~Scripted record deletion~~ — done, all three rows

`delete_records` does what those rows asked xEdit to do by hand. All three are
encoded and verified:

| row | what it removes | result |
|---|---|---|
| 11 #23 | the `WRLD` group from `Harvest [Flora] - DLCFrostcrag.esp` | **byte-identical** to the Oracle |
| 26a #6 | `xx010F43` (a Light) and `0000003C` (Tamriel), 593 entries | same records, same contents |
| 26b #12 | three wild edits, 5 entries | **byte-identical** to the Oracle |

Two details worth keeping. Removing a record takes the group holding its
children — a CELL and the GRUP of its references are separate entries, so
deleting the record alone leaves children parented to nothing. And a group left
holding nothing collapses, which is why 26b row 12's "delete Block -11, 2" can
be written as the one cell inside it: the sub-block and block go with it.

### A3. The Bashed Patch itself — Part 38 row 1

The download is the **configuration only**: one `.dat` under `Bash Patches/`,
which installs identically to the Oracle's. The patch plugin,
`Bashed Patch, 0.esp`, is what Wrye Bash writes after the row's twelve-step GUI
procedure, and mudcrab has no way to produce it.

The mod folder and its configuration are staged, so the procedure has somewhere
to start and the Import step has a file to import. What remains is the row as
written: open Wrye Bash through MO2, place the patch above Conflict Resolution
and below `NPC Merge.esp`, Deactivate All, Activate Non-Mergeable, tick
`Conflict Resolution.esp`, `OOO Patches Merged.esp` and `NPC Merge.esp`, Rebuild
Patch, deselect the latter two in the "Deactivate Prior to Patching" popup,
Import the configuration, Build Patch, then move the plugin out of Overwrite into
the mod.

**Effect if left**: no bashed patch, so levelled lists and the tweaks the guide
routes through it never merge. This is the single thing between the build and a
playthrough.

### The one trap: it must be declared before it is built

`install` rewrites `plugins.txt` from the modlist on every run. Build the patch
without declaring it and the next mudcrab run drops it from the profile — the
file stays on disk, the game stops loading it, and nothing says so.

So the `Bashed Patch` mod already declares `plugins = ["Bashed Patch, 0.esp"]`
and the load order already lists it at the published position, between
`NPC Merge.esp` and `Conflict Resolution.esp`, which is exactly where the row
says to put it. Until the file exists, `install` leaves it out and logs
`omitting plugins from plugins.txt because they are not installed yet`. The
moment Wrye Bash writes it into the mod folder, it appears in the profile at that
position with no further edit.

### Before starting

- The row's own preamble: open xEdit and **Sort Masters** across the whole load
  order first.
- `MOFAM - Conflict Resolution` is installed and active — the row requires it,
  because its `BashTags/` folder carries the tags for the merges.
- The three plugins the row has you tick all exist and are active:
  `Conflict Resolution.esp`, `OOO Patches Merged.esp`, `NPC Merge.esp`.
- The configuration to Import is at
  `mods/Bashed Patch/Bash Patches/Bashed Patch, 0.esp_Configuration.dat`.

### A4. BAIN Wizard scripts — Part 28 row 5 (Configuration Items Begone)

The package ships a `Wizard.txt`. mudcrab selects subpackages, which covers the
plugins, but the wizard also *writes* files based on the answers.

**One file is missing**: `INI Tweaks/Oscuro's_Oblivion_Overhaul.ini`.

`INI Tweaks/` is a Wrye Bash convention — a folder of INI fragments Bash can
apply — so the practical effect is small unless you use that feature. It is
listed because it is the first and so far only case in the whole list where a
wizard's *output* is actually absent rather than merely unused.

**Fix**: either a wizard interpreter (large, and probably not worth it for one
file) or copy the file in by hand.

---

## B. Deferred by choice, not by capability

### B1. ~~QAC~~ — run, and all nine differences explained

26 mods, 27 plugins. **21 are byte-identical to the Oracle's copies.** The pass
is unattended; `src/config/tools/xedit.rs` says how, and why it drives real
xEdit rather than reimplementing "identical to master".

The six that differ are not QAC failures. Every one is now accounted for:

| plugin | what it is |
|---|---|
| `EVE_StockEquipmentReplacer for OOO.esp` | Oracle never cleaned it — 1 ITM |
| `All Natural - Real Lights.esp` | Oracle never cleaned it — 1 ITM |
| `The Imperial Water.esp` | Oracle never cleaned it — 19 ITM |
| `Nobody Goes into the Mountains but Hunters.esp` | Oracle never cleaned it — 17 ITM |
| `The Lost Spires.esp` | Oracle never cleaned it — **159 ITM, 433 UDR** |
| `ImpeREAL City … Merged.esp` | same size, same 8845 records, same contents — record **order** only |

**"Never cleaned" is measured, not inferred.** For all five, the Oracle's copy
is byte-for-byte the plugin as it comes out of the mod's archive. The dirt
counts are xEdit's own, from its log when we cleaned ours. All five carry
`[QAC]` in the guide, so this build follows the guide and the Oracle skipped
those five rows.

`The Lost Spires`' 433 UDRs are the "443 records differ" that looked mysterious
earlier — undeleted references, not ITM removal.

**ImpeREAL City** is the one cosmetic case. Its groups are FormID-sorted in the
Oracle and in archive order here: a full xEdit load-and-save sorts records
within a group, QuickAutoClean does not, and the Oracle's copy went through a
manual xEdit session for its row-6 deletions while ours is edited by mudcrab.
Record order inside a group carries no meaning — the engine indexes by FormID —
so this sits with the BSA payload-ordering difference as a known non-issue.

**Every merge now matches the Oracle's record count exactly**: Unique Forts
7912, OOO Patches 1759, TACE 8533, Prebash 4505, Late Loaders 4361, NPC 2278 —
and the renumbering counts (2004 and 1170) match too.

### B2. ~~LOOT / load order~~ — settled at Part 37

`loot-sort` was never needed. The load order is the modlist's `plugins` array,
and Part 37 made that array identical to the guide's published `loadorder.txt`,
entry for entry. No GUI, no interim.

## C. Open on the Oracle side

Not mudcrab gaps — things in `MOFAM-03.25` still outstanding as of 2026-08-22.
Most of what was here has been fixed; see section D.

### C1. `DLCFrostcrag.esp` has had its Worldspace group deleted, and should not have

Guide Part 11 row 23 says:

> Open **Harvest [Flora] - DLCFrostcrag.esp** in xEdit & remove the **Worldspace** group.

On the Oracle that removal has been applied to **two** files. Top-level groups:

| file | groups |
|---|---|
| `Harvest [Flora] - DLCFrostcrag.esp` (Oracle) | `FLOR` — correct, the row asks for this |
| `Harvest [Flora] - DLCFrostcrag.esp` (ours) | `FLOR`, `WRLD` — **we are missing the step**, see A2 |
| `DLCFrostcrag.esp` (Oracle) | no `WRLD` group at all |
| `DLCFrostcrag.esp` (ours) | `WRLD` present |

Nothing in the guide asks for the DLC master itself to be touched, and the
consequences are not cosmetic. Against the vanilla file (1316 records), our QAC
removes 70 and the Oracle's copy is short by 227. The extra 157 are:

- 4 WRLD — `Tamriel`, `SkingradWorld`, `ICTheArcaneUniversity`, and Bethesda's
  leftover `TestGragtown`
- their child CELLs, including `FrostcragSpireExterior`
- 134 REFR, 5 LAND, 4 PGRD

**71 of the removed records are DLCFrostcrag's own**, not overrides — QAC never
removes those, because a plugin's own records have no master to be identical
to. They are object placements: they position 24 distinct base objects, and 9 of
those base objects are still defined in the file with nothing left to place
them.

So the Oracle's Frostcrag Spire has its interiors (the `CELL` group survives)
and no exterior placement. **Worth checking in game** — travel to Frostcrag
Spire and see whether the building is there — before assuming this note is
right about the consequence. The file difference itself is not in doubt.

`DLCFrostcrag.esp` is also a master for `Harvest [Flora] - DLCFrostcrag.esp`,
`DLCFrostcrag - OOO Adaptation.esp` and `Bruma Frostcrag Spire LOD.esp`, so
anything overriding those 71 records now points at a record its master no longer
defines.

### C2. Nine of Ultimate Leveling's fifteen edits are still unapplied

Guide row 1 lists fifteen `set` edits. Two are now correct (`ini_xp_level_base`
1000, `ini_xp_level_mult` 400). Nine still hold the archive's defaults:

| setting | archive & Oracle | guide |
|---|---|---|
| `ini_xp_skill_level_points_journeyman` | 3 | 2 |
| `ini_xp_skill_level_points_expert` | 4 | 3 |
| `ini_xp_skill_level_points_minor` | 15 | 10 |
| `ini_xp_read_skillbook_minor` | 2 | 4 |
| `ini_xp_read_skillbook_major` | 4 | 8 |
| `ini_xp_train_minor` | 3 | 2 |
| `ini_xp_train_major` | 5 | 3 |
| `ini_rested_bonus` | 50 | 20 |
| `ini_horseshoe_total` | 150 | 0 |

Plus `ini_xp_kill_other`, which the guide does not mention and the Oracle sets
to 10 against the archive's 25.

### C3. Cosmetic, listed so they are not re-investigated

- `MigTraining.ini` line 65 (`fTrainerAdvancedMult`) has one tab in the Oracle
  where the archive has two. No instruction touches that line; the file has been
  hand-edited. The value edit itself is now correct.
- `YourMotherWasAHamster.ini` reads `1.0` in the Oracle and `1` here. Same
  number.

## D. Fixed — kept for the record

- ~~Thorn Addon (Part 24 row 7c) plugin deletion~~ — done; the mod is now
  byte-identical.
- ~~Mehrunes Dagon archive~~ — refreshed; the mod is now byte-identical, and
  the stale-cache defect it exposed is fixed in mudcrab.
- ~~`migTrainingQ.bDisplaySkillNumbers`~~ — now 1, per the guide.
- ~~Ultimate Leveling's `ini_xp_kill_companion` / `_pet` / `_follower`~~ — back
  to the archive's 50/50/50, matching ours.
- ~~Urasek's repack kept the base mod's bytes~~ — repacked correctly on the
  second pass. `mudcrab inspect` now reports 58,223,187 bytes of payload on both
  sides, so the "Replace All" overlay took.
- ~~`AFK Weye.bsa` was named with a space where the plugin has an underscore~~ —
  fixed by Steven.
- ~~Bank of Cyrodiil was never repacked, its voice copy split into
  `sound/voice` and `sound/Voice`, and the voices mod was left enabled~~ — fixed
  by Steven.
- ~~The three Base Object Swapper integrations had their ini at the mod root
  rather than under `BaseObjectSwapper/`~~ — fixed by Steven, who notes it is a
  known misbehaviour of MO2's installer with these archives. The plugin's own
  binary settles the requirement: `BaseObjectSwapperOB.dll` contains the strings
  `Data\BaseObjectSwapper` and *"No .ini files were found in
  Data\BaseObjectSwapper folder, aborting..."*.
- ~~SupreMe Overhaul kept its sound folder~~ — deleted by Steven, per the guide.
- ~~Camping and the Greed Arena voiced addon were not installed~~ — installed by
  Steven.
