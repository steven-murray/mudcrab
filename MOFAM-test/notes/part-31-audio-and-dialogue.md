# Part 31 — Audio & Dialogue Improvements

Nineteen rows, eighteen mods. **10 of 18 identical.** Seven differ only by a
Prebash merge source hidden in the Oracle.

## The eighth: another INI the Oracle tuned its own way

`YourMotherWasAHamster.ini`, guide row 15 — "set aaTauntQuest.aaTauntMult to 1":

| | value |
|---|---|
| archive ships | 0.5 |
| guide says | **1** |
| ours | 1 |
| Oracle | **0.85** |

Neither the default nor the guide's number. This is the third row in two
sections where the Oracle's INI settings are the user's own tuning rather than
the guide's — see `part-30-skills-and-levelling.md` for the Ultimate Leveling
and Dynamic Training Cost cases. Worth treating as a pattern now: **where an
INI differs, check the archive's original before assuming either side is
wrong.**

Row 5's edit (`Dialog TFR Costs.ini`, `TrespassDialogRestore` to 0) came out
identical, so the pattern is not universal.

## Row 14 omitted

Swearing Rats. The guide says of it: *"I can understand if you omit it"*, and
the Oracle has no such mod. Omitted here on that reading rather than installed
as an extra. Flagged in report 4 in case you want it after all — the archive is
not on this machine either way.
