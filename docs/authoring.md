# Writing a modlist

This page builds a small modlist from an empty file: four real Oblivion mods,
one optional, installed and checked. Every command and output below comes from
an actual run. Installing someone else's list is covered in
[getting-started.md](getting-started.md); every field a list can use is in
[modlist-format.md](modlist-format.md).

The list is a TOML file you write, and mostly *grow* with mudcrab's commands
rather than by typing: `inspect` reads an archive, `add` writes its entry, and
`compile` tells you straight away if something does not fit. Comments and
layout you write by hand are never disturbed, because the commands insert
lines rather than rewriting the file. Use comments freely: they are where a
list keeps its reasons.

## 1. Start the file

```toml
name = "Little Oblivion"

# The load order, top to bottom: the game's own plugins first.
plugins = [
    "Oblivion.esm",
]
```

`plugins` is the load order: every plugin the list installs, in the order the
game loads them. mudcrab writes it into MO2 as given and never sorts it, so it
says exactly what you decided.

## 2. Get the archives

mudcrab works from the archives themselves, so download each mod first, the
way you normally would: MO2's "Mod Manager Download" button, or by hand into
your downloads folder (the one you set as `downloads_dir`; see
[getting-started.md](getting-started.md#1-tell-mudcrab-where-things-live)).
For anything MO2 downloads, it writes a `.meta` file beside the archive
recording its Nexus ids, and mudcrab reads it, so nothing has to be copied
from a web page. For a file you downloaded in the browser, give the ids with
`--nexus` in step 4.

To pick a file without the browser, `nexus-files` lists a mod's current
files, with the lines a list needs for each (an API key is required, but any
account will do):

```console
$ mudcrab nexus-files 47085
MAIN          EngineBugFixes v2.22  v2.22  0.2 MB
              path = "nexus:oblivion/47085/1000022995"
              file_name = "EngineBugFixes v2.22-47085-2-22-1569169531.7z"
MISCELLANEOUS EngineBugFixes v2.22 Source (partial)  v2.22  0.1 MB
              path = "nexus:oblivion/47085/1000022996"
              file_name = "EngineBugFixes v2.22 Source (partial)-47085-2-22-1569169612.7z"
(28 old or archived file(s) not shown; --all lists them)
```

## 3. Look inside

`inspect` reports how an archive is laid out, and the entry it needs:

```console
$ mudcrab inspect "HD Septims-49017-1-4a-1543613153.zip"
...
layout guess: FOMOD
  installer at fomod/ModuleConfig.xml

FOMOD: HD Septim
  step "Step1"
    group "Main" (SelectExactlyOne)
      * "Textures"                     Optional       3 install entries
    group "Meshes" (SelectExactlyOne)
      * "Normal Size Mesh"             Optional       1 install entry
        "Smaller Mesh"                 Optional       1 install entry

  * marks what install picks for a group with no fomod_selections entry.
...
```

The common cases:

- **A plain archive**: the data is at the top, or in one folder MO2 would
  unwrap (a `Data/` folder, say). Nothing to declare.
- **Data in a subfolder** MO2 would not find by itself: `data_folder`.
- **A FOMOD installer**: `layout = "fomod"`, with `fomod_selections` saying
  which option to pick in each group. `inspect` shows them all.
- **A BAIN package** (numbered folders such as `00 Core`, `01 Option`):
  `layout = "bain"`, with `bain_subpackages` naming the ones to install.

`inspect` also lists the plugins in the archive, which you need for the next
step.

## 4. Add the mods

`add --archive` writes the mod's entry into your list, worked out from the
archive: where it came from, its file name, its layout and installer choices.
`--section` files it under an MO2 separator.

`add` only **declares** the mod. Nothing is installed or unpacked, and the
archive stays as it is; the list now says what to do with it, and `install`
does it, later, for the whole list at once (step 6).

```console
$ mudcrab add little.toml --id "Engine Bug Fixes" --section Fixes \
    --archive "EngineBugFixes v2.22-47085-2-22-1569169531.7z"
added 'Engine Bug Fixes' to little.toml at line 8, at the end of the file
```

A mod with a plugin needs a place in the load order too, and mudcrab will not
guess one: a plugin in the wrong place fails silently in game. Name the plugin
with `--plugin`, and say where it goes with `--load-after <plugin>` or
`--load-last`:

```console
$ mudcrab add little.toml --id "More Butterflies" --section Creatures \
    --archive "More New Butterflies-46785-.zip" \
    --plugin BettysButterflies.esp --load-last
added 'More Butterflies' to little.toml at line 18, at the end of the file
```

Forget the position and `add` stops before writing anything:

```console
error: BettysButterflies.esp is not in the load order yet; say where with
--load-after <plugin> or --load-last (or add it to `plugins` yourself first)
```

**An installer's choices are put to you**, as MO2 would put them, and your
answers go into the entry. A FOMOD is walked step by step -- a step that only
appears after certain answers appears here exactly when it would during the
install -- and a BAIN package asks which subpackages to take. Enter accepts
the installer's own choice; a group with only one possible answer is not
asked at all.

```console
$ mudcrab add little.toml --id "HD Septim" --section Clutter \
    --archive "HD Septims-49017-1-4a-1543613153.zip"
'HD Septim' has a FOMOD installer. Enter takes the installer's own choice.

Step1: Main (choose one)
  1. Textures  (default)
       installs the textures
  -> Textures

Step1: Meshes (choose one)
  1. Normal Size Mesh  (default)
       Slartibart has provided me with his mesh to use, this is the original mesh size.
  2. Smaller Mesh
       Slartibart has provided me with his improved mesh to use, this mesh is 25% smaller.
  Your choice [1]:
added 'HD Septim' to little.toml at line 28, at the end of the file

$ mudcrab add little.toml --id "Savilla's Stone Enhanced" --section Clutter \
    --archive "Savilla's Stone Enhanced-52713-1-1a-1675795943.7z" \
    --plugin SavillaStoneEnhanced.esp --load-after BettysButterflies.esp
'Savilla's Stone Enhanced' is a BAIN package. Pick the subpackages to install;
numbered alike (01 ..., 01 ...) usually means alternatives.

Subpackages (choose one or more, separated by spaces)
  1. 01 - Iced Style (.esp FREE)  (default)
  2. 01 - Marbled Style - Alternative  (default)
  Your choice [1 2]: 2
added 'Savilla's Stone Enhanced' to little.toml at line 48, after the last mod in section Clutter
```

The two styles are alternatives, so only the second is taken. Many BAIN
packages ship a wizard (`wizard.txt`) instead; `add` runs it as Wrye Bash
would, asks its questions, and records what it does -- the subpackages, any
plugins it leaves out, its INI tweaks. Some wizards check whether another
mod's file is already installed; `add` answers that from your load order when
it can and asks you when it cannot.

In a script, or with `--defaults`, `add` asks nothing: it takes every default
(a wizard's own, or all of a BAIN package's subpackages) and notes what you may
want to change.

A few more ways in:

- **`--nexus <modid>/<fileid>`** instead of `--archive`, for a mod you have not
  downloaded yet. It writes the source and (with an API key) the file name; add
  the layout once you can `inspect` the archive.
- **`--archive` with `--nexus`**, for an archive with no `.meta`.
- **`--manual`**, with `--archive`, for a file that is not on Nexus. The entry
  says `path = "manual:<file name>"`, and whoever installs the list downloads
  it themselves. Fill in the `download_page` and `download_note` lines it
  leaves commented out: `download` opens the page and prints the note, so
  nobody has to open the list to find out where to go.
- **`--before-mod`**, to put an entry somewhere other than the end of its
  section. Order within a section is MO2's priority order, which decides whose
  files win a conflict. `mudcrab move` moves an entry later.

## 5. Finish by hand

Some things no command asks, and the file is plain text.

To make a mod optional, declare a question and give the mod an `if`:

```console
$ mudcrab input add little.toml butterflies --type bool \
    --query "Add more butterflies to the wilderness?" --default true
✔ Declared input butterflies
$ mudcrab set little.toml --mod "More Butterflies" if=butterflies
✔ Set on More Butterflies: if = "butterflies"
```

which writes, at the top of the file and in the mod's block:

```toml
inputs.butterflies.type = "bool"
inputs.butterflies.query = "Add more butterflies to the wilderness?"
inputs.butterflies.default = true
```

```toml
[[mods]]
id = "More Butterflies"
section = ["Creatures"]
plugins = ["BettysButterflies.esp"]
if = "butterflies"
```

When the answer is no, the mod is left out and its plugin leaves the load
order. Conditions can also ask about other mods (`if = 'mod:"HD Septim"'`) and
combine with `&&`, `||` and `!`; see
[modlist-format.md](modlist-format.md#optional-mods-and-inputs).

The rest of what you would otherwise change by hand has a command too, each
keeping your comments and refusing an edit that would not validate: `set` for
any field of a mod or archive, `remove`, `plugins move` for the load order,
`files` to see what an archive installs and change its filters, `action add`
and `action remove`. See [Editing a list](usage.md#editing-a-list). The file
is still plain text, and editing it by hand is always fine.

If your guide has a step no command can take -- building the Bashed Patch in
Wrye Bash is the usual one -- write it down as a `[[manual-steps]]` table, and
`install` tells whoever installs the list what is left to do, at the end of
every run, until it is done. See
[modlist-format.md](modlist-format.md#steps-by-hand).

## 6. Build it

Check and build the list as you go, not only at the end. `validate` is
instant and silent when all is well:

```console
$ mudcrab validate little.toml
$ mudcrab compile little.toml --output build/compiled.json
✔ Compiled 4 mod(s), 3 plugin(s), 1 input(s) → build/compiled.json
$ mudcrab query build/compiled.json --output build/plan.json --set butterflies=false
✔ Plan: 3 of 4 mod(s) selected (butterflies=false) → build/plan.json
$ mudcrab download build/plan.json
✔ 4 archive(s) ready in /home/you/Games/OblivionModDownloads (0 downloaded now)
$ mudcrab check build/plan.json
✔ Checked 4 archive(s) in 4 mod(s), and the masters of 3 plugin(s): 0 problem(s), 0 warning(s)
```

To see the result without an MO2 instance, install into a plain folder and
look:

```console
$ mudcrab install build/plan.json --mods-dir build/mods
4 mod(s) installed or updated, 0 unchanged
$ ls "build/mods/Savilla's Stone Enhanced"
meshes  SavillaStoneEnhanced.esp  textures
```

Then install into a test MO2 instance (`--mo2-instance-dir`) and play. On a
large list, work a section at a time: `download`, `check`, `install` and
`diff` all take `--section` and `--only`.

## 7. Beyond the basics

The four mods above needed nothing more. Real lists usually do:

- **Actions** change a mod after it is unpacked: set an INI value
  (`ini_set`), leave files out (`file_prune`), clean a plugin with xEdit
  (`qac`), pack loose files into a BSA (`pack_bsa`), and more. They are
  `[[mods.actions]]` entries, run in order;
  see [modlist-format.md](modlist-format.md#actions).
- **Conflicts.** When two mods provide the same file, MO2's order decides.
  After an install, `mudcrab conflicts` with no `--mod` sweeps the list and
  names every mod that shares files, with how many it wins and loses and to
  whom; `--mod <id> --with <other>` then lists the files themselves, and
  `file_prune` with `conflicts_with` removes them from one side.
- **Merged plugins** keep the plugin count down: a `type = "merge"` entry
  names the source plugins, and `install` builds the merged one
  (`mudcrab new-merge` drafts the sources from a list of plugin names);
  see [merge-standalone.md](merge-standalone.md) and
  [modlist-format.md](modlist-format.md#merges).
- **Checking against a reference.** If you have built the list by hand
  before (e.g. directly in MO2), `mudcrab diff` compares each installed mod
  against that instance, file by file.
- **A readable guide.** `docs` tables add prose and pictures to the list,
  section and mods, and `mudcrab export` turns the list into a guide for
  people installing by hand, as a Nexus page, Markdown or a website; see
  [exporters.md](exporters.md).
