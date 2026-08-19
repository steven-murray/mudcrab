# Part 20 — Weapons, Armour & Clothing Improvements

30 guide rows (1a/1b/1c count separately), 31 mods — row 10 installs two files
from one page. **23 of 31 identical against the Oracle, and all eight remaining
differences are the same thing**: nine plugins across eight mods, hidden in the
Oracle because the Part 36 merge consumes them, active here until that merge
exists. Same pattern as Parts 7, 8 and 19.

## A layout bug this section found

`Knights of the Nine_Weapon Improvement Project Patch` installed two folders too
deep. Its archive is:

```
Knights of the Nine Weapon Improvement Project Patch V2/
  Knights Of the Nine Mesh Patch/
    Meshes/armor/NDArmorStatic/ndstaticmace_gnd.nif
```

Two levels of naming — the archive, then the patch — before anything the game
reads. Auto-detection unwrapped one level and stopped, so the two meshes landed
under `knights of the nine weapon improvement project patch v2/...` instead of
`meshes/...`. **Nothing failed.** Every file was present, in a folder nothing
would ever look in, and only the Oracle diff noticed.

Worse, `inspect` had it right all along: it reported *"nested data folder: data
folder is `.../Knights Of the Nine Mesh Patch`"*. Two implementations of the same
question, disagreeing, with the one that does the installing being the wrong one
— the exact failure the layout planner was built to remove, surviving in the one
handler that still had its own detection.

`detect_content_wrapper` now descends rather than looking exactly one level down,
and **ambiguity is an error at every depth rather than only at the root**. That
second half matters as much as the first: bailing at the top and shrugging one
level below it would leave the same silent failure in place, just deeper. Two
rival content folders under a wrapper used to return "no wrapper", after which
detection fell through to installing *both alternatives at once* at the archive
root, silently. Now it says so.

Both `bail!` paths finally have tests. Neither ever had one, before or after the
original single-level version.

## Where the guide is thin

- **Row 1a** says *"Install manually & deselect the Textures and Meshes folders"*.
  The archive has a third folder, `Docs/`, holding 48 screenshots and a readme,
  which the guide does not mention. The Oracle does not have it either — that
  folder's mod ends up as one file, the plugin. Excluded here with the other
  two, on the grounds that an instruction about which folders to drop was not
  meant to keep a screenshot gallery. Flagged because it is the guide being
  incomplete rather than mudcrab making a choice.
- **Row 8** says *"select 00 Core only"*. The subpackage is spelled
  **`00 core patch`**. The other three are `Optional`-prefixed alternatives, so
  the intent is unambiguous even though the name is not.
- **Row 8 is POST-GUIDE, and unresolved.** Its archive,
  `VGR Reasonable patch-51851-3-4-7-1743341721.7z`, is dated **2025-03-30**, so
  `diff` flags it as newer than the guide. Version **3.4.5**
  (`...-3-4-5-1719631116.7z`, 2024-06-29) is sitting in the same downloads
  folder and would predate the guide comfortably.

  A guide called *MOFAM 03.25* could mean either: 30 March is inside its own
  month, so "the top file on the page" may well have been 3.4.7 by the time it
  was written — or may not. **Settled by Steven: 3.4.7 is fine.** The section keeps the Oracle's file, and
  the POST-GUIDE flag on this row stays as a true statement about the archive
  rather than a problem to fix.

  Worth noting the evidence is muddier than usual: both archives' MO2 `.meta`
  sidecars claim the *same* `fileID=1000038748`, so the file id alone does not
  distinguish them. Only the filename timestamps do.
- **Rows 20 and 21** are Ebony then Umbra in the guide; the Oracle installed
  Umbra first. They replace different swords and share no files, so the order
  decides nothing. Following the guide, and noting it.

## Row 1c is the list's first mega.nz row

*Weapon Improvement Project - fixes (NO ESP)* is hosted on mega.nz — the first
such row, not the only one: Part 25 row 6b (*Sutch Village - VA*) is another.
Nothing
automated can fetch it, so it is `manual:` like the tesall.ru rows — with the
same caveat recorded in `oracle-dependence.md`: the archive being on disk
already is availability, not reproducibility, and a mega.nz link is if anything
more fragile than a mod page.
