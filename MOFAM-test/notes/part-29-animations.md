# Part 29 — Animations

Ten rows. **7 of 10 identical.** The other three differ only by a Prebash merge
source hidden in the Oracle: Faster Horses, Mehrunes Dagon Walk (the mergeable
build from row 8) and both Unique Wolf Animations plugins.

## Row 7 ships six other people's mods

`Mehrunes Dagon Walking Animation-52126-1-1-1656778512.zip` contains, at its
root, **six unrelated mod archives** the author appears to have packaged by
accident:

    Nirnroot retexture-45018-1-0.rar
    Oblivion Races Unlocked 3.5-48323-3-5.zip
    Painters Touch 1024-43678-1-0.7z
    Paladin Mod - OBME Patch-49096-1-0-1577041773.7z
    Paladin Mod 1_4 - Manual-41095-1-4.7z
    Pet your animals-48398-3-0-1542606226.zip

Extracting the archive faithfully puts all six in the mod folder, which is
exactly what happened the first time this row ran — mudcrab was behaving
correctly and the archive is wrong. The row now uses `include = ["meshes/**"]`,
which takes only what the mod is and incidentally satisfies the guide's "delete
MehrunesDagonWalk.esp" by never unpacking it.

Worth knowing because nothing about the install *looked* wrong: the mod folder
had the meshes the game needs, plus 14 MB of inert junk beside them.

## Structure

- **Row 4** `data_folder = "Core"` — the archive also ships Variations.
- **Row 5** `data_folder = "Stylish Jump 1.0/Normal"` — Feminine and Optional
  are the alternatives.
- **Row 9** two archive entries over one file, to lift one patch plugin out of
  `01 Patches` into `00 Core`, the same shape as Part 26a's Legion Forester
  Outposts.
