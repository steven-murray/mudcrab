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

All but one are now closed. What is left is A3, the Bashed Patch, which is a
GUI procedure rather than a missing feature.

### A1. ~~`ini_append_block`~~ — done, Part 30 row 7

The guide asks for nine lines to be pasted into `MigFEA - Custom Trainers.ini`,
registering Uurwen, Calindil and Contumeliorus Florius as custom enchanting
trainers. `ini_set` could not express it: the same two keys are set three times
each and the `SetStage` between them is what commits each triple, so order and
repetition are the content rather than a list of key/value pairs.

`ini_append_block` takes the raw block and appends it verbatim. The file is now
**byte-identical to the Oracle** at 2181 bytes, up from 1788.

Three details decided it, and all three are about landing a TOML string in a
Windows INI without it looking pasted by a machine:

- **Line endings come from the target file.** This one is CRLF throughout; a
  block appended with bare LFs would put two conventions in one file.
- **Leading blank lines are content.** The gap between the mod's own entries and
  the pasted ones is part of the paste, and the Oracle has two blank lines
  there. TOML eats the newline immediately after the opening delimiter, so the
  modlist carries two visually-blank lines and a comment saying why.
- **One trailing newline is dropped, then the file's shape restored.** A
  multi-line TOML string always ends with a newline nobody typed. This file
  ended mid-line before the append and still does.

The append is idempotent — it matches the whole block, not a key, since these
lines individually recur throughout the file. Nothing in the pipeline re-applies
an action to an already-staged folder today, so that guard is insurance rather
than something the build relies on.

The block itself is checked against the guide's transcription line for line, not
copied off the Oracle.

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

### A4. ~~BAIN Wizard scripts~~ — done, Part 28 row 5 (Configuration Items Begone)

The package ships a `Wizard.txt`. Selecting its two subpackages covers
everything the wizard *installs*; what was missing was
`INI Tweaks/Oscuro's_Oblivion_Overhaul.ini`, 66 bytes, which the wizard
*generates*.

Reading the script settled what the row actually needs. The wizard asks five
questions, and four of them do nothing here:

| question | why it is silent |
|---|---|
| filter patch + LINK++ support | `SelectSubPackage` — already covered |
| Maskar's factions rating scroll | the guide leaves it unselected |
| Basic Primary Needs (dishes, canteens) | gated on `DataFileExists`; not in this list |
| Basic Personal Hygiene (toilet paper) | same |
| **OOO torch hotkey item** | `EditINI` — the one output nothing modelled |

So this never needed a wizard interpreter. It needed the one primitive those
questions are written in, and `ini_tweak` is it.

**`EditINI` does not edit the INI it names.** It writes a fragment into the
package's `INI Tweaks/` folder that Wrye Bash's INI Tweaks tab applies later, on
request — which is why the earlier note read the effect as small. The file is
now **byte-identical to the Oracle**, header line included: the action's job is
to produce the artefact a wizard would have, and every copy of that file in the
wild carries MO2's `; Generated by ...` line.

The folder is staged as `ini tweaks/`, lower-cased like every other directory
mudcrab writes, with the file name left alone.

If the three unfired questions ever matter — a list that installs Basic Primary
Needs, say — they are three more `ini_tweak` lines, not a new feature.

---

## B. Deferred by choice, not by capability

### B1. ~~QAC~~ — run, and every difference explained

26 mods, 27 plugins. **26 are byte-identical to the Oracle's copies**, and the
only one that is not differs in record order alone. The pass
is unattended; `src/config/tools/xedit.rs` says how, and why it drives real
xEdit rather than reimplementing "identical to master".

Five plugins were originally uncleaned on the Oracle side — measured, not
inferred: each copy was byte-for-byte the plugin as it comes out of the mod's
archive. The dirt counts below are xEdit's own, from its log when we cleaned
ours. All five carry `[QAC]` in the guide, so this build followed the guide and
flagged the gap; the Oracle has since been cleaned to match.

| plugin | dirt xEdit found |
|---|---|
| `All Natural - Real Lights.esp` | 1 ITM |
| `The Imperial Water.esp` | 19 ITM |
| `Nobody Goes into the Mountains but Hunters.esp` | 17 ITM |
| `The Lost Spires.esp` | **159 ITM, 433 UDR** |
| `EVE_StockEquipmentReplacer for OOO.esp` | 1 ITM — but in a copy that is now deleted; see below |

`The Lost Spires`' 433 UDRs are the "443 records differ" that looked mysterious
earlier — undeleted references, not ITM removal.

**Two of them took a second pass to get right, and one plugin still differs.**

**`Nobody Goes into the Mountains but Hunters.esp` taught the general rule.**
Two mods ship a plugin of that name — guide row 31 (main file) and row 32 (UL
Compilation Compatible) — and only row 32 carries `[QAC]`. That marker is
load-bearing: row 32 installs later, so its copy is the one the VFS resolves.
Hanging the action off row 31 cleans a file the game never reads and leaves the
one it does read dirty. **A `qac` action belongs on the mod that wins the path,
which is not always the mod that declares the plugin** — the action is now on
the UL-compat mod, which declares no plugins at all, and both mods are
byte-identical to the Oracle.

**`EVE_StockEquipmentReplacer for OOO.esp` was the same shape, unresolved in
the guide itself — and is now settled.** Row 7 (EVE HGEC) carries `[QAC]`; row 8
(Seamless - HGEC Female) ships its own copy of the same plugin and installs
after it, so row 8 wins the path and the guide's `[QAC]` would have landed on a
file the game never loads. The resolution, Steven's call and mirrored on both
sides: **delete** row 7's copy and clean row 8's.

Deleting rather than hiding or shadowing is deliberate. Merge Plugins Hide walks
mod folders rather than the VFS, so a second file of the same name is a hazard
to it whether or not the game ever sees it — and in a declarative list there is
nothing to be gained by keeping a file no tool should find.

xEdit's verdict on row 8's copy: **0 UDR, 0 ITM**, and a LOOT masterlist `clean`
entry at CRC `0xE35796B1`. It was already clean, so the 1 ITM reported earlier
lived only in row 7's copy — which no longer exists on either side. The dirt is
gone rather than left live.

**`ImpeREAL City … Merged.esp` is cosmetic.** Same size, same 8845 records,
same contents; its groups are FormID-sorted in the Oracle and in archive order
here. A full xEdit load-and-save sorts records within a group, QuickAutoClean
does not, and the Oracle's copy went through a manual xEdit session for its
row-6 deletions while ours is edited by mudcrab. Record order inside a group
carries no meaning — the engine indexes by FormID — so this sits with the BSA
payload-ordering difference as a known non-issue.

One further diff line on the QAC'd set is not about cleaning at all: `Harvest
Flora`'s three DLC plugins are `.mohidden` here and plain files in the Oracle,
because Steven unhid them there to re-clean them. Both sides leave them out of
`plugins.txt`, and the four plugins are byte-identical across instances. Hiding
is how mudcrab retires every merge source; the Oracle unticks instead. Same
effective load order. See C1 for the consequence on the Oracle's Prebash.

**Every merge matches the Oracle's record count exactly**: Unique Forts 7912,
OOO Patches 1759, TACE 8533, Prebash 4505, Late Loaders 4361, NPC 2278 — and
the renumbering counts (2004 and 1170) match too.

### B2. ~~LOOT / load order~~ — settled, and the earlier version of this note was wrong

The load order is the modlist's `plugins` array, and Part 37 made that array
identical to the guide's published `loadorder.txt`, entry for entry. `loot-sort`
was never needed.

What the earlier note missed is that **writing `plugins.txt` does not apply a
load order.** Oblivion has no load-order file: the order *is* the plugin files'
modification times, oldest first, and `plugins.txt` records only which plugins
are active. Steven caught it by looking at MO2 and seeing
`YourMotherWasAHamster.esp` at the bottom of the list.

The failure was worse than "the order is ignored". MO2 read the real order off
the timestamps, found it disagreed with a stale `loadorder.txt`, and rewrote
`loadorder.txt` to match the files — so what the profile displayed was the order
the archives happened to be extracted in, and mudcrab's order was actively
overwritten. Checking it is one line: the order MO2 showed was byte-for-byte the
mtime order.

`src/config/mo2/load_order.rs` now puts the order in both places that hold it,
from one list, so there is nothing for MO2 to reconcile:

- `loadorder.txt`, which MO2 reads and displays.
- the plugin files' mtimes, which the game reads. One minute apart from
  2000-01-01, far enough apart that no filesystem's timestamp granularity can
  tie two neighbours.

Three details that are not arbitrary:

- **Every copy of a name is stamped**, not the one MO2 would pick. Two mods can
  ship a plugin of the same name — see B1 — and stamping both means the result
  does not depend on this code agreeing with MO2 about mod priority.
- **Hidden plugins are skipped.** A `.mohidden` merge source is not in the load
  order, and its mtime is part of the merge's input hash: stamping one would
  rebuild every merge on the next run.
- **Mod roots only, not a recursive walk.** MO2 exposes a mod's root as `Data`,
  so a plugin under `optional/` is not in the load order.

`Bashed Patch, 0.esp` is reported as having no file to stamp, which is correct
until Wrye Bash writes it. It stays in the profile: MO2 ignores a name with
nothing behind it, and a staging install with no `--game-dir` cannot see the
base masters either, so absence is not evidence of anything.

## C. Open on the Oracle side

Not mudcrab gaps — things in `MOFAM-03.25` still outstanding as of 2026-08-22.
Most of what was here has been fixed; see section D.

### C1. The Oracle's merges are now built from stale sources

Steven unhid `Harvest Flora`'s three DLC plugins to re-clean them. Those three
are Prebash Merge sources — the only merge any of this round's cleaning touches;
none of the other re-cleaned plugins feeds a merge. So **the Oracle's
`Prebash Merge.esp` predates its own inputs**, and the zMerge GUI issue means it
cannot be rebuilt there.

This costs no verification that was ever available. The Oracle was never a byte
reference for merge *output*: zMerge and mudcrab allocate FormIDs differently
and zMerge retains masters mudcrab drops, so every merge has always differed at
the record level while matching exactly on count. Those counts are unchanged —
Prebash is 4505 on both sides, and every source plugin is byte-identical across
instances.

What it does mean: **the Oracle instance itself should not be played on that
merge**, and a future Oracle-side comparison of `Prebash Merge.esp` proves
nothing. mudcrab rebuilds a merge whenever a source changes — the last full run
reported all six `skipped: inputs unchanged since the last build`, so this
build's merges are current.

### C2. ~~Ultimate Leveling's fifteen edits~~ — applied; one guide-silent value left

Steven applied the nine outstanding edits by hand. `Ultimate Leveling for
advanced users.ini` now differs from ours on **one line only**:

| setting | ours (archive default) | Oracle | guide |
|---|---|---|---|
| `ini_xp_kill_other` | 25 | 10 | not mentioned |

The guide never names this setting, so the build keeps the archive's value.
Standing rule: where the guide and the Oracle disagree, follow the guide and say
so. If 10 is wanted it is one `ini_set` away, but it should be a decision rather
than a diff that quietly gets copied.

### C3. Cosmetic, listed so they are not re-investigated

- `MigTraining.ini` line 65 (`fTrainerAdvancedMult`) is now **split across two
  lines** in the Oracle: the hand-edit broke the line at a tab, so the trailing
  comment became its own `;`-prefixed line. The value the game reads is still
  `1.25`, and both files happen to be 6091 bytes — a tab lost on one line and a
  CRLF gained cancel out. Worth a tidy in the Oracle if it is ever reopened; it
  changes nothing.
- ~~`YourMotherWasAHamster.ini` reads `1.0` in the Oracle and `1` here~~ — now
  identical.

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
- ~~`DLCFrostcrag.esp` had its Worldspace group deleted when only
  `Harvest [Flora] - DLCFrostcrag.esp` should have~~ — re-cleaned by Steven. The
  DLC master is byte-identical on both sides again, so the 71 own records that
  had gone missing (the exterior placements for Frostcrag Spire, plus the LOD
  and OOO-adaptation overrides that pointed at them) are back.
