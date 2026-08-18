# Part 19 — Creature & Animal Retextures

29 guide rows, 31 mods. **25 of 31 identical against the Oracle; all six
differences explained**, and none of them is a mistake in this section.

The row count is worth noting: a first read of the guide stopped at row 20,
because rows 21–29 sit below a long block of prose about Coop's mods and a
FOMOD answer list. The Oracle's 31 folders are what caught it.

## The six differences

### Five merge-source plugins (expected until Part 36)

`Better minotaurs.esp`, `KingofMisc.esp`, `BattlehornLich.esp`,
`BetterLorgrenBenirus_NoStaffEdit.esp`, and Coop's two —
`CoopArmoredLegionHorses.esp` and `DLCHorseArmor - Mane Enabled.esp` — are
hidden in the Oracle and active here. Six plugins across five mods.

Same pattern as Parts 7 and 8: the Oracle hides them because the Part 36 merge
exists there and consumes them. Confirmed by the load order — none of the six is
in the Oracle's `loadorder.txt`, while the section's three non-merge plugins
(`Ducks and Swans.esp`, `BettysButterflies.esp`, `Simple Horse Utilities.esp`)
all are.

### One QAC difference

`Ducks and Swans.esp` — ours 60562 B, the Oracle's 50084 B. Guide row 16 is
marked `[QAC]`, and the `qac` action is commented out list-wide to keep rebuilds
fast, with a TODO at the top of the modlist to re-enable it at the end. Ours is
uncleaned, the Oracle's is cleaned; the direction of the size difference agrees.
Identical in kind to the `Cleaned DLC Masters` entry in the backlog.

## Guide row 9 could not be built

*Beautiful Creatures - Spider Daedra* (mod 43297, "main file only") is **not in
the Oracle and its archive is not in the downloads folder** — so unlike Part
18's vegetable garden, this one was never fetched at all. Building it needs a
Nexus download, which needs Steven's API key.

The entry is written out in the modlist, commented, ready to uncomment once the
archive exists. Until then this is a row the guide asks for and neither install
has.

## Where the guide and the installers disagree

- **Row 20**, Coop's TW3 Oblivion Horse Replacer: the guide answers
  `ArmoredManeFix: MergeablePatch`; the FOMOD spells the option
  **`MergablePatch`**, no `e`. Written the installer's way, or the selection
  would not resolve.
- **Row 20** again: the guide's title is the installer's, but the file is
  `Coop's Roach Horse Replacer`. Only confusing if you go looking on Nexus.
- **Rows 23/24**: the Oracle's folder for row 23 is `Coop's MOO and Vanilla Wolf
  Remesh` although the file it installed is the *Vanilla* one. Kept, since the
  id only has to pair with the Oracle.

`KlenPatch: None` and `Horns: None` are declared as empty selections rather than
omitted. Not the same thing: both groups default to picking something, so
leaving them out would install what the guide is declining.

## Small things

- **Row 6** ships an alternative alongside the default — "delete the Alt Ghost
  Texture (Rags) folder" — which is a `file_prune`. Leaving it would put a
  second ghost texture into the VFS.
- **Row 10** and **row 17** are `data_folder` rows, and auto-detection correctly
  refuses both: each archive has two sibling folders that look like data
  folders, and choosing between them is the guide's job, not a heuristic's.
  Row 17 is one archive installed twice, into `ducks/data` and `swans/data`.
- **Rows 1, 5, 7, 15** are marked `[MI]` but hosted on Nexus. There "manual
  install" means "do not run the installer", which is what mudcrab does anyway.
- **Rows 25–29** are tesall.ru and use `manual:`, like Part 18's. See
  `oracle-dependence.md` for why "the archives were already there" is not the
  same as reproducible.

## A `diff` bug this section found

`Mehrunes Dagon Retex` was reported POST-GUIDE — "the Oracle's archive is newer
than the March 2025 guide". It is not: its Nexus file id is **54124**, an
old-scheme upload from around 2011. What misled `diff` was
`nexusLastModified=2026-01-26`, which is when the file was *downloaded to this
machine*, not when it was published.

Eight mods across the whole list were being flagged this way. `classify_guide_age`
now treats a file id below 1,000,000 as decisive: Nexus allocates current-scheme
ids from 1,000,000,000 up, so a short id predates the guide by the better part of
a decade whatever the meta.ini says. A timestamped filename still wins over both,
and a modern id still defers to `nexusLastModified` — so real drift, like row
19's Simple Horse Utilities at 2025-05-17, still reports.

That leaves exactly one POST-GUIDE flag in this section, and it is a real one.
