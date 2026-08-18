# Part 18 — Katkat's Location Retextures

15 guide rows, 16 mods (row 3 is one page installed twice). **16 of 16 identical
against the Oracle**, after Steven acted on the two divergences this section
found — both recorded below, because finding them was the point.

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
whose partner lives thirteen sections earlier. `mudcrab conflicts --mod "Ships from
katkat" --with "VWD Ships"` prints the same list without installing anything.

Re-running the section surfaced an idempotence bug and it is now fixed: a file
hidden by a previous run is called `x.nif.mohidden`, so it no longer matches its
own name, and the "selected no files" check — which exists to catch a selector
that was *wrong* — fired on one that had already worked. Comparison now ignores
the suffix.

## Two divergences found, both since resolved in the Oracle

### Guide row 14 was missing from the Oracle

*Katkat's VEGETABLE GARDEN* had no Oracle folder, yet `Ogorod 1.1.rar` was
sitting in the downloads folder — fetched and then not installed. Built here per
the standing rule; **Steven has since installed it as `KatKat's Vegetable
Garden`**, and our id was renamed to match.

That rename left a stale `mods/Ogorod` folder behind, since `install` writes the
profile from the plan but does not remove folders the plan no longer names.
Harmless, and Steven's to delete.

### Row 7's follow-up was never done in the Oracle

All four `_far.nif` files were present and unhidden there. **Steven has since
hidden them**, and the two sides now agree.

Worth keeping: the priorities make the instruction load-bearing. `Ships from
katkat` sits at `modlist.txt` line 432 and `VWD Ships` at 679, so without the
hide the Katkat meshes win and the VWD LOD meshes never load at distance.

Note the priorities make the instruction meaningful: `Ships from katkat` sits at
`modlist.txt` line 432 and `VWD Ships` at 679, so without the hide the Katkat
meshes win and the VWD LOD meshes never load.

## Thirteen non-Nexus rows

Rows 1–7 and 9–12, 14, 15 are hosted on **tesall.ru**, all marked `[MI]` in the
guide. They use `manual:`, which says in the modlist that no automated fetch can
reach them: `download` stops with the filename and the paths it searched, which
is an instruction rather than an error. Every one was already in the MO2
downloads folder.

The cost is that `diff` cannot date them — **14** of this section's mods report
UNKNOWN AGE, because a mod with `modid=0` has no Nexus file whose timestamp
could be compared against the March 2025 guide. Fourteen, not thirteen: row 3 is
one guide row installed as two mods, and both halves are manual. That is honest rather than
useful; there is nothing better available for a non-Nexus source.

## Small things

- Every archive has a top-level `Data/` or is a data folder already, so
  auto-detection handled all 16. No declared layouts in this section.
- Row 8's archive spells the author **KatKat47**; the mod page and the Oracle
  folder both say **KatKat74**. Filename kept as it downloads.
- Row 3 is one page and two installs, in the order the guide gives, so
  `Ayleid Ruins Unofficial Oblivion Patch Meshes` wins over
  `Ayleid Ruins HD BaseMetal`.
