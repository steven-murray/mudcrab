# Part 5 (LOD)

Ten mods. Built 2026-08-16 with
`./MOFAM-test/scripts/run-full.sh --section "5 - LOD"`.

**Result: 10 compared, 9 byte-for-byte identical, 1 differing — explained
below. Nothing missing, nothing extra.**

## The one differing mod

`Evenstars Colourwheel LOD Update` is the section's only row with actions:
BAIN-select two subpackages, pack them into a BSA, write a dummy plugin, delete
the loose folders. Both differences are in files mudcrab *generates*, not files
it extracts — every one of the 1672 assets is identical.

### `.bsa`: ours 40,025,637 B, Oracle 39,249,381 B

Same 1672 files in the same 54 folders, same names, same content, same flags.
Reading both archives' records:

| | ours | Oracle |
|---|---|---|
| sum of file sizes | 39,961,100 | 39,961,100 |
| distinct payloads | 1672 | 1618 |
| bytes saved by sharing | 0 | 776,256 |
| metadata | 64,537 | 64,537 |

776,256 is exactly the file-size difference. **BSArch deduplicates identical
payloads**: 54 of the files are byte-identical to another file in the archive,
and BSArch points both records at one copy. mudcrab's writer stores a payload
per record. The metadata blocks are the same length to the byte, so this is the
whole of the difference.

Functionally identical — both archives serve the same 1672 files. Worth doing
one day for the ~2%, but it is an optimisation, not a fix.

### `.esp`: ours 139 B, Oracle 85 B

Both are a bare TES4 header with no records, which is all a dummy plugin is.
The bytes differ because they came from different generators:

- Oracle: `CNAM "nmcdyer"`, `MAST Oblivion.esm`
- ours: `CNAM "mudcrab"`, an `SNAM` description, `MAST Oblivion.esm`

The extra 54 bytes are our `SNAM`. Attributing mudcrab's output to a person's
CNAM would be wrong, so this one stays different on purpose.

## Two fixes this section forced

Both were silent — an install that reported success and produced the wrong
tree. The Oracle diff is what caught them, not the install.

- **`file_prune` did nothing.** `paths = ["meshes"]` is what the guide means by
  "delete the loose meshes folder", but as a raw glob it matches only a *file*
  called `meshes`. All 1672 loose files survived, shadowing the BSA in the VFS.
  A bare directory name now means the folder and everything under it, and **a
  pattern that matches nothing is an error** rather than a no-op.
- **`pack_bsa` wrote `file_flags: 0`.** The header's asset-kind flags tell
  Oblivion which kinds of asset an archive can serve; an archive of meshes
  declaring none is invisible to the engine while still parsing perfectly. The
  inherited comment claimed the engine recomputes them. It does not, and not
  one of the 65 real archives on this machine writes zero. Now derived from
  content (`src/bsa/file_flags.rs`), and gated against the real corpus.

  Exact agreement with any one tool is not achievable — Bethesda's own archives
  carry junk in the high bits (`Oblivion - Voices2.bsa` declares `0x3a8d0010`)
  and authors disagree about whether `.lip` is a sound or a voice. The bar is
  never declaring *fewer* kinds than the contents call for. Across 48 archives:
  35 exact, 11 superset, 4 where the archive over-declares a kind it does not
  contain.

## Version notes

- **`Merged LOD` is flagged POST-GUIDE (2025-03-30), and that is correct.**
  It is from mod page 52949, which *is* MOFAM: the guide's own companion
  download for step 10, named `03-25`, version 3.25, uploaded five days after
  publication. Step 10 says to create an empty mod or "use my download on this
  modpage", so this is the guide's file, not drift.
- `Evenstars` and `Landscape LOD Textures by Xerus` were flagged UNKNOWN AGE:
  their filenames end in a version, not a Nexus timestamp. `diff` now falls
  back to `nexusLastModified` from the Oracle's `meta.ini` (present for 704 of
  745 mods), which dates them 2012 and 2008 — comfortably pre-guide.

## Row notes worth keeping

- **Evenstars subpackage case.** The guide says "04 Statues and Shrines"; the
  archive spells it "04 Statues and shrines". The Oracle's `meta.ini` records
  the lowercase form under `[Plugins] BAIN%20Installer`, so the guide has the
  typo.
- **J3 Atlassed VWD 2** takes its 14 subpackages from the archive's own
  `Wizard.txt`, not from the Oracle's installed files. Three sources agree.
- **Xerus** is the same archive listed twice with different `data_folder`s:
  Cyrodiil, then Shivering Isles overlaid on top. That is what "drag the
  Textures folder over the Cyrodiil folder, then set Cyrodiil as the data
  directory" means. 50 + 8 files, no overlap, matching the Oracle's 58.
- **Bruma Frostcrag Spire LOD** is an optional file on the *guide's* mod page,
  so its Oracle folder is named after the guide. Aliased via `oracle_name`.
