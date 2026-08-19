# 3. Structural decisions in the mudcrab code

Everything here changed behaviour beyond one modlist row. Each was driven by a
real failure in a real section, not by tidiness.

## `diff` learned to date archives properly

**The problem.** Mods were being reported as "newer than the March 2025 guide"
on the strength of `nexusLastModified` — which for many mods is *the day you
downloaded it*, not the day it was published. Eleven false alarms across the
list, including three 2018 files stamped 2026-01-26.

**The fix, in two steps.** Part 19 caught the legacy-file-id version of this.
Part 21 forced the stronger observation: **Nexus allocates file ids in ascending
order, so an id doubles as an upload date.** Calibrated against every archive in
the list carrying both a file id and a filename timestamp — 406, of which 405
were usable — and the two orderings agree exactly. The boundary is clean:

| | file id | date |
|---|---|---|
| largest pre-guide | 1_000_040_927 | 2025-02-23 |
| smallest post-guide | 1_000_040_999 | 2025-03-01 |

**Why it matters to you specifically:** the constant is baked into the binary, so
someone with no reference instance gets the same answers. That is a real
reduction in Oracle-dependence, not a cosmetic one.

The 406th archive is Part 20 row 8, whose two versions' MO2 sidecars claim the
same file id — already flagged as unresolved, and excluded rather than allowed to
widen the boundary.

**What it removed:** `nexusLastModified` is no longer consulted at all. It was
only reachable when a mod had a real Nexus file id, which is the condition under
which the id now answers first, so the branch became unreachable. It and its date
parser are deleted rather than left as dead code.

## Layout detection descends through nested wrappers

Part 20's Knights of the Nine patch wraps its content **twice** —
`<archive name> V2/<patch name>/Meshes/…`. Detection unwrapped one level and
stopped, so two meshes installed into a folder nothing would ever read. Nothing
failed. Every file was present, in the wrong place.

`inspect` had it right the whole time. Two implementations of "where is the
content root", disagreeing, with the one that does the installing being wrong.

Fixed by descending, and — after the review — **ambiguity is now an error at
every depth rather than only at the root**. That second half matters as much:
bailing at the top and shrugging one level down left the same silent failure in
place. Two rival content folders under a wrapper used to return "no wrapper",
after which detection installed *both alternatives at once* at the archive root.

## `ini_set` stopped vandalising the files it edits

Fourth round of the same lesson, and the worst instance. `set X to Y` lines were
re-rendered from scratch, because `replace_value_in_place` returned `None` for
anything that was not the standard format. `Dynamic Oblivion Combat.ini` is
tab-aligned into columns and annotates every line with its default; four edits
cost it 82 bytes of the author's formatting.

**This one was retroactive.** Fixing it repaired three mods in earlier sections —
`Extended UI`, `Follower Status`, `Migck's Miscellaneous fixes` — which the
backlog had filed as *"probably a guide edit not yet applied"*. They were the
opposite: an edit mudcrab **had** been applying, destructively, to every
`set X to Y` file it touched.

Also: a file with no final newline no longer gains one, and `ini_set` now reports
what it did — it was the only action that logged nothing on success, which made
its most surprising branch (inventing a `[section]` the file lacks) its quietest.

## Archive listing became directory-aware

Three bugs, one shape: a directory reported as a file. Invisible while installs
walked an extracted tree; fatal the moment they stopped.

- `bsdtar -tf` marks directories only with a trailing slash, which plenty of
  archives do not write.
- 7z's directory flag is a letter *among* letters — `OOO Enhanced - Resources`
  writes `RD` — and a `starts_with("D")` test read every folder as a file.
- 7z's header block carries a `Path =` of its own, so collecting from the top
  made the archive its own first entry.

A structural check now runs over every listing regardless of tool: a path that
other paths sit inside cannot be a file.

## `is_documentation` was generalised, then corrected

A `.gif` at a mod root was being reported as a difference. Generalising from the
existing `obmm_bsa_settings.jpg` special case seemed obvious: Oblivion reads
`.dds` and nothing else, so any other raster is a screenshot.

**The review falsified that with mods already in your Oracle.** ORC's
`textures/Effects/bluenoise.png` is a shader resource its DLL loads; Pek's COBL
book jackets are sixteen `.png` textures that constitute the whole mod;
`Dagger_Data` puts `.bmp` skies under `textures/dag/sky/`. An extension-only rule
would have dropped all three from the report in silence.

Now gated on *where* the file sits: outside the game's content folders it is a
screenshot, inside one it is an asset.

Writing the test also found a flaw predating all of this: the `readme`/`credits`/
`license` name markers were matched against *any* file, so
`textures/menus/license.dds` counted as documentation. Live since the rule was
written.

## `mudcrab conflicts` — new command

Prints the files two mods both provide, from the same code `conflicts_with` acts
on. A row that hides hundreds of files is not a thing to run blind.

After the review it also says when matches were excluded because they are
*already hidden* — asking about a row whose hide has run otherwise answers "0",
which reads as "the row was wrong" when it means "the row worked".

## What the reviews caught

Five reviews, five real findings. In order of severity:

1. **The raster rule was falsified** by three mods already in your Oracle.
2. **`names_the_same_archive` was too permissive** — it treated `Base_Metal.7z`
   as the same archive as `Metal.7z`, and archive names use underscores as word
   separators constantly.
3. **The legacy-file-id rule was not gated on provenance**, so a stray number in
   a `meta.ini` could have become a confident answer.
4. **A 35-line doc comment had attached itself to the wrong item**, leaving a
   public function undocumented.
5. **`mudcrab conflicts` answered misleadingly** on an already-applied row.

Plus a confidently-wrong count in three of the five. That is the number I would
watch: my arithmetic in prose is the least reliable thing in these notes.
