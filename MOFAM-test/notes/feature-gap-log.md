# MOFAM Feature Gap Log

Track required mudcrab capabilities discovered while translating MOFAM.

## Open Gaps

| ID | Category | Requirement | Evidence (source section) | Status |
|---|---|---|---|---|
| GAP-004 | Actions | User-guided custom procedures | Parts with `[MI]` manual-install steps | open |
| GAP-005 | Actions | BSA packing (`action.pack_archive_bsa`) | Part 11 baseline textures, `[DP]` rows | open |
| GAP-006 | Actions | Dummy plugin creation (`action.create_dummy_plugin`) | `[DP]` rows | open |
| GAP-007 | Actions | File prune / hide (`action.file_prune`) | Part 4 "delete X after install" notes | open |
| GAP-008 | Merges | Headless zMerge replacement | Part 36 (6 merges) | in progress — see `merge-recon.md` |

## Closed

| ID | Category | Requirement | Closed by |
|---|---|---|---|
| GAP-001 | Archive Formats | Support `.7z` extraction | `src/archive/mod.rs` — external `7z` binary; verified by `MOFAM-test/output/vkvii-install.log` |
| GAP-002 | Archive Formats | Support `.rar` extraction | `src/archive/mod.rs` — `bsdtar` then `7z` fallback |
| GAP-003 | Layout | Rich FOMOD choice mapping | `src/config/install.rs` — `ModuleConfig.xml` engine with flag/file dependencies and all group types; `fomod_selections` in `schema.rs` |

## Triage Rules

1. If a gap blocks minimal TOML execution, mark as `critical`.
2. If a gap affects only full TOML parity, mark as `deferred`.
3. Each closed gap should cite the implementing commit or file references.
