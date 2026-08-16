# Part 8 (Overhauls: Maskar)

Ten guide rows, eleven mod folders — row 7 says "install both main files
separately". Built 2026-08-16.

**Result: 11 compared, 9 identical, 2 differing, both explained. Nothing missing,
nothing extra. All 37 of MOO's INI settings verified applied.**

## The version decision: MOO 4.9.4.2, not 5.0.5

The guide pins row 1 to **OLD FILES 4.9.4.2** — a deliberate choice, phrased
differently from every other row, not "the top file on the page". The Oracle
runs **5.0.5**, uploaded 2025-11-23, eight months after the guide.

Both archives were already on disk, so this was a free choice. Decided in favour
of the guide, because MudCrab Test exists to reproduce the guide and MOFAM's
balance — plus its later MOO patches — is tuned around 4.9.4.2.

The deciding evidence: of the guide's 37 INI settings,

- **4.9.4.2 has all 37.**
- **5.0.5 has 36.** `MOO.ini_levelscaling_npc_overridden` was removed in 5.x, so
  that instruction would silently do nothing on the Oracle's build.

This began as a permanent divergence, like ORC -- 16 differing files, 8 only in
the Oracle, plus an `archive mismatch`. **The user then downgraded the Oracle to
4.9.4.2 as well (2026-08-16), so the two now agree and this mod matches
byte-for-byte.** The old `Maskars Oblivion Overhaul 5_0_5` folder is still in the
Oracle and shows as one of the many "missing from ours"; the new folder is named
`Maskar's Oblivion Overhaul`, which is our mod id, so no `oracle_name` alias is
needed.

The Oracle's `meta.ini` records `1\fileid=0`, so `add --from-oracle` could not
build a URL for this row at all; the entry was written from the download's own
`.meta` sidecar (`fileID=1000022848`).

## The 37 INI settings

Written as 37 `ini_set` actions with `format = "set-to"`, against
`Maskar's Oblivion Overhaul.ini` in the mod's own folder, grouped by the guide's
own grouping.

Every key was checked to exist in 4.9.4.2 before authoring, and **every one
changes a value away from its default** — there are no no-op settings in the
list. After install, all 37 were read back from the staged INI and confirmed.

Worth noting `set X to Y` is a script command, not an assignment, so it is
exempt from the INI spacing rule that Part 7's DarNified fonts needed.

## The two remaining differences

`MOBS patch for Maskar's Oblivion Overhaul` and `OCOv2 -MOO Patch` each differ
only by a plugin hidden in the Oracle and active here. Both are Part 36 merge
sources — the same expected pattern as Part 7's ten.

## Rows that needed more than an archive reference

| # | mod | what it needed |
|---|---|---|
| 1 | Maskar's Oblivion Overhaul | `--nexus` entry, 37 `ini_set` |
| 2 | MOO - Non-Elder Scrolls Franchise Recolors | nothing, since the auto-layout fix |
| 3 | Hill Giant Eye Fix | nothing, since the auto-layout fix |
| 4 | Basic harvest | `data_folder` — `01_MOO_DefaultProbabilities` of three alternatives |
| 5 | MOO Themed Loading Screens | `file_prune` of `textures` |
| 7b | …WEPON | `target_subdir = "Menus/Strings"` |

Row 5 is worth understanding rather than copying: Part 6's upscaled loading
screens supply the same 55 images at higher resolution, so keeping MOO's would
mean 55 textures shadowed in the VFS for nothing. The Oracle deleted rather than
hid them, so `file_prune` matches.

## The wrapper-`Data/` pattern, now fixed

`Data/` nested inside a folder named after the *archive* was the most common
reason a row needed an explicit `data_folder`. Six occurrences:

- Part 7: Warpaints Argonian and Khajiit Patch, Warpaints Argonian Patch for
  Seamless
- Unofficial Patches: UOTP, USITP (silently broken for months -- see
  `earlier-sections-backlog.md`)
- Part 8: MOO Recolors, Hill Giant Eye Fix

The auto layout unwrapped `<mod id>/Data/` but not `<anything>/Data/`, and the
mod id is *our* name for the mod rather than a property of the archive, so the
rule keyed off the wrong thing. It now unwraps a lone top-level folder that
contains a `Data/`, whatever that folder is called.

All six explicit overrides were **removed** and the whole instance rebuilt with
`--force`: 164 mods re-resolved, all six still byte-identical to the Oracle, and
no other mod changed. That is the check that mattered -- fixtures show the rule
works, only a full rebuild shows it broke nothing.

The no-`Data/` case stays name-gated on purpose. A lone folder with no `Data/`
inside is as likely to *be* the content (an archive that is just `textures/`) as
to wrap it, and unwrapping that would install the textures' contents at the mod
root.
