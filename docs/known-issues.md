# Known issues and limitations

Things that can bite you, and what to do about them. Ordered roughly by how
likely you are to meet one.

## Scope

mudcrab targets **TES4: Oblivion** and exports to **Mod Organizer 2**. The
schema is not Oblivion-specific and the pipeline is not MO2-specific, but no
other game or mod manager has been exercised, and the plugin/BSA code is TES4
format. Treat anything else as unimplemented rather than untested.

## Archives

**The download cache key includes the mod id.** It is
`{mod_id}_{archive_index}_{fileid}`, so renaming a mod orphans its cached
archive, and two mods sourcing the same Nexus file cache two copies of it.

> Give every archive a `file_name` and pass `--archive-search-path`. Resolution
> then matches on the real filename and a rename costs nothing. This is worth
> doing for every entry regardless — it is what makes a build work offline.

**Manual (non-Nexus) archives are matched by exact filename.** A `manual:`
source resolves by looking for its `file_name` in the search paths. If the host
has re-uploaded the file under a different name, you get "archive must be
downloaded by hand" — with no hint that renaming your download would fix it.
The error does name the file it wanted and the paths it searched.

**`--parallel` is accepted and ignored.** Downloads are sequential.

## Nexus pinning

A `nexus:` source names an exact file id, which is the only reproducible way to
name a file. Guides usually do not: *"the top file on the page"* means different
things on different days.

`mudcrab diff` flags any archive whose Nexus file postdates the guide, so drift
is visible rather than silent. What it cannot catch is a guide whose selector is
a **position rather than a name** — *"1st main file"* on a page hosting two
files that are not versions of each other. Pick the wrong one and you get a
working install of the wrong mod, and nothing anywhere says so. When authoring,
record *which* file a row means in words, in a comment.

## Load order

**MO2 must be restarted to pick up a new load order.** Oblivion has no
load-order file — the order is the plugin files' modification times, and
`plugins.txt` records only which plugins are active. mudcrab writes
`loadorder.txt`; MO2 reads it when it opens the profile and applies it to the
files. A running MO2 will not see it, and will write its own in-memory order
back on exit.

**A plugin written after MO2 last saved the order keeps its own timestamp**, so
it can sit out of position until MO2 restamps. Wrye Bash's `Bashed Patch, 0.esp`
is the usual case. Restart MO2 after building one.

## Merges

**A source plugin whose reference uses an out-of-range mod index is rejected.**
`merge::rewrite` treats it as a dangling reference and errors. Real tools —
zMerge among them — emit such indices routinely, and every reader resolves them
as "my own record", so the refusal is stricter than the de facto format. The
practical consequence is that **mudcrab cannot currently merge a zMerge output
as a source**. Nothing in a normal list triggers it.

**Merge output is not byte-identical to zMerge's**, and is not meant to be. Both
produce the same record set and the same reference graph; the FormIDs allocated
to renumbered records differ, and zMerge retains some masters mudcrab drops.

## INI edits

**There is no `game-root` INI scope.** `scope = "game"` resolves to the MO2
profile's copy of `Oblivion.ini` and never touches the game directory. A file
that genuinely has to land in the game install root — an ENB config, say — needs
`game_root_files` on an archive instead, which places the file but cannot edit
one in place.

## Layout detection

**`inspect` and `install` decide an archive's content root by different code.**
`install` uses the layout planner; `inspect` has its own shallowest-content-root
search. They agree on every archive tried so far, and nothing guarantees they
agree on the next one. If `inspect` suggests a layout that installs wrongly,
this is why — set `data_folder` explicitly and report it.

## Not implemented

- **Composition.** A modlist is one flat file; there is no include or import.
- **A `custom` mod type** running arbitrary user commands.
- **`export`** (modlist → readable guide) is a command stub.
- **Download resume**, checksum manifests, signature verification.

## External tools

- `bsdtar` and `7z` on `PATH` for `.7z` and `.rar`. Everything else is pure Rust.
- `qac` needs xEdit configured in `tools.toml`. On Linux it runs under
  Proton/Wine; mudcrab drives it headlessly and detects completion from xEdit's
  log, because `-autoexit` is not honoured in Quick Clean mode.
- `loot-sort` needs LOOT, and is optional — a modlist that declares its full
  `plugins` order does not need it.
