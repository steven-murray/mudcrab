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

Note: Part 5 (LOD) was skipped, not just deferred — it sits *behind* the
current frontier and is the next section to be built (needs BSA packing and
dummy plugin creation, GAP-005 / GAP-006).

Deliverable:
- [input/mofam.full.toml](input/mofam.full.toml)

Translation rule:
- Every mod row in condensed markdown must map to at least one TOML block.

## Mod ids vs Nexus mod pages

A mod id names **one installed thing**. A Nexus mod *page* often is not one
thing, so the two do not reliably correspond and the id cannot be derived
mechanically from the page name.

Three cases, all of which occur in MOFAM:

1. **One page, one archive** -- the ordinary case. Id = the mod name.
2. **One page, several archives that are separate mods.** Each gets its own
   `[[mods]]` entry with its own id. Example: page 50770 is called *"Market
   District Landscape Fix and Imperial City Landscape Fix"* and offers both as
   separate main files, but the guide says "only the 1st main file", so the
   list contains **only** `Imperial City Landscape Fix`. Had it wanted both,
   they would be two mods, not one.
3. **One page, several archives combined into one mod.** Several
   `[[mods.archives]]` under a single id -- the same shape used for the
   combine/repack rows in Part 25.

Consequences when authoring:

- `add --from-oracle` defaults the id to the Oracle's **folder** name, which
  MO2 takes from the page. That is right for case 1 and wrong for cases 2 and
  3; override with `--id` and record the real folder name via `oracle_name`
  so `diff` still matches.
- The `fileid` is the thing that identifies what is actually installed. When
  a name looks ambiguous, check `[installedFiles] 1\fileid` against ours
  rather than trusting either name. That check is what caught a bad mapping
  between `Diverse Effect Icons` (modid 10254) and `Diverse Effect Icons
  OBSE` (modid 49511), which share a name prefix and nothing else.

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
