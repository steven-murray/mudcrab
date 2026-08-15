# MOFAM Real-World Iteration Plan

Target: eventually automate install flow for MOFAM (Nexus Oblivion mod 52949) using mudcrab.

## Step 1: Condense Source HTML -> Markdown

Status: complete. `input/mofam-source.md` holds the full manual capture (~158 KB,
40 Parts). `notes/mofam-condensed.md` holds the structured condensation.

Deliverables:
- Raw capture in [input/mofam-source.md](input/mofam-source.md)
- Structured condensation in [notes/mofam-condensed.md](notes/mofam-condensed.md)

## Step 2: Translate Condensed Markdown -> Full Source TOML

Status: in progress. Parts 1, 2, 3, 4, and 6 are fully translated in
`input/mofam.full.toml` and have been installed end-to-end (see
`output/mo2-instance` and `output/vkvii-install.log`). Remaining Parts (5, 7-40)
are not yet translated.

Deliverable:
- [input/mofam.full.toml](input/mofam.full.toml)

Translation rule:
- Every mod row in condensed markdown must map to at least one TOML block.

## Step 3: Maintain Minimal High-Signal TOML

Status: in progress. [input/mofam.minimal.toml](input/mofam.minimal.toml) and
[input/mofam.minimal-qac.toml](input/mofam.minimal-qac.toml) exist and have been
run through the pipeline; new behaviorally-interesting entries are still being
added as translation of the full TOML continues.

Deliverable:
- [input/mofam.minimal.toml](input/mofam.minimal.toml)

Scope:
- Include only entries that demonstrate new behavior (conditionals, custom layouts/actions, unsupported archive formats).

## Step 4: Setup Processing Configuration

Status: complete.

Configured:
- Environment template: [MOFAM-test/.env.example](.env.example)
- Run script: [MOFAM-test/scripts/run-minimal.sh](scripts/run-minimal.sh)
- Output workspace: [MOFAM-test/output](output)

## Step 5: Iterate Features Until Minimal TOML Installs

Status: started.

Tracking:
- Gap log: [notes/feature-gap-log.md](notes/feature-gap-log.md)

Loop:
1. Run minimal pipeline.
2. Capture failing behavior.
3. Implement missing feature in mudcrab.
4. Re-run until green.
5. Promote behavior to full TOML translation.
