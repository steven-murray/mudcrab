# Known issues and limitations

Things that can bite you, and what to do about them. Ordered roughly by how
likely you are to meet one.

## Scope

mudcrab targets **TES4: Oblivion** and exports to **Mod Organizer 2**. The
schema is not Oblivion-specific and the pipeline is not MO2-specific, but no
other game or mod manager has been exercised, and the plugin/BSA code is TES4
format. Treat anything else as unimplemented rather than untested.

## Archives

**A downloads folder from before 0.2 needs converting once.** Archives used to
be kept under derived names (`{mod}_{index}_{source}` + `.orig-name`); lookup
now uses real names and MO2 `.meta` files, and does not see the old names.
`mudcrab downloads tidy <dir> --plan <plan.json>` prints the conversion, and
`--apply` does it.

**Manual (non-Nexus) archives are matched by filename.** If the host re-uploads
under a new name, lookup misses; the "download it by hand" error then names the
closest-matching files already in the folders it searched.

**Lookup needs a `.meta` for a Nexus archive the modlist gives no `file_name`.**
MO2 writes one for every download through it, and mudcrab for every download it
makes; an archive dropped into the folder by hand has none, and is only found if
its entry declares `file_name`.

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

**A plugin using Oblivion Magic Extender cannot be merged.** OBME adds fields to
ALCH, ENCH, INGR, SPEL and MGEF whose parameters are FormIDs or not depending on
a byte in a neighbouring subrecord, which mudcrab's per-field table cannot
express. It refuses rather than guessing. Every other field the format uses is
covered: an audit of 431 plugins and 758,464 records leaves only these.

> The error names the field and where to add it. This is deliberate: mudcrab
> cannot tell whether an unknown field holds a FormID, and guessing wrong would
> silently corrupt the merge. Run `cargo run --bin plugin-audit -- <mods dir>`
> to see the whole worklist for your own setup.

**Merge output is not byte-identical to zMerge's**, and is not meant to be. Both
produce the same record set and the same reference graph; the FormIDs allocated
to renumbered records differ, and zMerge retains some masters mudcrab drops.

## INI edits

**There is no `game-root` INI scope.** `scope = "game"` resolves to the MO2
profile's copy of `Oblivion.ini` and never touches the game directory. A file
that genuinely has to land in the game install root — an ENB config, say — needs
`game_root_files` on an archive (or a `generated` mod's `write_file` under
`root/`) instead, which places the file but cannot edit one in place.

## Game-root files

**They need the Root Builder MO2 plugin.** `game_root_files` stages files in
the mod's own `root/` folder, and Root Builder copies them beside `Oblivion.exe`
when MO2 launches the game and restores what was there when it exits. mudcrab
never writes the game folder itself. A list installs Root Builder as a
`type = "mo2-plugin"` entry, as MOFAM does; without it, `root/` just sits in the
mod and the files never reach the game.

**Root Builder cannot be downloaded through the Nexus API** without a premium
account, like any Nexus file: `download` opens its page for you to download
by hand. Use copy mode (`copyfiles=**`); the author's link mode has a
known bug with MO2 2.5.2 before Root Builder 5.0.5.

**Several MO2 instances share one game folder.** Root Builder restores the game
folder on exit, so each instance gets its own root files while it runs. A crash
can leave them deployed until MO2 next starts and cleans up.

## Linux / Proton

Two fixes for DXVK on NVIDIA under Proton, each an `[inputs]` switch in MOFAM
that defaults on and should be off on Windows:

- **`decompress_dxt1_alpha`** — Oblivion's menus draw DXT1 punch-through alpha
  as opaque black (DarNified UI's compass frame, map markers, inventory icons).
  `dds_decompress` rewrites those textures as uncompressed BGRA; `mudcrab dds
  audit` lists them. Found by reproducing on a near-vanilla profile with DarN
  alone.
- **`dxvk_report_amd_vendor`** — on an NVIDIA vendor id ORC copies the depth
  buffer through NVAPI, which returns nothing under Proton: water vanishes and
  every depth effect breaks. A `dxvk.conf` with `d3d9.customVendorId = 1002`
  (deployed by Root Builder from the "DXVK Linux Config" mod) sends ORC down its
  RESZ path. `ORC.log` line 6 says which path it took. Hiding NVAPI with a DLL
  override does *not* work: ORC decides by vendor id.

**ENB does not run** (input `enb`, default `none` for that reason). Both v0500
and v0181 crash inside ENB's `d3d9.dll` on save load, deployed correctly by
Root Builder over DXVK. A `PROTON_LOG=1` run shows 711 `d3dcompiler:
assemble_shader Asm reading failed` from the moment ENB loads: Wine's D3DX9
cannot assemble ENB's shaders. Microsoft's `d3dx9_43`/`d3dcompiler_43` are in the
prefix (Steam's DirectX redistributable), but neither `WINEDLLOVERRIDES` nor a
winecfg override made the game load them -- the log still says `builtin` -- and
the crash persisted. Untried: why the override does not take (MO2/usvfs is the
suspect), and chaining ENB to DXVK through `ProxyLibrary`, which the reference
instance's hand-edited `enbseries.ini` attempted earlier.

## MO2 must be closed while installing

`install` writes the profile's `modlist.txt` and `plugins.txt`; a running MO2
holds its own copy and writes it back on exit, silently undoing the install. It
happened once in testing: MO2 re-enabled the previous ENB binary and dropped
the new one, so a whole test ran against the wrong version.

## Layout detection

**With no `layout`, an archive installs only if its content root is
unambiguous.** Install follows the plugins, unwraps a lone wrapper folder, takes
a top-level `Data/`, or uses the root -- and refuses when game content sits in
two places, or below a root that has none, rather than guess. `inspect` asks the
same code, so its advice is what install will do; when install would refuse, it
suggests a `data_folder` that install's own data-folder path accepts.

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
