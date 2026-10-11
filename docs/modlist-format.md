# The modlist format

A modlist is a single TOML file. This is the reference for what it may contain;
[usage.md](usage.md) covers the commands that consume it, and
[known-issues.md](known-issues.md) covers what is not implemented.

## Top level

* `name`: the name of modlist
* `inputs`: a table where each table entry has a key corresponding to a variable name
  that is able to be used throughout the rest of the modlist, and values that specify
  how the input data is to be captured:
  * `type`: either `bool`, `choice` or `text`
  * `query`: a string specifying the question to put to the user
  * `choices`: (if the type is `choice`) specifying the possible choices.
* `ini`: an optional table of game-scope `Oblivion.ini` edits to apply independently
   of any specific mod. Values must be scalar TOML values. Keys containing spaces must
   be quoted.
* `mods`: an ordered array of mod entries. Each entry has an `id` (unique; also the
   mod's directory name) and an optional `section`, a list naming its MO2 separator
   path from outermost to innermost. Every level of the path becomes a separator.
* `plugins`: an ordered list of plugins. Each entry should be an exact filename. 
* `inactive-plugins`: a subset of `plugins` that is installed and ordered but
   **not active**. They keep their slot in `loadorder.txt` and are left out of
   `plugins.txt`. This is for plugins another tool has absorbed — Wrye Bash
   unticks whatever it merges into the Bashed Patch — so that reinstalling the
   list does not re-activate them and force the patch to be rebuilt. 
* `post-install-actions`: optional ordered list of install-wide actions to run after
   extraction and MO2 export. Currently supports `"loot-sort"`.

Example with nested sections:

```toml
name = "Nested Sections Example"

[[mods]]
id = "base"
section = ["foundation"]
dependencies = []

[[mods]]
id = "combat"
section = ["gameplay"]
dependencies = ["base"]

[[mods]]
id = "magic"
section = ["gameplay"]
dependencies = ["base"]
```

Example top-level ini edits:

```toml
[ini]
"bFull Screen" = 0
"iSize W" = 1920
"iSize H" = 1080
```

Example top-level post-install actions:

```toml
post-install-actions = ["loot-sort"]
```

## Archive layouts

`mudcrab` supports a few common archive layouts.

For a normal archive where the mod data is already at the archive root, no layout field is required.

For archives where the real data lives under a known subdirectory, use `data_folder` and optionally `target_subdir`.

For BAIN-style archives, use `layout = "bain"` and list the top-level package directories you want to install in `bain_subpackages`. The contents of each selected package are merged into the mod root in the order listed.

Example:

```toml
[[mods]]
id = "DLC Lore Books"
section = ["example"]

[[mods.archives]]
path = "nexus:oblivion/46715/1000012857"
layout = "bain"
bain_subpackages = ["00 Merged"]
```

If a BAIN archive contains:

- `00 Option1/plugin1.esp`
- `01 Option2/plugin2.esp`
- `01 Option2/Textures/...`

then selecting both `00 Option1` and `01 Option2` produces a mod root containing `plugin1.esp`, `plugin2.esp`, and `Textures/...` directly, without preserving the `00 ...` and `01 ...` folder names.

Each entry in `mods` has the following format:

```toml
[[mods]]
id = "modname"
if = "<conditional>"  # only install this mod if conditional is true

[[mods.archives]]
path = "nexus:oblivion/<mod_id>/<file_id>"
download_handler = "nexus"
layout = "fomod"  # or omit for a plain archive, or "custom-data-folder"/"bain"
include = ["Textures/*"]
exclude = ["Meshes/*"]

[[mods.archives.fomod_selections]]
step = "Textures"
group = "Resolution"
options = ["2K"]

[[mods.archives.fomod_selections]]
step = "Options"
group = "Style"
options = ["Orange Styling"]

[[mods.archives]]
download_handler = "nexus"
layout = "custom-data-folder"
data_folder = "."  # e.g. if layout='custom-data-folder' and the data/ folder is the root
include = ["Meshes/arg*"]
```

Post-install actions on a mod (e.g. Quick Auto Clean) are declared per-archive/per-mod
via the `actions` machinery in `src/config/actions/` -- see `MOFAM-test/input/mofam.full.toml`
for real examples.

## Actions

`actions` is an **ordered** list; each entry runs in the order it is declared. The
`action` key selects which one, and the rest of the table carries its parameters. An
unrecognised name is a parse error naming the supported values, not a silent skip.

| `action` | What it does |
| --- | --- |
| `ini_set` | Set a key in an INI file (`scope = "mod"` or `"game"`, optional `section`). |
| `ini_append_block` | Append a verbatim block of lines to an INI, for the guide's "paste this in" rows. |
| `ini_tweak` | Write a Wrye Bash INI Tweak fragment under `ini tweaks/`, as a BAIN wizard's `EditINI` does. |
| `qac` | Run xEdit's Quick Auto Clean over the named plugins. |
| `delete_records` | Remove records or whole groups from a plugin, replacing a hand pass in xEdit. |
| `pack_bsa` | Pack the mod's staged files into a BSA. |
| `extract_bsa` | Unpack a BSA the mod ships, leaving its contents loose. |
| `create_dummy_plugin` | Write an empty `.esp` so Oblivion loads a BSA of the same name. |
| `file_prune` | Delete staged files, by glob or by `conflicts_with` another mod. |
| `file_hide` | Rename to `.mohidden`, MO2's way of dropping a file out of the VFS. |
| `file_move` | Move a staged file, e.g. a plugin into `optional/`. |

Full per-action reference, with examples and the reasoning behind each, is in
[usage.md](usage.md#actions).

The last three exist to be composed, in this order, for mods that ship loose assets
the guide wants archived:

```toml
[[mods]]
id = "Example Mod"

actions = [
  { action = "pack_bsa", output = "Example Mod.bsa", exclude = ["*.esp"] },
  { action = "create_dummy_plugin", output = "Example Mod.esp" },
  { action = "file_prune", paths = ["meshes/**", "textures/**", "sound/**"] },
]
```

`file_prune` is a separate action rather than a `pack_bsa` option because it has to run
*after* the archive is written -- the loose files must still exist to be packed. Ordering
is the only mechanism involved.

Oblivion loads `Foo.bsa` only when a plugin named `Foo.esp` is active, which is what
`create_dummy_plugin` is for: a mod distributed as a bare archive needs an empty plugin
beside it. The plugin is built through mudcrab's own plugin writer, so it is a real TES4
file rather than a blob of hand-written bytes.

Paths in all three are relative to the mod's staged data folder and may not escape it.
A BSA cannot store a file outside a folder, so anything at the top level of the staged
mod (a readme, a plugin) is left loose and logged rather than packed.

Packing and unpacking are native -- see `src/bsa/` -- so no BSArch.exe under Wine.

Along with standard archive-based mods, a mod can be **built** rather than extracted.
Two `type` values do this today:

- `"build-from-files"` assembles a mod's contents from local files and layers instead
  of a downloaded archive (see `BuildLayer` in `src/config/schema.rs`).
- `"merge"` produces a single merged plugin from several other mods' `.esp` files --
  a headless, native replacement for zEdit's zMerge, so a modlist requiring merges can
  be installed without driving a GUI tool.

## Merges

```toml
[[mods]]
id      = "Unique Forts Merged"
section = ["36 - zMERGED PLUGINS"]
type    = "merge"

  [mods.merge]
  output       = "Unique Forts Merged.esp"
  method       = "clobber"   # the default; last source wins on conflicts
  hide_sources = true        # the default
  sources = [
    { mod = "Better Fort Aurus",       plugin = "Unique Forts Fort Aurus.esp" },
    { mod = "Better Fort Doublecross", plugin = "Unique Forts Fort Doublecross.esp" },
    # ...
  ]
```

`sources` is **ordered**: the order defines both clobber precedence and FormID
allocation, so reordering changes the output. Each source names a **mod id**, not a
data-folder path -- the path is resolved at install time, so it survives renames.
There is no `load_order` field; the load order comes from the modlist's own `plugins`.

`hide_sources` renames each source plugin to `<name>.esp.mohidden`, which drops it from
MO2's virtual filesystem while leaving its mod **enabled** so its meshes, textures and
BSAs keep loading -- the same mechanism as MO2's "Merge Plugins Hide" plugin. Undo it
with `mudcrab unhide-merges`.

Because sources are hidden, the modlist's `plugins` list must contain the merge's
`output` and must **not** contain any source plugin. `mudcrab validate` enforces that,
along with: every source mod exists, no plugin is merged twice, and a merge mod
declares no archives of its own.

## Not implemented

A `custom` mod type running arbitrary commands is on the roadmap, not in the
schema:

```toml
# NOT SUPPORTED -- see docs/roadmap.md, Phase F
[[mods]]
id = "modname"
type = "custom"
actions = ["my-custom-script --mod A"]
```

So is `include`-ing one modlist from another. Every modlist today is one flat
file.

## Optional mods and inputs

An input is a question the list leaves to whoever installs it. `query` asks it;
`query --headless` takes the declared `default` (and fails if there is none,
rather than guessing); `--set name=value` answers it on the command line and
beats both.

```toml
[inputs.use_hd_textures]
type = "bool"        # or "choice" (with `choices`) / "text"
query = "Install HD textures?"
default = false      # must match the type; a choice's must be one of its choices

[[mods]]
id = "hd_pack"
if = "use_hd_textures"          # whole mod

[[mods]]
id = "some_ui"

[[mods.actions]]
if = "!use_hd_textures"         # just this action
action = "file_prune"
paths = ["textures/menus/big.dds"]
```

Since TOML puts every table after the top-level keys, an input declared with a
`[inputs.*]` header lands below `plugins`. Dotted keys
(`inputs.use_hd_textures.type = "bool"`) are plain keys and can sit at the top
of the file instead; MOFAM does this.

Conditions accept `flag`, `!flag`, `key == value` and `key != value`, and
`validate` rejects one that names an undeclared input. They are resolved by
`query`: the plan holds only the mods and actions that apply, so a changed
answer changes the affected mods' definitions and `install` restages them.
Conditions cannot yet ask about other mods, nor sit on archives or FOMOD
selections -- see Phase B of [roadmap.md](roadmap.md).
