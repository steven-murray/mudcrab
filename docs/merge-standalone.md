# Using mudcrab as a zMerge replacement

You do not need the modlist system to use the merge engine. `mudcrab merge`
reads installed mod folders and writes merged plugins, and nothing else.

**Before you start**, read [Limits](#limits) at the bottom. There is one that
will probably affect you.

## What it does

Given several plugins, it produces one, with FormIDs renumbered where they
collide and every reference rewritten to match. It is a headless replacement for
zEdit's zMerge — useful mainly if you cannot get zEdit's GUI running, which on
Linux is most people.

It is **read-only except for its output directory**: source mods are never
modified, nothing is hidden, and no profile is touched.

## Scaffold a merge

You need a mods directory and the plugins you want merged. Order matters — it
decides which mod wins a conflict, so later beats earlier.

```bash
mudcrab new-merge \
  --mods-dir ~/mo2/MyInstance/mods \
  --name "Village Merge" \
  --plugin "Feldscar.esp" \
  --plugin "Vergayun.esp" \
  --plugin "Molapi.esp" \
  --output village.toml
```

That works out which mod folder ships each plugin, reads your load order from
the instance's profile, and writes a small TOML file. `--plugins-from list.txt`
reads names from a file instead, one per line.

Two things it will stop and ask about:

- **Two mods ship the same plugin name.** It refuses rather than picking, and
  names the candidates. Write `--plugin "Some Mod/Shared.esp"` to choose.
- **More than one profile.** Pass `--load-order <path to loadorder.txt>`. The
  load order is not optional: the merge orders the output's masters by it.

## Build it

```bash
mudcrab merge village.toml \
  --mods-dir ~/mo2/MyInstance/mods \
  --output ~/merged
```

The plugin lands in `~/merged/Village Merge/Village Merge.esp`, which is already
the shape of an MO2 mod folder — copy or symlink it into `mods/` and enable it.

Then, in this order:

1. Open the result in TES4Edit and run **Check for Errors**.
2. Enable the merged mod, **disable the source plugins**, and load a save.
3. Only once you are happy, set `hide_sources = true` in the TOML if you want
   mudcrab to hide the sources for you on future builds. The scaffold leaves it
   `false` so that building changes nothing in your instance.

## Writing the file by hand

The scaffold just writes this. It is small enough to edit directly:

```toml
name = "Village Merge"

# Your load order, with the sources replaced by the merged plugin.
plugins = [
  "Oblivion.esm",
  "Village Merge.esp",
]

# Source mods: bare ids. A merge only ever reads their folders.
[[mods]]
id = "Feldscar"

[[mods]]
id = "Vergayun"

[[mods]]
id = "Village Merge"
type = "merge"

  [mods.merge]
  output = "Village Merge.esp"
  hide_sources = false
  sources = [
    { mod = "Feldscar", plugin = "Feldscar.esp" },
    { mod = "Vergayun", plugin = "Vergayun.esp" },
  ]
```

`--only "<name>"` on `merge` builds one merge from a file declaring several.

## Limits

**It refuses plugins that use Oblivion Magic Extender.** OBME's fields carry
parameters that are FormIDs or not depending on a byte elsewhere in the record,
which mudcrab's field table cannot express, so it stops rather than guess. The
rest of the format is covered — an audit of 431 plugins and 758,464 records
leaves only OBME.

That refusal is deliberate. mudcrab cannot tell whether an unknown field holds a
FormID, and guessing wrong would silently corrupt the merge instead of failing.
`cargo run --bin plugin-audit -- <mods dir>` lists every gap your own setup would
hit; adding them to `src/plugin/schema/tes4.rs` is mechanical, using xEdit's
`Core/wbDefinitionsTES4.pas` as the source of truth. This is the main thing
standing between the merge engine and general use — see
[roadmap A4b](roadmap.md#a4b-close-the-tes4-schema-gap--the-blocker-for-sharing-the-merge-engine).

**It refuses plugins with voice directories or FormIDs embedded in script
bytecode**, for the same reason. Neither occurs in Oblivion in practice — zero
occurrences across a 752-mod corpus — but the detectors are hard errors so the
assumption cannot fail quietly.

**Output is not byte-identical to zMerge's**, and is not trying to be. Same
record set, same reference graph; different FormIDs for renumbered records, and
zMerge keeps some masters mudcrab drops. Details in
[design/merge-engine.md](design/merge-engine.md).

**Evidence base**: six merges, 171 source plugins, checked against zEdit's own
output record-for-record and reference-for-reference, two of them verified in
game. One modlist, one game.
