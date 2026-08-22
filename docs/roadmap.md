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

### A1. Key the download cache by source, not by mod id

`cache_file_name` is `{mod_id}_{archive_index}_{fileid}`. Renaming a mod orphans
its archive; two mods sourcing one Nexus file cache it twice.

The key should be a function of the source. The reason it has not changed is
that re-keying orphans an existing cache and forces a re-download, so this wants
a migration path — read the old name, fall back to it, write the new one — not a
flag day.

### A2. Make manual archives resolvable when the host renames the file

Today a `manual:` source matches its `file_name` exactly, and a re-upload under
a new name reads to the user as "go download it by hand" when they already have
it. Three options, none obviously right:

- match on content hash, with the filename as a hint (robust, but the hash is
  only knowable after someone has downloaded it once, so it does not help a
  first-time user);
- allow `file_name` to be a set of alternates or a glob;
- keep exact matching and improve the error: if exactly one archive in the
  search paths is unclaimed by any other mod, say so.

The third is cheap and helps immediately; it does not preclude the others.

### A3. One implementation of "where is the content root"

`install` uses the layout planner. `inspect` has a separate shallowest-root
search. Porting `inspect` onto the planner is the fix — the planner exists so
that one piece of code answers this.

### A4. Tolerate out-of-range mod indices in merge sources

`merge::rewrite` errors on a reference whose mod index exceeds the plugin's
master list. Every real reader treats that as "my own record", and zMerge emits
them routinely, so mudcrab cannot currently merge a zMerge output. Clamp with a
warning rather than refuse.

### A5. Honour `--parallel`, or remove it

It is accepted and ignored. Either is fine; claiming a flag that does nothing is
not.

---

## Phase B — Optional mods, and inputs that earn their keep

**The machinery half-exists.** `[inputs]` (`bool` / `choice` / `text`) and
per-mod `if = "<expr>"` are in the schema, and `query` evaluates conditions and
drops excluded mods from the plan. But the MOFAM list's `[inputs]` table is
**empty** and not one of its 700 rows is conditional, so none of it has been
exercised at scale — and the gaps show on inspection:

### B1. Conditions can only ask about inputs

`evaluate_condition` reads the input responses and nothing else. The readme
promises more than that — *"conditionally include Mod B depending on whether Mod
A is included"* — which needs conditions to see the resolved mod set. That is
the whole reason the format is declarative and compiled rather than executed in
order, so it should be true.

### B2. Conditions apply to whole mods only

An optional mod is the easy case. The harder and more common one is a mod that
is always installed but whose **FOMOD selection, action, or archive** depends on
an answer — "1K or 2K textures?" is one input that should reach dozens of rows.
`if` needs to be available on archives and actions, not just `[[mods]]`.

### B3. Headless runs need declared defaults

`query --headless` currently defaults a `bool` to false and a `choice` to its
first entry. That is a silent policy, not a decision by the modlist author.
Inputs should carry an explicit `default`, and `query` should take
`--set key=value` so a scripted or CI run can pin answers without a TTY.

### B4. Validate conditions

A condition naming an input that does not exist should be a compile error. Today
it evaluates to false and silently drops the mod.

### B5. Prove it on MOFAM

Swearing Rats is the natural first case: the guide itself says it may be
omitted, and it is the one plugin our load order deliberately differs from the
published `loadorder.txt` by. Turn that difference into an input with a default,
and the list becomes self-describing where it is currently a footnote. A
texture-resolution input across several sections would exercise B2.

---

## Phase C — `export`: turn a modlist back into a guide

`mudcrab export --format markdown|html|json` is a **command stub** that errors.
Implementing it is the other half of the project's premise: a declarative
modlist should be able to produce the human guide, so an author maintains one
artefact instead of a TOML and a wiki page that drift apart.

What the compiled modlist already carries: sections and their order, mod names,
Nexus mod and file ids (so page links are derivable), archive layouts, FOMOD
selections, actions, merges, the load order.

What a readable guide needs that the schema does **not** carry yet:

- **Prose.** A per-mod and per-section `description`/`note` field. Today the
  reasoning lives in TOML comments, which the compiler discards. This is the
  single biggest missing piece, and it is also the project's stated advantage
  over Wabbajack — *the author's intent is not made clear* is the problem
  statement in the readme. Comments cannot carry intent into an artefact.
- **Attribution**: author, mod page URL, licence or permissions where a list
  redistributes anything.
- **Ordering and headings** good enough to read top to bottom, which the
  section paths mostly give already.

Suggested shape: a `[docs]` block per mod (`summary`, `why`, `manual_steps`),
plus a template so a publisher can theme the HTML. `json` should be the same
data, for anyone building their own renderer.

A worthwhile acceptance test: export the MOFAM list and compare it, by eye,
against the guide it was transcribed from.

---

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

Nothing here is hard; all of it is missing.

- **Licence.** There is no licence file. Nothing else on this list matters until
  there is one.
- **Cargo metadata.** `description`, `license`, `repository`, `keywords`,
  `categories`, `rust-version`. Currently just name/version/edition.
- **CI.** `cargo test` and `cargo clippy --all-targets -- -D warnings` on push.
  Both pass today by convention, which is not the same as by rule.
- **Releases.** Tagged versions and prebuilt binaries for Linux and Windows —
  the audience is modders, and `cargo install` is not a reasonable ask of them.
- **Contributing guide and issue templates**, including how to report a modlist
  that installs wrongly (the plan JSON and the diff output are the useful
  attachments).
- **A scope statement in the readme**: Oblivion and MO2 today; the design is not
  limited to either, but nothing else has been run.

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
- **A modlist test-suite command** — `mudcrab check --strict` against a fresh
  machine, so an author can find out that a row broke before a user does.
