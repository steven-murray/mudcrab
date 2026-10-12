# The modlist format

A modlist is a TOML file: which mods, where each comes from, how to unpack
it, what to change afterwards, and the order everything loads in. This page
lists everything a modlist can contain, with a line on when you need each
field. [authoring.md](authoring.md) walks through writing one, and
[usage.md](usage.md) covers the commands that read it.

Most of a list is written by `mudcrab add`, which inserts lines without
disturbing anything else, so comments you write are kept. Use them: a list's
comments are where its reasons live. `mudcrab validate` checks a list in
seconds.

## A small, complete list

```toml
name = "Little Oblivion"

# The load order, top to bottom.
plugins = [
    "Oblivion.esm",
    "BettysButterflies.esp",
]

[inputs.butterflies]
type = "bool"
query = "Add more butterflies to the wilderness?"
default = true

[[mods]]
id = "Engine Bug Fixes"
section = ["Fixes"]

[[mods.archives]]
path = "nexus:oblivion/47085/1000022995"
file_name = "EngineBugFixes v2.22-47085-2-22-1569169531.7z"

[[mods]]
id = "More Butterflies"
section = ["Creatures"]
if = "butterflies"
plugins = ["BettysButterflies.esp"]

[[mods.archives]]
path = "nexus:oblivion/46785/1000013111"
file_name = "More New Butterflies-46785-.zip"
```

TOML puts every `[table]` after the plain keys at the top of the file, so
`name` and `plugins` come first; a key written after a table header belongs to
that table.

## Top level

| Key | When you need it |
| --- | --- |
| `name` | Always: the list's name. |
| `plugins` | Whenever a mod ships a plugin: the load order, top to bottom. See [Load order](#load-order). |
| `inactive-plugins` | When a tool absorbs some plugins (Wrye Bash's Bashed Patch): installed and ordered, but not ticked. |
| `inputs` | When the list offers choices: the questions `query` asks. See [Optional mods and inputs](#optional-mods-and-inputs). |
| `ini` | To change the game's own `Oblivion.ini`: `key = value` pairs, applied to the MO2 profile's copy. |
| `manual-steps` | When something has to be done by hand after installing (building a Bashed Patch in Wrye Bash): `install` lists it at the end until it is done. See [Steps by hand](#steps-by-hand). |
| `post-install-actions` | To run LOOT after installing: `["loot-sort"]`. Most lists keep a fixed `plugins` order instead. |
| `guide` | When the list transcribes a published guide: `published = "YYYY-MM-DD"` (and optionally `file_id`), so `diff` can flag archives newer than the guide. |
| `docs`, `sections`, `docs_file` | For `export`: prose for the guide and each section. See [Docs](#docs-prose-for-export). |
| `export` | To set an exporter's options for this list: `[export.<exporter>]`. See [exporters.md](exporters.md). |
| `mods` | Always: one `[[mods]]` table per mod, in MO2's order (later wins a file conflict). |

`ini` keys are matched anywhere in `Oblivion.ini`, with no section, so they
must be keys that occur once (all of the usual ones do); an `ini_set` action
with `scope = "game"` can name a section.

```toml
[ini]
SIntroSequence = ""
bAllowScriptedAutosave = 0
```

## Mods

Each `[[mods]]` table is one mod folder in MO2.

| Key | When you need it |
| --- | --- |
| `id` | Always. Unique; also the mod's folder name in MO2. |
| `section` | Nearly always: the MO2 separator it sits under, as a list from outermost in (`["Visuals", "Textures"]`). Every level becomes a separator. |
| `archives` | For any mod made from downloads: one `[[mods.archives]]` per archive, installed in order, later ones overwriting earlier. See [Archives](#archives). |
| `plugins` | When the mod ships plugins: their names. Each must also be in the top-level `plugins` (or be merged). |
| `if` | When the mod is optional: a condition. See [Optional mods and inputs](#optional-mods-and-inputs). |
| `dependencies` | When the mod needs others to work: their ids. `query` refuses a plan that includes this mod but not them. |
| `actions` | To change the mod after unpacking it: INI edits, files left out, plugins cleaned, BSAs packed. See [Actions](#actions). |
| `type` | When the mod is not made from archives. See below. |
| `merge` | With `type = "merge"`: what to merge. See [Merges](#merges). |
| `files` | With `type = "build-from-files"`: the files to assemble it from. |
| `docs` | For `export`: prose and pictures for this mod. |
| `oracle_name` | Only for `diff`: the mod's folder name in a reference instance, when it differs from `id`. Has no effect on installing. |

### Kinds of mod (`type`)

Without `type`, a mod is installed from its archives. Otherwise:

| `type` | What it is |
| --- | --- |
| `"merge"` | One plugin merged from other mods' plugins, built by `install`: a headless replacement for zEdit's zMerge. |
| `"build-from-files"` | Assembled from files already on disk, named in `files` (globs; `%GAME_DIR%` is the game folder). |
| `"empty"` | A deliberately empty folder, for a tool's output to land in rather than in Overwrite. |
| `"generated"` | Holds only what its own actions write: `write_file` authors a file (`path`, `content`). |
| `"mo2-plugin"` | An MO2 plugin such as Root Builder: installed into the instance's `plugins/<id>`, not `mods/`, and left out of the mod list. |

### Game-root files

Files that belong beside `Oblivion.exe` -- a script extender's loader, a
`dxvk.conf` -- go in a mod's `root/` folder: `game_root_files = ["*.dll"]` on an
archive stages matching files there, or a `generated` mod writes one with
`write_file`. The Root Builder MO2 plugin (installed as a `mo2-plugin` mod)
deploys `root/` to the game folder when MO2 launches the game and restores the
folder afterwards. mudcrab never writes the game folder.

## Archives

Each `[[mods.archives]]` table is one download. `mudcrab add --archive` writes
these from the archive itself.

| Key | When you need it |
| --- | --- |
| `path` | Always: where the archive comes from. See below. |
| `file_name` | Nearly always: the archive's name as the host serves it. It is how a copy already on disk is found, and how `check` names a missing one. |
| `size`, `md5` | For a `manual:` archive: its size in bytes and MD5, so a copy its host has since renamed is still found, by content. Both or neither; `add --archive --manual` writes them, and `downloads record` adds them to an existing list. |
| `download_page`, `download_note` | For a `manual:` archive: the web page to get it from, and a sentence on how (which file, a login it needs). `download` opens the page and prints the note; `check` prints both. |
| `layout` | When the archive needs telling how it is laid out: `"fomod"`, `"bain"` or `"custom-data-folder"`. Without it the layout is detected (data at the top, or in one folder MO2 would unwrap). |
| `data_folder` | With `layout = "custom-data-folder"`: the folder inside the archive that holds the data. |
| `target_subdir` | When the files belong in a folder the archive lacks: where in the mod to put them (`"obse/plugins"`). |
| `fomod_selections` | With `layout = "fomod"`: the option chosen in each group. A group left out gets the installer's default. |
| `bain_subpackages` | With `layout = "bain"`: the subpackages to install, merged in order. |
| `include`, `exclude` | To install only some of the files, or leave some out: globs matched against paths inside the mod's data (for BAIN, inside each subpackage), case-sensitively; `*` crosses folders. |
| `game_root_files` | When some files belong next to `Oblivion.exe`: globs; matches go to the mod's `root/`. See [Game-root files](#game-root-files). |
| `inner_archive` | When the download is a wrapper around the real archive: the inner one's path, which is unpacked as the mod. |
| `if` | When the archive only applies for some answers: a condition. |
| `download_handler` | Rarely: `"nexus"`, `"http"`, `"manual"` or `"local"`, when it cannot be told from `path`. |
| `build` | Rarely: layers (`path`, `file_name`, `dest_prefix`) whose archives are overlaid in order into one. |

### Where an archive comes from (`path`)

| Form | Means |
| --- | --- |
| `nexus:oblivion/<mod id>/<file id>` | A file on Nexus Mods. Downloaded through the API (premium accounts), or by hand. `mudcrab nexus-files <mod id>` lists a mod's files with this line ready to paste. |
| `https://...` | Any other direct download. |
| `manual:<file name>` | A file no download link can reach (a forum host behind a login, say): whoever installs the list downloads it by hand. Say where with `download_page` and `download_note`, and give `size` and `md5` so a renamed copy is still recognised. |
| a file path | A file on this machine. Ties the list to one computer, so mostly for testing. |

Archives are kept in one downloads folder under their real names; see
[usage.md](usage.md#the-downloads-folder).

### Installers

A FOMOD installer is answered group by group; `mudcrab inspect` lists the
steps, groups and options:

```toml
[[mods.archives]]
path = "nexus:oblivion/49017/1000020879"
file_name = "HD Septims-49017-1-4a-1543613153.zip"
layout = "fomod"

[[mods.archives.fomod_selections]]
step = "Step1"
group = "Meshes"
options = ["Normal Size Mesh"]
```

A BAIN package names the subpackages (its numbered top-level folders) to
install; their contents are merged into the mod in the order given:

```toml
[[mods.archives]]
path = "nexus:oblivion/21104/1000025650"
file_name = "Cobl-21104-174-1598559328.7z"
layout = "bain"
bain_subpackages = ["00 Cobl Core", "01 StableCore"]
```

A BAIN wizard's choices are recorded the same way: the subpackages it selects,
plugins it leaves unselected as `exclude`, and its INI edits as `ini_tweak`
actions. `mudcrab add --archive` runs the wizard and writes them.

## Load order

`plugins` is the whole load order, top to bottom, and mudcrab applies it as
written -- it never sorts. Every plugin a mod declares must be in it, except
one consumed by a merge (whose output is listed instead). A plugin listed here
that no installed mod provides and the game does not have is simply skipped,
so a list can name the game's own plugins (`Oblivion.esm`, the DLCs).

`inactive-plugins` is a subset of `plugins`: installed and placed, but not
ticked in MO2. It is for what another tool takes over -- the plugins Wrye Bash
merges into the Bashed Patch -- so reinstalling does not tick them again.

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
of the file instead.

A condition is one or more terms joined by `&&` and `||` (`&&` binds tighter;
no parentheses). Each term may be negated with `!` and is one of:

| Term | True when |
|---|---|
| `flag` | the bool input is true, or the text input is non-empty |
| `key == value`, `key != value` | the input's answer is (not) `value`; quotes optional |
| `mod:id` | mod `id` is in the resolved list; `mod:"Some Mod"` when the id has spaces |

```toml
[[mods]]
id = "HD Pack Compatibility Patch"
if = 'mod:"HD Pack" && !mod:"Other Overhaul"'   # follows whichever way HD Pack went
```

A mod condition may name a mod declared further down the file: `query` answers
each mod's condition when it is first needed. `validate` rejects a condition
naming an undeclared input or a mod that does not exist, a mod whose `if` asks
about itself, and mod conditions that ask about each other in a loop (which
have no answer, or two). A mod listed in another's `dependencies` must be
included whenever that one is; `query` refuses a plan where a condition left
it out, and says which `mod:` condition would keep the two together.

### Conditions inside a mod

`if` also sits on an archive, a FOMOD selection row and a BAIN subpackage, so
one answer can reach every place a choice shows up:

```toml
[[mods.archives]]
if = "res == 2k"                # only fetch and install this archive for 2K
path = "nexus:oblivion/123/456"

[[mods.archives.fomod_selections]]
if = "res == 1k"
step = "Textures"
group = "Resolution"
options = ["1K"]

[[mods.archives.fomod_selections]]
if = "res == 2k"
step = "Textures"
group = "Resolution"
options = ["2K"]

[[mods.archives]]
layout = "bain"
bain_subpackages = ["00 Core", { name = "01 2K Textures", if = "res == 2k" }]
```

A FOMOD group whose rows are all left out gets the installer's own default, as
if it had no row. A mod whose archives are all left out, or a BAIN archive
whose subpackages all are, is refused by `query`: it would install nothing, and
the condition belongs on the mod (or the archive) instead.

Conditions are resolved by `query`: the plan holds only the mods, actions,
archives and selections that apply, with no conditions left in it, so
`download` fetches only what applies, and a changed answer changes the
affected mods' definitions and `install` restages them.

## Actions

`[[mods.actions]]` change a mod after its archives are unpacked. They run **in
the order written**, and each may carry an `if`. Paths are relative to the
mod's own folder and cannot leave it. An unknown `action` is an error, not a
silent skip. [usage.md](usage.md#actions) has each action in full, with
examples.

| `action` | Fields | When you need it |
| --- | --- | --- |
| `ini_set` | `file`, `key`, `value`, `section`, `format`, `scope` | To change a setting: in an INI the mod ships (`scope = "mod"`, the default) or in the game's `Oblivion.ini` (`scope = "game"`). `section` when the key occurs in more than one; `format = "set-to"` for script INIs written `set key to value`. |
| `ini_comment` | `file`, `key`, `format` | To comment a setting out, so the mod's script falls back to its own default. |
| `ini_append_block` | `file`, `block` | When a guide says "paste these lines into the INI": appended verbatim, order and repeats kept. |
| `ini_tweak` | `file`, `key`, `value`, `format` | For a Wrye Bash INI Tweak, as a BAIN wizard's `EditINI` writes: a fragment under `ini tweaks/<file>`, applied from Wrye Bash. |
| `file_prune` | `paths` or `conflicts_with` (+ `under`), `except`; or `keep` alone | To delete files: by glob (`paths`, with exceptions); every file another mod also provides (`conflicts_with`, optionally `under` one folder); or everything except a few (`keep`). |
| `file_hide` | `paths` or `conflicts_with` (+ `under`), `except` | To hide files from MO2 (renamed `.mohidden`) rather than delete them. Same selectors as `file_prune`; paths are literal, not globs. |
| `file_move` | `from`, `to` | To move a file within the mod, e.g. a plugin into `optional/` so it is kept but not loaded. |
| `write_file` | `path`, `content` | To add a file no archive has, such as a `dxvk.conf`; never overwrites one. |
| `qac` | `plugins` | To clean plugins with xEdit's Quick Auto Clean. Needs xEdit set up in `tools.toml`. |
| `delete_records` | `plugin`, `groups`, `form_ids` | When a guide says "remove these records in xEdit": whole groups (`WRLD`) or records (`xx010F43`). |
| `pack_bsa` | `output`, `include`, `exclude`, `prune_packed` | To pack loose files into a BSA; `prune_packed` deletes what went in. |
| `create_dummy_plugin` | `output` | For a BSA with no plugin of the same name, which Oblivion would otherwise not load. |
| `extract_bsa` | `archive`, `keep_archive` | To unpack a BSA the mod ships, to change what is in it. |
| `dds_decompress` | `paths` | On Linux/Proton: menu textures with DXT1 transparency, which DXVK draws black. See [known-issues.md](known-issues.md#linux--proton). |

`pack_bsa`, `create_dummy_plugin` and `file_prune` exist to be composed, in this order, for mods that ship loose assets
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

## Merges

```toml
[[mods]]
id      = "Unique Forts Merged"
section = ["Merges"]
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

## Steps by hand

Some steps no command can take: Wrye Bash builds a Bashed Patch only from its
own window. Say so in the list, and `install` ends every run by listing what is
still to do:

```toml
[[manual-steps]]
title = "build the Bashed Patch in Wrye Bash"
done_when_plugin = "Bashed Patch, 0.esp"
steps = """
1. Start Wrye Bash from MO2.
2. Right-click Bashed Patch, 0.esp and choose Rebuild Patch.
3. Build Patch, close Wrye Bash, and move the patch from Overwrite into its mod.
"""
```

`steps` is printed line by line. With `done_when_plugin`, the step stops being
listed once a plugin of that name is in a mod folder or in MO2's `overwrite/`;
without it, it is listed after every install. Put these tables after the
top-level keys, like `[[mods]]` -- at the end of the file is simplest.

A list whose `plugins` include a `Bashed Patch, N.esp` and that declares no
step for it still gets a general one from `install`; a step of the list's own
can say more (which configuration to import, which plugins to tick).

## Docs: prose for `export`

`export` turns a modlist into a guide, and the prose comes from `docs` tables:
one for the guide as a whole, one per section, one per mod. Text fields are
Markdown; each exporter converts it for its target (BBCode for Nexus, HTML for
a site).

```toml
[docs]                                    # the guide itself
title = "My Oblivion List"
summary = "A modern-looking Oblivion that stays true to the original."
body = """
Read the *Prologue* first...
"""

[sections."7 - CHARACTER AND NPCS"]       # named as MO2 shows the separator
summary = "Faces, bodies and NPC behaviour."
images = [{ path = "shots/npcs.png", url = "https://i.imgur.com/abc.png", caption = "Before and after" }]

[[mods]]
id = "Swearing Rats"
docs.summary = "Rats that insult you."
docs.body = "Optional: the guide says you may omit it."
docs.links = [{ label = "Swearing Mudcrabs (Skyrim)", url = "https://..." }]
```

| Key | Meaning |
|---|---|
| `title` | What to show instead of the id or section name |
| `summary` | One line; for the guide, a short introduction (a site's landing page) |
| `body` | Longer prose, Markdown |
| `footer` | Prose after the contents: the guide's closing notes and credits, or a section's steps once its mods are in |
| `body_title` | (Guide only) the title the guide's `body` gets where it is a page of its own, e.g. "Part 0 - Setup" on a GitHub Pages site; with tools and game settings, it is what to do before the first section |
| `footer_title` | (Guide only) the title the closing text gets where it is a page of its own, e.g. on a GitHub Pages site |
| `images` | `{ path, url, caption }`: a local `path` (relative to the modlist) for exporters that write a site, a `url` for ones pasted somewhere that cannot hold files, such as Nexus; give both and each takes what it can |
| `links` | `{ label, url }` |

Any other key is kept and handed to the exporter's templates unchanged, so a
custom exporter can use fields no built-in one knows (`difficulty`, `video`,
...).

A nested section is named with its levels joined by `" - "`
(`"OBSE PLUGINS - Core"`). `validate` rejects docs for a section no mod is in,
and an image whose `path` does not exist.

### Keeping prose in its own file

A long list can keep its prose apart from its install data:

```toml
docs_file = "mylist.docs.toml"   # relative to the modlist
```

```toml
# mylist.docs.toml
[docs]
summary = "..."

[sections."7 - CHARACTER AND NPCS"]
summary = "..."

[mods."Swearing Rats"]
summary = "..."
```

The file is merged in when the modlist is loaded. A mod id it names that the
modlist does not have is an error, and so is documenting the same thing in both
places.

## Not implemented

A `custom` mod type running arbitrary commands is on the roadmap, not in the
schema, and so is `include`-ing one modlist from another: every modlist today
is one file. See [roadmap.md](roadmap.md), Phase F.
