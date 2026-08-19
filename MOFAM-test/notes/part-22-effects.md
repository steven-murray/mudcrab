# Part 22 — Effects

5 guide rows, 8 mods (row 3 installs four files from one page, in the guide's
order). **6 of 8 identical.**

## The third `conflicts_with`, and the count matches again

Guide row 2:

> *Once installed, open the conflicts tab & hide the 2 winning texture files
> over Katkat's Ayleid Ruins HD mod.*

```toml
[[mods.actions]]
action = "file_hide"
conflicts_with = ["Ayleid Ruins HD BaseMetal"]
under = "textures"
```

Hid **exactly 2**, and they are unmistakably the right two:

```
textures/dungeons/ayleidruins/arparticle01.dds
textures/dungeons/ayleidruins/arparticle01_g.dds
```

Three sections, three conflict rows, three counts stated by the guide, three
matches. Note the partner had to be picked: Part 18 row 3 installs Katkat's
Ayleid Ruins page as *two* mods, and the guide's "Katkat's Ayleid Ruins HD mod"
names the page. The textures come from the BaseMetal half.

**The Oracle did not do this step**, so the two files show as hidden on our side
only. That is now three guide instructions in a row — Parts 18, 21 and 22 — that
the hand-built instance skipped and this build performs.

## The other difference

`Alternate ghost shader.esp` and `Improved Fires and Flames - Increased Sound.esp`
are Part 36 merge sources, hidden in the Oracle, active here.

Row 3's own instruction — *"hide Meshes > Lights > IronLampHangingShort01Fake.nif"*
— is a plain `file_hide` with a named path, not a conflict relationship: the
guide points at one file and does not say what it loses to. The Oracle had
already done this one, so the two sides agree.

## A `diff` rule generalised, and a real flaw in it fixed

`IMPROVED Fire Spell Animation.gif` sits at that mod's root. MO2 keeps top-level
straggler files and so do we, by Steven's own decision (S4); the Oracle does not
have it. That is a documentation difference, which by standing decision is not a
finding — but `is_documentation` matched by *name*, and the file is called after
the mod.

`obmm_bsa_settings.jpg` was already special-cased there, which was the same
observation made one filename at a time. Generalised — but the first attempt
generalised too far.

**"Oblivion reads `.dds` and nothing else" is true of the engine and false of
what ships in a mod.** The review found three counterexamples already in the
Oracle: ORC's `textures/Effects/bluenoise.png` is a shader resource its own DLL
loads, Pek's COBL book jackets are sixteen `.png` textures that constitute the
entire mod, and `Dagger_Data` puts `.bmp` skies under `textures/dag/sky/`. An
extension-only rule would have dropped all of those from the report in silence —
causing the exact failure it was written to prevent.

The rule is now gated on *where* the file sits: a raster format outside the
game's content folders (`meshes/`, `textures/`, `sound/`, `menus/`, …) is a
screenshot; inside one it is an asset. That still covers the `.gif` at a mod
root, Part 20's `Docs/` gallery, and the old special case.

Writing the test for it turned up a genuine flaw that predates this change. The
name markers (`readme`, `credits`, `license`, …) were matched with `contains`
against *any* file, so **`textures/menus/license.dds` counted as documentation**
— a real texture, silently dropped from the report. The loose match is only safe
on a file the game would never load, so the markers now apply only to
documentation extensions (`.txt`, `.rtf`, `.pdf`, `.md`, …) or no extension.

A quiet under-report is the one failure mode this whole comparison exists to
avoid, so it is worth saying plainly: that bug had been live since the rule was
written, and only a test with a deliberately awkward case found it.
