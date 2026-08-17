# Part 11 (Baseline Textures)

28 guide rows, 29 Oracle folders, 23 mods here. The largest section so far and,
once scoped, one of the easiest: **26 of 27 archives need no layout declaration
at all** — no BAIN, no FOMOD, plain auto-detected data folders. All the work is
in what happens after extraction.

**Status: 23 compared, 13 identical, 10 differing, all ten explained.**

| difference | mods | why |
|---|---|---|
| BSA size + dummy `.esp` | 5 | no payload dedup; `.esp` differs by design (Part 5) |
| plugin hidden in the Oracle | 2 | Part 36 merge sources, as in Parts 7-10 |
| plugin not yet cleaned | 1 | the deferred QAC and Worldspace removal |
| readme / settings image | 3 | files outside the data folder, dropped when it is unwrapped |

The five BSAs are **identical in contents** — 1294, 696, 2909, 1014 and 243
files, in 144, 13, 295, 61 and 1 folders, matching the Oracle on every count.
Only the byte size differs, the same BSArch payload-dedup gap as Parts 5, 9
and 10.

## Rows 1-6 are one mod, not six

> *"Create the empty mod 'OUT Essentials' and download & install mods 1-5 whilst
> copying their contents to this empty mod... Once all of mods 1-5 have been
> copied here, pack the mod through BSArch."*

The same archetype as Part 9's OOO + voice files, and writing it as one mod with
five archives makes the copying step disappear: the archives layer into one
staged folder, `pack_bsa` writes it, `prune_packed` removes exactly what went
in, `create_dummy_plugin` supplies the plugin Oblivion needs to load a BSA at
all. 1294 files, matching the Oracle exactly.

So the guide's rows 1-5 have **no mod of their own here**, and `diff` reports
five Oracle folders missing from ours. They hold nothing: each is a bare
`meta.ini`, emptied when the Oracle was built in January, which is the guide's
own optional step — *"I also delete mods 1-5 after this step to save on instance
space"*. An empty MO2 folder is bookkeeping, not a build artifact.

## The guide contradicts itself on one BSA name

Row 7's title is **OUT Dungeons**; row 8's instruction says *"ensure the bsa
naming matches '**OUT - Dungeons**'"*, with a hyphen. Only the `.bsa` and `.esp`
stems matching **each other** is load-bearing — that pairing is how Oblivion
decides to load an archive at all — so the hyphen is free to go either way. The
Oracle chose unhyphenated and that is followed here.

## Two `file_prune` bugs, both silent in opposite directions

This section is where `file_prune`'s glob semantics stopped being good enough.
It was matching with *archive-entry* rules against a *staged tree*, and the two
differ in two ways that both bite.

### Case: `NoMushroomStalks` matched nothing

Row 23 says "Delete the NoMushroomStalks folder". Staged directories are folded
to lowercase (see `copy_filtered_tree_folded`), so the folder on disk is
`nomushroomstalks` and a case-sensitive glob found nothing. This one at least
**failed loudly**, because of the Part 5 fix that made a zero-match prune an
error. Without that it would have shipped 36 unwanted meshes.

### Separators: `textures/rocks/*.dds` ate the folder it was told to keep

Row 27 is the interesting one. The guide says delete everything under
`textures/rocks` **except** the `underwater` folder. Under globset's defaults a
bare `*` also matches `/`, so `textures/rocks/*.dds` reached straight through
into `underwater/` and deleted all ten files the guide explicitly protects.

Nothing failed. The prune reported a healthy number and the install succeeded.
**Only the Oracle diff noticed** — the same lesson as Part 9's `prune_packed`,
which also reported a plausible count while doing the wrong thing.

Both are fixed by `ArchiveFilters::new_for_staged_tree`: `/` is a real
separator, matching is case-insensitive, and `**` still crosses so a bare folder
name keeps meaning the folder and everything under it. Archive extraction
filters are deliberately left alone — over a hundred authored `include`/
`exclude` patterns depend on the current behaviour and cannot be re-verified
mid-build.

**The "everything except" case needed no new feature.** Under `textures/rocks`
there is exactly one other directory and 28 loose `.dds` files, so naming them
is shorter and more honest than an exception rule. 286 files in, 10 left.

## `ini_set` had to learn about sections first

Row 14's `[Grass]` block is the first INI edit in the build that names a
section, and it exposed the latent bug the plan had flagged: `apply_ini_set`
matched a key **anywhere** in the file, rewrote **every** match, and appended
missing keys at EOF. For a sectioned INI all three are wrong.

Now `section = "Grass"` scopes the edit, a key the section lacks is inserted
inside it, and a section the file does not have is created — Oblivion.ini omits
sections it has no non-default keys for, which is exactly the state `[Grass]` is
usually in. Without a section, a key matching more than one line is a hard error
naming the sections it found, rather than a silent write to both. That is
Part 14's `Fog.ini` waiting to happen: `Amount` under both `[World]` and
`[Interior]`, where setting the interior fog would have changed the weather.

## Documentation files outside the data folder are dropped

Three mods differ only by files the archive keeps *beside* its data folder:

| mod | dropped |
|---|---|
| Improved Doors and Flora | `obmm_BSA_settings.jpg`, `readme.txt` |
| Improved Trees and Flora | `obmm_BSA_settings.jpg`, `readme.txt` |
| HD Photorealistic Ivy | `Readme and Credits.txt` |

All three have a `Data/` (or wrapper + `Data/`) folder, and unwrapping takes the
data folder as the mod root, so root-level siblings never make it across. MO2's
installer keeps them. Part 9 dropped a readme the same way.

Harmless — Oblivion reads none of these — but it is now the fourth occurrence,
so it belongs in the backlog rather than being re-explained each section.

## Deferred

Row 23's remaining steps, all on Part 36 merge sources so nothing downstream
depends on them yet:

- QAC on `Harvest [Flora] - DLCFrostcrag.esp` and `Harvest [Flora] - Shivering Isles.esp`, joining the batch at the end of the build
- **Remove the Worldspace group** from `Harvest [Flora] - DLCFrostcrag.esp` in
  xEdit — scripted record deletion, which mudcrab cannot do. Parts 26a #6 and
  26b #12 need it too.

Both uncleaned plugins show in the diff as larger than the Oracle's (1230 vs
800 B, 9055 vs 8526 B), which is exactly the cleaning that has not happened.
