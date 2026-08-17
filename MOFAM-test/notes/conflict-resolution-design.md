# Deriving conflict file lists without the Oracle

**Status: designed, not built.** First needed at Part 18; load-bearing at Part 24.

Part 9's OOO Enhanced row ends with "hide the files that conflict with WAC and
Colorful Clothing, delete them, then pack the rest". We answered it by reading
the 1024 paths off the Oracle
(`ooo-enhanced-conflict-hidden-files.txt`). That is fine as a fixture and no
good as a mechanism: a real mudcrab build has no Oracle to read from, and must
derive the list from the modlist itself.

It can, because the modlist is declarative — every upcoming mod and its archive
are known before a single mod is installed. What is missing is the ability to
ask *"which files would mod X contribute?"* without installing X.

## Direction is not derivable from priority order

The obvious approach — compute who wins from MO2 priority — is wrong, and the
Oracle proves it. In `profiles/MOFAM 03.25/modlist.txt` (top = highest
priority):

```
568:+WAC Waalx Animals & Creatures
571:+OOO Enhanced
```

WAC **outranks** OOO Enhanced, so by priority WAC wins. But the guide files WAC
under *"Winning File conflicts → Overwritten mods"* — OOO Enhanced is winning.
The explanation is MO2's second tier: **loose files beat BSA contents
regardless of mod priority**, and WAC ships its assets packed. That is the same
fact that makes ["Enable Parsing of Archives"](part-09-overhauls-oscuro.md)
load-bearing.

It gets worse inside that one row: its last step packs OOO Enhanced's textures
*into a BSA*, flipping OOO Enhanced's own tier and changing the answer for
every comparison after it.

So simulating MO2's VFS means modelling a two-tier rule over a modlist that
mutates its own packing mid-build. **Don't.** The guide is making an authorial
decision ("these mods win over this one"); express it directly and let mudcrab
compute only the file set it implies.

Note the two directions both occur:

| guide wording | meaning | rows |
|---|---|---|
| "Losing file conflicts → Providing Mod" | *my* files lose to X; drop mine | Part 9 #11 (Colorful Clothing) |
| "Winning File conflicts → Overwritten mods" | *my* files beat X; yield to X | Part 9 #11 (WAC), Parts 18 #7, 21 #3, 22 #2 |

## Three pieces

### 1. A staged file index — the new primitive

"What relative paths would mod X stage, including inside any BSA it ships."

The obvious implementation is to extract into a temp dir and walk it. That
works, but it is wasteful — some of these archives are gigabytes — and, worse,
it invites a *second* implementation of layout that drifts from the real one.

**Both problems have the same fix: make layout a path planner that install and
the index share.** Given a list of relative paths in an archive, produce the
selection predicate and the old→new mapping. Install applies the mapping while
copying; the index just keeps it. One implementation, no drift, no extraction.

This is viable because layout decisions are already structural. Checked against
the code: every layout handler reaches for `read_dir` only — the sole place
that reads file *content* is `fomod.rs:209` (`ModuleConfig.xml`).

| input | how to get it without extracting |
|---|---|
| archive paths | `archive::list_archive_paths` (`src/archive/mod.rs:269`) — already exists, used by `inspect` |
| BSA contents | the native BSA reader lists the file table without decompressing payloads |
| auto-layout heuristics | structural; operate on the path list |
| BAIN subpackages, `build` layers, `include`/`exclude` | path-level already |
| FOMOD | needs one small XML — extract that single entry, not the archive |

Cache the result under the mod's existing `definition_hash`, so it recomputes
only when the definition or archive changes. Installed mods can record their
real staged paths for free at install time, which means the index is only ever
computed for mods not yet installed — in section-by-section building, exactly
the upcoming ones we need.

Worth noting: `InstalledMod` currently records `extracted_files` as a *count*.
Recording the paths makes `diff` cheaper too.

Where the planner genuinely cannot answer (an installer flavour we don't
model), fall back to real staging — but say so loudly rather than silently
returning an incomplete set. A quiet under-count here looks exactly like "no
conflicts", which is the failure mode that cost us the first pass at Part 9.

### 2. A selector on the actions we already have

Not a new action — `file_prune` and `file_hide` gain a conflict selector:

```toml
[[mods.actions]]
action = "file_prune"
conflicts_with = ["wac-waalx-animals-and-creatures", "colorful-clothing-collection"]
under = "textures/"
```

Comparison is case-insensitive on lowercased paths throughout, matching
`diff.rs`'s `comparison_key` — BSAs store names lowercased and we fold
directories on Linux anyway.

Naming **mudcrab mod ids** rather than the guide's prose is itself the point:
`validate` catches a typo at compile time. The guide's names in this one row
were wrong three separate ways (`Colourful` vs `Colorful`, one guide name
covering two mods, a `- Seamless OCOv2` suffix no mod has) and every one of
those cost a rebuild.

Two conditions must be hard errors, not warnings:

- a named mod that does not exist in the modlist at all — a typo
- a selector that resolves to **zero** files — same rule as `file_prune`'s
  bare-directory fix from Part 5, and the exact shape of the first failed pass
  at Part 9

A named mod outside the section range currently being built is *not* an error;
it is the normal case for a partial build, and should skip with a clear note.

### 3. Resolve to explicit paths

Worth doing, per Steven: a 1024-line list is reviewable in a way a selector is
not, and freezing it makes the build reproducible.

The wrinkle: `compile` is today a pure TOML→JSON transform with no archive
access (`src/config/compiler.rs`), and this needs the cache. Making `compile`
archive-dependent changes what the command means. Better to add a distinct
resolve phase that writes a lockfile — `download` and `check` already establish
"needs the cache" as its own phase.

## Validation is unusually cheap here

We already have the answer. Implement, declare WAC and Colorful Clothing on the
OOO Enhanced row, and check the computed set against the 1024 paths in
`ooo-enhanced-conflict-hidden-files.txt`. A new feature with a known-correct
expected output is rare — normally this would be a guess.

The Oracle-derived list stays as the test fixture. It just stops being the
mechanism.

## Timing

Author the OOO Enhanced row in its final `conflicts_with` form now, so it is
not carrying a shape we intend to throw away, and leave it inert until both
partners exist.

| when | why |
|---|---|
| Part 10 | WAC arrives — first partner exists |
| Parts 18 #7, 21 #3, 22 #2 | three small "hide the winning files" rows, 2-5 files each — cheap first real call sites |
| Part 24 | Colorful Clothing arrives; the OOO Enhanced prune + repack can finally run |
