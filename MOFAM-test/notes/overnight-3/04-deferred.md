# 4. Decisions deferred — these need your call

## D0. The list is at 254 plugins of 255. Nothing more fits.

**This is the blocker, and it is why the run stops at Part 26a.**

Part 26a took the load order to 254. Part 26b adds ~20 more, Part 27 more again.
The next section cannot be built until a merge frees space, which is exactly
what your own plan predicted (SP7 = Part 26a + Unique Forts).

The obvious merge is **Unique Forts**: it folds the ten Better Forts installed
tonight into one plugin, saving nine slots. Its sources are eleven, and ten are
now installed. The eleventh is `MOFAM - UFM Consistency Patch`, from the guide's
own mod page — a **Part 35/36 mod**.

So there are three ways forward and I did not want to pick one for you:

1. **Pull the consistency patches forward.** Install `MOFAM - UFM Consistency
   Patch` out of guide order so Unique Forts can be built complete and matches
   your Oracle's merge exactly. Cleanest result, breaks guide order.
2. **Build merges from available sources**, behind a `--allow-missing-sources`
   flag that does not exist yet. Your plan anticipated this for SP9. The merged
   plugin then differs from your Oracle's and has to be rebuilt at the end
   anyway — and FormIDs shift, so any save made against it is throwaway.
3. **Stop adding plugins**, and build the remaining sections' *assets* only,
   leaving their plugins out of the load order until the merges exist.

My recommendation is **(1)**. The consistency patches are small, they are from
the guide's own page rather than a third party, and they are the difference
between a merge that matches yours and one that does not. Guide order is a
presentation choice; the 255 limit is not.

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
