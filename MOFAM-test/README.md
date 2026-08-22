# MOFAM: the case study mudcrab was built against

[MOFAM](https://www.nexusmods.com/oblivion/mods/52949) is a 40-part, ~700-mod
Oblivion guide. This workspace holds the whole of it expressed as a single
mudcrab modlist, and it exists because a declarative installer is easy to
believe in on a ten-mod example and hard to believe in on a real one.

Almost every feature mudcrab has was added because a MOFAM row needed it.

## What is here

| | |
| --- | --- |
| `input/mofam.full.toml` | The modlist. ~700 mods, six merges, a fixed 242-plugin load order. |
| `input/mofam-source.md` | The guide, transcribed. The authoring input. |
| `input/mofam.merges.toml` | The merges alone, for building and inspecting them standalone. |
| `input/mofam.minimal*.toml` | Small lists of behaviourally interesting rows, for fast iteration. |
| `loadorder.txt` | The guide's published load order, used to verify ours. |
| `scripts/run-full.sh` | The whole pipeline against the real list; takes `--section` / `--only`. |
| `notes/open-items.md` | What is still outstanding in this particular build. |

`output/` is generated and gitignored.

## What it established

The list installs end to end and the result has been played. Verification was
per-section against a hand-built reference instance of the same guide (an
"Oracle"), with every difference explained rather than accepted — that discipline
is what found most of the bugs worth finding.

Against that reference, of 737 mods compared:

- **559 byte-for-byte identical.**
- **86** differ only in how a merged-away plugin is retired: mudcrab renames it
  `.mohidden`; the reference unticks it instead. All 86 are inactive on both
  sides, so the effective load order is the same.
- **41** differ in content, and they fall into a few systematic groups — the six
  merges (mudcrab and zMerge allocate FormIDs differently and retain different
  masters, while producing the same record set and reference graph), the repacked
  BSAs (same payload, different internal ordering), the independently-built
  Bashed Patch, and a handful of hand edits made on the reference side.
- The rest are mods the reference has and this list does not, or the reverse.

The load order matches the guide's own published `loadorder.txt` entry for
entry, with one deliberate omission (`Swearing Rats.esp`, which the guide says
may be skipped).

## Running it

Needs the archives. Most resolve from a local MO2 downloads folder via
`--archive-search-path`; anything genuinely missing needs `NEXUS_API_KEY`.

```bash
./MOFAM-test/scripts/run-full.sh --section "12 - WEATHER & LIGHTING"
```

The last stage is `mudcrab diff` against the reference instance, which is the
point of the exercise. Paths to the game, the instances and the downloads folder
are set at the top of the script.

## A caveat about reproducibility

This list pins Nexus **file ids**, which the guide does not — it usually says
"the top file on the page". Those ids were recovered from the reference
instance's `meta.ini` files. That makes the list reproducible in a way the guide
is not, but it also means the pinning came from one person's install rather than
from the guide text. `diff` flags any archive that postdates the guide, so drift
is visible; see [known-issues.md](../docs/known-issues.md#nexus-pinning).
