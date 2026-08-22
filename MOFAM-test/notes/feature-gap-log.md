# MOFAM Feature Gap Log

Track required mudcrab capabilities discovered while translating MOFAM.

## Open Gaps

| ID | Category | Requirement | Evidence (source section) | Status |
|---|---|---|---|---|
| GAP-004a | Actions | `extract_bsa` (BAE) | Parts with `[MI]` manual-install steps | open |
| GAP-004b | Actions | `rename` after extraction | Parts with `[MI]` manual-install steps | open |
| GAP-004c | Actions | Run a self-extracting `.exe` inside a staged mod | Parts with `[MI]` manual-install steps | open |
| GAP-004d | Actions | Cross-mod conflict-driven file hiding | Parts with `[MI]` manual-install steps | open |
| GAP-005 | Actions | BSA packing (`action.pack_archive_bsa`) | Part 11 baseline textures, `[DP]` rows; first required by Part 5 (LOD) | open |
| GAP-006 | Actions | Dummy plugin creation (`action.create_dummy_plugin`) | `[DP]` rows; first required by Part 5 (LOD) | open |
| GAP-007 | Actions | Post-install ordered file prune | Part 4 "delete X after install" notes | open — narrowed, see note below |
| GAP-009 | Actions | Section-aware `ini_set` | `apply_ini_set` in `src/config/actions/ini_set.rs` matches keys anywhere in the file and appends missing keys at EOF; a key destined for `[Grass]` or `[LIMITER]` lands in the wrong section | open — live correctness bug |
| GAP-010 | Actions | INI scope `game-root` | INI edits that must target the game install root rather than `mod`/`game` (MO2 profile) scope | open |
| GAP-011 | Actions | `ini_append_block` | Multi-line raw block append (not a single key/value `ini_set`) | **done** — `src/config/actions/ini_append_block.rs`, Part 30 row 7 byte-identical |
| GAP-012 | Actions | Move plugin to `optional/` | MO2 optional-plugins convention | open |
| GAP-013 | Layout | MO2 "Ignore Missing Data" flag | Mods that reference files MO2 would otherwise flag as missing | open |
| GAP-014 | Actions | Explicit `loadorder.txt` post-install action | Direct load-order file write, distinct from LOOT sorting | open |
| GAP-015 | Actions | xEdit scripted record deletion | Cases needing an xEdit script to delete records, not just merge/rewrite them | open |

### GAP-007 note

`exclude` glob support on archives (see Closed) covers *extract-time* pruning.
Several MOFAM steps need deletion *after* another action has run — e.g.
delete an archive after BAE has extracted it, delete loose folders after
BSArch has packed them. `exclude` cannot express this ordering, so the gap
stays open, narrowed to "post-install ordered file_prune".

## Closed

| ID | Category | Requirement | Closed by |
|---|---|---|---|
| GAP-001 | Archive Formats | Support `.7z` extraction | `src/archive/mod.rs` — external `7z` binary; verified by `MOFAM-test/output/vkvii-install.log` |
| GAP-002 | Archive Formats | Support `.rar` extraction | `src/archive/mod.rs` — `bsdtar` then `7z` fallback |
| GAP-003 | Layout | Rich FOMOD choice mapping | `src/config/install.rs` — `ModuleConfig.xml` engine with flag/file dependencies and all group types; `fomod_selections` in `schema.rs` |
| GAP-008 | Merges | Headless zMerge replacement | Native merge engine in `src/merge/`, surfaced as `type = "merge"` in the modlist TOML; all six MOFAM merges (Part 36) reproduced and verified against zEdit's output — see `MOFAM-test/notes/merge-verification.md` and `tests/merge_oracle.rs` |

## Triage Rules

1. If a gap blocks minimal TOML execution, mark as `critical`.
2. If a gap affects only full TOML parity, mark as `deferred`.
3. Each closed gap should cite the implementing commit or file references.
