# Part 30 — Skills & Levelling

Fifteen rows. **8 of 15 identical.** Three differ only by a Prebash merge
source hidden in the Oracle. The other four are INI files, and three of those
turned out to be real disagreements rather than formatting.

Each was checked against the **archive's pristine INI**, not just against the
Oracle, so "who changed what" is settled rather than inferred.

## 30.1 The Oracle did not apply two of the guide's Ultimate Leveling edits

Guide row 1 lists fifteen `set` edits. Two of them are not in the Oracle:

| setting | archive ships | guide says | ours | Oracle |
|---|---|---|---|---|
| `ULVL.ini_xp_level_base` | 3000 | **1000** | 1000 | 3000 |
| `ULVL.ini_xp_level_mult` | 500 | **400** | 400 | 500 |

The Oracle still has the archive's values, so the edits were never made. These
two are the levelling curve itself — the guide's own prose explains the intent
("by level 20 you'll need ~20k XP, at level 30, 30k") and that arithmetic only
works with 1000/400.

## 30.2 And it applied three the guide never asked for

| setting | archive ships | ours | Oracle |
|---|---|---|---|
| `ULVL.ini_xp_kill_companion` | 50 | 50 | **30** |
| `ULVL.ini_xp_kill_pet` | 50 | 50 | **25** |
| `ULVL.ini_xp_kill_follower` | 50 | 50 | **25** |

Not in the guide at all. Deliberate tuning, presumably — but it means the
Oracle's levelling is not the guide's levelling in five places.

## 30.3 Dynamic Training Cost: one guide edit missing, one lost tab

Guide row 9 sets five parameters to 1. `bDisplaySkillNumbers` is still **0** in
the Oracle — the archive's value — so that edit was not applied. Ours sets it.

Separately, the Oracle's line 65 (`fTrainerAdvancedMult`, which no instruction
touches) has **one tab where the archive has two**. Ours preserves the
archive's whitespace exactly; the Oracle's file lost a character to hand
editing. Ours is the faithful one.

## 30.4 AULIAS: cosmetic

Guide row 10: "change Set AULIAS.FotMCostMult to 0". Ours writes `0`, the
Oracle wrote `0.0`. Same number.

## Not applied

Row 7 asks for nine lines to be pasted into `Custom Trainers.ini` — three
`set`/`set`/`SetStage` triples registering custom trainers. That is a raw block
append, not a key/value edit, and mudcrab has no action for it. This is the
`ini_append_block` gap the plan lists against "Part 30 #7", and it is the whole
393-byte difference in that file.
