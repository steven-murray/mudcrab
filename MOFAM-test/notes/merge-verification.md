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

## TACE Merge — verified 2026-08-16

Built with `mudcrab merge`, installed as a lower-priority MO2 mod, tested via
`coc AnvilMagesGuild` and a walk around Anvil.

- 8533 records / 1170 renumbered / 282 clobbered — exact oracle match
- No merge-attributable problems

One missing mesh was found in Anvil. It is **missing in the Oracle too**, so
it predates any mudcrab involvement and is tracked separately.

## Prebash Merge (no ORC) — built, in-game result inconclusive

4499 records; the 33-record delta from the ORC-inclusive build is exactly ORC
v194's whole contribution, with the clobber count unchanged. See the commit.

In game it shows heavy visual artifacts — glowing trees, purple sky. These are
**not attributable to the merge**: they are ORC 3.1.5f's own rendering, and
later ORC versions have a reputation for instability. Nothing about a plugin
merge produces sky colour. Separately tunable via ORC's INI, and out of scope
here.

## ORC: the Oracle and MudCrab Test deliberately diverge

Worth stating plainly, because the two builds now want *different* Prebash
merges:

- **The Oracle** (this user's personal Linux install) runs **ORC 3.1.5f**,
  which ships no plugin, because ENB never worked on this machine and ORC is
  carrying the visuals alone. Its Prebash therefore has **no ORC.esp** — that
  is what `MOFAM-test/input/mofam.merges.toml` builds.
- **MudCrab Test**, which reproduces the *guide*, should use **ORC v1.8.0** as
  the guide specifies. v1.8.0 **does** ship `ORC.esp`, so its Prebash merge
  **must include it** — 86 sources, not 85.

So `mofam.merges.toml` as it stands is the Oracle's shape, not the guide's.
When Part 36 is authored into `mofam.full.toml`, its Prebash must list
`ORC.esp` from the v180 folder. Do not copy the merge block across unchanged.

## Not yet verified in game

- The other five merges. TACE is the next most interesting: it is the only
  other merge that renumbers FormIDs (1170), so it exercises the same
  machinery Unique Forts does.
- Prebash rebuilt against ORC v194, which no zMerge output exists for. That
  one has no oracle at all, so in-game testing is the only check available.
- The install path end to end: merges built during `install`, sources hidden
  via `.mohidden`, LOOT sorting the result. Only the standalone `merge`
  command has been exercised against the real instance.
