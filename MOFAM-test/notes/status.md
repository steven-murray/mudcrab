# Where the build is

Rolling status. Update it when a section moves; it exists so a fresh context
can pick up without re-reading the transcript.

Last updated: 2026-08-16.

## Done

- **MO2 profile**: mudcrab owns `Default` in MudCrab Test, set by `PROFILE` in
  `run-full.sh` (override with `MOFAM_PROFILE`). `install` rewrites that
  profile's `modlist.txt` and `plugins.txt` every run. MO2 auto-adds new mod
  folders to *other* profiles as disabled, so any other profile will look
  mostly switched off -- that is MO2, not a build failure.
- **Tooling (plan Phase 2) is complete.** `add`, `diff`, `inspect`,
  `--section`/`--only` filters, `--archive-search-path`, `install --force`,
  `add --before-mod`, `inspect` of a `.bsa`, native BSA reader/writer,
  `pack_bsa` / `create_dummy_plugin` / `file_prune` / `file_hide`.
  334 tests, clippy clean under `-D warnings`.
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

## Next sections

Guide order from Part 9 (Oscuro), the hardest early section: BAE extraction,
BSArch repacking and conflict-tab hiding. Most rows are trivial; the ones needing new features
are listed in `feature-gap-log.md` with the section that first needs them.
The next real features are section-aware `ini_set` (Part 11's `[Grass]`, a
live correctness bug -- see GAP-009) and the combine/repack archetype at
Part 25, which is modelled as one mod with several archives.

**SP1 (Parts 5-7) passed** -- the user confirmed the game runs. Getting there
also flushed out four real defects: 21 mods silently absent from the build, two
tree patches installing two levels too deep, and the two INI bugs that made the
UI unusable. See `earlier-sections-backlog.md`.

**SP2 is after Part 9.**

## Open threads

- `mofam.merges.toml` is the **Oracle's** shape (85 sources, no `ORC.esp`).
  MudCrab Test reproduces the guide, which uses ORC v1.8.0 and therefore
  needs all 86. Do not copy the merge block across unchanged.
- `Bruma Frostcrag Spire LOD.esp` is in `plugins` for now. Part 36 merges it
  into Prebash, and the validator will then require its removal. The same
  applies to 9 of Part 7's 14 plugins -- see that section's notes.
- Section names are inconsistent: Parts 5 and 7 use the Oracle's numbered
  separators (`7 - CHARACTER AND NPCS`), Parts 1-4 and 6 use bare names. Worth
  one normalising pass, not worth churning mid-build.
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
