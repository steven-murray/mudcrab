# The merge engine: what the corpus established

`type = "merge"` is a native, headless replacement for zEdit's zMerge. It was
scoped by measuring a real 752-mod install and the six merges zEdit had built
from it, before any merge code was written, so that the engine could implement
what Oblivion plugins actually contain rather than what the format permits.

This is the evidence behind several decisions that look arbitrary in the code.
Reproduce with `MOFAM-test/scripts/recon-plugins.py`.

## 1. The FormID allocation algorithm

This reproduces zEdit's `map.json` **exactly for all six merges**:

```
used   = {}                       # object indices already claimed
cursor = 0x000801
for plugin in merge.plugins:                      # listed order
    for oi in sorted(own_object_indices(plugin)): # ASCENDING, deduped
        if oi not in used: used.add(oi); continue # free -> keep it
        while cursor in used: cursor += 1
        map[plugin][oi] = cursor; used.add(cursor); cursor += 1
```

Three details fall out of the data and are easy to get wrong:

- iteration is by **ascending object index**, not file order;
- the own-index set must be **deduped** — some plugins contain duplicate
  FormIDs (Fort Vlastarus has three), and without deduping phantom allocations
  appear;
- `cursor` is monotonic **across** plugins and never reset.

Four of the six merges remap nothing at all, and for those the entire
transformation is master-index remapping.

## 2. Script bytecode does not need patching for TES4

Oblivion's compiled script bytecode references forms by **16-bit index into the
record's SCRO list**, not by inline FormID, so renumbering SCRO in place is
sufficient.

A naive "does any SCRO value appear as raw bytes in SCDA" scan over the whole
install yields 396 hits, and every one is a false positive: only 14 distinct
values, all small integers (`0x00000014` is 377 of them — both the Player
FormID and the number 20), 312 of 432 unaligned, and 428 of 432 at mod index 0,
which never changes. The test that matters — *does any SCDA contain an aligned
raw FormID this merge would change* — returns **zero for all six merges**.

`merge::audit_scripts` keeps that detector as a **hard error**, so the
assumption invalidates itself loudly if a future list breaks it. Do not build a
bytecode disassembler without a reproducer.

## 3. Asset handling is a no-op for TES4

- **FaceGen** lives *inside* the `NPC_` record (`FGGS`, `FGGA`, `FGTS`). There
  is not one `FaceGeom` directory in the entire install; `handleFaceData` is a
  Skyrim-era concern.
- **Voice** directories are keyed by plugin name, and no merged plugin has one.
- Sources stay **enabled** in MO2, so they keep serving their own loose assets
  and BSAs. Merging never needs to copy assets or build an archive.

Hence `merge::audit_assets`, also a hard error rather than a handler.

## 4. Schema completeness

`tests/fixtures/plugin/subrecord_matrix.txt` holds **436 (record, subrecord)
pairs across 47 record types**, taken from the 171 source plugins. It is the
completeness target for `src/plugin/schema/tes4.rs` and is asserted by test.

## 5. Plugin names contain glob metacharacters

`Harvest [Flora] - DLCFrostcrag.esp` is a **glob character class**. Looking a
plugin up with a glob silently finds nothing. Compare filenames literally and
case-insensitively. This is a live hazard anywhere mudcrab interpolates a mod or
plugin name into a pattern, since `globset` backs `include`, `exclude`, `files`
and `game_root_files`.

## 6. Out-of-range mod indices are valid in practice

zMerge's output contains references whose mod index is greater than the
plugin's own index — past the end of its master list. The bad index is always
the source plugin's position in the load order, and it is common: 1322 of TACE
Merge's 8533 records, 718 of Unique Forts' 7912.

**These are not dangling references.** xEdit's Check for Errors reports zero
errors on the worst-affected merge, because an out-of-range mod index is a
non-canonical way of writing "my own record" and every reader resolves it as
such. Modelling that tolerance in `tests/merge_oracle.rs` — clamping any index
above the plugin's own down to its own — makes all six reference graphs match
exactly, with no special-casing. mudcrab writes the canonical value; zMerge
writes one that resolves to the same record.

The measurement that *would* indicate a real defect is a reference to an
own-record object index that was never allocated. That is zero for all six, and
the oracle test asserts it.

**Open**: `merge::rewrite` still hard-errors on such an index in a *source*
plugin, which means mudcrab cannot merge a zMerge output. Given that every
reader accepts them, that should become clamp-with-a-warning. See
[roadmap A4](../roadmap.md#a4-tolerate-out-of-range-mod-indices-in-merge-sources).
