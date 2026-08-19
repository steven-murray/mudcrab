# Second overnight run — start here

**Parts 18 through 23 are built and verified**, plus Part 19's row 9 once you
resolved its file id. 130 mods added, and every difference is accounted for.

| Part | rows | mods | identical | notes file |
|---|---|---|---|---|
| 18 Katkat's Locations | 15 | 16 | **16** | `part-18-katkats-location-retextures.md` |
| 19 Creatures & Animals | 29 | 32 | 25 | `part-19-creature-and-animal-retextures.md` |
| 20 Weapons & Armour | 30 | 31 | 23 | `part-20-weapons-armour-and-clothing.md` |
| 21 Clutter & Misc | 35 | 44 | 37 | `part-21-clutter-and-miscellaneous.md` |
| 22 Effects | 5 | 8 | 6 | `part-22-effects.md` |
| 23 Combat & Magic | 13 | 14 | 9 | `part-23-combat-and-magic.md` |

Full-list state: **328 of 727 compared are byte-for-byte identical**, up from 209
when the run started. 436 tests, clippy clean, working tree clean.

## The four reports

1. `01-oracle-disagreements.md` — where our build and yours differ on purpose
2. `02-guide-problems.md` — where the guide is wrong, ambiguous or incomplete
3. `03-structural-decisions.md` — changes to mudcrab you should know about
4. `04-deferred.md` — what needs your call

## How much to trust this

Every section was reviewed by a separate agent before moving on, and **each of
the five reviews found something real**. Not style notes — a wrong archive
matcher, an ungated provenance check, a doc comment attached to the wrong item,
a misleading CLI output, and one rule that was outright falsified by mods already
sitting in your Oracle. Those are listed in `03-structural-decisions.md` under
"What the reviews caught", because they are the best evidence for how much to
trust everything else here.

**59% of all remaining differences are one thing**: a plugin hidden in your
Oracle because the Part 36 merge consumes it, and active here because that merge
does not exist yet. 51 mods, every one of them fully explained by
`mofam.merges.toml`. I proposed suppressing them and you said move on, so they
are still reported — but it is worth knowing that the signal-to-noise in every
future section's diff is roughly one real finding per twelve reported ones.
