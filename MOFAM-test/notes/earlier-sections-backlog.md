# Parts 2, 3, 4, 6: differences never checked

Parts 1-4 and 6 were built before `mudcrab diff` existed, so they were never
compared against the Oracle. The first full-instance diff (2026-08-16) found 37
differing mods across those sections. Two were broken installs and are fixed;
the rest are triaged here and none is urgent.

This is the cost of building a section without a way to verify it. Every
section from Part 5 onward is diffed as it is authored.

## Fixed

- **`Unofficial Oblivion Tree Patch - UOTP`** (114 files) and **`Unofficial
  Shivering Isles Tree Patch - USITP`** (26 files) installed two levels too
  deep. Both archives wrap their content in `<mod name>/Data/`, and `trees/` is
  not a folder name the auto layout recognises as game content, so nothing
  unwrapped them. Every tree in both patches was inert. Now byte-identical to
  the Oracle.

  Worth considering: adding `trees` to the auto layout's known content-folder
  list would have caught this without an override.

- **21 mods were installed under pre-rename ids and never reinstalled.** The
  P2g id rename changed each mod's cache key, because the key is
  `{mod_id}_{archive_index}_{fileid}` -- so the archives were orphaned and the
  mods silently absent from the profile. All 21 now carry `file_name`, which is
  what `--archive-search-path` matches on, and all resolve offline.

- **A stray word in a BAIN subpackage name** (`"Vanilla Style Loading Screens
  Addon derpy"`) made the whole install abort. Latent since Part 6 was authored,
  because that section was never reinstalled.

## Still open, in rough priority order

### Layout / content, worth a look

| mod | difference | likely cause |
|---|---|---|
| `Better Enemy Health` | 8 `fonts/*` only in ours, 1 content differs | FOMOD selection differs from the Oracle's |
| `Loot Feed` | 8 `fonts/*` only in ours | same |
| `Loot Menu` | 4 files differ | not investigated |
| `Dynamic Map` | 2 files differ | not investigated |

### INI content — probably guide edits not yet applied

`AveSithis Engine Fixes`, `Oblivion Display Tweaks`, `Extended UI`,
`Follower Status`, `Map Marker Overhaul`, `Marking the Landmarks`,
`QZ Easy Menus Update`, `Better Letters`, `Migck's Miscellaneous fixes`.

Each differs by one file, usually an `.ini`. The guide specifies INI edits for
several of these and `ini_set` is not yet wired up for them. Note GAP-009:
`ini_set` is not section-aware, which must be fixed before trusting any of it.

### Deliberate, no action

- **`Cleaned DLC Masters`** — 9 plugins differ in size because the `qac` action
  is commented out in the modlist to save rebuild time. The TOML says so, with a
  TODO to re-enable at the end. Ours are uncleaned; the Oracle's are cleaned.
- **`xOBSE`** — extra in ours by design; the Oracle has no equivalent folder.
- **Construction Set DLLs** excluded on purpose: `AddActorValues_CS.dll`,
  `OBME_CS.dll`.
- **Readmes and docs** dropped when a `Data/` wrapper is unwrapped:
  `EngineBugFixes`, `MessageLogger`, `Walk through Oblivion Gates`,
  `DarnifiedUI FOMOD Conversion`. Same class as OCOv2's readme in Part 7.

### Hidden plugins — expected until Part 36

14 mods differ only in a plugin hidden in the Oracle and active here:
`Bibliophilia`, `Collection of Cleaned - Updated - Fixed - UOP Compatible`,
`Goblin Tribes Fixed`, `Guard Infamy Greeting Fix`, `Imperial City Landscape
Fix`, `Locked Fighters Guild Doors Bug Fix`, `Minotaur Horn Drop Fix`,
`No Annoying Conjurer Attack`, `Thieves Den Barter for Upgrades`, `UOP Talos
Bridge Collision Fix`, `Vile Lair DLC - Tweaks and Fixes`, `Icons for Alchemy
Apparatus`, `Vanilla Style Loading Screens Addon`, plus `UODP` and `UOP`
(`DLCSpellTomes - Unofficial Patch.esp`, `Oblivion Citadel Door Fix.esp`).

Same pattern as Part 7: the Oracle hides them because their merge exists there.
Each should be confirmed as a merge source before being accepted, exactly as
Part 7's were.

## The cache-key design flaw

`cache_file_name` (`src/config/download.rs`) is
`{mod_id}_{archive_index}_{fileid}`. Two consequences, both real:

1. Renaming a mod orphans its cached archive, which is what hid the 21 mods
   above.
2. Two mods sourcing the same Nexus file cache it twice. Part 5's Xerus entry
   lists the same archive twice already.

The key should be a function of the source, not of the mod id. Not changed yet
because re-keying orphans the ~1000-file cache and forces a full re-download.
`file_name` plus `--archive-search-path` works around it completely, so this is
a cleanup, not a blocker -- but every entry needs `file_name` for that to hold,
and any new mod without one is exposed to the same failure.
