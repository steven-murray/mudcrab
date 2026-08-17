# Part 14 (ENB & Oblivion Reloaded Combined)

Six guide rows, ten Oracle folders, **three of them enabled**. The most
experimental section in the Oracle and the one where the least should be
improvised.

**Status: 3 mods built, 2 identical, 1 differing and explained. Rows 1-3 (ENB)
are pinned, not built — see below.**

## What the Oracle actually has

```
-enbseries_oblivion_v            -Oblivion Reloaded Combined(ORC) v194
-enbseries_oblivion_v0180        -ORC 311
-CandidENB_Reborn                -ORC 315F
-Candid ENB Tweaked ENBSeries Ini
+Oblivion Reloaded Combined(ORC) v180   +ORC Main Ini   +Vanilla Remastered 1K Whiteflame fix
```

Four ENB folders and four ORC versions, of which exactly three mods are on.
**The enabled ORC is v180, which is the version the guide names** — the 194/311/
315F folders are experiments, and the plan's "use ORC 315F" note was about
rebuilding the Prebash merge on Linux, a different question from what this
section installs.

## Why rows 1-3 are pinned rather than built

Three reasons, any one of which is enough:

1. **It is not Data content.** The guide says *"Extract the d3d9.dll from the
   wrapper version to the oblivion **root folder**"*. A d3d9.dll placed in the
   virtual Data folder does nothing.
2. **All four ENB folders are disabled in the Oracle**, and `Mo2ModlistEntry`
   has no enabled/disabled flag — everything mudcrab installs is enabled. There
   is no way to reproduce "present but off".
3. **Two of the four are the guide's own version dilemma, kept side by side.**
   `enbseries_oblivion_v` is `enbseries_oblivion_v0500.zip` and
   `enbseries_oblivion_v0180` is `enbseries_oblivion_v0181.zip` — exactly the
   two builds row 1 hedges between: *"Some users have reported issues with the
   latest (500) version so use .181 if visual glitches arise."* So the Oracle
   holds both and commits to neither, which is a decision the guide explicitly
   leaves to the user and hardware.

mudcrab does have `game_root_files`, so the honest expression probably exists —
but choosing it means deciding whether mudcrab owns files outside the MO2
instance, and that is a structural call. Pinned.

The same goes for row 6's closing bullets: `fMaxTime` in Oblivion.ini and
`EnableFPSLimit`/`FPSLimit` in enbseries.ini. The guide calls these
**user-specific** and ties them to monitor refresh rate; the Oracle has
`fMaxTime=0.0167` (60fps) where the guide suggests `0.0111` (90fps). Not
something to guess, and the enbseries half depends on the pinned ENB question.

## Fog.ini is why `ini_set` learned about sections

> *"Open up ORC\Fog\Fog.ini … & set both [World] & [Interior] Amount values to
> 0.0."*

`Amount` appears under both headings and means a different thing in each. This
is the exact file the plan predicted would break a section-blind `ini_set`, and
it now takes two `section`-scoped actions. An unqualified edit is refused
outright.

Getting it byte-identical took two further fixes, both found here:

- **Alignment.** The file writes `Amount        =0.0`, padding keys into a
  column. Replacing a value now keeps the line's left-hand side when the file
  is written that way, so a one-number change stays a one-number change.
  `dominant_spacing` had been counting that padding as "this file likes spaces
  around `=`", which is wrong — the space *after* the `=` is the half Oblivion
  reads literally, and the padding before it is cosmetic. The two are now
  measured separately.
- **Line endings.** The file is CRLF, as most of these archives are. We were
  rewriting it as LF, which changed all nine lines of a file we had been asked
  to change two values in. `ini_set` now writes CRLF if the file contains any
  CRLF, and LF otherwise. Note that is a *normalisation*, not preservation: a
  file with mixed endings comes out uniform. No such file has turned up, and
  uniform is the better of the two answers if one does.

Both were invisible until a section had an INI the Oracle had also edited.

## The one remaining difference: `DumpWeathers.ini`

Ours is the shipped file; the Oracle's has an extra `[InteriorGeneric]` section
and completely different values.

**ORC writes this at runtime.** It is a dump of observed weather lighting, so
the Oracle's copy is a record of Steven having played the game and ours is the
pristine original. Not a build difference, and not fixable — it would reappear
the moment either instance is played.

Worth noting as a category: **a mod folder can contain runtime state**, so not
every difference under `mods/` is something the build controls.

## `ORC.esp`: an undeclared plugin, now hidden

Not the usual merge-source case. `mofam.merges.toml` deliberately contains no
`ORC.esp` — the Linux build uses ORC 315F, which ships no plugin — so it is not
in our `plugins` array either. But the ORC180 **archive ships one**, and simply
leaving it out of the load order does not remove it from the mod folder: it sat
there loose and visible, in an enabled mod, where MO2 would offer it as a new
undecided plugin. The Oracle hides it. Now so do we, with the same `file_hide`
already hiding three data files on this mod.

**The general case is worth a decision.** An archive can ship plugins the list
never declares, and today mudcrab notices — the load-order step warns about
"discovered plugins not listed in top-level plugins" — but does nothing about
them. Hiding them automatically is probably right and is a behaviour change, so
it is pinned rather than done.

## The caveat mudcrab cannot honour

> ***IMPORTANT**: … fire up the game to the main menu & start a new game; then
> simply quit. The crash was occurring if the following ini edits were applied
> before the game had been started with the mod present.*

A play-test ordering constraint that needs a human and a running game. The end
state written here is the same either way; only the route differs. Flagged for
the first real play-test.
