# Part 18 — Katkat's Location Retextures

15 guide rows, 16 mods (row 3 is one page installed twice). **14 of 16 identical
against the Oracle; both differences are deliberate and explained below.**

Nothing here ships a plugin — the whole section is texture and mesh replacers —
so the load order is untouched.

## The first real `conflicts_with` call site

Row 7's follow-up is the reason the conflict index exists:

> *Once installed, open the Conflicts tab & hide the files that win over
> 2 VWD Ships - KatKat74's Textures (4 files)*

Written as a relationship rather than as four paths:

```toml
[[mods.actions]]
action = "file_hide"
conflicts_with = ["VWD Ships"]
```

It hid **exactly four files**, which is the number the guide states, and they are
exactly VWD Ships' four `_far.nif` meshes:

```
meshes/architecture/ships/bloatedfloathouse01_far.nif
meshes/architecture/ships/piratecabin01_far.nif
meshes/architecture/ships/shipcabin01_far.nif
meshes/architecture/ships/shipwreck01_far.nif
```

That is the mechanism checked against a count written in the guide, on a mod
whose partner lives eleven sections earlier. `mudcrab conflicts --mod "Ships from
katkat" --with "VWD Ships"` prints the same list without installing anything.

Re-running the section surfaced an idempotence bug and it is now fixed: a file
hidden by a previous run is called `x.nif.mohidden`, so it no longer matches its
own name, and the "selected no files" check — which exists to catch a selector
that was *wrong* — fired on one that had already worked. Comparison now ignores
the suffix.

## Two differences from the Oracle, both deliberate

### `Ogorod` — extra in ours (guide row 14)

Guide 14 is *Katkat's VEGETABLE GARDEN*. **The Oracle has no folder for it**, yet
`Ogorod 1.1.rar` is sitting in the downloads folder — so it was fetched and then
not installed. Built here because the guide says to, per the standing rule.
Worth a look: if it was skipped on purpose, the reason is not recorded anywhere.

### `Ships from katkat` — 4 files hidden in ours

**The Oracle did not perform row 7's follow-up.** All four `_far.nif` files are
still present and unhidden there. Ours follows the guide, so this shows as
"hidden on one side only (4)" until the Oracle is changed by hand or the
instruction is decided against.

Note the priorities make the instruction meaningful: `Ships from katkat` sits at
`modlist.txt` line 431 and `VWD Ships` at 678, so without the hide the Katkat
meshes win and the VWD LOD meshes never load.

## Thirteen non-Nexus rows

Rows 1–7 and 9–12, 14, 15 are hosted on **tesall.ru**, all marked `[MI]` in the
guide. They use `manual:`, which says in the modlist that no automated fetch can
reach them: `download` stops with the filename and the paths it searched, which
is an instruction rather than an error. Every one was already in the MO2
downloads folder.

The cost is that `diff` cannot date them — 13 of this section's mods report
UNKNOWN AGE, because a mod with `modid=0` has no Nexus file whose timestamp
could be compared against the March 2025 guide. That is honest rather than
useful; there is nothing better available for a non-Nexus source.

## Small things

- Every archive has a top-level `Data/` or is a data folder already, so
  auto-detection handled all 16. No declared layouts in this section.
- Row 8's archive spells the author **KatKat47**; the mod page and the Oracle
  folder both say **KatKat74**. Filename kept as it downloads.
- Row 3 is one page and two installs, in the order the guide gives, so
  `Ayleid Ruins Unofficial Oblivion Patch Meshes` wins over
  `Ayleid Ruins HD BaseMetal`.
