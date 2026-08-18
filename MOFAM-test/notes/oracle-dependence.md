# Where this build leans on the Oracle

A running audit, opened after the Part 18 review, of every place mudcrab's
MOFAM list produced the right answer **only because the Oracle was there to look
at**. A real user of mudcrab has no Oracle. Anything listed here is a place the
modlist is not yet self-sufficient, and each entry should end with what a real
user would have to do instead.

Kept separate from the per-section notes because it is a property of the whole
list, and because it is easy to fix a section and forget the dependency it left
behind. Each section's review asks for this explicitly.

## 1. Manual (non-Nexus) archive filenames — **structural, unresolved**

*Found in the Part 18 review; applies list-wide.*

Thirteen guide rows in Part 18 are hosted on tesall.ru and marked `[MI]`. They
are declared as `manual:<filename>` with an exact `file_name`, and `download`
resolves them by looking for that exact name in the archive search paths.

The dependency: **those filenames were read off archives that were already on
disk**, in
`~/Games/mod-organizer-2-oblivion/modorganizer2/downloads`. That folder is not
neutral — the Oracle's own `ModOrganizer.ini` sets it as its `download_directory`,
so it *is* the Oracle's download stash, accumulated over years. The Part 18
archives were all timestamped 2026-01-24, seven months before the section was
authored. They were never re-fetched from tesall.ru to confirm what a download
today actually produces.

So a real user following the guide would:

1. visit each tesall.ru page by hand (the guide gives the URLs, and they are
   recorded in the TOML comments, so this part is fine),
2. download whatever the site serves them,
3. and need that file to be named *exactly* what the plan says.

If tesall.ru has since re-uploaded any of these under a different name — very
plausible for a Russian mod-hosting site over a decade — the user gets "archive
must be downloaded by hand" with no hint that the fix is to rename their
download. The error names the file it wanted and the paths it searched, which is
better than nothing, but it does not say "you have this archive, under a
different name".

**Options, none taken yet** (a structural decision, so pinned for Steven):

- match manual archives by content hash rather than filename, with the filename
  as a hint. Robust, and the hash is knowable only after someone has downloaded
  it once — so it moves the problem rather than solving it for a first-time user.
- allow `file_name` to be a small set of alternates, or a glob.
- keep exact matching but improve the error: if exactly one archive in the search
  paths is unclaimed by any other mod, say so.

None of this blocks the build. It is recorded because "the archives were already
there" is availability, not reproducibility, and the notes had been stating it as
if it were the latter.

## 2. Mod folder names — **by design, but worth naming**

Mod ids in the list are chosen to match the Oracle's folder names (task P2g did
this deliberately, so `diff` can pair them without a mapping table). Part 18's
ids — `basementsections`, `wayshrine`, `VilverinFlora`, `KatKat's Vegetable
Garden` — are the Oracle's spellings, not anything derivable from the guide or
the archives.

This is harmless: a mod's folder name affects nothing but the MO2 display and
mudcrab's own bookkeeping. A real user would get whatever names they chose, and
the build would be identical. Recorded so it is not mistaken for a real
dependency later.

`oracle_name` exists for the cases where our id and the Oracle's folder
deliberately differ, and using it more widely would remove even this.

## 3. Nexus file ids — **the largest dependency in the list**

Rows pinned as `nexus:oblivion/<mod>/<file>` name an exact file id, and those ids
come from the Oracle's `meta.ini` `[installedFiles]`. Not *some* of them: the
Part 19 review spot-checked ten of that section's twenty-six Nexus rows and every
one matched the Oracle's recorded file id exactly. Treat it as universal.

The guide usually says only "the top file on the page", so a real user in March
2025 and a real user today would get different files.

`diff`'s guide-age check is the mitigation: any archive postdating the guide is
flagged POST-GUIDE rather than silently accepted, and those flags are read every
section. But the *pinning itself* — knowing which file id the guide meant — came
from the Oracle for most rows.

This one is genuinely load-bearing and has no clean answer: a guide that says
"the top file" is not reproducible, by construction. Recorded as a property of
the guide, not a defect in the list.

### The harder case: when "which file" is not a version question

Part 19 row 22 is the shape that has no fallback at all. The guide says
*"Coop's Mudcrab Remake (1st main file)"*. That page hosts **two main files that
are not versions of each other** — one for MOO users and one for people without
it. "1st" is a position in a list that Nexus is free to reorder, naming a
functional choice the guide never states.

A version drift a user can at least reason about: the guide meant an older file,
here is a newer one, and `diff` flags it. This is not that. A user who picks the
other one gets a working install of the wrong mod, and nothing anywhere says so.

The only honest fixes are outside mudcrab: read the mod page and record *which*
file the guide meant, in the row's comment, in words. Done for row 22 as of this
entry. Worth doing for every row whose selector is a position rather than a name
-- "1st main file", "main file only", "optional file only" -- since those are
ambiguous exactly when the page has more than one of the kind.

## 4. Install order within a section — **derivable, but checked against the Oracle**

Section order is taken from the guide's numbered rows, which is correct and
self-sufficient. But when the guide is ambiguous — a row installed twice, two
mods from one page — the Oracle's `modlist.txt` order was used to confirm.
Part 18 row 3 is an example: the guide says "Install 'Base Metal' then
'Unofficial Oblivion Patch Meshes' separately", which is explicit, so no
dependency there. Other sections may be less clear.

Flagged as a thing to watch rather than a known problem.
