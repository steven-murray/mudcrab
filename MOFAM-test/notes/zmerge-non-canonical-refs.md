# zMerge writes non-canonical mod indices — and they are fine

**Status: closed.** Cosmetic, not a defect. This note previously claimed the
opposite; the correction is recorded below because the reasoning error is the
useful part.

## What is actually there

In four of the six merges, zMerge's output contains references whose mod index
is greater than the plugin's own index — i.e. past the end of its master list.
The bad index is always the source plugin's **position in the load order**, and
the object index is carried over untouched:

| merge | remapped | records | non-canonical refs |
|---|---|---|---|
| OOO Patches Merged | 0 | 1759 | 0 |
| NPC Merge | 0 | 2278 | 0 |
| Late Loaders Merged | 0 | 4361 | 12 |
| Prebash Merge | 0 | 4505 | 19 |
| Unique Forts Merged | 2004 | 7912 | **718** |
| TACE Merge | 1170 | 8533 | **1322** |

## Why it does not matter

**xEdit resolves them.** TES4Edit's Check for Errors on the worst-affected
merge: *"Processed Records: 7913, Errors found: 0"*. An out-of-range mod index
is not a dangling pointer — it is a non-canonical way of writing "my own
record", and xEdit (and the engine it models) reads it as such.

That is confirmed independently here. Modelling the same tolerance in
`tests/merge_oracle.rs` — clamp any mod index above the plugin's own index down
to its own index — makes all six reference graphs match **exactly**, with no
special-casing. mudcrab writes the canonical value; zMerge writes a value that
resolves to the same record. Same graph.

The column that *would* indicate a real defect is a reference to an own-record
object index that was never allocated. That is **zero for all six merges**, and
the oracle test now asserts it.

## The correction, and how the error happened

The first pass concluded these were dangling references, that four of six
installed merges were "faulty in game", and that Fort Naso and Fort Doublecross
would show missing NPCs. **All of that was wrong**, and a test plan was built
on it before the claim was checked.

The chain of reasoning that failed:

1. Correct: the TES4 spec says a plugin addresses masters `0..n-1` and its own
   records at index `n`.
2. Correct: these references use indices above `n`.
3. **Wrong**: therefore they resolve to nothing.

Step 3 confused *what the format specifies* with *what implementations accept*.
Readers are more tolerant than the spec, and tolerance is part of the de facto
format. Nothing in the measurement was wrong — 718 is the right number, and the
load-order-index signature is real. The error was entirely in the consequence
inferred from it, and it was stated with far more confidence than a
never-executed inference deserved.

What would have caught it sooner: the merges have been installed and played,
and 718 dangling references in a fort mod would have been noticed. The theory
predicted visible breakage that had conspicuously never been reported. The
cheapest possible check — open the file in TES4Edit — was left until after the
conclusion had been written down twice.

## Consequences that remain true

- **Tier 3 (near-byte-exact) is out of reach**, but for a mundane reason: we
  write a different high byte than zMerge does. Not a defect on either side.
- **Tier 2 is now stricter than before.** Removing the special-casing that the
  wrong theory required turned a count-based escape hatch into an exact graph
  comparison. The mistake left the test better than it found it.

## Open question this raises

`merge::rewrite` hard-errors (`RewriteError::DanglingReference`) on a *source*
plugin whose mod index exceeds its master list. Given that real tools emit such
indices and every reader accepts them, that refusal is probably too strict: it
means mudcrab cannot merge a zMerge output as a source. None of the 170 MOFAM
sources trigger it, so nothing is blocked today. Worth revisiting as
clamp-with-a-warning rather than an error.

## Unrelated oracle drift found along the way

Still true, still worth not re-diagnosing:

- **`merges.json` load orders are stale** for Late Loaders and NPC Merge.
  zMerge's master lists there are supersets of what any source requires and are
  not monotonic in the recorded load order; NPC Merge's names a plugin absent
  from it entirely. Extra unused masters are harmless.
- **`merges.json` points ORC.esp at the v194 folder, but the built Prebash
  merge came from v180.** The definition was updated when the ORC upgrade
  began; the merge was never rebuilt. The oracle fixture pins v180 to match
  what is installed; `MOFAM-test/input/mofam.merges.toml` uses v194 because
  that is what a real rebuild should consume. Hence 4532 records there against
  the oracle's 4505 — exactly ORC v194's 27 extra own records.
