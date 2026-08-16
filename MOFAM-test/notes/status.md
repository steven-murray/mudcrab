# Where the build is

Rolling status. Update it when a section moves; it exists so a fresh context
can pick up without re-reading the transcript.

Last updated: 2026-08-16.

## Done

- **Tooling (plan Phase 2) is complete.** `add`, `diff`, `inspect`,
  `--section`/`--only` filters, `--archive-search-path`, native BSA
  reader/writer, `pack_bsa` / `create_dummy_plugin` / `file_prune`.
  307 tests, clippy clean under `-D warnings`.
- **Merges**: all six reproduce zMerge semantically. Unique Forts and TACE
  verified in game. Prebash rebuilt without `ORC.esp` for the Oracle.
- **Parts 1, 2, 3, 4, 6** authored (105 mods, pre-existing).

## In progress: Part 5 (LOD)

Nine of ten rows authored in `MOFAM-test/input/mofam.full.toml`; 114 `[[mods]]`
total. **Nothing installed or diffed yet** -- that is the next step.

Row 1, Evenstars Colourwheel LOD Update, is **not yet authored**. It was
blocked on BSA support, which now exists. It needs, in this order:

1. `layout = "bain"` with `bain_subpackages = ["00 Textures", "04 Statues and shrines"]`
   (note the archive's lowercase "shrines"; the guide capitalises it)
2. `pack_bsa` to `Evenstars Colourwheel LOD Update.bsa`
3. `create_dummy_plugin` for `Evenstars Colourwheel LOD Update.esp`
4. `file_prune` of the loose `meshes` and `textures` folders

The Oracle's copy of that mod contains exactly the `.bsa` + `.esp`, which is
what to diff against.

Then: `install --section "5 - LOD"`, `diff --section "5 - LOD"`, explain every
difference, and stop-point SP1 covers Parts 5-7 together.

## Next sections

Guide order from Part 7. Most rows are trivial; the ones needing new features
are listed in `feature-gap-log.md` with the section that first needs them.
The next real features are section-aware `ini_set` (Part 11's `[Grass]`, a
live correctness bug -- see GAP-009) and the combine/repack archetype at
Part 25, which is modelled as one mod with several archives.

## Open threads

- `mofam.merges.toml` is the **Oracle's** shape (85 sources, no `ORC.esp`).
  MudCrab Test reproduces the guide, which uses ORC v1.8.0 and therefore
  needs all 86. Do not copy the merge block across unchanged.
- `Bruma Frostcrag Spire LOD.esp` is in `plugins` for now. Part 36 merges it
  into Prebash, and the validator will then require its removal.
- Six mods legitimately have no Oracle counterpart and show as extras in
  `diff`: `xOBSE`, four `No Havoc Objects` splits, `T4UT - Menus Repolished`.
- 51 Oracle mods post-date the March 2025 guide; `diff` flags them
  `POST-GUIDE`. Each needs a conscious accept when its section is built.
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
