# Part 9 (Overhauls: Oscuro)

Eleven guide rows, fourteen Oracle folders. The plan called this the hardest
early section and it was, though not for the reasons expected: the BSA round
trip turned out to be routine, and the two things that actually stopped work
were a missing download and a corrupt one.

**Status: 13 mods, 8 byte-for-byte identical — 5 identical, 6 differing and
all six explained. Two rows blocked on downloads, one step deferred by the
guide's own instruction.**

| difference | mods | why |
|---|---|---|
| plugin hidden in the Oracle | 2 | Part 36 merge sources, as in Parts 7-8 |
| `Oscuro's_Oblivion_Overhaul.bsa` size | 1 | no payload dedup — same 5960 files, same payload bytes, same flags |
| loose textures vs a packed BSA | 1 | the repack is deferred until after Part 24, deliberately |
| a readme | 1 | dropped when the `Data/` wrapper is unwrapped |

## Both download blockers are resolved

Two rows were blocked on downloads; both are in.

- **Row 7 (EVE)** now uses the BAIN build the guide names,
  `nexus:oblivion/24078/42364`. The Oracle had installed the **OMOD** build; it
  has since been reinstalled from the same BAIN with the same three
  subpackages, so the two agree.
- **Row 11's Resources** is `nexus:oblivion/47187/1000041194`, the 5.3b file.
  The 3.5 GB `OOO Enhanced-47187-5-33-...7z` on this machine is a **corrupt**
  download of a different build; 7z reports a headers error and bsdtar an
  unexpected property id. It is not used.

## OOO Enhanced: the two halves must be the same version

Row 11 installs two archives, plugins and resources, and they have to match.
Getting it wrong is easy and the diff is what caught it.

`add --from-oracle` took the plugin half from the Oracle's `meta.ini`, which
recorded **5.33**. The guide asks for **"5.3 - PreRelease & 5.3b Resources"**.
That went unnoticed until the resources half arrived as 5.3b and the diff showed
197 differing meshes — a mismatched pair that neither the guide nor the Oracle
ever ran.

**Resolved**: 5.33 has since been pulled from Nexus, so the guide's pair is the
only coherent option. Both halves are now 5.3 PreRelease
(`nexus:oblivion/47187/1000040942`) plus 5.3b Resources, and the Oracle was
moved to match. Subpackage names are identical between 5.3 and 5.33, so the
seven selections are unchanged.

The plugin half is the one archive here that neither MO2 nor a hash lookup could
identify — it lives in the mod's OLD FILES section, which Nexus's MD5 index does
not cover, and MO2 recorded `fileID=0`. That is what `identify`'s file-list
fallback exists for.

The lesson restates Part 7's: **`--from-oracle` is a scaffold for provenance,
not an authority on which file the guide meant.** Where a guide row names a
version, the version comes from the guide.

## The conflict-hiding step, and what it actually removes

Row 11's post-install block hides files conflicting with **Colorful Clothing**
(Part 24) and **WAC** (Part 10), deletes them, then packs the remaining textures
into `OOO Enhanced.bsa`. Neither mod exists in our build yet, and the guide says
to return after Part 24, so **this stays deferred** — packing early would archive
exactly the files the hiding removes.

The Oracle has now performed it, so the answer is read off rather than guessed.
Against the 5.3b archive's 8427 files, **1024 are removed**, listed in
`ooo-enhanced-conflict-hidden-files.txt`:

| files | area | conflicting mod |
|---|---|---|
| 472 | `textures/clothes` | Colorful Clothing (AI Enhanced upper/middleclass, and the Collection) |
| 321 | `textures/menus` | WAC |
| 196 | `textures/realswords` | WAC |
| 28 | `textures/menus50`, `menus80` | WAC |
| 7 | creatures, clutter, armor | WAC |

Plus 13 genuine deletions, mostly `thumbs.db`.

That makes our follow-up much simpler than the plan feared: a `file_prune` of
1024 recorded paths, then `pack_bsa` with `prune_packed`. **No conflict-tab
logic is needed, because the answer is already written down.** The plan's open
question about conflict hiding requiring a whole-modlist design pass is answered
for this section: not needed.

### "Enable Parsing of Archives" is the load-bearing detail

The guide mentions it in passing and it decides whether this step works at all.
**WAC ships its assets inside a BSA**, so without archive parsing MO2's conflict
tabs show nothing against it, and two of the guide's three categories look like
they simply do not apply. A first pass found only the clothing conflicts for
exactly that reason.

### The guide's mod names do not match the mods

The other reason a first pass comes up short, and a trap that will recur:

| the guide says | the mod is actually called |
|---|---|
| AI Enhanced - Colourful Clothing - Upperclass + Middleclass | `AI Enhanced - Colorful Clothing - Upperclass` **and** `- Middleclass` (two mods, American spelling) |
| Colourful Clothing - Collection - Seamless OCOv2 | `Colorful Clothing - Collection` (no `- Seamless OCOv2` suffix) |
| Waalx's Animals and Creatures | `WAC Waalx Animals & Creatures` |

### Hide, then delete, *then* pack

The ordering matters: *"search for 'mohidden' & delete the files. **Lastly**,
BSArch the textures folder"*. Packing before deleting puts the hidden files
*into* the archive. It still works — the game asks for `x.dds` and the archive
holds `x.dds.mohidden` — but the archive carries them. On the first attempt that
was **1024 of 4604 files, about 467 MiB of a 1892 MiB archive**, unreachable.
Since repacked.

## Guide and Oracle disagree — `Seamless - HGEC Female` cup size

**Resolved: the guide was right.** The Oracle has been reinstalled with C-Cup on
both EVE and Seamless, from the BAIN build, so the two now agree.

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

Followed the guide, as in Part 7's Khajiit head. That is now two out of two for
the guide over the Oracle where the two disagreed, which is the whole reason for
the practice.

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

## A side effect of reinstalling in the Oracle

Reinstalling `OOO Enhanced` cleared its eight `.mohidden` plugins, because a
fresh install has none. All eight are Prebash merge sources, so the Oracle now
has both the merge *and* its sources active — which double-loads those records
until the hides are reapplied or the merge is rebuilt.

Harmless for our comparison (it makes that mod match ours exactly, since ours
are active too), but worth knowing before playing the Oracle. The same applies
to any Oracle mod reinstalled after its merge was built.
