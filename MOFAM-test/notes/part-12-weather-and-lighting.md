# Part 12 (Weather & Lighting)

19 guide rows, 20 Oracle folders (row 15 installs two archives separately).

**Status: 20 compared, 14 identical, 6 differing, all six explained.**

| difference | mods | why |
|---|---|---|
| plugin hidden in the Oracle | 2 | Part 36 merge sources |
| plugin not yet cleaned | 2 | the guide marks both `[QAC]`; deferred |
| guide/Oracle disagreement | 2 | see below — guide followed in both |
| readme + OMOD metadata | 1 | the Oracle dropped them; we keep them |

## All Natural is the most interesting row in the section

Four things have to be true at once, and the guide states them in a sentence
and a parenthetical.

> *"Select 'All Natural - Real Lights ONLY'"* … *"Did you read the Notes? Make
> sure to manually rename the bsa to 'All Natural - Real Lights'!"*

"Real Lights ONLY" is about which **plugin** ends up in the load order, not
which subpackage is installed — `00 Core` carries the BSA and the INI, both of
which Real Lights needs. So: install both subpackages, park Core's three
plugins in `optional/`, and rename the BSA.

**The rename is the load-bearing part.** Oblivion auto-loads a BSA when a
plugin of the same stem is active; the other route in is being named explicitly
in `Oblivion.ini`'s `SArchiveList`, which is how the vanilla archives load and
which mods essentially never touch. Nothing in this section touches it. So once
`All Natural.esp` moves to `optional/`, nothing would load `All Natural.bsa` —
the mod's entire asset payload — and the archive has to take the name of the
plugin that *is* active. A one-line aside decides whether the mod works at all.

Reproduced with four `file_move` actions and no new machinery. Matches the
Oracle exactly.

## Two guide/Oracle disagreements, guide followed in both

### NightSkies Overhaul: the Oracle skipped the fifth subpackage

The guide lists five, ending `05 - OVERLAY - Aurora - 2k`. The Oracle's
`meta.ini` records four, and its folder has no `textures/sky/overlay.dds`. Not
an ambiguity — the guide names the subpackage explicitly. Followed; the extra
file shows in the diff.

### Drifting mist: the guide parks a plugin the Oracle left active

Guide row 4: *"Once installed move drifting mist.esp to the Optional folder."*
The Oracle left it at the mod root.

Row 5 ships a corrected `drifting mist.esp` of its own, which is presumably why
the guide parks row 4's. Both builds behave identically — row 5 has higher
priority, so its copy wins the VFS either way — but the guide's version says so
explicitly rather than relying on priority order. Followed.

## Where the Oracle is inconsistent with itself

Cava Obscura is installed manually (`[MI]`), and the Oracle's folder has neither
`Cava Obscura ReadMe.txt` nor the `omod conversion data/` directory the archive
ships. Part 11's Harvest Flora, also `[MI]`, **kept** its `omod conversion
data/`.

So the Oracle drops installer leftovers sometimes and keeps them other times.
The guide says nothing either way. We keep them, consistently, which is the only
rule a compiler can follow — and it costs four lines in the diff for this mod.

## `[QAC]` markers are worth reading as diff predictions

Both plugin-content differences in this section are rows the guide marks
`[QAC]`: `drifting mist.esp` (row 5) and `Better Rainbows.esp` (row 16), ours
larger by 2173 and 101 bytes respectively — the records cleaning would remove.
Row 1's `All Natural - Real Lights.esp` is also marked and does **not** differ,
which is worth noting rather than glossing: QAC is a no-op on a plugin with
nothing to clean.

All three are TODOs in the TOML for the batch run at the end.

## Version drift

Two archives postdate the March 2025 guide, so "the top file on the page" is not
what the Oracle installed:

| mod | dated | version |
|---|---|---|
| Lights of Oblivion - Road Lanterns | 2025-05-07 | 1.6.0.0 |
| T4UT - Skies Repolished | 2025-03-18 | SecondEdition |

Both matched the Oracle byte-for-byte, so the drift is shared rather than
divergent — but the guide cannot have meant these files, and if either later
turns out to behave differently this is where to look.

## Layouts

Nothing exotic. Four BAIN (All Natural, Falling Leaves, DOWNPOUR,
NightSkies), one FOMOD (`Fantasy Mesh Type`, per the guide's
terse "(Fantasy Mesh)"), one nested data folder (NAO's *"Right click 'Data' &
set as your directory"*), and one `exclude` for Cava Obscura's filter patch —
which the archive spells `Filter Patch For Mods.esp`, capital F, where the guide
writes "for".
