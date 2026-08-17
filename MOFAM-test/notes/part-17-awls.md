# Part 17 (Animated Window Lighting System)

Three guide rows, five Oracle folders (row 2 installs three archives).

**Status: 5 compared, 4 identical, 1 differing — AWLS itself, where our
installer answers and the Oracle's disagree. Needs Steven's eye; it is a visual
choice, not a defect.**

## AWLS is a real FOMOD, and the guide answers it in full

The plan flagged this row as possibly an OMOD or Wizard script. It is neither —
a proper `FOMod/ModuleConfig.xml` with **13 groups across 8 steps**, several of
them conditional on earlier answers. The guide lists an answer for every one.

All 13 are written into the TOML, not just the two that differ from the
installer's defaults (`BomretSI` and *"Choose a combo pack"*). Stating a default
explicitly is what makes it survive the archive changing its mind about what the
default is — and this archive is at 5.6.3, well past the guide's writing.

### Three of the guide's answers are misspelled

| the guide writes | the installer offers |
|---|---|
| Brunbek Yellow Multi-Colour (Recommended) | **Brumbek** Yellow Multi-**Color** (Recommended) |
| Blue-purple (Recommended) | Blue-**P**urple (Recommended) |
| More Colours (Recommended) | More Col**o**rs (Recommended) |

A typo and two British spellings of an American author's option names. Harmless
to a human clicking the nearest match, fatal to an exact-match selector — the
same class of failure as Part 9's `Colourful`/`Colorful`.

## Where we differ from the Oracle

25 files in one mod: 20 with differing content, 2 only in ours, 3 only in the
Oracle. Every one is a window texture or a Shivering Isles mesh:

- ours has `textures/architecture/city/dementia/dementiawindowAWLS*`
- the Oracle has `Textures/Architecture/Bravil/glasswindow11L.dds` and
  `Textures/Architecture/ImperialCity/ictemplewindow01L*`
- the 20 content differences are `city/mania/*`, `palace/exterior/*` and
  `dementiawindow*` — Shivering Isles settlements, palace and Bliss/Crucible

**This is a different set of installer answers, not a build error.** One
concrete pointer: the guide says *Imperial City Temple Windows: Blue*, and
"Blue" is the option that installs **nothing** (0 install entries). The Oracle
has two `ictemplewindow01L` files that only a non-Blue answer produces. So the
Oracle answered at least that group differently from the guide.

MO2 records BAIN subpackage choices in `meta.ini` but **not FOMOD choices**, so
there is no way to read the Oracle's answers back. Reconstructing them would
mean guessing from the installed files across 13 groups.

Our build follows the guide's 13 answers exactly. Flagged for Steven to decide:
either the Oracle gets reinstalled to the guide's answers, or the guide's list
is out of date for AWLS 5.6.3 and the Oracle's look is the intended one.

## The rest of the section is clean

The three `TD and AWLS patch` archives and `Diverse Chapels Vanilla` (BAIN,
`00 Core` + `10 AWLS Support`) are byte-identical to the Oracle.
