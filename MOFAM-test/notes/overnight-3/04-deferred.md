# 4. Decisions deferred — these need your call

## D0. The 255-plugin ceiling — resolved by following the plan, but check me

Part 26a took the load order to **254 of Oblivion's 255**. Nothing else fitted.

I did not defer this, because on reading the guide it turned out not to be a
decision. The obvious merge — Unique Forts, which folds the ten Better Forts
installed tonight into one plugin — needs an eleventh source, the UFM
Consistency Patch. I first read that as a Part 35/36 mod that would have to be
pulled forward out of order. It is not: **the guide's Part 36 row 3 is the
consistency patch and row 4 is the merge that eats it**, so the patch arrives
*with* the merge. Building it early is the plan's own SP7 and nothing else
moves.

Done: **254 → 245**. The merge reports the same counts as the build you and I
verified by hand in August (7912 records, 2004 remapped, 56 clobbered; TES4Edit
0 errors; Fort Naso walked), and the oracle test still passes.

**What I want you to check**: that building a Part 36 merge at this point,
rather than at the end, is what you want. The cost is that the merge will need
rebuilding once the rest of the list exists — FormIDs shift, so any save made
against it now is throwaway, which your plan already says. The benefit is that
the build can continue at all.

The same reasoning applies to the next section. Part 26b's twenty rows are
**exactly** the TACE merge's twenty sources, so 26b + TACE is a net **+1**
plugin, not +20. That is SP8, and it is the natural next step.

## D1. Merge-source hides are now 86 of 136 differences

63% of every remaining difference is one thing: a plugin hidden in your Oracle
because a Part 36 merge consumes it, active here because that merge does not
exist yet. All 86 are derivable from `mofam.merges.toml`.

I proposed suppressing them last time and you said move on. Restating it only
because the ratio has got worse — the signal-to-noise in a section diff is now
about one real finding per fourteen reported ones, and Part 26a alone
contributed 18.

## D2. `mudcrab`'s dummy plugins differ from yours by 54 bytes

Five mods differ only in their `create_dummy_plugin` output: ours 139 bytes,
yours 85. The difference is an author record (`mudcrab` vs `nmcdyer`) and a
description — *"Dummy plugin so Oblivion loads the matching BSA"* — which ours
writes and yours does not. Same header, same master, no records either side.

Keep the description (it tells anyone opening the file in TES4Edit why it
exists) and accept the five differences, or drop it to match? I lean keep.

## D3. Part 26a row 6's xEdit record deletions are not applied

The guide asks for two records to be deleted from ImpeREAL City - Unique
Districts in xEdit — a Light and the Tamriel worldspace override — to drop the
Waterfront district. mudcrab has no scripted record deletion, so **this row is
knowingly incomplete**. It is the only such row in the section.

It is also on the backlog for Parts 11 #23, 26a #6, 26b #12. Worth building, or
worth doing by hand at the end?

## D4. QAC is still commented out list-wide

Six of Part 26a's 24 differences are QAC rows, and that class now spans the
whole list. Each of ours was verified byte-identical to what the archive ships,
so we know exactly what the difference is — but it will keep growing, and the
final build needs the pass run. There is a TODO at the top of `mofam.full.toml`.

## Carried over, unchanged

- **`inspect::guess_layout` duplicates the layout detection** it is supposed to
  describe, so its advice can disagree with what install does. It did not bite
  tonight, but Part 26a's Arena Champion's Villa is a case where its guess and
  the install path differ.
- **Renaming a mod leaves a stale folder** and orphans its cached archive.
- **The archive-glob/staged-glob case-sensitivity asymmetry**: archive-entry
  patterns are case-sensitive, staged-tree patterns are not. It cost a rebuild
  in Part 24 (`Thumbs.db` vs `thumbs.db`).
- **Should mudcrab strip `Thumbs.db` globally?** MO2 does. One is still sitting
  in OOO Enhanced.
- **LOOT is out of the build** and `post-install-actions` is commented out.
