# 4. Deferred, pending your call

Things I stopped short of, and why. Roughly in the order I would take them.

## D1. Merge-source hides are 59% of all reported differences

**I proposed this and you said move on, so it is recorded rather than done.**

Of the 86 differing mods in the full-list diff, **51 differ by nothing except a
plugin hidden in your Oracle and active here**, because the Part 36 merge exists
there and not yet here.

It is fully derivable — 258 plugins are named as merge sources in
`mofam.merges.toml`, and **all 51 mods have every hidden plugin covered, zero
uncovered**. So `diff` could know this from the modlist alone.

What suppressing them would do:

| section | now | suppressed |
|---|---|---|
| Part 7 | 28/38 | **38/38** |
| Part 19 | 25/32 | 30/32 |
| Part 20 | 23/31 | **31/31** |
| Part 23 | 9/14 | **13/14** |

Part 19's survivor is the QAC difference — a real finding currently one line
among six.

The reason I keep raising it: Part 20's genuine layout bug was 1 of 9 reported
differences. The next real bug will be 1 of 12. This is the same reasoning as
your "documentation differences are not findings" call, with a stronger evidence
base.

**Suggested shape if you want it:** suppress only when all four hold — hidden in
the Oracle, active in ours, named as a source of a declared merge, and that
merge's output not yet built — then print one summary line per section rather
than going silent.

## D2. `inspect` and `install` still answer "where is the content root" separately

Part 20's doubly-wrapped archive is the case where they disagreed and `install`
was wrong. `install` was fixed. **The duplication was not** —
`inspect::guess_layout` still uses its own shallowest-content-root search, and
got that archive right by a different route.

They agree on every archive in the list today. Nothing makes them agree tomorrow.
The layout planner exists precisely so one implementation answers this question,
and `inspect` is the last caller that does not use it. Porting `guess_layout`
onto `plan_archive` is the fix; it is a real refactor, so it is pinned.

## D3. Renaming a mod leaves debris

It cost two folders and 166 MB this run (`Ogorod`, `T4UTXL - Architecture_BETA2 -
City Gates`), which you asked me to delete and I did. `install` writes the profile
from the plan but never removes folders the plan stopped naming.

Same root cause as the cache-key flaw already in the backlog: `cache_file_name`
embeds the mod id, so renaming also orphans the cached archive. One fix could
cover both. It deletes data, so it wants a flag and your say-so.

## D4. Manual archives are matched by exact filename

Thirteen tesall.ru rows in Part 18, five in Part 19, two mediafire and one moddb
in Part 21, one mega.nz in Part 20. All resolved by exact `file_name`, and all
of those filenames were read off archives already in **your** download folder —
which MO2's own config names as the Oracle's `download_directory`.

A first-time user downloads from those hosts and needs the filename to match
exactly. If a host re-uploaded under a different name, they get "must be
downloaded by hand" with no hint that the fix is a rename.

Options, none taken: match by content hash with the filename as a hint; allow
alternates or a glob; or keep exact matching and improve the error — "you have
one unclaimed archive in the search paths, is it this?"

## D5. Two `ini_set` gaps with no caller yet

- Appending a **new** key to a `set-to` file renders it plainly; the file's tab
  alignment is only consulted for the standard format.
- A file ending in two or more blank lines keeps only one.

Both are the same shape as the bugs fixed this run. Neither has been hit.
Building for them now would be machinery without a caller, so they are recorded
instead.

## D6. Still parked from before

- **QAC is commented out list-wide** to keep rebuilds fast, with a TODO to
  re-enable at the end. It is the sole cause of Part 19's `Ducks and Swans.esp`
  difference and the DLC masters'.
- **LOOT is out of the build** — it needs a human to approve its masterlist
  prompt, so an unattended run stalls until timeout.
- **Task #25**, the OOO Enhanced conflict prune and repack, waits for Part 24 to
  bring Colourful Clothing. It will need the `chainmailm1.nif` exception you
  decided on.

## Where I stopped, and why

**Part 24 is next and I deliberately did not start it.** It is where Colourful
Clothing arrives, which unblocks task #25 — the largest single piece of remaining
work, and the first `conflicts_with` row whose count the guide does *not* state,
so the check that has validated the mechanism three times is not available there.
It wants a clear run rather than the tail of one.
