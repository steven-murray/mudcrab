# Manual verification: Unique Forts Merged

The end of the loop the plan set out. Everything else about the merge engine
is checkable from a test; this is not.

## Result

| | |
|---|---|
| date | 2026-08-16 |
| merge | Unique Forts Merged (11 sources, 7912 records, 2004 renumbered, 56 clobbered) |
| built by | `mudcrab merge`, from `MOFAM-test/input/mofam.merges.toml` |
| TES4Edit Check for Errors | **7913 records, 0 errors** |
| in game | loaded, Fort Naso visited, everything in order |

Installed as a separate MO2 mod at lower priority than the zMerge original, so
the same filename wins by conflict resolution and unticking it reverts.

## What this establishes, and what it does not

**Does**: the native merge produces a plugin the game loads and plays. The
whole chain — hand-rolled TES4 parser, FormID allocator, schema-driven
reference rewriting, GRUP re-derivation, writer — round-trips through the
actual engine. No test can substitute for that.

**Does not**: prove parity in general. One merge, one location, one session.
The evidence for the other five is the oracle comparison in
`tests/merge_oracle.rs`: identical record sets and identical reference graphs
against zMerge's output, which is a much stronger check than a walk around a
fort, but is a check against zMerge rather than against the game.

Fort Naso was chosen because an earlier (wrong) theory predicted it would be
visibly broken. It is not — see `zmerge-non-canonical-refs.md`. Its value here
is as the most reference-dense location in the merge, not as a controlled
comparison.

## Not yet verified in game

- The other five merges. TACE is the next most interesting: it is the only
  other merge that renumbers FormIDs (1170), so it exercises the same
  machinery Unique Forts does.
- Prebash rebuilt against ORC v194, which no zMerge output exists for. That
  one has no oracle at all, so in-game testing is the only check available.
- The install path end to end: merges built during `install`, sources hidden
  via `.mohidden`, LOOT sorting the result. Only the standalone `merge`
  command has been exercised against the real instance.
