# Part 26b — Town & City Extras (TACE)

Twenty rows, every one of them a source for the TACE merge.

**20 of 20 differ, and all 20 differences are the same thing**: the plugin is
hidden in the Oracle because TACE is built there. Seven of them additionally
differ in the plugin's bytes, and those seven are exactly the seven `[QAC]`
rows — each verified byte-identical to what its archive ships, so the
difference is the Oracle's cleaning pass and nothing else.

| `[QAC]` row | ours | Oracle |
|---|---|---|
| 5 Cheydinhal Peach Tree Island | 85183 | 50151 |
| 7 Chorrol Castle Courtyard | 36800 | 27183 |
| 9 Chorrol Park | 105622 | 55224 |
| 12 People Live Here (Skingrad) | 171703 | 171484 |
| 14 Anvil the city of Dibella | 121900 | 90093 |
| 15 Anvil Morning Glory | 5890 | 5787 |
| 20 Knights of the Thorn Lodge | 67540 | 45296 |

## Why the merge is not here

An earlier attempt built TACE at this point, to keep the load order under 255.
It cannot be built here:

    Error: master Tales of Cyrodiil.esp (required by TACE Consistency Patch.esp)
    is not in the load order.

TACE's consistency patch takes a **Part 28** mod as a master (row 37, Tales of
Cyrodiil). In guide order the question never arises, because Part 28 comes long
before Part 36. It only bit because the merge was being built early to make
room.

The load order therefore runs over Oblivion's limit from here until Part 36 —
**266** with this section in. That is now a warning rather than an error, on
Steven's call: over the limit means the list cannot be *played*, not that it
cannot be *built*, and every merge that brings it back under lives in Part 36.

## Structure

Four rows needed more than a plain archive entry:

- **Row 2** `data_folder = "Enhanced Cyrodiil - Cities/Standard"` — the archive
  also ships an Alternative build.
- **Row 8** `data_folder = ".../01 Classic Bark"` — one of four bark variants.
- **Row 10** BAIN, `["00 Core", "01 Vanilla"]` — the other two `01` options are
  Better Cities and Open Cities builds of the same plugin.
- **Row 15** `data_folder = "Anvil Morning Glory/Data"` plus an `exclude` for
  the three colour variants the guide deselects.

## Not applied

Row 12 `[QAC]` asks for three wild edits to be removed in xEdit — a cell block,
a reference, and a sub-block override. mudcrab has no scripted record deletion,
so this is not done. Same gap as Part 26a row 6.
