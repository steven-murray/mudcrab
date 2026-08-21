# Parts 37–40 — Plugin sorting, Conflict Resolution, slowLODGen, Utilities

The endgame. The load order now matches the guide's published one exactly, and
one Wrye Bash GUI step is all that stands between this build and a playthrough.

## Part 37 — the load order matches the published list, entry for entry

The row says to paste the guide's `loadorder.txt` into the MO2 profile. Here the
modlist's `plugins` array *is* the load order — `install` writes the profile from
it — so the row is this list agreeing with the published one.

The published file was fetched from the guide's own link
(<https://loadorderlibrary.com/lists/mofam-oblivion-2>, 244 entries, "All plugins
active prior to BP"). `WebFetch` gets a 403 there; the page loads in a browser
and exposes the file through a public API endpoint, which is what was read. **No
Oracle involvement** — a user with the guide gets the same file by clicking the
link.

Our profile's `plugins.txt` is now **identical to it, with no renaming needed**,
after two documented omissions:

| Omission | Why |
|---|---|
| `Swearing Rats.esp` | The guide's own Part 31 row 14 says "I can understand if you omit it". The Oracle omits it. |
| `Bashed Patch, 0.esp` | Wrye Bash writes it; see Part 38 below. |

A first pass found four differences rather than two, and three of them turned out
to be the Oracle rather than this build:

- **`OUT - Dungeons`**. This list had packed it unhyphenated, following the
  Oracle. The guide's row 8 says *"ensure the bsa naming matches 'OUT -
  Dungeons'"* and the published order agrees; row 7's unhyphenated mention is the
  guide contradicting itself. Steven has since renamed the Oracle's copy, so the
  guide, the published order and both instances now agree, and the `pack_bsa` and
  `create_dummy_plugin` outputs here were renamed to match.
- **`Diverse Effect Icons OBSE.esp`** and **`Street Vendors of Cyrodiil.esp`**
  were missing from the Oracle and present in the published order. Both are now
  installed there; both mods are byte-identical between the instances.
- **Street Vendors' version.** This list had used 2.92, because 2.91 — the
  version the guide names — was not on the machine and the Oracle had no copy to
  compare against. Both are now fixed and the entry is 2.91 exactly, matched
  pair, byte-identical on both halves.

### The ORC.esp inconsistency this exposed

Reconciling the orders turned up a contradiction in our own build. Part 14 row 4
installs **ORC v180**, the version the guide names, and v180 ships `ORC.esp`. But
Part 36's Prebash merge omitted `ORC.esp`, on a decision recorded long ago that
"the Linux build uses ORC 315F, which ships no plugin".

315F is not what this list installs. The premise never applied, and with it gone
`ORC.esp` sat loose in our load order where neither the published order nor the
Oracle has one — because in both of those it is consumed by Prebash.

`ORC.esp` is now a Prebash source, as the guide's list says. Prebash rebuilt at
**86 sources, 4508 records** against the Oracle's 4505; the +3 is the two `[QAC]`
Harvest [Flora] sources, which carry 4 records the cleaned copies do not:

```
Harvest [Flora] - DLCFrostcrag.esp        3 records only in ours;  6 vs 3
Harvest [Flora] - Shivering Isles.esp     1 record  only in ours; 53 vs 52
```

Four extra source records, three extra merged ones — one collides with a FormID
another source already claims and is clobbered rather than added.

## Part 38 — Conflict Resolution installs clean; the Bashed Patch does not

**`MOFAM - Conflict Resolution` is byte-identical to the Oracle.** That matters
beyond the file count: its `BashTags/` folder is the naming contract Part 36
keeps warning about, one file per plugin Wrye Bash needs tag hints for. Four of
our six merges are named there —

```
Late Loaders Merged.txt   NPC Merge.txt   OOO Patches Merged.txt   Prebash Merge.txt
```

— and all four match our merge outputs exactly. `TACE Merge` and
`Unique Forts Merged` have no tag file, so nothing checks their names here; they
are unverified rather than wrong.

Row 1's download is the **configuration only** (`Bash Patches/Bashed Patch,
0.esp_Configuration.dat`), which installs identically. The patch itself,
`Bashed Patch, 0.esp`, is what Wrye Bash writes after the row's twelve-step GUI
procedure. mudcrab cannot produce it; the mod folder and its configuration are
staged so that procedure has somewhere to start. Recorded in `incomplete-rows.md`.

## Part 39 — already done, at Part 5

Nothing to install. The row says to run slowLODGen and paste its two BSAs and two
plugins into the empty `Merged LOD` mod, and offers the author's own output as a
download for anyone who would rather not. This build took that download at Part
5, so `Merged LOD` already holds all four files and is identical to the Oracle's.

Both of the row's placement instructions are satisfied by Part 37's order, and
were checked rather than assumed:

```
  3: MergedLOD.esm     <- immediately after Av Latta Magicka.esm (line 2)
 43: IC LOD.esp
 44: MergedLOD.esp     <- last in the Part 5 block, after IC LOD.esp
```

## Part 40 — three rows to do, four with nothing to do

| Row | Outcome |
|---|---|
| 1 4gb Ram Patcher | Not a mod. It patches `Oblivion.exe` in the game root and the row says it was already run during setup, kept on the page "for safekeeping". |
| 2 Dummy ESP | Installed. `DUMMY.esp` is **hidden**: the guide never asks for it to be active and the published order does not contain it, but left visible in an enabled mod MO2 would offer it as a new undecided plugin on every launch. |
| 3 FormID Finder | No archive on disk, and the row marks it optional ("activate as-and-when"). Skipped. |
| 4 RefScope | Archive is on disk but the row marks it optional alongside row 3. Skipped, so the pair stays consistent. |
| 5 OBSE Logs & Inis, ConScribe Logs | Created empty. |
| 6 ConScribe settings | Installed at Part 33. |
| 7 TES4Edit Cache | Created empty. |

### New in mudcrab: `type = "empty"`

Rows 5 and 7 needed the last feature on the plan's backlog. An empty mod is a
folder with nothing in it on purpose — MO2 users make these routinely so a tool's
output has somewhere to live other than Overwrite. The folder and its `meta.ini`
are the whole point; the contents arrive at runtime from outside the modlist.

`validate` rejects an empty mod that declares archives, files or plugins, because
that contradiction always resolves the wrong way: install would write the folder
and silently ignore everything else the entry asked for.

The Oracle's `TES4Edit Cache` proves the shape — `modid=0`, no
`installationFile`, and 67 `.refcache` and `.backup` files that came from running
xEdit, not from any download. Ours is correctly empty; those 67 files are the
Oracle's play history, and `diff` reporting them is the tool being right.

`ConScribe Logs`, `OBSE Logs & Inis` and `Dummy ESP` show as "not in the Oracle".
The guide asks for all three and the Oracle skipped them.
