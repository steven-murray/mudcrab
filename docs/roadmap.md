# Roadmap to a publishable mudcrab

mudcrab now compiles, downloads, installs, patches, merges and verifies a
complete 700-mod modlist without driving a single GUI tool, and the result has
been played. That closes the question of whether the approach works. What
remains is the distance between *works for its author* and *works for a
stranger*.

The phases below are ordered by that distance. A and B are the ones a first
outside user would actually hit.

---

## Phase A — Debts a real user would meet

Each of these is a place the build worked partly because of something true of
this machine. Details in [known-issues.md](known-issues.md).

### A1. ~~Key the download cache by source, not by mod id~~ — done

The cache is gone as a separate idea: the downloads folder holds archives under
their real names with MO2 `.meta` files, found by Nexus ids or filename, and MO2
is pointed at it rather than given copies. `downloads tidy` converts a folder
from the old derived names.

### A2. Make manual archives resolvable when the host renames the file — error done

Today a `manual:` source matches its `file_name` exactly. The cheap part is
done: when one is missing, the error now names the best-matching files in the
search paths (same words, ignoring version-like ones), so "go download it by
hand" no longer hides the copy already on disk. Matching on content hash, or
letting `file_name` be a set of alternates, would make it resolve by itself;
neither is done.

### A3. ~~One implementation of "where is the content root"~~ — done

`inspect` now asks install's detection what it would do, instead of guessing
with rules of its own. Install's own rule was tightened where `inspect`'s was
better: a root with no content, with content in a folder below it, is now
refused rather than installed one level too deep. Measured over MOFAM's 589
archives with no declared layout, the change altered no decision. It did turn up
one entry install would already have refused (Mythic Ghosts and Goblins, saved
only by having been installed before that check existed), now declared.

### A4. ~~Tolerate out-of-range mod indices in merge sources~~ — already done

Recorded here as open from a stale note; `merge::rewrite` has clamped them with
a counted warning since commit `7ed2b6d`, and the merge report prints the total.

### A4b. ~~Close the TES4 schema gap~~ — done, except for OBME

`plugin-audit` over the 431-plugin instance reported **125 gaps**. Working each
one against xEdit's `wbDefinitionsTES4.pas` brought that to **17**, all of them
Oblivion Magic Extender fields.

Those 17 stay refused on purpose. `EFIX`'s parameter is a union whose type is
decided by a byte in the *`EFME` subrecord of the same record*, so a per-field
table cannot answer it — and marking the harmless OBME fields `Opaque` would let
such a plugin reach the point where `EFIT` is read as inert bytes and its FormID
silently not renumbered. The header of `src/plugin/schema/tes4.rs` says so, at
length, because it looks like an easy win.

**Remaining work**: a field kind with access to its sibling subrecords, which
would close OBME and let `EFIT` be handled correctly too. Until then a clearer
refusal ("this plugin uses OBME") would be worth more than the current
per-field error.

### A4c. The CTDA table was wrong in seven places — fixed

Condition parameters were classified empirically, by checking whether zMerge
rewrote them. Against xEdit's own `wbCTDAFunctions`, seven disagreed:
`GetScriptVariable`, `GetIsClass`, `GetIsRace`, `GetFactionRank`,
`GetDisposition`, `GetDeadCount` and `GetInCellParam` all take a FormID that
mudcrab treated as a plain integer.

They slipped through because the records those conditions pointed at were
vanilla, so their ids never changed during a merge. "Observed unchanged" was
true and meant nothing — a corpus shows which fields *did* change, never which
ones *can*. All 192 functions now come from xEdit's table.

All six MOFAM merges rebuild **byte-identical** with the correction, so nothing
already built was affected. The next list would not have been so lucky.

### A4d. ~~Refuse to install while MO2 is running~~ — done (Linux)

`install` refuses while an MO2 running from this instance is visible in
`/proc`, which covers MO2 under Wine; `--ignore-running-mo2` overrides. No check
on Windows yet.

### A5. ~~Honour `--parallel`, or remove it~~ — done

`download` now fetches `--parallel` archives at once.

---

## Phase B — Optional mods, and inputs that earn their keep

**Done**, apart from a MOFAM input that uses B2. `[inputs]` take a declared
`default` and `query` takes `--set`. Conditions sit on mods, actions,
archives, FOMOD rows and BAIN subpackages, can ask about other mods, and
`validate` rejects one naming an undeclared input or mod. MOFAM has four
inputs: `decompress_dxt1_alpha`, `dxvk_report_amd_vendor`, `enb` and
`swearing_rats`.

### B1. ~~Conditions can ask about mods~~ — done

`mod:id` is a condition term, terms combine with `&&`, `||` and `!`, and
`query` resolves mods in whatever order their conditions need, so the readme's
*"conditionally include Mod B depending on whether Mod A is included"* is true.
`validate` refuses unknown mods and conditions that loop; `query` refuses a
plan that leaves out a mod another included mod lists in `dependencies`.

### B2. ~~Conditions on archives and FOMOD selections~~ — done

`if` sits on archive rows, FOMOD selection rows and BAIN subpackages
(`{ name = "...", if = "..." }`). `query` resolves them, so the plan --
and `download`, `check` and `install` -- see only what applies.

### B5. ~~Prove it on MOFAM~~ — done for mods; B2 waits for a real choice

Swearing Rats is now the `swearing_rats` input (default on, as the guide has
it): its plugin sits at its published position, so the load order now matches
`loadorder.txt` entry for entry, and `query` takes it out of `plugins.txt` when
the answer is no. With it off the plan is identical to the one before the
input existed.

Leaving a mod out needed two things beyond `query` choosing mods: a left-out
mod's plugins leave the load order (unless an included mod ships the same
name), and a merge that takes a plugin from a left-out mod is refused with the
`if` that would fix it.

B2 is exercised by tests but not yet by MOFAM. The guide fixes every
resolution, and the obvious input -- texture resolution across sections --
needs the alternative archives, which nobody has downloaded. It belongs with
the first real choice that has them.

---

## Phase C — `export`: turn a modlist back into a guide

The other half of the project's premise: a declarative modlist should produce
the human guide, so an author maintains one artefact instead of a TOML and a
wiki page that drift apart.

### Design

- **One document is the contract.** `export` builds a versioned document --
  sections nesting sections and mods in install order, prose attached, install
  data also in words (Nexus links, installer choices, INI edits, conditions,
  the load order). `--to json` writes it as is, for anyone rendering a list
  their own way. All knowledge of how to *say* an install step lives in
  `src/export/describe.rs`, once.
- **Prose lives in `docs` tables** -- guide, section, mod -- inline or in a
  `docs_file`. Markdown, converted per target. Unknown keys pass through to
  templates: that is the extension point for content (per-section screenshots,
  videos, difficulty ratings) that no built-in exporter anticipates.
- **Exporters are template folders** (minijinja, i.e. Jinja2): an
  `exporter.toml` (output mode, entry template, options with defaults, assets)
  plus templates. Built-ins ship the same way inside the binary. Customising
  one: `{% extends %}` it and override a block, or `--eject` a copy to edit.
  Options from `--opt key=value` or the modlist's `[export.<name>]`.
- **Images resolve per target**: a site exporter copies local `path`s in; the
  Nexus exporter needs a `url` and says which images lack one.
- **No executable plugin protocol**: the JSON document already gives that power.

### Progress

1. ~~Docs schema, `docs_file`, the document, `--to json`~~ -- done.
2. ~~Template engine, exporter folders, options, `--eject`; the `nexus` (BBCode)
   exporter~~ -- done; see [exporters.md](exporters.md).
3. ~~`markdown` and `github-pages` exporters; image copying~~ -- done. The
   site has not been through a real Jekyll build yet.
4. ~~MOFAM: import the guide's prose into a docs file~~ -- done: 637 of the
   guide's 655 rows land on a mod (the rest are tools, empty mods and merges,
   kept beside the row before), every Part on its section, banners included.
   **Left:** compare the export against the published guide by eye.

Known differences an export of MOFAM shows, not bugs in `export`:

- **Order is install order, not the guide's.** A few mods sit in MO2 under a
  section other than their Part's (Part 24's armours among Part 9, two merges
  after Part 26a), so those sections appear twice.
- **Instructions appear twice**: the guide's own words, then mudcrab's reading
  of the install data. `--opt steps=false` leaves out the second.
- **The transcription lost some formatting** (stray emphasis around lists):
  a fresh conversion of the page's HTML would do better than
  `mofam-source.md`.

## Phase D — User-facing documentation

Currently: `docs/usage.md` (command reference, good),
`docs/mo2-output-structure.md`, and a readme carrying half a format reference.

Needed:

1. **Getting started** — install mudcrab, install a published modlist, in about
   a page. This is the document most readers will need and it does not exist.
2. **Authoring guide** — writing a modlist from scratch: `inspect` an archive,
   `add` a mod, layouts, actions, sections, the compile/query/download/install
   loop. Worked example, small but real.
3. **Format reference** — [modlist-format.md](modlist-format.md) covers the
   shape; it needs to stay in step with the schema, and every field wants a
   one-line "when you need this".
4. **Troubleshooting** — the failure messages a user will actually see, and what
   each one means.
5. **Doc comments are the strength here** and should stay that way: the code
   explains its own reasoning throughout. User docs should link into it rather
   than restate it.

---

## Phase E — Publishing mechanics

Done: dual MIT/Apache licence, Cargo metadata, CI (`cargo test` and
`clippy -D warnings` on push), and tagged releases with Linux and Windows
binaries. Left:

- **Contributing guide and issue templates**, including how to report a modlist
  that installs wrongly (the plan JSON and the diff output are the useful
  attachments).
- **A scope statement in the readme**: Oblivion and MO2 today; the design is not
  limited to either, but nothing else has been run.
- **Pin CI's toolchain**, or keep a matching one locally: CI's newer clippy has
  twice failed a push that passed here (`chunks_exact` with a constant size).

- **An end-to-end run by hand before a release.** Steven installs the full
  MOFAM list from scratch -- empty instance, empty cache -- driving mudcrab
  himself from the published docs, to judge whether the whole thing is elegant
  as well as correct. Everything so far has been run section by section, by the
  person who wrote the code, with the code's author watching.

---

## Phase F — Beyond a first release

Genuinely wanted, none of it blocking.

- **Composition** — `include`-ing another modlist, so small reusable lists can
  be published and pointed at. The readme has claimed this as a design property
  for a while; it has never been implemented.
- **A `custom` mod type** for arbitrary user-supplied build steps.
- **Download resume, checksum manifests, signature verification.**
- **Other games and other mod managers.** The plugin and BSA code is TES4; the
  pipeline above it is not.
- **Check referenced asset paths against the install.** A mesh or texture a
  plugin or another mod names, and that nothing provides, would have caught the
  OOO v1.7 pin (1009 files gone) outright, where the POST-GUIDE flag was read
  as harmless version drift.
- **A GUI for editing a modlist.** Something that reads the TOML and offers
  more intuitive editing -- sections, mods, archives, actions, inputs -- than a
  700-entry text file. It must preserve comments and formatting the way
  `mudcrab add` does (line-based splicing, never re-emitting parsed TOML),
  since the comments are where the list's reasoning lives.
- **A modlist test-suite command** — `mudcrab check --strict` against a fresh
  machine, so an author can find out that a row broke before a user does.
