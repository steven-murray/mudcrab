# mudcrab

The rust-based platform-independent declarative modlist creator, compiler, manager and 
installer. 

**I've seen mudcrabs more fearsome than you!**

## What Is `mudcrab`?

Mudcrab aims to make it easy to create, share, manage and install modlists for games
like TES Oblivion and Skyrim. 

## Usage (Current MVP)

Detailed command reference is also available in `docs/usage.md`.

The CLI currently supports MVP implementations for all four pipeline stages:

1. `compile` (source TOML -> compiled JSON)
2. `query` (compiled JSON -> personalized plan JSON)
3. `download` (personalized plan -> cached archives)
4. `install` (personalized plan + cache -> staged mod archive layout)

The `export` command is scaffolded but not implemented yet.

### Command Overview

Print help:

```bash
cargo run -- --help
```

Compile a source modlist:

```bash
cargo run -- compile path/to/modlist.toml --output build/compiled.json
```

Resolve inputs into a personalized plan (headless mode):

```bash
cargo run -- query build/compiled.json --output build/plan.json --headless
```

Download required archives into a cache directory:

```bash
cargo run -- download build/plan.json --cache .mudcrab-cache --retry 3
```

Validate source modlist without writing compiled output:

```bash
cargo run -- validate path/to/modlist.toml
```

### End-to-End Example

```bash
cargo run -- compile tests/fixtures/modlists/simple.toml --output build/compiled.json
cargo run -- query build/compiled.json --output build/plan.json --headless
cargo run -- download build/plan.json --cache .mudcrab-cache
```

### Input and Output Files

1. Source modlist (`.toml`): author-facing input.
2. Compiled modlist (`compiled.json`): validated machine-friendly intermediate format.
3. Personalized plan (`plan.json`): selected mods and resolved input responses.
4. Cache directory: downloaded archives for later install stage.
5. Mods directory: staged archive layout and an `install_manifest.json` summary.

### NexusMods Support (Early)

Nexus is supported early in development through two source patterns:

1. Direct URL containing `nexusmods.com` (downloaded via HTTP handler).
2. API descriptor format: `nexus:<game>/<mod_id>/<file_id>`.

For API descriptor sources, set:

```bash
export NEXUS_API_KEY="your-api-key"
```

Optional override for API base URL (useful for local testing/mocking):

```bash
export NEXUS_API_BASE="https://api.nexusmods.com/v1"
```

### Current Modlist TOML Shape

At minimum:

```toml
name = "Example Modlist"

[[mods]]
id = "core"
dependencies = []

[[mods.archives]]
path = "https://example.com/mod.zip"
download_handler = "http"
```

Conditional inclusion example:

```toml
name = "Conditional Example"

[inputs.use_hd_textures]
type = "bool"
query = "Install HD textures?"

[[mods]]
id = "base"
dependencies = []

[[mods]]
id = "hd_pack"
dependencies = ["base"]
if = "use_hd_textures"
```

### Notes and Limitations

1. Query supports basic expressions: `flag`, `!flag`, `key == value`, `key != value`.
2. Download `--parallel` is accepted but currently processed sequentially.
3. Install MVP now unpacks `.zip`, `.tar`, `.tar.gz`, and `.tgz` archives into per-mod directories.
4. `.7z` and `.rar` archive extraction is planned but not implemented yet.

## Wishlist

These are features we want, but are intentionally deferred for later milestones.

1. True concurrent downloads with bounded parallelism honoring `--parallel`.
2. Download resume and stronger integrity verification (checksums/signatures).
3. Full Nexus workflow polish (expanded metadata support and richer auth UX).
4. Add `.7z` and `.rar` extraction support and expand archive-format coverage.
5. Install layout handlers (e.g. FOMOD/custom data folder) and post-install actions.
6. Export phase implementation (Markdown/HTML output from compiled plans).

`mudcrab` is **not** a mod manager. It doesn't replace Mod Organizer 2. 
It's **more** like Wabbajack -- an automatic way to install an entire cohesive
set of mods that is possibly curated by someone else.
However, unlike Wabbajack, it uses a declarative approach, i.e. where the mods
to be installed, and any custom actions that need to be applied to those mods, are
statically **declared** in both a human-friendly and computer-friendly way.
This avoids the biggest drawback of Wabbajack mods: the difficulty of applying 
modifications to the list once it is installed. This drawback has two main 
common problems:

1. You want to make a small modification to the list post-install (like, changing
   the resolution at which the game is played) but it's hard to find where that setting
   is set (you didn't create the modlist after all!) and you're unsure what other
   settings you might break in doing so.
2. Related to (1) but a bit different in practice, you may want to make a bigger change.
   For example, you want to add in a new mod. But that mod requires several patches
   with other mods in the list. So installing that mod means going back through the 
   entire list and re-installing any mod that might include patches to check the 
   relevant tick-box in its FOMOD procedure. Problem is, on the surface you don't even
   know which mods might need to be patched. You can find out, but then you might as 
   well have installed the list yourself manually!

These problematic outcomes stem from two properties of a Wabbajack install:

1. The data just "is". You install it, it exists, but there are no encoded relationships
   between any of the installed mods. So if I change a mod, there's no way to tell if
   this affects another mod. 
2. The wabbajack author's *intent* is not made clear. While the author *can* add notes
   to each mod to enlighten the user a little bit, often this is not done, and even if 
   it is, it is to a minimal degree. Without knowing *why* a certain mod was included
   or altered, it's hard to make judgments about how to add your own changes other than
   by asking the author. 

`mudcrab` tries to get around these problems by providing:

1. A definite, human-readable (and machine-interpretable) text-based "language" for 
   defining a full mod-list: including the ability to specify:
    
    * Local or remote file locations for downloading mod archives (including from 
      the nexus)
    * The ability to specify the name of the mod as it will appear in the final
      modlist (with sensible defaults, for example when the mod is simply a single 
      archive on the Nexus, just using the name of the Mod as defined there).
    * The ability to specify the archive format and how to unpack it.
    * The ability to specify (with glob-style syntax) files to include or exclude
      from the archive.
    * The ability to specify custom (or built-in) actions on the mod (e.g. applying
      xEdit's Quick Auto Clean) after install.
    * Conditionals: the ability to install mods or perform actions based on the result
      of other data (e.g. only install Mod B if also installing Mod A).
    * Ability to query the user for information and use that information in conditionals.
      For example, a modlist install could ask the user if they want to use Mod A or
      Mod B, and then use that info throughout the rest of the entire list. 
2. A staged set of actions to go from author-defined list to installed set of mods:

    1. Compile: take an author-defined list and verify that it is valid, verify that
       the mods exist, and fill in any missing information to obtain a detailed
       machine-readable modlist format.
    2. Query: ask the user for any required inputs and proceed to create a unique
       modlist based on that info.
    3. Download: proceed to download all the required mod archives in the unique modlist.
    4. Install: Unpack each archive, and apply each custom action, to arrive at a fully
       installed set of mods.

3. A set of command-line-friendly tools to help manage each step. Being command-line
   friendly helps with compatibility on different platforms, but also helps with
   automation by other tools. The command-line tools can help with things like:

   * Adding mods to the list as an author. While the author can always directly edit
     the modlist with a text editor, using the command-line tools can make this process
     faster/easier.
   * Ability to export the modlist into different formats, including markdown or 
     HTML, so you can *at the same time* provide an easy manual guide to users that
     prefer to do it themselves the old way. 


Some Features Include:


1. Since the modlist format is declarative and compiled, the logic need not be ordered.
   That is, you can conditionally include Mod B depending on if Mod A is included, even
   if Mod A is installed after Mod B (and therefore takes precedence in terms of file
   loading).
2. It is composable. If a node in the modlist tree refers to an external file that has
   the format of a modlist, that modlist will be included at that point in the tree. 
   Thus, you can publish small, re-useable modlists that other lists can point to. 
   This can be useful even for single mods that require non-standard installations.
3. The ability to specify modding tools that must be installed alongside the mods
   (either simply for the benefit of the end-user, or as dependencies for the custom
   actions applied to the mods).
4. The ability to "install" the modlist to different formats (e.g. directly to the 
   Game install folder, or in Mod Organizer 2 format, or other mod manager formats).


## The Mod-List Format

The modlist format is TOML with the following allowed fields:

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

### Archive Layouts

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

Each `mod` in the `modlist` has the following format:

```
[modname]
if: <conditional>  # only install this mod if conditional is true

[[archives]]
[[<path/to/modarchive1>]]
download_handler = nexus
layout = fomod  # or 'simple' or 'custom-data-folder'
queries = [
    "FOMOD first question?", "True",
    "FOMOD second question?", "Orange Styling",
    ... etc
]
include = "Textures/*"
exclude = "Meshes/*"

[[<path/to/modarchive2>]]
download_handler = nexus
layout = "custom-data-folder"
data_folder = "."  # e.g. if layout='custom-data-folder' and the data/ folder is the root
include = "Meshes/arg*"

[actions]
[actions.qac]  # Quick-Auto-Clean all esps (specify specific esps by giving args)
[actions.bsa]  # Pack all Textures/ Meshes/ and Sound/ into a BSA archive.
```

Along with standard "archive"-based mods, it is possible to specify custom mods that
might depend on the other mods. For example, to create a new mod that contains a merged
plugin:

```
[modname]
dependencies = [
    "mod A",
    "mod B",
]
type = "zmerge"
plugins = [
    "modA.esp",
    "modB.esp",
]
```

TO perform completely custom actions:

```
[modname]
dependencies = [
    "mod A",
    "mod B",
]
type = "custom"

actions = [
    "my-custom-script --mod A",
    "my-other-script --args etc"
]
```