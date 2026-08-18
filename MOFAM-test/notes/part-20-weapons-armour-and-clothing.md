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

`detect_content_wrapper` now descends rather than looking exactly one level down.
The shape that justifies descending is re-checked at every level: exactly one
directory, no loose files beside it, that directory is not itself game content,
and this level holds none of its own. Any of those failing ends the descent. The
three cases that must keep working are tested — a file beside the wrapper, two
candidate folders, and a lone `textures/` which is content and not a wrapper.

## Where the guide is thin

- **Row 1a** says *"Install manually & deselect the Textures and Meshes folders"*.
  The archive has a third folder, `Docs/`, holding about a hundred screenshots,
  which the guide does not mention. The Oracle does not have it either — that
  folder's mod ends up as one file, the plugin. Excluded here with the other
  two, on the grounds that an instruction about which folders to drop was not
  meant to keep a screenshot gallery. Flagged because it is the guide being
  incomplete rather than mudcrab making a choice.
- **Row 8** says *"select 00 Core only"*. The subpackage is spelled
  **`00 core patch`**. The other three are `Optional`-prefixed alternatives, so
  the intent is unambiguous even though the name is not.
- **Rows 20 and 21** are Ebony then Umbra in the guide; the Oracle installed
  Umbra first. They replace different swords and share no files, so the order
  decides nothing. Following the guide, and noting it.

## Row 1c is the list's only mega.nz row

*Weapon Improvement Project - fixes (NO ESP)* is hosted on mega.nz. Nothing
automated can fetch it, so it is `manual:` like the tesall.ru rows — with the
same caveat recorded in `oracle-dependence.md`: the archive being on disk
already is availability, not reproducibility, and a mega.nz link is if anything
more fragile than a mod page.
