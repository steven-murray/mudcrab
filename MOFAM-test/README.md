# MOFAM Real-World Test Workspace

Goal: drive mudcrab feature development toward automated install support for MOFAM on Nexus:
https://www.nexusmods.com/oblivion/mods/52949

This folder tracks a practical translation pipeline from human instructions to machine-installable TOML.

## Current Status

- Web extraction of the Nexus description page is currently blocked by anti-bot/ad redirects in the fetch tool.
- Workspace scaffolding is ready for structured transcription and iterative implementation.

## Workflow

1. Put captured source instructions in [input/mofam-source.md](input/mofam-source.md).
2. Condense and normalize into [notes/mofam-condensed.md](notes/mofam-condensed.md).
3. Translate into full TOML draft in [input/mofam.full.toml](input/mofam.full.toml).
4. Keep a fast-path test TOML in [input/mofam.minimal.toml](input/mofam.minimal.toml) with only behaviorally interesting entries.
5. Iterate mudcrab against minimal TOML first, then promote patterns to full TOML.

## Quick Run (Minimal)

```bash
./MOFAM-test/scripts/run-minimal.sh
```

Artifacts are written under [MOFAM-test/output](output).
