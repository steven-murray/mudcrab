# Roadmap

mudcrab compiles, checks, downloads, installs, patches and merges a complete
700-mod list, turns it back into a guide, and has installed MOFAM from scratch
on a fresh machine, driven from the published docs. This page lists what is
still to do. What is done is in the [releases](https://github.com/steven-murray/mudcrab/releases)
and the git history.

## Next

### Composability

`include` another modlist -- a file, or a file in a git repository -- so
reusable pieces can be published once and pointed at: a shim that fixes a
badly packaged mod, or a core group of mods several lists share. The
design is in [the plan](https://github.com/steven-murray/mudcrab/blob/main/docs/design/composability.md).

### A GUI, alongside the CLI

An authoring mode (drag mods into order, pick a mod's files, move folders) and
an install mode (view the list, one button from compile to install). The CLI
stays complete: nothing will ever need the GUI. The library API and the CLI
commands it needs are done; next is the framework spike. The
design is in [the plan](https://github.com/steven-murray/mudcrab/blob/main/docs/design/gui.md).

## Open

### Plugins and merges

- **OBME fields.** 17 Oblivion Magic Extender fields are refused rather than
  merged. `EFIX`'s parameter is a union whose type is chosen by the `EFME`
  subrecord of the same record, so it needs a field kind that can see its
  siblings; that would also handle `EFIT` properly. Until then a merge with an
  OBME plugin is refused, saying so. (The header of
  `src/plugin/schema/tes4.rs` explains why marking them opaque would be wrong.)

### Installing

- **Is MO2 running, on Windows.** `install` refuses while MO2 has the instance
  open on Linux (from `/proc`); Windows has no check yet.
- **Parallel unpacking.** A first install unpacks archives one at a time;
  `check` already runs a thread per CPU. Install's ordering (mods recorded as
  they finish, conflict partners first, xEdit one plugin at a time, merges
  last) needs care.
- **`nxm:` links on Linux.** Only one MO2 receives them, and the usual handler
  starts the game's Steam entry, which does nothing while it runs. `download`
  opens the browser's own download there instead. A handler that routes a
  link to the instance being installed would bring "Mod Manager Download" back.

### Checking

- **Partners whose own actions move files.** A `conflicts_with` naming a mod
  whose files change during its install (a `file_move`, say) cannot be
  rehearsed by `check`, which notes it. Rehearsing the partner first would
  close it.
- **`assets` is noisy, and blind to textures.** It reports what plugins name
  and nothing provides, minus what the game already lacks. Known-harmless
  references (a mod's optional add-ons) need a way to be marked; textures
  need the meshes read.
- **Conditions on archives, in MOFAM.** `if` on archives, FOMOD rows and BAIN
  subpackages is tested but no MOFAM choice uses it yet: the obvious one,
  texture resolution, needs the alternative archives downloaded.

### Output

- `inspect` and `diff` print reports of their own, not yet in the styled
  output every other command uses.

### Export

- The MOFAM docs were transcribed from the guide's Markdown, which lost some
  formatting (stray emphasis around lists); converting the page's HTML afresh
  would do better.

## Later

- **A `custom` mod type** for build steps a list supplies itself.
- **Downloads:** resume, checksum manifests, signature verification.
- **Other games and other mod managers.** The plugin and BSA code is TES4; the
  pipeline above it is not.
- **Building a Bashed Patch.** Only Wrye Bash builds one, from its window, so
  `install` lists it as a step by hand. Leveled-list merging alone would cover
  much of what lists use it for.

Known limitations with workarounds are in [Known issues](known-issues.md).
