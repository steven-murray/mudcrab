# Part 16 (Town and City Retextures)

32 guide rows, 35 Oracle folders, **36 mods here** — one more than the Oracle,
deliberately.

**Status: 36 compared, 32 identical, 3 differing, 1 extra in ours. All four
explained; two are deliberate divergences from the Oracle.**

| difference | mod | why |
|---|---|---|
| readme + settings image | Improved Chorrol | files beside `Data/`, the systemic case |
| plugin hidden in the Oracle | Signs of Mage Guilde - Mergeable | Part 36 merge source |
| **extra mod** | Signs of Mage Guilde English version | guide row 27; the Oracle skipped it |
| 2 extra files | T4UTXL - City Gates | guide names them; the Oracle lacks them |

## T4UTXL: one mod name, two archives, and a misleading `meta.ini`

The most instructive row in the section, and it only came apart because the diff
was checked rather than assumed.

The guide installs mod 54904 **twice under different names** — once keeping only
Priory textures, once keeping only city gates. Straightforward. But:

- The guide says **BETA1**; the Oracle installed **BETA2**, dated 2025-04-15,
  well after the guide. Version drift, shared rather than divergent.
- **BETA2 ships as two archives**, Part 1 and Part 2, and the city-gate textures
  are split across both. The Oracle's `meta.ini` records only Part 2 — because
  `installationFile` holds the *last* archive installed into a folder, not all
  of them.

Building City Gates from Part 2 alone gave 5 files against the Oracle's 11. The
five it did produce were Leyawiin and Skingrad; Anvil, Bravil and Bruma live in
Part 1. **The `meta.ini` was not lying so much as answering a narrower question
than the one being asked of it** — a caution for every mod scaffolded from
Oracle provenance.

With both parts in, all six of the guide's paths resolve — including
`Castle > Cheydinhal > cheydinhalcitydoor01*`, which the **Oracle does not
have**. So the Oracle installed only one part here too. Guide followed: 13 files
where the Oracle has 11.

## Guide row 27 is missing from the Oracle entirely

Row 27 is *Signs of Mage Guilde English version* (mod 25122), with *"Once
installed move MageGuild_simbol.esp to the optional folder."* Row 28 is the
separate *- Mergeable* variant.

The Oracle has only row 28. The row 27 archive is on disk (`Mage Guild Sign-25122.rar`),
so it was downloaded and never installed. Guide followed; it shows as **extra in
ours**, and its plugin is parked in `optional/` exactly as the guide says, so it
adds textures without touching the load order.

## Instructions stated as keep-lists

Three rows say "delete everything except", and all three are written as
`include` filters rather than prunes — the direct translation, and the form that
cannot go stale:

- Row 1 (Priory): keep `textures/architecture/priory/**`, minus `priorydoor01*`
  and `weynondoor01*`. 48 − 4 = 44, matching the Oracle exactly.
- Row 32 (City Gates): the six named paths.
- Row 17 (Farm fence): *"deselect the textures folder"* — an `exclude`.

## Four archives that are not on Nexus

`VKVII Oblivion Castles` (ModDB), `TD_Unique_Skingrad` and `TD_Unique_Anvil`
(MediaFire, linked directly from the guide), and `TD_aesthetics` (a blogspot
page). All `manual:`.

`TD_Unique_Skingrad` also needed a layout: the archive scatters plugins across
**four** locations — `Data/`, `Eng/Data/`, `TD_Unique_Skingrad_BC patch/Data/`,
and `Eng/TD_Unique_Skingrad_BC patch/`, the last with its `.esp` sitting
directly in the folder rather than under a `Data/` of its own. Auto-detection
correctly refuses to guess between them. The Oracle installed the plain
top-level `Data`.
