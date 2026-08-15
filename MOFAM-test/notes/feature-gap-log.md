# MOFAM Feature Gap Log

Track required mudcrab capabilities discovered while translating MOFAM.

## Open Gaps

| ID | Category | Requirement | Evidence (source section) | Minimal TOML entry | Status |
|---|---|---|---|---|---|
| GAP-001 | Archive Formats | Support `.7z` extraction | pending source import | pending | open |
| GAP-002 | Archive Formats | Support `.rar` extraction | pending source import | pending | open |
| GAP-003 | Layout | Rich FOMOD choice mapping | pending source import | pending | open |
| GAP-004 | Actions | User-guided custom procedures | pending source import | pending | open |

## Triage Rules

1. If a gap blocks minimal TOML execution, mark as `critical`.
2. If a gap affects only full TOML parity, mark as `deferred`.
3. Each closed gap should include the implementing commit or file references.
