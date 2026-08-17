# The profile INI was being reset on every install

Found by the INI audit that Part 13 made obvious was necessary. It is the most
consequential bug of the overnight run, and `diff` structurally could not have
caught it.

## What was happening

`prepare_mo2_profile` copied the game's `Oblivion.ini` over the MO2 profile's
copy **unconditionally, at the start of every install run**.

Every `ini_set` with `scope = "game"` writes to that profile copy. So in a
section-by-section build:

1. Build Part 11 → the `[Grass]` block lands.
2. Build Part 12 → the profile INI is reset to vanilla. Part 12 has no
   Oblivion.ini edits, so nothing re-applies. **The grass settings are gone.**
3. …repeat for every subsequent section.

Only the most recently built section's game-scoped settings survived. When the
audit ran, **six of eighteen were wrong on disk**:

| setting | wanted | was |
|---|---|---|
| `bFull Screen` | 0 | 1 |
| `SFontFile_2` | `DarN_Kingthings_Petrock_14.fnt` | `Kingthings_Shadowed.fnt` |
| `SFontFile_3` | `DarN_Kingthings_Petrock_16.fnt` | `Tahoma_Bold_Small.fnt` |
| `SFontFile_4` | `DarN_Oblivion_28.fnt` | `Daedric_Font.fnt` |
| `fGrassEndDistance` | 8000 | 3000 |
| `fGrassStartFadeDistance` | 7000 | 2000 |
| `iMaxGrassTypesPerTexure` | 5 | 2 |
| `bUseRefractionShader` | 0 | 1 |

The three `SFontFile_*` entries are the DarNified UI font paths. **That is a
broken interface** — and it is the same failure that cost a play session earlier
in this build, when it was diagnosed as an `ini_set` spacing bug and fixed at
the wrong level. The spacing bug was real; this was underneath it.

## Why nothing noticed

- `mudcrab diff` compares **mod folders**. Profile INIs are not mod folders.
- The install reported success every time, because every individual action *did*
  apply — to a file that was about to be thrown away by the next run.
- Building the whole list in one run would have masked it entirely. It only
  shows up in the section-by-section workflow this project is built around.

## The fix

Seed the profile `Oblivion.ini` once, from the game directory, and never
overwrite it again. `ini_set` is idempotent and edits in place, so an existing
profile INI is the accumulated result of every section built so far — which is
exactly the thing worth keeping.

Regression test in `tests/install_mo2_command.rs`: run install, add a marker
line no action would reproduce, run install again, assert the marker survives
*and* that the run's own `ini_set` still applied.

## After the fix

Rebuilt the four sections that carry game-scoped edits — `UI & UX IMPROVEMENTS`,
`OBSE PLUGINS`, `11 - BASELINE TEXTURES`, `13 - OBLIVION REALM`. **All 18
settings now match the modlist.**

Nine still differ from the Oracle, and every one is a known guide-vs-Oracle
disagreement rather than a defect: the Oracle never applied
`bUseRefractionShader=0`, has different grass distances, and has
`bGrassPointLighting=1`. Those are in report 1.

## The lesson worth keeping

**A verification tool that covers most of the output is not a verification tool
that covers the output.** Every section from 5 to 17 reported clean diffs while
this was silently true. The audit that found it existed only because Part 13
happened to expose that INIs are outside `diff`'s remit — and the same question
should be asked of anything else the build writes that `diff` does not read:
`plugins.txt`, `modlist.txt`, `archives.txt`, and the game-root files Part 14
pinned.
