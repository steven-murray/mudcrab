# Why conflict direction is declared, not derived

`file_prune`'s `conflicts_with` names the mods a mod should yield to, and
mudcrab computes the file set that implies. The obvious alternative — work out
who wins from MO2 priority order — is wrong, and a real install proves it.

In the MOFAM reference instance's `modlist.txt` (top = highest priority):

```
568:+WAC Waalx Animals & Creatures
571:+OOO Enhanced
```

WAC **outranks** OOO Enhanced, so by priority WAC wins. But the guide files WAC
under *"Winning File conflicts → Overwritten mods"* — OOO Enhanced is winning.
The explanation is MO2's second tier: **loose files beat BSA contents regardless
of mod priority**, and WAC ships its assets packed.

It gets worse inside that one row, whose last step packs OOO Enhanced's own
textures into a BSA — flipping its tier and changing the answer for every
comparison after it.

So deriving direction means simulating a two-tier VFS over a modlist that
mutates its own packing mid-build. Don't. The guide is making an authorial
decision — *these mods win over this one* — so express that directly and let
mudcrab compute only the file set it implies.

## What is computed

Everything else. The modlist is declarative, so a mod's file set is knowable
before anything is installed, including for mods later in the list than the one
being pruned. That is what makes `conflicts_with` usable during a
section-by-section build.

Two implementation notes that cost real time:

- **BSA paths are stored `folder\file`.** Comparing them to filesystem paths
  without normalising the separator gives *zero* overlap — which reads exactly
  like "these mods do not conflict", and does so for precisely the mods that
  pack their assets, the whole category the feature exists for.
- **MO2's conflict tab names the winner, not every provider.** A file with two
  competitors is filed under one of them and drops out of the other's list. That
  is a property of the UI, not of the conflict, and it is a good reason not to
  derive these lists by reading them off a finished install.
