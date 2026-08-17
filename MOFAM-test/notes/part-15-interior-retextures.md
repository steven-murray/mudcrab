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

## Everything else resolves on its own

Exactly one mod carries a `layout` field: Double Sided Cobwebs. Two others need
a non-default archive setting of a different kind — row 4's `include` filter and
row 8's `manual:` path — and the remaining ten are plain replacers with nothing
declared.

Worth noting the near-miss: **Improved Candles also ships a nested data folder**
(`Azhurel's Improved Candles/Data/...`) and `inspect` suggests declaring
`layout = "custom-data-folder"` for it — but install's auto-detection resolves
it unaided, and the built folder matches the Oracle. The same shape of ambiguity
needed a hand-written answer for Double Sided Cobwebs and needed none here,
because that archive has *two* candidate data folders and this one has a single
unambiguous one. `inspect`'s suggestion is generic; detection is not.
