# Composability: lists built from other lists

**Status: done** (see [the format](../modlist-format.md#lists-in-several-files-include)),
but for publishing a first shims repository. Where a phase settled something
differently from the plan below, the notes under [Phases](#phases) say so.

Before this, a modlist was one file. This plan lets a list pull in other modlist files
-- from its own folder or from a git repository -- and have them resolve as if
they had been written in place. It has two uses in mind, and one side effect:

1. **Shims.** A popular mod packaged badly (its plugin in a stray folder, a
   FOMOD that installs the wrong thing, a texture pack that needs pruning)
   gets one fix, kept in a repository everyone points at. The shim does
   nothing except describe how to install that mod properly; every list that
   uses the mod uses the shim, and a fix to it reaches them all.
2. **Shared groups.** A set of mods several lists all want -- the OBSE
   usability plugins, the unofficial patches, a UI stack -- defined once and
   included wherever it is wanted.
3. **Splitting a long list.** MOFAM could be a short `mofam.toml` that
   includes each Part from its own file. Nothing needs this, but it falls out
   of the same mechanism, and it makes a 10,000-line file navigable.

## What stays true

- **The compiled plan is flat.** Includes are resolved by `compile`, and
  everything after it -- `query`, `download`, `check`, `install` -- sees one
  list, as today. No later stage knows includes existed, except to say where a
  mod came from.
- **Data, never code.** An included file is TOML, interpreted by mudcrab. It
  cannot run anything: actions are the closed set they are today. Including a
  repository is trusting its author the way installing their modlist is, no
  more.
- **Reproducible.** A list resolves to the same mods tomorrow as today: a git
  include names a tag or commit, and a lock file records exactly what was
  used.
- **Comments survive editing.** `add`, `move` and `downloads record` keep
  splicing lines rather than re-emitting TOML, so a component file is edited
  the same way a list is.

## Shape

### An include is an entry in `[[mods]]`

`[[mods]]` order is MO2's priority order, and that order has to say where an
included group goes. A separate top-level `[[include]]` table would lose it --
TOML keeps each array's order, not the order between arrays -- so an include is
a `[[mods]]` entry with `include` instead of archives:

```toml
[[mods]]
id = "Unofficial Patches"            # names the include, for messages and overrides
section = ["01 - Core"]              # where its mods land, as a prefix
include = "git+https://github.com/someone/oblivion-common?tag=v3#patches.toml"

[[mods]]
id = "Part 3"
include = "sections/03-ui.toml"      # a path, relative to this file
```

At compile time the entry is replaced, in place, by the included file's mods,
each with the entry's `section` prefixed to its own.

### What an included file is

An ordinary modlist file -- it validates and compiles on its own -- with two
differences in how its top-level keys are read when included:

| Key | Included |
| --- | --- |
| `[[mods]]` | Spliced in at the include's position. |
| `plugins` | A fragment of the load order, placed by the includer (below). |
| `inputs` | Questions passed up to the includer, which can answer them (below). |
| `ini`, `manual-steps`, `post-install-actions` | Merged in; a conflicting `ini` key is an error naming both files. |
| `docs`, `sections` | Carried along, so `export` documents an included group. |
| `name`, `mo2`, `export`, `guide` | Ignored: they describe a whole list. |

A file meant only to be included can say so with `component = true`, which
lets it leave out `name` and stops `install` being run on it directly.

### Where included plugins go

The including list's `plugins` places each include's fragment with a table
entry naming it (TOML 1.0 allows mixed arrays):

```toml
plugins = [
  "Oblivion.esm",
  { include = "Unofficial Patches" },   # the fragment, in its own order
  "Oscuro's_Oblivion_Overhaul.esm",
]
```

An include whose fragment is not placed is an error, as an unlisted plugin is
today. `inactive-plugins` and merges name included plugins like any other.

### Shims: one mod, defined elsewhere

A shim is a component holding one mod. Using it in place of writing the entry:

```toml
[[mods]]
id = "Oblivion Trees"
section = ["13 - OBLIVION REALM"]
use = "git+https://github.com/oblivion-shims/shims?tag=2026.10#oblivion-trees.toml"
```

`use` takes the shim's single mod -- archives, layout, actions, plugins,
download page -- and merges the entry's own fields over it, so a list can
still add an action or answer a FOMOD question differently. `include` brings a
group in whole; `use` adopts one definition into an entry the list owns.

### Customising what comes in

Two mechanisms, for the two people involved.

**The component's author declares knobs** -- ordinary `[inputs]`. An included
input reaches the includer namespaced by the include's id
(`"Unofficial Patches".install_dlc_patches`), and the includer either answers
it -- fixing it for everyone who installs the list -- or leaves it, so `query`
asks the player:

```toml
[mods.set]                       # on the include entry
install_dlc_patches = false
```

Conditions in the component (`if = "install_dlc_patches"`) resolve against
the namespaced input, so components cannot collide over names.

**The includer adjusts anything**, without forking, by overriding entries by
id:

```toml
[mods.override."Unofficial Oblivion Patch"]
skip = true                                  # leave this one out
[mods.override."Display Stats"]
fomod_selections = [{ step = "Main", group = "Size", option = "Large" }]
actions.append = [{ action = "file_hide", paths = ["textures/menus/stats.dds"] }]
```

An override names fields that replace the component's, with `.append` for the
lists where adding is the usual intent (`actions`, `exclude`, `plugins`). An
override naming a mod the component does not have is an error: an override
must not quietly outlive the mod it was for.

**mudcrab's own include options** -- the "global hooks" -- are transforms any
include can ask for, implemented once in mudcrab:

| Option | Effect |
| --- | --- |
| `section` | Prefix every included mod's section. |
| `if` | A condition on the whole group: every included mod takes it. |
| `prefix` | Prefix every included mod id, when two components use the same names. |
| `only` / `except` | Take a subset of the component's mods by id. |

These cover what an includer needs without code. If a real case turns up that
they cannot express, the answer is another declarative option, not a script.

### Fetching

- **Sources**: a path (relative to the including file), or
  `git+<url>?tag=<tag>|rev=<commit>|branch=<branch>#<path in repo>`. `tag` or
  `rev` is the norm; `branch` is allowed, for a component being developed,
  with a warning.
- **How**: the `git` command, as `7z` is used today (no libgit2 build
  dependency). Repositories are cloned once into `~/.cache/mudcrab/git/`, keyed
  by URL, and fetched only when the lock does not hold the commit wanted.
- **The lock file**: `mudcrab.lock`, next to the list, records each git
  include's resolved commit and the hash of the file read. `compile` uses it
  as is; `mudcrab update [<include id>]` moves an include to the newest commit
  its `tag`/`branch` allows, and shows the difference in mods.
- **Offline**: `compile --offline` (the flag exists and is hidden) uses only
  the cache and the lock, and says which include it cannot resolve.
- **Includes include**: resolved recursively, with cycles refused and each
  repository's lock entries carried up, so the top-level lock pins everything.

### Where a mod came from

Every compiled mod records its origin -- file, and for git the URL and commit
-- and every message about it uses it: `check`, `validate` and `install`
errors say "in oblivion-common@3f2a1c0:patches.toml, mod 'X'", not just the
mod id. `export` can render it ("from the Oblivion Common patches, v3").

## Commands

- **`compile`** resolves includes; `validate` too.
- **`mudcrab update`**: move includes forward within their constraints, rewrite
  the lock, print what changed (mods added, removed, archives changed).
- **`mudcrab includes`**: list each include, where it resolved to, and how many
  mods it brought.
- **`add`, `move`, `downloads record`** take a component file like any list.
  Run against a list, they refuse to edit a mod that came from a git include
  and say to use an override (or to edit the component's own repository).
- **`add --use <shim>`** writes a `use` entry, filling the shim's mod id and
  plugins into the list's load order.
- **`check`** names the component in each finding.

## Phases

1. **Local includes.** `include = "<path>"` in `[[mods]]`, splicing, section
   prefixing, the `plugins` fragment, ini/manual-step merging, origins in
   messages, cycle detection. Proved by splitting a copy of MOFAM into a file
   per Part and compiling it to a plan **identical** to today's. *Done.* As
   built:
   - Includes are resolved by the loader, not `compile`, so `validate`,
     `export` and the editing commands see the same list `compile` does.
   - An included file's mods may leave their plugins to the including list's
     `plugins`. A fragment is placed in one piece, and MOFAM's load order
     interleaves its Parts, so splitting MOFAM by Part keeps the whole order in
     the top file; the plan was identical for three sets of answers.
   - Until phase 4, an included file cannot declare `inputs`, and its
     conditions ask the including list's questions; `component = true` lets
     it validate on its own regardless.
   - Editing commands change an included mod in its own file and check the
     whole list with the change. A mod moves only within its file; a plugin
     only within the file that places it. A remove or disable that touches a
     mod in one file and its plugins in another is one edit, checked as one.
   - `origin` is in the compiled list and the plan but not in a mod's
     definition hash, so splitting a list does not reinstall anything.
2. **Overrides and include options.** `override`, `skip`, `.append`, `section`,
   `if`, `prefix`, `only`/`except`. Tests for each, and for an override that
   names nothing. *Done.* As built:
   - `.append` works on any list field of a mod, not a chosen few. `exclude`
     is an archive's field, not a mod's, so changing it means overriding
     `archives` whole.
   - An override is applied by turning the mod back into TOML, replacing the
     fields, and reading it again, so it is checked exactly as a mod written
     out in full would be.
   - Conditions have no parentheses, so a group's `if` is joined to a mod's
     own by multiplying out to an `||` of `&&`s.
   - `prefix` renames mentions within the included group only: an including
     list naming one of its mods uses the prefixed id.
   - Edits still go to the file a mod is written in, by its id there; a field
     an override sets is refused rather than written where it would do
     nothing. Writing overrides from the editing commands (for a git
     include's mods) is left for later.
3. **Git sources and the lock.** Fetch, cache, `tag`/`rev`/`branch`,
   `mudcrab.lock`, `update`, `includes`, `--offline`. *Done.* As built:
   - The lock file is `<list>.lock`, not `mudcrab.lock`: one folder often
     holds several lists (MOFAM's full and minimal), which should not move
     together. It records the commit for each URL and reference; a commit
     pins the file's content, so no file hash is kept.
   - Each commit used is unpacked (`git archive`) into the cache, read only,
     so a git include is then a file on disk: its own relative includes, docs
     file and pictures resolve inside the same commit with no more code, and
     a path leading out of the repository is refused.
   - Nested git includes are recorded in the top list's lock like the rest;
     a component repository's own lock file is not read.
   - A fetch happens only when the lock lacks a reference (or the cache a
     commit), so loading a list that resolved once never touches the network.
     `update` drops the chosen lock entries and loads again; a `rev=` include
     resolves to the same commit and so never moves.
   - Following a branch is a `validate` warning.
   - The editing commands refuse a file from a git include, pointing to its
     repository and `update`, until overrides (phase 2) give the includer a
     place to make such a change.
4. **Inputs across includes.** Namespacing, `set`, conditions resolving
   against namespaced inputs, `query` asking only what is left. *Done.* As
   built:
   - The namespace is the include's id with anything but letters, digits,
     `-` and `_` turned into `_` (`Unofficial_Patches.install_dlc_patches`),
     as a condition's input names cannot be quoted.
   - A `set` answer is kept on the question as `fixed`: `query` uses it and
     refuses `--set` for it, and `export` (so the GUI) leaves it out. Keeping
     the question, rather than folding the answer into each condition, means
     nothing about the conditions has to be rewritten.
   - An included file's conditions can still ask the including list's
     questions by their own names, as in phase 1: only names the file
     declares are namespaced.
5. **Shims.** `use`, field merging, `add --use`. Write the first real shims
   from MOFAM's own fixes (Oblivion Trees' layout, the T4UTXL split, WAC's
   prune) into a public `oblivion-shims` repository, and point MOFAM at them.
   *Done, but for publishing.* As built:
   - The entry's fields are read as written (from the file's TOML, not the
     parsed entry, which cannot tell a field left out from one set to its
     default) and laid over the shim's mod with the override mechanism, so
     the result is checked as a mod written in full.
   - The entry's `actions` are added after the shim's, since adding one is
     what a list using a shim wants; every other field replaces.
   - The mod is the list's own: its `origin` is the list, edits write the
     entry, and its definition hash is the result's, so moving a mod into a
     shim reinstalls nothing.
   - The first three shims (WAC's include-and-rename, T4UTXL's Priory keep
     list, Oblivion Trees' hand-download details) are written into a local
     `oblivion-shims` repository, and a copy of MOFAM using them from it,
     through a git source and lock file, plans identically. Publishing the
     repository and pointing MOFAM at it is left to the list's author.
6. **A shared group.** Move MOFAM's OBSE plugins section into a component
   and include it, as the proof that groups work in a real list. *Done.*
   MOFAM's Part 2 (35 mods) moved to `components/obse-plugins.toml`, included
   under its section; the compiled list, the plans for three sets of answers
   and the exported guide are identical to before. (OBSE plugins ship no
   plugins, so the load order did not have to move with them.) A test has two
   lists take different things from one group: one as it is, asking its
   question; the other renamed, its question answered, a field overridden.

Each phase ends with MOFAM compiling to the same plan it does today, except
where the phase changes what is installed on purpose.

## Questions to settle

- **Who owns a shim's archive versions?** A shim pins a Nexus file id; when
  the mod updates, every list on that shim moves when the shim does. That is
  the point, but a list may need to stay behind: an override of `archives`
  does that, and `update` should show archive changes prominently.
- **Docs for included mods.** Carry the component's prose into `export` as is,
  or let the includer override it per mod? Probably both, by the same
  override mechanism.
- **Naming.** `include`/`use`/`override` read well in TOML; check they do in
  error messages and `--help` before settling.
