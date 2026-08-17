# Report 4 — deferred, because doing it properly needs your call

Nothing here is blocked on effort; each is blocked on a decision that would
change mudcrab's shape. Ordered by when the build actually needs it.

---

## D1. Conflict resolution: derive the file list instead of reading it off the Oracle
**Needed by Part 18. Load-bearing at Part 24.**

Designed in full (`conflict-resolution-design.md`), validated by hand against a
known-correct answer, not built. Three pieces: a staged file index, a
`conflicts_with` selector on `file_prune`/`file_hide`, and resolution to explicit
paths in a lockfile.

The reason it is pinned rather than done: piece one is a **refactor of ~1600
lines of layout code** into a path planner shared with `install`. That is the
right design and it is not something to land unattended.

Interim: Part 9's 1725 paths are recorded and become the test fixture.

## D2. xEdit scripted record deletion
**Needed by Part 11 #23, and again at 26a #6 and 26b #12.**

Part 11 asks for the **Worldspace group** to be removed from
`Harvest [Flora] - DLCFrostcrag.esp`. mudcrab can read and write plugins but has
no way to express "delete this record group". Designing that surface — which
groups, by what selector, with what safety — is a schema decision.

Both affected plugins are Part 36 merge sources, so nothing downstream depends
on them until that merge exists.

## D3. Files beside the data folder
**Cosmetic, four occurrences and counting.**

Unwrapping a `Data/` folder drops root-level siblings (readmes,
`obmm_BSA_settings.jpg`). MO2 keeps them. Always documentation, never anything
the engine reads.

Pinned because the fix changes auto-layout's contract — "the data folder is the
mod root" would become "the data folder plus any loose files beside it" — and
that affects every mod already built, not just the ones that show a diff.

## D4. The glob-semantics asymmetry
**A wart, not a bug.**

`file_prune` now treats `/` as a separator and matches case-insensitively;
archive `include`/`exclude` still use archive-entry semantics, where `*` crosses
separators and case matters. Two different meanings for the same syntax in one
file is a trap.

The honest fix is one semantics everywhere, which means re-verifying every
authored `include`/`exclude` in the modlist. Not something to do unattended
mid-build.

## D5. QAC batch run
**Not structural — just ordering.**

Deferred QACs accumulate: `Cleaned DLC Masters` (Part 1), `EVE HGEC Equipment
Replacer for OOO` (Part 9 #7), `Harvest [Flora] - DLCFrostcrag.esp` and
`Harvest [Flora] - Shivering Isles.esp` (Part 11 #23). Each is a TODO in the
TOML. To be run in one pass at the end, since QAC shells out to xEdit and is
slow.

## D6. LOOT sorting without the GUI
Steven's own words: *"It would be really great to be able to fix this and do
LOOT sorting via CLI but I suppose that's for another time."* `libloot` has a
usable API. Not needed before Part 37, which replaces sorting with a fixed
`loadorder.txt` anyway.

## D7. `cache_file_name` embeds the mod id
Renaming a mod orphans its cached archives, which is how 21 mods silently
vanished during the P2g rename. Known design flaw; the fix is content-addressed
cache keys.
