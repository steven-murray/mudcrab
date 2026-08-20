# 2. Problems with the guide

## Part 25 — Arthmoor's Towns

1. **Row 5b names the wrong archive.** "Drag the sound folder from **4b**" —
   that is Molapi's VA archive, not Reedstand's. 4b holds
   `sound/voice/Molapi.esp/`, which `Reedstand.esp` cannot address. Plainly a
   copy-paste slip; read as 5b.

2. **Row 7 omits Urasek's own `sound/` folder.** The first bullet says to drag
   "meshes & textures" from 7a; the third says to delete the loose "meshes,
   sound & textures" folders. The row contradicts itself, and taking the first
   bullet literally throws away 440 voice files the mod ships. Read as
   including sound — which is also what your Oracle contains, so this one is
   settled from both directions.

3. **Reedstand ships a `video/` folder the guide never mentions.** Rows list
   "meshes, sound & textures" throughout, but 5a also has `video/arthmoor`. You
   packed it; we pack it. Worth a line in the guide.

## Part 26a — New & Modified Locations

4. **The numbering is broken in four ways.** 27 and 45 are absent entirely; 19
   is printed as 29; and there are **two rows labelled 47a**. The section has 57
   rows regardless of what the numbers say.

5. **Row 26 names a folder that does not exist.** The guide says "HESU Skyrim
   Temple v1.2"; the archive's folder is "HESU **The** Skyrim Temple v1.2". A
   user following it literally finds nothing.

6. **Row 46 names a different mod than it links.** The heading is "Glowing
   Stones"; the archive is `Glowing Wonders-43331-v1-0.zip` and the plugin is
   `Glowing Wonders.esp`. Same mod page, different name everywhere else.

7. **Row 7's "disable" is ambiguous.** "During MI, right click Files & select
   Set as \ Directory, and disable The Imperial Water - BETTER CITIES.esp
   plugin." Read as part of the manual-install step — deselect it there, so the
   file never lands — which is what your Oracle contains. Read as "untick it in
   the plugins tab" it would mean leaving the file installed and inactive. We
   took the first reading.

8. **Row 48's keep-list is a keep-list.** "Delete everything except:" followed
   by six folders. Expressed here as the twenty things to delete, because that
   is what the action can say; every one of those patterns must match, so if the
   mod ever stops shipping one, the install fails rather than quietly keeping
   something new.

## Carried over

The guide's own version is dated only "03.25", and `published = "2025-03-18"`
remains a judgement call you signed off. Six of Part 26a's archives postdate it
(four Unique Landscapes rebuilds from December 2025, and two guild files from
mid-2025); the guide says "top file on the page", so newer is arguably the
instruction rather than drift, but they are listed in the section notes.
