# Rows the build knowingly does not complete

**This is the list to work through before calling the build finished.** Every
entry is a place where mudcrab installs the mod but does *not* do everything the
guide's row asks. Nothing here fails; each one succeeds and leaves the instance
slightly short of the guide, which is exactly why it needs writing down rather
than trusting to a diff that has other things to say.

Kept current as sections are built. Last updated after Part 31.

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

### A2. Scripted record deletion — Parts 11, 26a, 26b

Three rows ask for records to be deleted in xEdit after install.

| row | mod | what to delete |
|---|---|---|
| 26a #6 | ImpeREAL City - Unique Districts | `Light > xx010F43 CityStreetlightWaterfrontDistrict01`, and `Worldspace > 0000003C Tamriel` |
| 26b #12 | People Live Here - Skingrad | `Worldspace > 0001C31D > Block -11, 2`; `0001C31D > Block -1,0 > Sub-Block -2,0 > 0000A7E9 > xx002136`; `0001C31D > Block -1,-1 > Sub-Block -3,-1 > xx0020EE` |
| 11 #23 | Harvest [Flora] - DLCFrostcrag | the Worldspace group (see `TODO(xedit)` in the modlist) |

**Effect if left**: 26a #6 leaves the Waterfront district in the ImpeREAL merge,
which the guide removes for performance. 26b #12 leaves three wild edits that
can fight other mods over the same cells.

**Fix**: mudcrab already has a full TES4 reader/writer and a reference-rewriting
engine — deleting a record by FormID and re-deriving the GRUPs is well within
what the merge code already does. This is the largest missing feature in the
list and the one most worth building.

### A3. BAIN Wizard scripts — Part 28 row 5 (Configuration Items Begone)

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

### B1. QAC — the whole list

30 rows in the guide carry `[QAC]` (Quick Auto Clean). The action exists and
works; it is **commented out list-wide** to keep rebuilds fast, with a
`TODO: uncomment this at the end!` at the top of `mofam.full.toml` and four
`TODO(qac)` markers on specific rows.

**Effect if left**: every `[QAC]` mod's plugin keeps its ITMs and UDRs. This is
the single largest source of "content differs" against the Oracle — Parts 26a
and 26b alone account for 13 of them, each verified byte-identical to what its
archive ships.

**Fix**: uncomment before the final build. Budget the time; it is a plugin-load
per marked row.

### B2. LOOT / load order

`post-install-actions = ["loot-sort"]` is commented out: LOOT opens a GUI that
needs a human, so an unattended run stalls until the 180-second timeout. The
load order is whatever `mofam.full.toml`'s `plugins` array says.

Part 37 replaces this with a fixed `loadorder.txt` anyway, so the interim only
has to be sane, not correct.

---

## C. Fixed — kept for the record

- ~~AFK Weye (Part 28 row 33) repack~~ — done in the Oracle 2026-08-21.
- ~~Urasek (Part 25 row 7) "Replace All"~~ — repacked in the Oracle 2026-08-21.
- ~~Thorn Addon (Part 24 row 7c) plugin deletion~~ — done in the Oracle.
- ~~Part 30/31 INI values~~ — corrected in the Oracle.
