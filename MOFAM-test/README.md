# MOFAM Real-World Test Workspace

Goal: drive mudcrab feature development toward automated install support for MOFAM on Nexus:
https://www.nexusmods.com/oblivion/mods/52949

This folder tracks a practical translation pipeline from human instructions to machine-installable TOML.

## Current Status

- Source capture is complete: [input/mofam-source.md](input/mofam-source.md) holds the
  full manually-transcribed page (~158 KB, 40 Parts), since automated web extraction of
  the Nexus description page was blocked by anti-bot/ad redirects in the fetch tool.
- Translation to TOML is in progress: Parts 1, 2, 3, 4, and 6 are fully translated in
  [input/mofam.full.toml](input/mofam.full.toml) and have been installed end-to-end
  (see [output/mo2-instance](output/mo2-instance) and the install logs under `output/`).
  The remaining Parts are not yet translated.

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
