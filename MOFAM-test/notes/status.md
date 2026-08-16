# Where the build is

Rolling status. Update it when a section moves; it exists so a fresh context
can pick up without re-reading the transcript.

Last updated: 2026-08-16.

## Done

- **Tooling (plan Phase 2) is complete.** `add`, `diff`, `inspect`,
  `--section`/`--only` filters, `--archive-search-path`, `install --force`,
  `add --before-mod`, `inspect` of a `.bsa`, native BSA
  reader/writer, `pack_bsa` / `create_dummy_plugin` / `file_prune`.
  324 tests, clippy clean under `-D warnings`.
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

## Next sections

Guide order from Part 7. Most rows are trivial; the ones needing new features
are listed in `feature-gap-log.md` with the section that first needs them.
The next real features are section-aware `ini_set` (Part 11's `[Grass]`, a
live correctness bug -- see GAP-009) and the combine/repack archetype at
Part 25, which is modelled as one mod with several archives.

**Stop-point SP1 covers Parts 5-7**, so Parts 6 (already built) and 7 come
before the user next loads the game.

## Open threads

- `mofam.merges.toml` is the **Oracle's** shape (85 sources, no `ORC.esp`).
  MudCrab Test reproduces the guide, which uses ORC v1.8.0 and therefore
  needs all 86. Do not copy the merge block across unchanged.
- `Bruma Frostcrag Spire LOD.esp` is in `plugins` for now. Part 36 merges it
  into Prebash, and the validator will then require its removal.
- Six mods legitimately have no Oracle counterpart and show as extras in
  `diff`: `xOBSE`, four `No Havoc Objects` splits, `T4UT - Menus Repolished`.
- 51 Oracle mods post-date the March 2025 guide; `diff` flags them
  `POST-GUIDE`. Each needs a conscious accept when its section is built. Note
  that a file from mod page **52949 is the guide's own** -- MOFAM is 52949 --
  so a post-guide date there is expected, not drift.
- BSArch deduplicates identical payloads within an archive; mudcrab's BSA
  writer does not. Costs ~2% on an archive with repeated files. Cosmetic, and
  the only reason a packed mod ever differs from the Oracle by size alone.
- MudCrab Test has ~21 stale mod folders under pre-rename ids. They will show
  as extras until cleaned up.

## Working practice

- **Read the guide and the archive, not just the Oracle.** The Oracle is one
  person's manual build and can be wrong. `mofam-source.md` is the
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
