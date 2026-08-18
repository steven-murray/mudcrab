# The layout planner

The refactor behind D1 (derive conflict file lists) and, as a dividend, behind
extracting less.

## What layout does today

Every handler works the same way:

1. `with_staged_archive` extracts the **whole archive** into a scratch dir.
2. The handler inspects that tree with `read_dir` to decide what goes where.
3. `copy_filtered_tree_folded` copies the chosen subtree into the mod folder.
4. The scratch dir is deleted.

Two consequences:

- **We unpack everything to find out we wanted a fraction.** T4UTXL BETA1 is
  8.8 GB unpacked; the Priory row keeps 48 files, 93 MB. That archive is
  installed twice.
- **"Which files would mod X contribute?" cannot be answered without doing the
  install**, which is what blocks deriving conflict lists.

## The change

Split the decision from the doing:

```
plan_layout(paths: &[String], archive, mod_id) -> LayoutPlan
```

`paths` is the archive's entry list — obtainable without extracting. The plan is
a set of `(archive_path -> destination_path)` entries plus the set of archive
paths actually needed.

Then:

- **install** extracts (only what the plan names, where the format allows) and
  applies the mapping.
- **the file index** keeps the destination paths and throws the rest away.

One implementation, so the index cannot drift from what install really does.
That drift is the whole reason not to write a separate predictor.

## Why this is possible

Layout decisions are already structural. Checked against the code:

| handler | what it needs | from a listing? |
|---|---|---|
| `bain` | top-level directory names | yes |
| `auto` | plugin locations, top-level shape, presence of `Data/` | yes |
| `build` | layer `dest_prefix`, per-layer filters | yes |
| `fomod` | `fomod/ModuleConfig.xml` | one small file, extracted alone |

`fomod.rs:209` is the only place in any handler that reads file *content*.

## What it does not fix

**Staging does not disappear.** 142 of this modlist's archives are 7z and 32 are
rar — 83% go through `bsdtar`/`7z`, which extract to a directory and cannot
rebase paths on the way out. Those still need a scratch dir to rebase from.

What changes is its *size*: both tools accept include patterns, so the scratch
dir holds what the plan keeps rather than the whole archive. Verified:

```
7z l "T4UTXL - Architecture_BETA1...7z"                          3854 files, 8.8 GB
7z l "T4UTXL ...7z" "textures/architecture/priory/*"               48 files,  93 MB
```

The 36 zips use mudcrab's native extractor, which already writes entry by entry
and can go straight to the target with no scratch dir at all.

And for a mod with no filters that wants the whole archive, staging stays
archive-sized — which is why staging was moved off tmpfs separately, and why
that was the more urgent fix.

## Order of work

1. **`LayoutPlan` + `plan_layout`**, one handler at a time, install switched over
   as each lands. Tests green throughout.
2. **Selective extraction** — pass the plan's wanted paths to the extractor.
3. **File index** — `plan_layout` over an archive listing, cached by the mod's
   existing `definition_hash`; installed mods record their real staged paths.
4. **`conflicts_with`** on `file_prune`/`file_hide`, validated against the 1725
   recorded OOO Enhanced paths.
5. **Lockfile** resolving selectors to explicit paths.
