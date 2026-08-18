# Where the build is

Rolling status. Update it when a section moves; it exists so a fresh context
can pick up without re-reading the transcript.

Last updated: 2026-08-18.

## Done

- **MO2 profile**: mudcrab owns `Default` in MudCrab Test, set by `PROFILE` in
  `run-full.sh` (override with `MOFAM_PROFILE`). `install` rewrites that
  profile's `modlist.txt` and `plugins.txt` every run. MO2 auto-adds new mod
  folders to *other* profiles as disabled, so any other profile will look
  mostly switched off -- that is MO2, not a build failure.
- **Tooling (plan Phase 2) is complete.** `add`, `diff`, `inspect`,
  `--section`/`--only` filters, `--archive-search-path`, `install --force`,
  `add --before-mod`, `inspect` of a `.bsa`, native BSA reader/writer,
  `pack_bsa` (with `prune_packed`) / `extract_bsa` / `create_dummy_plugin` /
  `file_prune` / `file_hide` / `file_move`, and `identify` (Nexus descriptor
  from an archive's MD5, with a file-list fallback for OLD FILES downloads).
  367 tests, clippy clean.
- **Merges**: all six reproduce zMerge semantically. Unique Forts and TACE
  verified in game. Prebash rebuilt without `ORC.esp` for the Oracle.
- **Parts 1, 2, 3, 4, 6** authored (105 mods, pre-existing).

## Done: Part 5 (LOD)

All ten rows authored, installed and diffed. **9 of 10 byte-for-byte identical
against the Oracle; the tenth differs only in the two files mudcrab generates
(a BSA and a dummy plugin), for reasons written up in `part-05-lod.md`.**

Two silent bugs surfaced doing it, both now fixed and tested: `file_prune`
matched nothing for a bare directory name (and said nothing about it), and
`pack_bsa` wrote zero asset-kind flags, which makes an archive invisible to the
engine. Neither was visible from the install -- only from the diff.

## Done: Part 7 (Character & NPCs)

38 mods (33 guide rows; seven say "install separately"). **26 identical, 12
differing, all explained** in `part-07-character-and-npcs.md`:

- **10** are one plugin each, hidden in the Oracle and active here, and every
  one is a Part 36 merge source. Expected until the merge exists. The
  correspondence is exact -- of the 13 plugins the Oracle hides in this
  section, precisely the 3 that are *not* merge sources are the 3 the guide
  gives an explicit hide/delete instruction for.
- **1** is a **known Oracle error**, confirmed by the user: row 21 removed the
  standard Khajiit head instead of the Nuska variant. Our build follows the
  guide and is correct; those four lines stay in `diff` until the Oracle is
  repaired by hand.
- **1** is a readme sitting outside the archive's `Data/` folder.

Added `file_hide` (MO2's rename-to-`.mohidden`, for the guide's constant "hide
or delete"), and fixed `diff` being blind to hiding -- it stripped `.mohidden`
from filenames only, and never reported a hidden-state mismatch at all. Without
that fix this section would have reported 37 of 38 identical and been wrong.

## Done: Part 8 (Overhauls: Maskar)

11 mods (10 guide rows). **8 identical, 3 differing, all explained** in
`part-08-overhauls-maskar.md`. Two are Part 36 merge-source plugin hides.

The third is a deliberate divergence: the guide pins MOO to **OLD FILES
4.9.4.2** and the Oracle runs 5.0.5. Built 4.9.4.2, per the user. The deciding
evidence was that 5.x dropped `ini_levelscaling_npc_overridden`, so one of the
guide's 37 INI settings would silently do nothing there; 4.9.4.2 has all 37.
This mod now diffs wholesale against the Oracle, like ORC.

All 37 MOO settings were verified present in 4.9.4.2 before authoring and read
back from the staged INI after install.

## Done: Part 9 (Overhauls: Oscuro) — 13 mods

Both download blockers cleared. See `part-09-overhauls-oscuro.md`; the section
is worth reading in full because most of what it cost was learning, not typing.

- Rows 1+3 are the **combine/repack archetype** the plan predicted: one mod from
  two archives, `extract_bsa` then `pack_bsa --prune_packed`. No BAE, no BSArch,
  no WINE. 4406 + 1554 = the Oracle's 5960 files in its 721 folders.
- **OOO Enhanced is pinned to the guide's pair**, 5.3 PreRelease
  (`47187/1000040942`) + 5.3b Resources (`47187/1000041194`). 5.33 has been
  pulled from Nexus. `--from-oracle` had given 5.33 for the plugin half, which
  is how the halves ended up mismatched — a reminder that it is a scaffold for
  provenance, not an authority on which file the guide meant.
- **EVE and Seamless are both C-Cup**, from the BAIN build. The Oracle has been
  reinstalled to match, so the guide has now won both disagreements it has had
  with the Oracle.
- **The deferred conflict-hiding is specified and now verified**: **1725**
  conflict paths in `ooo-enhanced-conflict-hidden-files.txt`, plus 13
  `thumbs.db` listed separately. The follow-up after Part 24 is a `file_prune`
  of those, then `pack_bsa --prune_packed`. The guide's "Enable Parsing of
  Archives" line is load-bearing: WAC ships its assets inside a BSA, and
  without it two of the three conflict categories look like they do not apply.
- **The first cut of that list was wrong three ways** — claimed 1024, held 247,
  and covered textures only when 701 of the removals are meshes. Corrected in
  Part 10 by intersecting the two installs; see `part-09-overhauls-oscuro.md`.
- **…and reading it off the Oracle is a stopgap, not the mechanism.** A real
  build has no Oracle. Because the modlist is declarative, the list is
  derivable from the upcoming mods before anything installs — designed in
  `conflict-resolution-design.md`, to be built by Part 18. Part 10 ran the
  algorithm by hand and it accounts for all 1738 removals exactly, so the list
  is now a test fixture with a known-correct expected output.

## Part 10 (Overhauls: WAC) — complete

5 mods, every file byte-for-byte identical; the 3 reported as differing carry
only Part 36 merge-source plugin hides. Details in `part-10-overhauls-wac.md`.
Two latent bugs fixed on the way (`layout = "simple"` was never honoured; `diff`
flagged any non-Nexus mod as POST-GUIDE), and `manual:` was added for the list's
first archive that nothing can fetch.

**Before playing the Oracle**: its `OOO Enhanced.bsa` stores every path as
`textures\textures\…`, so all 3580 textures in it are unreachable by the game.
See `part-09-overhauls-oscuro.md`.

## Part 11 (Baseline Textures) — complete

23 mods, 13 identical, 10 differing and all ten explained: five BSA-packed mods
(identical contents, BSArch dedup on size, plus the deliberate dummy-plugin
difference from Part 5), three Part 36 merge-source hides, one deferred QAC, and
five documentation files that sit outside a mod's data folder. Details in
`part-11-baseline-textures.md`.

Guide rows 1-5 have no mod of their own — they are one mod, `OUT Essentials`,
built from five archives. The Oracle's five folders hold nothing but a meta.ini,
the guide's own optional cleanup, so `diff` reporting them missing is correct.

Two `file_prune` bugs found, in opposite directions: `NoMushroomStalks` matched
nothing (staged directories are lowercased) and `textures/rocks/*.dds` matched
too much (globset's `*` crosses `/`, so it deleted the `underwater` folder the
guide says to keep). The second was silent — only the Oracle diff caught it.
Both fixed by giving `file_prune` staged-tree glob semantics; archive extraction
filters are deliberately unchanged.

`ini_set` is now section-aware, which was the plan's flagged latent bug.

## Parts 12-17 — complete

Built during the overnight run of 2026-08-17/18. Per-part notes in
`part-12-*.md` through `part-17-awls.md`; the four reports written for Steven
are in `notes/overnight/`, starting with `00-READ-THIS-FIRST.md`.

Current standing after his decisions were applied:

| part | result |
|---|---|
| 12 Weather & Lighting | 20 mods, 16 identical |
| 13 Oblivion Realm | 6 of 6 identical |
| 14 ENB & ORC | 3 built, 2 identical; ENB rows pinned |
| 15 Interior Retextures | 13 of 13 identical |
| 16 Town & City | 36 mods, 34 identical |
| 17 AWLS | 5 mods, 4 identical |

What remains differing across all of them is merge sources hidden in the Oracle,
BSA payload dedup, deferred QACs, and AWLS's installer answers.

## Two bugs worth remembering

**The profile INI was reset on every install.** `prepare_mo2_profile` copied the
game's Oblivion.ini over the profile's copy at the start of every run, so a
section-by-section build kept only the most recently built section's settings.
Six of eighteen were wrong on disk when found, including all three DarNified
font paths. Written up in `ini-clobber-bug.md`. **`diff` compares mod folders,
so it cannot see INI edits** — that whole class of instruction needs checking by
hand or by the audit script in that note.

**Extraction used to happen in `/tmp`, which is a tmpfs.** Unpacking one 6.5 GB
archive twice filled 16 GB of RAM and wedged the machine. Staging now hangs off
the MO2 instance root, deliberately as a *sibling* of `mods/` since MO2 lists
every directory under `mods/` as a mod. A stale-dir sweep runs at install time.

## Documentation differences are not findings

Steven's call, after keeping straggler files (report 1, S4) turned out to fix
three mods and break five. The files are still installed; `diff` just stops
comparing readmes, credits, licences, `obmm_BSA_settings.jpg` and `.url`
shortcuts. Deliberately narrow — matching all `.txt` would hide real data files.

## Done: the layout planner and conflict resolution (D1)

`notes/layout-planner-design.md` has the design; `notes/conflict-resolution-design.md`
has the reasoning about conflict *direction*; `notes/conflict-index-validation.md`
records the result. The point was to answer "which files would mod X contribute?"
without installing X, and to extract only what a plan keeps.

All nine steps are done:

1. `LayoutPlan` + `Listing`
2. `bain` handler
3. `auto` handler (all five staged-tree helpers deleted, not kept)
4. `fomod` handler — the only one needing file *content*, so its
   `ModuleConfig.xml` is extracted on its own and the rest waits for the plan
5. `build` handler — the staging tree is now predicted from the layers' entry
   lists and checked against what actually appears
6. selective extraction
7. the file index (`install::index`)
8. `conflicts_with` on `file_prune`/`file_hide`, plus `mudcrab conflicts`
9. explicit-path resolution — **not done**, and deliberately: see below

### What it bought

`T4UTXL - Architecture_BETA1 - Priory` went from **5:00 to 0:22**, and from
writing 8.8 GB of staging to writing 93 MB. A full rebuild of the list is 8m42s.

Staging did not disappear, as predicted: 142 archives are 7z and 32 are rar, and
those shell out to tools that cannot rebase paths during extraction. What
changed is its size.

### Bugs it surfaced

Every one of these was invisible while installs walked an extracted tree, and
every one stopped an install dead the moment they stopped.

- **`bsdtar` cannot list directories reliably.** It marks them only with a
  trailing slash, and plenty of archives do not write one — `Arena
  Poster_0_44088.rar` stores `textures`, `textures/architecture` and
  `textures/architecture/imperialcity` bare. Listing now prefers `7z l -slt`.
- **7z's directory flag is a letter among letters.** `OOO Enhanced - Resources`
  writes its folders `Attributes = RD` with no `Folder` line at all, and a
  `starts_with("D")` test read every one as a file.
- **The 7z header block carries a `Path =` of its own**, so collecting from the
  top made the archive its own first entry.
- **BSAs store `folder\file`.** Comparing that to filesystem paths without
  normalising gave *zero* overlap against a 7628-file mod — and zero overlap
  reads exactly like "these mods do not conflict".
- **`bsdtar` reads its member list as fnmatch patterns**, so a selective
  extraction of `Harvest [Flora] - DLCVileLair.esp` matches nothing and reports
  "not found in archive". Verified before relying on 7z instead.

A structural check now runs over every archive listing regardless of tool: a
path that other paths sit inside cannot be a file.

### Step 9, and why it is not done

The design wanted selectors resolved to explicit paths in a lockfile, on the
grounds that a 1024-line list is reviewable in a way a selector is not. That
still holds. `mudcrab conflicts` gives the reviewability now — it prints exactly
what the action would act on — so the lockfile is a reproducibility measure
rather than a review one, and it needs a new pipeline phase (`compile` is a pure
TOML→JSON transform today and has no archive access). Left for when it is
wanted, not built speculatively.

## Done: Part 18 (Katkat's Location Retextures)

15 guide rows, 16 mods. **14 of 16 identical**, both differences deliberate —
see `part-18-katkats-location-retextures.md`. The first real `conflicts_with`
call site: row 7 says "hide the files that win over 2 VWD Ships (4 files)" and
it hid exactly four, matching the guide's own count.

Two things for Steven:

- **`Ogorod`** (guide row 14, Katkat's Vegetable Garden) has no Oracle folder,
  though its archive is in the downloads folder. Fetched and then skipped, for
  a reason nothing records.
- **`Ships from katkat`** — the Oracle never did row 7's follow-up; all four
  files are present and unhidden there.

## Next sections## Done: Part 18 (Katkat's Location Retextures)

15 guide rows, 16 mods. **14 of 16 identical**, both differences deliberate —
see `part-18-katkats-location-retextures.md`. The first real `conflicts_with`
call site: row 7 says "hide the files that win over 2 VWD Ships (4 files)" and
it hid exactly four, matching the guide's own count.

Two things for Steven:

- **`Ogorod`** (guide row 14, Katkat's Vegetable Garden) has no Oracle folder,
  though its archive is in the downloads folder. Fetched and then skipped, for
  a reason nothing records.
- **`Ships from katkat`** — the Oracle never did row 7's follow-up; all four
  files are present and unhidden there.

## Next sections

Guide order from **Part 18 (Katkat's Location Retextures)**, once the planner
work is finished. Most rows are trivial; the ones needing new features
are listed in `feature-gap-log.md` with the section that first needs them.
The next real features are section-aware `ini_set` (Part 11's `[Grass]`, a
live correctness bug -- see GAP-009) and the combine/repack archetype at
Part 25, which is modelled as one mod with several archives.

**SP1 (Parts 5-7) passed** -- the user confirmed the game runs. Getting there
also flushed out four real defects: 21 mods silently absent from the build, two
tree patches installing two levels too deep, and the two INI bugs that made the
UI unusable. See `earlier-sections-backlog.md`.

**SP2 is after Part 9** -- but hold it until the two blocked downloads land,
since OOO Enhanced's resources are most of that section's content.

## Open threads

- `mofam.merges.toml` is the **Oracle's** shape (85 sources, no `ORC.esp`).
  MudCrab Test reproduces the guide, which uses ORC v1.8.0 and therefore
  needs all 86. Do not copy the merge block across unchanged.
- `Bruma Frostcrag Spire LOD.esp` is in `plugins` for now. Part 36 merges it
  into Prebash, and the validator will then require its removal. The same
  applies to 9 of Part 7's 14 plugins -- see that section's notes.
- Section names are inconsistent: Parts 5, 7, 8 and 9 use the Oracle's numbered
  separators, Parts 1-4 and 6 use bare names. Worth one normalising pass, not
  worth churning mid-build.
- **Staged directories are lowercase everywhere** (3866 of them, verified). The
  rule: fold on the last hop into the mod's own folder, preserve in staging,
  because FOMOD and BAIN scripts name their sources in the archive's casing.
- **LOOT has a 180s timeout.** It opens a GUI needing manual sorting, so an
  unattended run stalls on it; the timeout turns a silent hang into an error.
  Doing the sort through libloot instead is the real fix, and is not done.
- Six mods legitimately have no Oracle counterpart and show as extras in
  `diff`: `xOBSE`, four `No Havoc Objects` splits, `T4UT - Menus Repolished`.
- 51 Oracle mods post-date the March 2025 guide; `diff` flags them
  `POST-GUIDE`. Each needs a conscious accept when its section is built. Note
  that a file from mod page **52949 is the guide's own** -- MOFAM is 52949 --
  so a post-guide date there is expected, not drift.
- BSArch deduplicates identical payloads within an archive; mudcrab's BSA
  writer does not. Costs ~2% on an archive with repeated files. Cosmetic, and
  the only reason a packed mod ever differs from the Oracle by size alone.
- MudCrab Test has ~21 stale mod folders under pre-rename ids, now superseded
  by correctly-named installs. Safe to delete, but they are the user's files --
  ask first.
- **Parts 2, 3, 4 and 6 were built before `diff` existed and have never been
  fully verified.** The first full-instance diff found 37 differing mods; two
  were broken installs, now fixed. See `earlier-sections-backlog.md`.
- **`cache_file_name` embeds the mod id**, so renaming a mod orphans its cached
  archive and two mods sharing an archive cache it twice. Worked around by
  giving every entry a `file_name` for `--archive-search-path` to match.

## Working practice

- **Read the guide and the archive, not just the Oracle.** The Oracle is one
  person's manual build and can be wrong -- Part 7 row 21 is a confirmed case,
  found by noticing the guide was internally consistent about two sibling
  folders while the Oracle treated them as one. `mofam-source.md` is the
  instruction; the archive's own `Wizard.txt` / `ModuleConfig.xml` is the
  authority on what an installer offers. Deriving from the Oracle's output
  risks encoding its mistakes as intent.
- **Pick the cheapest model that can do the job.** Most remaining rows are
  texture replacers needing no judgement; those belong on a small model, or
  inline. Reserve larger models for sections with real design content --
  9, 11, 25, 28, 33. Batch several trivial sections into one agent rather
  than paying cold-start context per section.
- Agents start cold and re-derive context, so a task worth 20 lines of edits
  is usually not worth an agent at all now that the tooling exists.
- **Improve the command rather than working around it.** Part 5 needed a row
  inserted at the *start* of a section, an install re-run after an action's
  behaviour changed, and a way to look inside a BSA. Those became
  `add --before-mod`, `install --force` and `inspect <file>.bsa` instead of
  three pieces of shell. Each will be wanted again.
- **A silent no-op is the failure mode to design against.** Both Part 5 bugs
  produced a successful install and a wrong tree. Where an action can match
  nothing, matching nothing should be an error.
