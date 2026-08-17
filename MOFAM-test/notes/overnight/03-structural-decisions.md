# Report 3 — structural decisions about mudcrab

Decisions that changed how mudcrab works, as opposed to what the modlist says.
Each states the alternative rejected, because that is the part worth arguing
with.

---

## Conflict resolution is declared, not simulated

**Context**: several guide rows say "open the Conflicts tab and hide the files
that conflict with X". Part 9's was answered by reading 1725 paths off the
Oracle, which a real build cannot do.

**Decision**: the modlist declares *which mod wins*; mudcrab computes the file
set that implies. It does **not** model MO2's virtual file system.

**Why**: direction cannot be derived from priority order. In the Oracle's
`modlist.txt`, WAC (line 568) outranks OOO Enhanced (571), yet the guide has OOO
Enhanced winning — because loose files beat BSA contents regardless of priority.
And that same row then packs OOO Enhanced's textures into a BSA, flipping its own
tier mid-step. Simulating this means modelling a two-tier rule over a modlist
that mutates its own packing.

**Validated**: intersecting OOO Enhanced's files with WAC's BSA yields exactly
the 577 the Oracle removed; the three clothing mods account for 1148 more; the
residue is 13 `thumbs.db`. All 1738 accounted for.

Design in `conflict-resolution-design.md`. **Not yet built** — see report 4.

## Layout should be a path planner shared with install

**Decision**: to answer "which files would mod X contribute" without installing
X, refactor layout into a function that maps a list of archive paths to a
selection plus an old→new mapping, used by both `install` and the file index.

**Why**: the obvious implementation — extract to a temp dir and walk it — is
wasteful on multi-gigabyte archives and, worse, invites a second implementation
of layout that drifts from the real one. Both problems have the same fix.

**Feasible because** layout decisions are structural: every handler uses
`read_dir` only; the sole place reading file *content* is `fomod.rs:209`
(`ModuleConfig.xml`), one small file that can be extracted alone.

## `ini_set` scopes to a section, and refuses ambiguity

**Decision**: `section = "<name>"` scopes an edit; without one, a key matching
more than one line is a hard error naming the sections found.

**Why**: `apply_ini_set` matched a key anywhere and rewrote **every** match.
Part 14's `Fog.ini` has `Amount` under both `[World]` and `[Interior]`; setting
interior fog would silently have changed the weather. Erroring preserves every
edit that currently works while making the ambiguous case impossible to get
wrong quietly.

## `file_prune` uses staged-tree glob semantics, extraction does not

**Decision**: `/` is a real separator and matching is case-insensitive for
`file_prune`. Archive `include`/`exclude` filters keep archive-entry semantics.

**Why**: both differences bit in Part 11 — `NoMushroomStalks` matched nothing
(staged directories are folded to lowercase), and `textures/rocks/*.dds` matched
*through* the separator and deleted the `underwater` folder the guide protects,
silently. Extraction filters were left alone because a hundred-odd authored
patterns depend on current behaviour and cannot be re-verified mid-build.

**This asymmetry is a wart.** It is the right call for now and the wrong shape
long-term; noted in report 4.

## `layout = "simple"` overrides detection instead of agreeing with it

Declared in the schema since the beginning and never honoured — it fell through
to auto-detection, so declaring the answer still got you the guess. Fatal for
Part 10's WAC, whose archive detection rejects outright.

## `manual:` for archives nothing can fetch

**Decision**: a `manual:` descriptor meaning "no automated source exists".
Resolves from `--archive-search-path`; fails naming the file and the directories
searched, without retrying.

**Why**: an absolute local path would work but bakes one machine into a modlist
that is otherwise host-agnostic.

## Age is only inferred from evidence that is about the file

`nexusLastModified` is ignored for `modid=0` mods. MO2 writes it regardless, and
for a mod that never came from Nexus it records when the entry was written —
dating Part 10's ~2010 WAC beta to 2026 and flagging it POST-GUIDE. Filename
timestamps still count, whoever hosted the file.

## LOOT is out of the build

`post-install-actions` no longer runs `loot-sort`. It opens a GUI needing human
approval, so an unattended run stalls until the timeout — Part 9 lost 22 minutes
to this. `plugins.txt` is written from the plan's own `plugins` array, and
Part 37 replaces it with a fixed `loadorder.txt` regardless.

## `ini_set` edits a value, it does not restyle the line

Three refinements, all forced by ORC's `Fog.ini` in Part 14 and all in the same
direction — **change what you were asked to change and nothing else**:

- **Alignment preserved.** The file pads keys into a column
  (`Amount        =0.0`). Replacing a value keeps the line's left-hand side when
  the file is written that way.
- **Padding is not spacing.** `dominant_spacing` had counted left-hand padding
  as "this file puts spaces around `=`". The space *after* the `=` is the half
  Oblivion reads literally — the DarNified font bug — and padding before it is
  cosmetic. Measured separately now.
- **Line endings kept.** We rewrote CRLF files as LF, changing every line of a
  file we were asked to change one value in. Now CRLF if the file has any CRLF,
  LF otherwise. That normalises a mixed-ending file rather than preserving it;
  none has turned up, and uniform is the better answer if one does.

## Plugins an archive ships but the list never declares

Part 14's ORC180 ships `ORC.esp`, which this build deliberately does not load.
Leaving it out of the `plugins` array does not remove it from the mod folder, so
it sat loose and visible in an enabled mod — where MO2 would offer it as a new
undecided plugin. The Oracle hides it; a `file_hide` now does too.

**The general case is pinned.** mudcrab already *notices* these (the load-order
step warns about "discovered plugins not listed in top-level plugins") and does
nothing about them. Auto-hiding is probably right and is a behaviour change.


## `--from-oracle` scaffolds provenance; it does not establish it

Three separate failures now trace to treating the Oracle's `meta.ini` as
authoritative about *what a mod is made of*:

1. **Part 9**: `--from-oracle` gave OOO Enhanced 5.33 against 5.3b resources —
   a mismatched pair neither the guide nor anyone else ever ran.
2. **Part 13/15/16**: `modid` values of 7, 1 and 1 for tesall.ru, ModDB and
   MediaFire downloads. MO2 writes a mod id regardless of where the file came
   from.
3. **Part 16**: `installationFile` records the **last** archive installed into a
   folder, not all of them. T4UTXL's City Gates is built from two archives and
   the Oracle names one, which is why building from it produced 5 files against
   the Oracle's 11.

The rule that has held: **`meta.ini` is a starting point for a lookup, and the
archive is the authority.** Every BAIN/FOMOD selection in this run has been
checked against `mudcrab inspect` rather than trusted from `meta.ini`, and that
is why the section diffs have been clean.

## Guide instructions phrased as keep-lists are written as keep-lists

"Delete everything except X" becomes `include = [X]`, not a `file_prune`
enumerating everything else. Three occurrences in Part 16 and one in Part 15.

Besides being the direct translation, it cannot go stale: a prune listing what
to remove silently under-deletes if the archive ever gains a file, while a
keep-list stays correct.
