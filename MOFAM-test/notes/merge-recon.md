# M0 recon: findings that scope the native merge

Evidence gathered before writing any merge code, by parsing the real MOFAM
install (752 mods) and the 6 merges zEdit built from it. Reproduce with
`MOFAM-test/scripts/recon-plugins.py`.

Ground truth used:
- `~/Games/OblivionModdingtools/zedit/profiles/Oblivion/merges.json` — the 6 merge definitions
- `~/Games/Wabbajack/Oblivion/MOFAM-03.25/mods/<Name>/merge - <Name>/map.json` — zEdit's old→new FormID map
- the 6 built `.esp` files — golden outputs

## 1. The FormID allocation algorithm is solved

This reproduces zEdit's `map.json` **exactly for all 6 merges**:

```
used   = {}                       # object indices already claimed
cursor = 0x000801
for plugin in merge.plugins:                      # listed order
    for oi in sorted(own_object_indices(plugin)): # ASCENDING, deduped
        if oi not in used: used.add(oi); continue # free -> keep it
        while cursor in used: cursor += 1
        map[plugin][oi] = cursor; used.add(cursor); cursor += 1
```

Two details that fall out of the data and are easy to get wrong:
- iteration must be by **ascending object index**, not file order
- the own-index set must be **deduped** (some plugins contain duplicate FormIDs;
  Fort Vlastarus has 3) — otherwise phantom extra allocations appear
- `cursor` is monotonic **across** plugins, never reset

| merge | sources | remaps | matches oracle |
|---|---|---|---|
| OOO Patches Merged | 19 | 0 | yes |
| Unique Forts Merged | 11 | 2004 | yes |
| TACE Merge | 21 | 1170 | yes |
| Late Loaders Merged | 20 | 0 | yes |
| NPC Merge | 14 | 0 | yes |
| Prebash Merge | 86 | 0 | yes |

4 of 6 merges remap nothing — for those the *entire* transformation is
master-index remapping, which applies to 100% of FormIDs everywhere.

## 2. SCDA script patching is NOT needed for TES4

Oblivion compiled script bytecode references forms by **16-bit index into the
record's SCRO list**, not by inline FormID. So renumbering SCRO in place
(preserving order) is sufficient.

Evidence:
- Across the whole install: 13,866 records carry both SCRO and SCDA
  (SCPT 8628, INFO 4523, QUST 715).
- A naive "does any SCRO value appear as raw bytes in SCDA" scan yields 396
  hits — **all false positives**:
  - only **14 distinct values** total (real embedding would give thousands)
  - they are tiny integers: 20, 15, 12, 10, 200 … i.e. ordinary bytecode
    literals. `0x00000014` alone is 377/432 hits — it is both the Player
    FormID *and* the integer 20.
  - **312 of 432 hits are 4-byte unaligned** — pure byte coincidence
  - **428 of 432 are mod index 0** (`Oblivion.esm`), which is always master
    index 0 in both source and merged output, so those FormIDs never change
- The test that actually matters — *does any SCDA contain an aligned raw FormID
  that this merge would change?* — returns **0 for all 6 merges**.

`audit_scripts` keeps this detector as a hard error so the assumption is
self-invalidating rather than silent. Do not build a bytecode disassembler
without a reproducer.

## 3. Asset handling is a no-op for TES4

- **FaceGen**: Oblivion stores it *inside* the `NPC_` record (`FGGS` 200B,
  `FGGA` 120B, `FGTS` 200B). There is not one `FaceGeom` directory in the whole
  752-mod install. `handleFaceData` is a Skyrim-era flag; it does nothing here.
- **Voice**: `Sound/Voice/<plugin>.esp/...` is keyed by plugin name and INFO
  FormID, but no *merged* plugin has any. The voice trees that exist belong to
  unmerged plugins sharing a mod folder.
- **`copyGeneralAssets: false`** in all 6 merges, and source mods stay
  **enabled** in MO2 — so they keep serving their own loose assets and BSAs.
- **`buildMergedArchive: false`**, `archiveAction: "Extract"` — no BSA reading
  or writing. **Do not build a BSA library.**

Net: only FormID-keyed and plugin-name-keyed assets would ever need handling,
and MOFAM has none. Implement detectors that hard-error, not handlers.

## 4. Schema scope

`tests/fixtures/plugin/subrecord_matrix.txt` — **436 (record, subrecord) pairs
across 47 record types**, from the 171 source plugins of the 6 merges. This is
the completeness target for `src/plugin/schema/tes4.rs` and is asserted by test.

## 5. Gotcha: plugin names contain glob metacharacters

`Harvest [Flora] - DLCFrostcrag.esp` is a **glob character class**. Looking
plugins up with `glob()`/`rglob()` silently finds nothing. Compare filenames
literally and case-insensitively. This cost a false "missing plugin" report
during recon and is a live hazard anywhere mudcrab interpolates a mod or plugin
name into a glob pattern (`globset` is used for `include`/`exclude`/`files`/
`game_root_files`).
