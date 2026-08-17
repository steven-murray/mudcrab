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

The Oracle has now performed it, so the answer can be read off. **The first
attempt at reading it off was wrong in three separate ways**, all corrected
during Part 10 once WAC was installed and the two sides could actually be
intersected. Recorded here because each mistake has a lesson.

Against the 5.3b archive's 8427 files, **1738 are removed** — not 1024. The
recomputation walks both installs, strips the Oracle BSA's doubled prefix (see
below), and lands with **nothing unaccounted for**:

| files | area | claimed by |
|---|---|---|
| 466 | `meshes/realswords` (270), `textures/realswords` (196) | WAC |
| 98 | `textures/menus` | WAC |
| 13 | creatures, clutter, armor, idleobjects | WAC |
| 898 | `meshes/clothes` (425), `textures/clothes` (473) | the three clothing mods |
| 250 | `textures/menus`, `menus50`, `menus80` | the three clothing mods |
| 13 | `thumbs.db` | nobody — plain junk |

**577 to WAC, 1148 to clothing, 13 to neither.** The conflict set (1725) is in
`ooo-enhanced-conflict-hidden-files.txt`; the `thumbs.db` are separate, in
`ooo-enhanced-thumbs-db.txt`, because they are not conflict-driven and a
`conflicts_with` selector will never produce them.

### What the first pass got wrong

**1. The count was never in the file.** The notes said 1024 paths; the file has
only ever held 247. Nobody checked the number against the artefact it described.
This is Part 9's own lesson — *a plausible number is not a verified one* —
recurring one level up, in the notes rather than the code.

**2. Meshes were missed entirely.** The original table lists only textures. 701
of the 1738 removals are meshes (`meshes/clothes`, `meshes/realswords`,
`meshes/creatures`, `meshes/idleobjects`). The guide's closing step says to pack
"the textures folder", which is easy to read backwards as "textures are all this
step touches" — but the hiding happens across the whole mod.

**3. `Colourful Clothing - Collection - Seamless OCOv2` is the mod's real
name**, spelled exactly as the guide spells it, suffix and British spelling and
all. The table below used to claim the suffix did not exist. It does; only the
two `AI Enhanced` mods use the American spelling. Correcting that took the
unexplained remainder from 926 to 13.

The follow-up after Part 24 is unchanged in shape — prune, then `pack_bsa` with
`prune_packed` — but this list is now a **test fixture, not the mechanism**. See
`conflict-resolution-design.md`: a real build has no Oracle, and the derivation
below shows the intended mechanism working.

### Deriving it instead of reading it — validated in Part 10

With WAC installed, the WAC half stops being something to look up. Intersect
OOO Enhanced's file set with WAC's BSA contents and **577 files fall out** — the
exact set the Oracle removed under "Winning File conflicts → Overwritten mods".
Do the same against the three clothing mods and the other **1148** fall out. The
only residue is 13 `thumbs.db`.

That is precisely the `conflicts_with` selector's algorithm, run by hand. The
design is sound; what remains is to build it.

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
| Waalx's Animals and Creatures | `WAC Waalx Animals & Creatures` |

`Colourful Clothing - Collection - Seamless OCOv2` was listed here as a third
mismatch. It is not one — that is the mod's exact name. Assuming the guide was
wrong a third time, because it had been wrong twice, put 926 files in the
"unexplained" pile until Part 10 checked the mod list instead of the memory of
it.

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

## The Oracle's `OOO Enhanced.bsa` is packed one level too deep

Found while recomputing the numbers above, and it affects the Oracle in play,
not just our comparison.

Every path inside the Oracle's `OOO Enhanced.bsa` is stored as
`textures\textures\...`. A known-good archive from the same install stores
`meshes\...` with no doubling, so this is not how mudcrab reads BSAs — it is
what the file contains:

```
Oscuro's_Oblivion_Overhaul.bsa   meshes\alexanderw\impwings.nif
OOO Enhanced.bsa                 textures\textures\akcreatures\lichking\boots.dds
```

Oblivion asks the archive for `textures\<path>`. Nothing answers to
`textures\textures\<path>`, so **all 3580 textures in that archive are
unreachable** and the game silently falls back to whatever else provides them.
The cause is BSArch being pointed one directory above the one intended, so the
`textures` folder name is included *and* prefixed.

Worth knowing before playing the Oracle, alongside the merge-source note below.
Our own `pack_bsa` writes paths relative to the folder it is given, so the
deferred repack after Part 24 will not reproduce it — but this is exactly the
kind of error a size-only comparison would never catch, which is why the
recomputation walked paths rather than counting bytes.

## A side effect of reinstalling in the Oracle

Reinstalling `OOO Enhanced` cleared its eight `.mohidden` plugins, because a
fresh install has none. All eight are Prebash merge sources, so the Oracle now
has both the merge *and* its sources active — which double-loads those records
until the hides are reapplied or the merge is rebuilt.

Harmless for our comparison (it makes that mod match ours exactly, since ours
are active too), but worth knowing before playing the Oracle. The same applies
to any Oracle mod reinstalled after its merge was built.
