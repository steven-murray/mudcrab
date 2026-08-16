# zMerge writes load-order indices into 4 of the 6 merges

**Status: reproduced across four merges with a single consistent signature.**
Superseded the earlier "possible operator error" framing — that hypothesis is
now ruled out (see below).

## The measurement

Each merge was rebuilt from zEdit's own definitions and compared semantically
against the installed output. All six agree on every record and every
reference edge. But zMerge's *own* output contains references whose mod index
exceeds its master count, so they address a master slot that does not exist:

| merge | sources | remapped | records | broken refs |
|---|---|---|---|---|
| OOO Patches Merged | 19 | 0 | 1759 | 0 |
| NPC Merge | 14 | 0 | 2278 | 0 |
| Late Loaders Merged | 20 | 0 | 4361 | **12** |
| Prebash Merge | 86 | 0 | 4505 | **19** |
| Unique Forts Merged | 11 | 2004 | 7912 | **718** |
| TACE Merge | 21 | 1170 | 8533 | **1322** |

Reproduce: `MUDCRAB_MOFAM_ROOT=... cargo test --test merge_oracle -- --nocapture`

Two distinct failure modes were counted separately, because conflating them
would have hidden the pattern: references *beyond the master list*, and
references claiming to be the plugin's own records at an object index it never
allocated. **All are the first kind; there are zero of the second.**

## The signature

In every case the bad mod index is exactly the source plugin's **position in
the load order**, while the object index is carried over untouched:

| merge | record | zMerge wrote | bad mod index resolves to |
|---|---|---|---|
| Unique Forts | FACT `02001F94` | `06001F94` | load order[6] = Unique Forts Fort Irony.esp |
| Unique Forts | FACT `02002636` | `03002636` | load order[3] = Unique Forts Fort Doublecross.esp |
| TACE | REFR `0001C5F4` | `0C005394` | load order[12] = Chorrol LCH.esp |
| Late Loaders | REFR `2200678B` | `2F00678A` | load order[47] = Improved MG Patch.esp |
| Prebash | CONT `000244A5` | `37000ED4` | load order[55] = Bibliophilia.esp |

So zMerge translated the object index correctly and then wrote a **load-order
index** into the mod-index byte instead of the merged plugin's master-index
space. These are references that should have pointed at the merged plugin's
own records.

That the object index survives is what makes our side verifiable: mudcrab emits
the same object index with the correct mod index, so ours resolves to a record
that exists and zMerge's resolves to nothing. Being different from a suspect
oracle is not the same as being right — here the difference is checkable, and
it is only the high byte.

## Hypotheses now ruled out

- **Operator error / GUI misfire.** The merges were built on the original
  prefix (22330), where the zEdit GUI works correctly. The GUI trouble was on
  the newly created prefix, which only ever received copies of already-built
  merges. Nothing about how these were invoked explains a malformed high byte.
- **Confined to one merge.** Four of six are affected.
- **Confined to the merges that renumber FormIDs.** Late Loaders and Prebash
  remap nothing and are still affected — so the renumbering path is not the
  (only) trigger. Renumbering does correlate with *magnitude* (718 and 1322 vs
  12 and 19), which suggests it multiplies an underlying fault rather than
  causing it.
- **Dirty source plugins.** mudcrab hard-errors on a reference whose mod index
  exceeds its plugin's own master list. All 170 sources parse and merge without
  triggering it, so the defect is introduced by the merge, not inherited.

## Consequences

1. **The installed merges are faulty in game.** Dangling references typically
   surface as missing NPCs, objects or containers, and as scripts failing to
   resolve their targets. Unique Forts and TACE are badly affected; Late
   Loaders and Prebash marginally.
2. **Tier 3 (near-byte-exact) is unreachable and should stay that way.**
   Matching zMerge byte-for-byte would mean reproducing 2071 broken
   references. Tier 2 — semantic reference-graph equivalence — is the real
   gate, and all six merges pass it.

## Still open

- **Which fields.** The affected records are FACT, SCPT, REFR and CONT so far.
  Whether the fault is per-field or per-record-type is unestablished.
- **Upstream.** Not reported. A reproducer would need a minimal merge, which
  none of these are.

## Unrelated oracle drift found along the way

Worth recording so it is not re-diagnosed as a bug:

- **`merges.json` load orders are stale** for Late Loaders and NPC Merge.
  zMerge's master lists for those are supersets of what any source requires and
  are not monotonic in the recorded load order; NPC Merge's even names a plugin
  absent from that load order entirely. Their exact master lists are therefore
  not reproducible from what was recorded. Extra unused masters are harmless —
  they shift mod indices, and tier 2 is immune to that by construction.
- **`merges.json` points ORC.esp at the v194 mod folder, but the built Prebash
  merge came from v180.** The definition was updated when the ORC upgrade
  began; the merge was never rebuilt. The oracle fixture pins v180 because
  that is what the installed `.esp` actually contains. Rebuilding Prebash for
  real should use v194.
