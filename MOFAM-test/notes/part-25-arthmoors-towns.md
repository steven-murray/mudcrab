# Part 25 — Arthmoor's Towns

Nine rows: eight villages, each a base archive plus a separately-hosted
voice-acting archive that has to be overlaid and packed into one BSA, then a
patch row out of Dispensation's collection.

## The archetype, and how it is modelled

The guide's instruction for each village is the same shape:

> Drag the meshes, sound & textures folder into BSArch Pro from Na
> Drag the sound folder from Nb into BSArch Pro & select Replace All
> Pack the mod ensuring the archive within Na is named '<Village>' then delete
> the now-loose folders & disable mod Nb.

Modelled as **one mod with two archives**, not two mods. mudcrab installs a
mod's archives in order into the same folder, later overwriting earlier, which
is exactly what "Replace All" does — so the b mod never exists as an
installed-but-disabled entry and no "disabled" state has to be invented.

`pack_bsa` needs no `include`: a BSA cannot hold a file outside a folder, so the
`.esp` and `.txt` at the mod root stay loose by themselves and everything below
`meshes/`, `sound/`, `textures/` and `video/` goes in. That is the Oracle's
shape in all eight cases.

**Verified before authoring, not after.** For each village, the Oracle's BSA
path set was compared against `(a's game folders) ∪ (b's sound)` and its exact
payload byte count against both the a-wins and b-wins compositions:

| | files | a-wins | b-wins | Oracle |
|---|---|---|---|---|
| Feldscar | 1849 | 129881894 | **123583707** | 123583707 |
| Frostcrag Village | 815 | — no overlap — | | 43056869 |
| Gottshaw Village | 275 | — no overlap — | | 18743275 |
| Molapi | 781 | 30591895 | **26314383** | 26314383 |
| Reedstand | 496 | — no overlap — | | 24289225 |
| Sutch Village | 2449 | — no overlap — | | 254520184 |
| Urasek | 1161 | **63083533** | 58223187 | 63083533 |
| Vergayun | 558 | — no overlap — | | 32403608 |

Overlap exists in exactly the three rows where the guide says "Replace All",
which is what that instruction is for. Two of the three took. Urasek did not —
see below.

## Guide problems

1. **Row 5b names the wrong archive.** "Drag the sound folder from **4b**" —
   Molapi's VA archive, not Reedstand's. 4b holds `sound/voice/Molapi.esp/`,
   which `Reedstand.esp` cannot address. Read as 5b.
2. **Row 7 omits Urasek's own `sound/`.** The first bullet says to drag "meshes
   & textures" from 7a, the third says to delete the loose "meshes, sound &
   textures" folders. Taking the first literally drops 440 voice files the mod
   ships. Read as including sound, which is also what the Oracle contains.
3. **Reedstand ships a `video/` folder the guide never mentions.** Packed here,
   and packed in the Oracle.

## Oracle disagreements

**Urasek's VA replacement did not take.** The guide says "Replace All" for rows
1, 4 and 7. Feldscar and Molapi hold the VA bytes for every overlapping file;
Urasek holds the base mod's, for all 608 of them. Ours follows the guide, so
Urasek's BSA is 58223187 bytes of payload against the Oracle's 63083533.
This is the one real content difference in the section.

## What is left, and why

`Compatibility Patches for Arthmoor's Mods` differs only by
`DispMiscPatch_Ducks and Swans - Reedstand Patch.esp` being hidden in the
Oracle — a Part 36 Prebash merge source, hidden there because that merge is
built. Same class as the other 50-odd merge-source hides.

Every village BSA now has **the same size, the same file list and the same
payloads** as the Oracle's. What remains is the archive-flags word (see
`notes/bsa-header-flags.md`) and the physical ordering of the payload region,
neither of which is content.

Reedstand additionally declares `0x100` (miscellaneous) in its *file* flags
where BSArch did not: it holds a `video/` file, and Bethesda's own
`Oblivion - Misc.bsa` sets that bit for exactly this kind of content. Ours is
the more accurate declaration.
