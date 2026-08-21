# Part 29 — Animations

Ten rows. **7 of 10 identical.** The other three differ only by a Prebash merge
source hidden in the Oracle: Faster Horses, Mehrunes Dagon Walk (the mergeable
build from row 8) and both Unique Wolf Animations plugins.

## Row 7's archive was locally modified — corrected

**An earlier version of this note said the mod author had packaged their
downloads folder by accident. That was wrong**, and the correction matters
because the fix I made rested on it.

`Mehrunes Dagon Walking Animation-52126-1-1-1656778512.zip` on this machine had
six unrelated mod archives at its root — Nirnroot retexture, Oblivion Races
Unlocked, Painters Touch, two Paladin Mod files, Pet your animals. Steven
modified the zip locally by accident; the file on Nexus was always fine. He has
since refreshed it: 14.7 MB down to 76 KB, containing exactly
`MehrunesDagonWalk.esp` and `meshes/`.

The row is back to a plain extract plus the guide's `file_prune` of the plugin.
The `include = ["meshes/**"]` workaround is gone — it was papering over a local
accident, not an upstream defect, and leaving it would have quietly dropped the
plugin if the archive ever changed again.

### What it exposed in mudcrab, which was real

Refreshing the archive did **not** fix the install. mudcrab's cache is keyed by
a derived name — mod id, archive index, file id — which says nothing about
content, and the cache hit short-circuits before adoption. So the next install
happily unpacked the stale 14.7 MB copy again.

`download::cache_entry_is_stale` now compares the cached entry's size against
the file it was adopted from and re-adopts when they differ:

    WARN cached archive no longer matches the file it came from; re-adopting
         cached_bytes=14755535 source_bytes=76159

Compared by size, not by hash: the cache holds tens of gigabytes, this runs once
per archive per install, and a size change is what a replaced download looks
like. A same-size different-content swap is not worth reading 50 GB to catch.

## Structure

- **Row 4** `data_folder = "Core"` — the archive also ships Variations.
- **Row 5** `data_folder = "Stylish Jump 1.0/Normal"` — Feminine and Optional
  are the alternatives.
- **Row 9** two archive entries over one file, to lift one patch plugin out of
  `01 Patches` into `00 Core`, the same shape as Part 26a's Legion Forester
  Outposts.
