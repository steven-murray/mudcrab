# Part 33 — Gameplay & Immersive Extras

21 guide rows → 25 mods. Diff: 11 identical, 12 differing, 2 not in the Oracle at
all. Every one is accounted for below.

## New in mudcrab: archives inside archives

Row 3 is the only row in the whole guide whose install instruction is "open the
mod folder and run this .exe". Bank of Cyrodiil is from 2006 and ships as a zip
containing a self-extracting installer.

Nothing has to be run. A self-extracting installer is a Windows executable with
an archive stapled to the end of it, and both `bsdtar` and `7z` open one
directly by scanning past the executable for the archive signature. mudcrab was
rejecting the file on its extension alone, so the fix was recognition, not
execution: a two-byte `MZ` probe, then one `7z l` to confirm the file really is
an archive before claiming it.

That left the nesting. The new `inner_archive` field names an entry that is
itself the mod's content:

```toml
[[mods.archives]]
file_name = "Bank of Cyrodiil-3172.zip"
inner_archive = "Bank of Cyrodiil 1-11.exe"
```

The inner archive goes through the whole layout pipeline; the container's own
files (here, a readme) stay at the mod root; the container entry is dropped as
packaging, which is the row's "optionally delete the .exe". The staged-file
index refuses to predict such a mod rather than reporting the container's
entries as if they were the mod's files — the same line `reject_unmodelled`
already draws for build-from-files mods.

Both capabilities are general. Neither mentions MOFAM.

## Rows 3 + 4 as one mod, and the case bug it sidesteps

Row 4 is the combine-and-repack archetype again: copy the ElevenLabs voices over
the base mod's, repack textures/meshes/sound as `za_bankmod.bsa`, delete the
loose folders, disable the voices mod. Modelled as one mod with two archives, so
layer order expresses "replacing when prompted" and no disabled-mod state
exists.

Result: `za_bankmod.esp`, `za_bankmod.bsa` (547 files, 5,126,781 bytes of
payload) and the readme. **The BSA's 547 paths are exactly the Oracle's 547 loose
asset paths**, compared case-insensitively.

The Oracle did this row by hand and it went wrong twice:

1. **The repack never happened.** `Bank of Cyrodiil` still holds loose
   `meshes/`, `sound/` and `Textures/`, and there is no BSA.
2. **The copy hit the case hazard.** The base mod (unpacked by the SFX) writes
   `sound/voice/`; the voices archive ships `Sound/Voice/`. On Linux those are
   two directories, so the folder now holds **both** — `sound/voice/` with the
   base mod's 31 files and `sound/Voice/` with the addon's 538. Nothing was
   replaced; the two sets sit side by side. mudcrab folds directory names to
   lowercase, so the overlay lands where it was meant to.
3. `Bank of Cyrodiil Voices (ElevenLabs)` is still **enabled** in the Oracle's
   profile (`+` in `modlist.txt`), where row 4 says to disable it.

## Where the guide and the Oracle disagree

| Row | Guide says | Oracle did | Here |
|---|---|---|---|
| 1 | SupreMe Overhaul: "delete the sound folder" | kept all 3234 files | deleted, per the guide |
| 5 | Crime has witnesses: "delete the omod conversion data folder" | kept it (2 files) | deleted, per the guide |
| 11b | Greed Arena Voiced Addon | not installed | installed |
| 12 | Camping | not installed | installed |

Rows 11b and 12 are unambiguous rows with archives already on disk, so they are
built. Both show as "not in the Oracle" in the diff, which is the correct
reading, not a defect.

Row 1's is the large one: 3234 of SupreMe Overhaul's 3238 files are voice lines.
Following the guide throws away almost the whole mod's bulk. Flagged rather than
second-guessed — see report 1.

## Row 18: three mods the Oracle installed to the wrong place

Base Object Swapper reads its rules from `Data/BaseObjectSwapper/*.ini`. Each of
the three integration archives is exactly one ini inside a `BaseObjectSwapper/`
folder — a single top-level directory, which is precisely the shape wrapper
detection strips.

The Oracle's three copies have the ini at the **mod root**, where the OBSE plugin
will not find it. That the folder is meant to survive is decidable without the
Oracle: `OCRAFT - Stations for Sale` (Part 30) has a proper `BaseObjectSwapper/`
folder in both instances, and the archives themselves declare the path.

`layout = "simple"` says the archive root is the data folder, which keeps the
folder. The resulting diff — one file at `baseobjectswapper/X.ini` here against
`X.ini` there, three times — is mudcrab being right.

## Guide defect

Row 11a names the file to edit as `GreedArena.ini`. The archive ships
`AoG - Greed Arena.ini` and no other ini. Both edits (`aogArn.arena24`,
`aogArn.corpseLoot`, 0 → 1) match the Oracle's values, so the intent is not in
doubt; only the filename is wrong.

## The rest of the diff

- **6 plugins hidden in the Oracle** — `WeightlessAmmoConsumablesPotions.esp`,
  `weightlessstones.esp`, the two `aesrespawning*` plugins (all Prebash), and
  `Improved FG/MG Patch.esp` (Late Loaders). Active here until Part 36.
- **`AoG - Greed Arena.esp` content differs** — ours is the archive's 233,114
  bytes exactly; the Oracle's 232,260 is the QAC-cleaned copy. This row is
  `[QAC]` and QAC is deferred list-wide.
- **Two ConScribe logs differ** — `Fundament.log` and `Static Log.log`. These are
  runtime logs. The Oracle's carry mtimes from playing the game (16 Aug 2026) and
  its `Static Log.log` is empty; ours are the archive's. Expected.
- **Two POST-GUIDE flags** — AsteriaSennall's Fixes and the ConScribe settings
  are both hosted on the guide's own mod page and dated 2025-03-30, twelve days
  after the guide's own date. The guide author updates their page after
  publishing; not drift.

Row 6 ships a plugin with the same filename as row 5's, so the two mods declare
one plugin between them and MO2 mod order decides which file wins. That is what
the guide intends by listing them as separate rows.
