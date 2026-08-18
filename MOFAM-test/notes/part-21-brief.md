# Part 21 — Clutter & Miscellaneous Retextures: brief

**Not started.** Everything below is reconnaissance done so the section can be
authored without re-deriving it. 35 guide rows, **44 Oracle mods**, and all 44
archives are already in the downloads folder.

## Two rows cannot be pinned without Steven's Nexus key

Both have `fileid=0` in the Oracle's `meta.ini` and no `fileID` in the download's
`.meta` sidecar, so there is nothing on disk that names the Nexus file:

| row | mod | archive |
|---|---|---|
| 22 | `Paintings Variation 2.0` | `Paintings Variation 2.0-46482-2-0.7z` |
| 30 | `Luna's Ironwood Nut Retex` | `Luna's Ironwood Nut Retex-49242-1-1553094439.7z` |

`mudcrab identify` resolves a descriptor from an archive's MD5 and would settle
both, but it needs `NEXUS_API_KEY`. Same shape as Part 19's row 9.

## Row 3 is the next `conflicts_with` call site

> *Once installed, open the Conflicts tab & hide the 5 winning mesh conflicts
> over Katkat's Vegetable Garden.*

The partner is `KatKat's Vegetable Garden`, added in Part 18 — the row Steven
installed after this build flagged it missing. So:

```toml
[[mods.actions]]
action = "file_hide"
conflicts_with = ["KatKat's Vegetable Garden"]
```

on **row 3** (`Improved Fruits Vegetables and Meats Update`). Expect **5** files;
the guide states the count, so it is checkable the way Part 18's four were.
Note the guide says *mesh* conflicts — if the selection comes back larger than
5, `under = "meshes"` is the likely reason and worth trying before assuming a
bug.

## Row 1 repeats Part 16's BETA1/BETA2 question

The guide names **T4UT - CLUTTER_BETA1**. The only archive on disk is
`T4UT - CLUTTER_BETA1-54904-CLUTTER-BETA2-1748437804.7z` — the mod *page* is
BETA1, the *file* is CLUTTER-BETA2. This is the same confusion that made Part 16
a divergence, where Steven redownloaded BETA1 to settle it. **Check with him
before assuming this archive is what the guide meant.**

Row 1 also needs an include filter rather than the whole archive: *"delete
everything except Textures > Clutter > Vinyard and Textures > Clutter
Farmhouse"*. And the guide asks for the mod to be **named**
`T4UT - CLUTTER_BETA1 - Farmhouse & Vinyard`, which the Oracle did not do — its
folder is `T4UTXL - CLUTTER_BETA1`. Another naming divergence to record.

## Rows needing more than a plain install

| row | what |
|---|---|
| 1 | include only `textures/clutter/vinyard` and `textures/clutter/farmhouse` |
| 3 | `conflicts_with` (above) |
| 7 | three optional files, installed separately |
| 9 | **mediafire** → `manual:TD_Lower_Clutter.7z` |
| 11 | BAIN: `00 Core Assets`, `01c Core Book Jackets ESP - Filter Version` |
| 15 | **mediafire** → `manual:TD_Alternative_Books_Covers.7z` |
| 17 | both main and optional, separately (Silver, Gold) |
| 18 | **moddb** → `manual:VKVII_Oblivion_MagesGuild_Clutter.1.7z` |
| 21 | BAIN: `00 Core` only |
| 22 | main + optional, separately (2.0, SI) |
| 23 | BAIN: three subpackages, all named in the guide |
| 30 | `data_folder = "LunasIronwoodNutRetex/Data"` |
| 32 | FOMOD: **Main** textures auto-selected, **Meshes** Normal Size Mesh |
| 34 | six separate installs, all named in the guide |
| 35 | BAIN: `01 - Marbled Style - Alternative` |

Run `mudcrab inspect` on rows 11, 21, 23, 32 and 35 before writing them: every
BAIN row so far has had at least one subpackage spelled differently from the
guide, and Part 20's row 8 (`00 Core` vs `00 core patch`) is the most recent.

## Plugins

Six, all hidden in the Oracle, so all six are expected Part 36 merge-source
differences: `Book Jackets Oblivion.esp`, `Knights - Book Jackets.esp`,
`Book Jackets DLC Misc.esp`, `Alluring Wine Bottles.esp`, `PotionReplacer.esp`,
`SavillaStoneEnhanced.esp`.

## Three non-Nexus hosts

Rows 9 and 15 are mediafire, row 18 is moddb. All three are `manual:`, and all
three carry the caveat in `oracle-dependence.md`: the archives being on disk is
availability, not reproducibility.
