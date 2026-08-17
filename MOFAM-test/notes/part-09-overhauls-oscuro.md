# Part 9 (Overhauls: Oscuro)

Eleven guide rows, fourteen Oracle folders. The plan called this the hardest
early section and it was, though not for the reasons expected: the BSA round
trip turned out to be routine, and the two things that actually stopped work
were a missing download and a corrupt one.

**Status: 11 of 14 mods authored and installed — 5 identical, 6 differing and
all six explained. Two rows blocked on downloads, one step deferred by the
guide's own instruction.**

| difference | mods | why |
|---|---|---|
| plugin hidden in the Oracle | 3 (10 plugins) | Part 36 merge sources, as in Parts 7-8 |
| `Oscuro's_Oblivion_Overhaul.bsa` size | 1 | no payload dedup — contents identical |
| a readme | 1 | dropped when the `Data/` wrapper is unwrapped |
| 32 body meshes | 1 | guide says C-Cup, Oracle used E-Cup — see below |

## Blocked: two rows need Nexus downloads

Neither is a modelling problem. Both need `NEXUS_API_KEY` set, or the files
fetched by hand.

### Row 7 — EVE HGEC Equipment Replacer for OOO

The guide asks for **"EVE for Oscuro Oblivion Overhaul 1_3 BAIN"**. The only
copy on this machine is
`EVE for Oscuros Oblivion Overhaul 1_3 OMOD-24078.omod` — the **OMOD** build of
the same mod, which is what the Oracle installed.

An OMOD is a zip holding `data` (one concatenated 7z blob of every file),
`data.crc` (the manifest), `plugins`, `config` and a `script`. Reading one means
implementing OBMM's container format: splitting the blob by the CRC manifest's
sizes. That is real work for a single mod, and pointless when the guide asks for
the BAIN archive anyway — which is a plain 7z that mudcrab already handles.

**Action: download the BAIN file from mod 24078.** Then the row is an ordinary
three-subpackage BAIN entry (00 Core, 10 Equipment Replacer Upperbody - Normal
C-Cup, 15 Equipment Replacer Lowerbody - Normal), identical in shape to row 8.

### Row 11, second archive — OOO Enhanced 5.3 Resources

`OOO Enhanced-47187-5-33-1748819369.7z`, 3.5 GB, **is corrupt**:

```
7z:      Headers Error / There are data after the end of archive
bsdtar:  Unexpected Property ID = 73
```

Two independent readers reject it, so this is the download and not a tool
quirk. `fileID=1000042163` on mod 47187.

**Action: re-download.** Until then the OOO Enhanced row installs only its first
archive, and the Oracle's `OOO Enhanced 5.3 (03.25) - Resources` folder (3633
files) has no counterpart on our side.

## Deferred by the guide: OOO Enhanced's conflict-hiding and repack

Row 11's post-install block hides files that lose conflicts against
**Colourful Clothing** (Part 24) and win over **Waalx's Animals and Creatures**
(Part 10), deletes the hidden ones, then packs the remaining textures into
`OOO Enhanced.bsa`.

None of those mods exist yet, so the step cannot be performed now — and the
guide says as much itself:

> Note we will return to this install of the Resources to optimise the build
> later, a REMINDER! will be added once Colourful Clothing Collection in Part 24
> has been concluded.

The packing has to wait with it: packing first would bake in exactly the files
the hiding is meant to remove. **Revisit after Part 24**, once the Resources
archive is downloadable again.

This also answers the plan's open question about conflict-tab hiding needing a
whole-modlist design pass. It does — but not yet, and not for this section.

## OOO Enhanced: the two halves must be the same version

Row 11 installs two archives, plugins and resources, and they have to match.
This was got wrong first time and is worth recording, because the mistake came
from trusting the wrong source.

`add --from-oracle` took the plugin half from the Oracle's `meta.ini`, which
records **5.33**. The guide asks for **"5.3 - PreRelease & 5.3b Resources"**.
Nobody noticed until the resources half arrived as 5.3b and the diff showed 197
differing meshes — a mismatched pair, 5.33 plugins against 5.3b resources, which
is a combination neither the guide nor the Oracle ever ran.

The Oracle runs a coherent updated pair (5.33 + 5.33). That is not available:
**5.33 has since been pulled from Nexus.** So this list uses the guide's exact
pair, 5.3 PreRelease + 5.3b, and diverges from the Oracle on both halves the way
MOO did before the Oracle was downgraded to match.

The subpackage names are identical between 5.3 and 5.33, so the seven
selections are unchanged.

The lesson is the one from Part 7 restated: `--from-oracle` is a scaffold for
provenance, not an authority on *which file the guide meant*. Where a guide row
names a version, the row's version comes from the guide.

## Guide and Oracle disagree — `Seamless - HGEC Female` cup size

**Unresolved. Following the guide; 32 meshes differ from the Oracle.**

Guide row 8 lists three subpackages, the same three as row 7:

> 00 Core / 10 Equipment Replacer Upperbody - **Normal C-Cup** / 15 Equipment
> Replacer Lowerbody - Normal

The Oracle's `meta.ini` records what it actually installed:

```
BAIN Installer\option1=10 Equipment Replacer Upperbody - Normal E-Cup
```

Rows 7 and 8 are a matched pair — Seamless exists to remove the seams from
EVE's body, so the two have to be built to the same figure or the meshes do not
line up. The guide names C-Cup in both rows, which is self-consistent; the
Oracle used E-Cup in row 8 and installed row 7 from an OMOD whose own script
chose, leaving no record of what it picked. Comparing meshes cannot settle it:
Seamless replaces EVE's files outright, so their sizes differ either way.

Following the guide, as in Part 7's Khajiit head, which the user later confirmed
the guide had right. **This one is visible in game on female NPCs wearing the
affected armours**, so it is worth a look once row 7's EVE download lands.

## Rows 1 + 3 as one mod: the combine/repack archetype

The guide's procedure is six steps: install the BSA, extract it with BAE,
install the voice files as a separate mod, paste them in, repack with BSArchPro
over the original archive, delete the loose files, and disable the voice-file
mod.

That is **one mod assembled from two archives**, and writing it as one makes the
last step vanish — there is no second mod left to disable. mudcrab reads and
writes BSAs natively, so neither BAE nor BSArch under WINE is involved:

```toml
[[mods.archives]]   # the BSA
[[mods.archives]]   # the voice files
[[mods.actions]] action = "extract_bsa"
[[mods.actions]] action = "pack_bsa"   prune_packed = true
```

The arithmetic confirms it: the shipped BSA holds **4406** files, the voice
archive **1554**, and the repacked archive has **5960** — exactly the Oracle's
count, in exactly the Oracle's 721 folders.

The Oracle keeps an empty `OOO Voice FIles` folder from having done it the
guide's way. `diff` reports that folder as missing from ours, correctly, and it
holds nothing.

## `prune_packed`, and why the hand-written glob had to go

The first attempt paired `pack_bsa` with a `file_prune` naming the folders to
delete. That list was wrong twice in one line:

- `menus` — not in this archive at all, so the prune failed loudly (which is
  what the Part 5 fix was for).
- `sound` — the extracted BSA writes `sound/`, but the voice archive stages
  `Sound/`, and globs are case-sensitive. The pattern matched one and left the
  other.

`pack_bsa` knows precisely which files it wrote, so `prune_packed = true` now
deletes exactly those and nothing else. A guess at an archive's top-level layout
is not something worth making twice.

The first cut of `prune_packed` then reproduced the *same* case bug from the
other side: it rebuilt each path from the archive, and a BSA stores names
lowercased, so `sound/voice/...` found nothing where the tree held
`Sound/Voice/...`. It reported a healthy-looking 4406 deletions and left OOO's
1554 voice files loose, shadowing the archive that contained them. It now
matches against what is on disk. The repack is 5960 files either way; only the
prune count told the truth, which is worth remembering — **a plausible number is
not a verified one**.

The repacked archive matches the Oracle's on every count that describes its
contents: 5960 files, 721 folders, 1,069,586,148 payload bytes, and identical
asset-kind flags (`0x1b`). It is larger on disk (1.07 GB against 947 MB) purely
because BSArch deduplicates identical payloads and mudcrab does not — the same
2% effect as Part 5's Evenstars, at a larger scale.

## LOOT can hang, and now cannot hang for long

The first full run of this section wedged for 22 minutes: `LOOT --auto-sort`
sitting in `ppoll` with one second of CPU, waiting on something it never
explained. It sorts this list in well under a minute when healthy.

`loot-sort` now kills LOOT after 180 seconds and says what to do about it. A
build that has already spent twenty minutes staging a gigabyte should not then
stall silently on the last step, with no way to tell a hang from slow progress.

## Two new actions

- **`extract_bsa`** — unpacks a BSA the mod ships and deletes the archive
  (keeping it would mean the next `pack_bsa` folded the old archive into the new
  one). Replaces the guide's BAE step.
- **`file_move`** — relocates a staged file, for the guide's "move X to the
  optional folder". Row 9 parks
  `OOOShiveringIsles_Optional_CrucibleEdits.esp` in `optional/`, MO2's
  convention for a plugin kept but out of the load order.
