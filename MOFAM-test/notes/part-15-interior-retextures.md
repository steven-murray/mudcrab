# Part 15 (Interior Retextures)

12 guide rows, 13 Oracle folders (row 5 installs two archives separately).

**Status: 13 compared, 12 identical, 1 differing — the known "file beside the
data folder" case.**

`HD Cobwebs - Readme.txt` sits next to the archive's `Data/` folder, so
unwrapping drops it where MO2 keeps it. Fifth occurrence; tracked as a single
systemic difference rather than re-argued per section.

## Row 4 states a keep-list, so it is written as one

> *"Once installed delete every file except: textures > dungeons > caves >
> cavefungus01\*, cavefungus02\*. This will leave 5 files remaining."*

An `include` filter rather than a `file_prune`, because that is what the
sentence actually says. It is also the more durable form: a prune enumerating
everything else would go stale if the archive ever gained a file, while a
keep-list cannot. Five files, matching the Oracle exactly.

## Two archives auto-detection could not resolve

- **Double Sided Cobwebs** holds both `Main files/data` and
  `Optional Textures/data`, so detection refuses to pick — correctly. The guide
  says which: *"Use the 'Main files > data' folder as the data directory."*
- **VKVII_Oblivion_Cathedrals** is on ModDB, not Nexus. Another `manual:`
  entry; its `modid=1` is MO2 bookkeeping, not provenance.

## Everything else is a plain replacer

Ten of thirteen need no layout declaration at all.
