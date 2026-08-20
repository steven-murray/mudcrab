# Part 26a — New & Modified Locations

The guide's largest section: **58 rows**, and the one the plan flagged as
crossing the 255-plugin limit. It does not cross it — it lands exactly on it.
With Part 26a in and no merges built the load order is **255 of 255**, with no
headroom at all.

(Both of those numbers were wrong in my first write-up: I counted the `plugins`
array by lines, and one line holds two entries, so I reported 254; and I counted
guide rows with a pattern that missed row 26, whose number is preceded by a
stray `*` left over from the previous row's italics. Parsed rather than grepped,
it is 58 rows and 255 plugins. The Part 26a review caught both.)

**35 of 59 identical on the first build** — 58 guide rows, plus the split of row 33 into two mods. All 24 differences are explained
below; none is a defect.

## Structure

Only five rows needed anything beyond a plain archive entry:

- **Row 7, The Imperial Waters** — `data_folder = "Files"`, and the guide's
  "disable The Imperial Water - BETTER CITIES.esp" happens in the installer, so
  it is an archive-level `exclude` rather than a post-install action.
- **Rows 25/26/28, Hesu** — three mods out of a fifteen-mod collection in one
  archive, each `data_folder`ed into its own folder.
- **Row 29, Legion Forester Outposts** — "drag these two plugins into 00 Core,
  then set 00 Core as the data directory". Three archive entries over the same
  file, since mudcrab installs a mod's archives in order into one folder; the
  second carries `include` because `01 Diversity Addons` ships three addons and
  the guide takes one.
- **Row 33, Better Dungeons** — "install separately", so two mods, one holding
  the plugin and one the BSA.
- **Row 48, Dagger_Data** — `extract_bsa` twice, then a 26-pattern `file_prune`
  spelling the guide's "delete everything except" as what to delete. Every
  pattern must match, so a folder that stops shipping fails the install rather
  than quietly changing what is kept. 2522 files unpacked, 1274 pruned.

## What the section needed from mudcrab

**Wrapper descent did not apply to archives containing a plugin.** Row 53's
archive wraps its plugins in `SIUnmarkedLocations [updated]/` — a folder named
after neither the mod nor `Data`, so none of the four canonical plugin roots
reached it and the install failed outright. Wrapper descent already handled this
shape, but only for archives with no plugin at all: plugin classification ran
first and bailed. It now falls back to the descent, and accepts it when a single
unambiguous wrapper holds every plugin. Sibling wrappers each holding a plugin
are still rejected, which is what that check is really for.

## Oracle disagreements

None. Every difference is one of two known, already-accepted classes.

## The 24 differences

**18 mods differ only by a hidden plugin** — all Part 36 merge sources, hidden
in the Oracle because those merges are built there and not here yet: the ten
Better Forts (Unique Forts), the three Reworked Posts village patches, the two
guild compatibility patches, Legion Forester's two addons, the LFO OCO eye
addon, and the Dispensation Add Some Flavor patch.

**6 mods differ by one plugin's bytes**, and all six are `[QAC]` rows — the
guide's Quick Auto Clean pass, commented out list-wide to keep rebuilds fast.
Each of ours was verified byte-identical to what the archive itself ships, so
the difference is entirely the Oracle's cleaning:

| mod | ours | Oracle |
|---|---|---|
| Better Odiil Farm | 77561 | 77474 |
| Cheydinhal Cemetery Overhaul | 12390 | 15046 |
| Glowing Stones | 17945 | 17866 |
| Gogan's Family Cemetery | 68575 | 50293 |
| ImpeREAL City - Unique Districts | 694440 | 655698 |
| Nobody Goes... - UL compat | 744915 | 579390 |

Cheydinhal is the one where the Oracle's file is *larger*; a QAC save rewrites
the whole plugin, so growth is as unremarkable as shrinkage.

The seventh `[QAC]` row, The Imperial Waters, is byte-identical — nothing there
for the pass to clean.

## Problems with the guide

1. **Numbering.** 27 and 45 are absent; 19 is printed as 29, so **29 appears
   twice**; there are two rows labelled 47a; and row 26's number carries a
   stray `*` from the previous row's unclosed italics, which is enough to hide
   it from anything scanning for a leading digit.
2. **Row 26 names the wrong folder.** "HESU Skyrim Temple v1.2"; the archive
   calls it "HESU The Skyrim Temple v1.2".
3. **Row 46 names the wrong mod.** The page is "Glowing Stones", the archive and
   plugin are "Glowing Wonders".
4. **Row 6 asks for xEdit record deletions** — a Light and the Tamriel
   worldspace override, to drop the Waterfront district. mudcrab has no scripted
   record deletion, so this is **not applied**; it is the section's one
   knowingly-incomplete row. See report 4.

## Version drift

Six of the Oracle's archives postdate the guide, all flagged by `diff`:
Improved Fighters Guild ENG (2025-06-13), the Improved Mages Guild patch
(2025-05-18), and four Unique Landscapes rebuilds (all 2025-12-28). The guide
says "top file on the page" for these, so a newer file is the instruction being
followed rather than drift — but the FormIDs and cell edits are not guaranteed
to match March 2025's, so it is worth knowing which six they are.

Four rows are UNKNOWN AGE: the three Hesu mods and Bruma Guild Reconstructed,
all hosted on afkmods with no Nexus file id to date them by.

## SP7 — the Unique Forts merge

Part 26a filled the load order exactly (255 of 255), so the next section could
not be authored at all until a merge freed space. The guide's Part 36 rows 3 and 4 are the
consistency patch and the merge that consumes it, and the plan's SP7 puts them
here for exactly this reason — the patch is installed *as part of* building the
merge, so nothing is pulled out of order beyond the merge itself.

Eleven plugins in, one out: **255 → 246**.

    merge: built  sources=11  records=7912  groups=319  remapped=2004
                  clobbered=56  hidden=11

Those are the same counts as the build verified in `merge-verification.md`
(TES4Edit: 7913 records, 0 errors; Fort Naso walked in game).

`Unique Forts Merged.esp` is 787647 bytes against zMerge's 785730. That is
serialisation, not content: `tests/merge_oracle.rs::merges_unique_forts`
compares the two record-for-record and edge-for-edge and passes. The `map.json`
one-byte difference and the `merge.json` / `fidCache.json` / `mudcrab-merge.json`
asymmetry are the two tools' own bookkeeping files.

### What it needed from the validator

Two rules were in direct contradiction: a mod must declare the plugins it ships
and every declared plugin must be in the load order, but a merge with
`hide_sources` requires its sources **out** of the load order. Authoring the
merge made both fire at once.

Both rules are right. `validate` now collects the plugins some merge swallows
before checking the mods, so a mod can declare a plugin a merge consumes — it
really does ship it, and that is worth recording — while a plugin *no* merge
consumes is still required to be in the load order, which is the silent failure
the original rule exists to catch.
