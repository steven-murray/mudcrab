# 2. Problems with the guide

Places the guide is wrong, ambiguous, incomplete, or names things the archives
do not.

## Wrong, in a way that would break an install

### Part 23 row 8 — "Restoration section" is not a section

> *"open the mod's INI Files tab & apply the following to Av Latta Magicka.ini's
> Restoration section"*

`Av Latta Magicka.ini` has **no `[Restoration]` header**. It marks its sections
with comment banners:

```
;============ Restoration =============
set almQ.bTurnRestoration to 1
set almQ.bDisableREHEShader to 1
```

Taking the guide literally — declaring `section = "Restoration"` — found nothing
and appended a *second* copy of the key under a header the game never reads,
leaving the setting the guide meant untouched. The key is unique in the file, so
no section is needed at all.

This is the one guide problem in the run that silently produces a wrong install.

## Names that do not match reality

| part | row | guide says | reality |
|---|---|---|---|
| 19 | 20 | `ArmoredManeFix: MergeablePatch` | FOMOD spells it **`MergablePatch`** |
| 19 | 20 | "Coop's TW3 Oblivion Horse Replacer" | file is *Coop's Roach Horse Replacer* |
| 20 | 8 | "select **00 Core** only" | subpackage is **`00 core patch`** |
| 21 | 29 | "Kat's Actually Decent **Enviroment** Map" | *Kinda* Actually Decent **Environment** Map |
| 23 | 9 | "Av Latta Magicka - **Migcks** Misc…" | archive says **Migck's** |
| 18 | 8 | "…for **KatKat74's** Ships Retexture" | archive says **KatKat47's** |

None is fatal — the intent is recoverable each time — but every one costs a
lookup, and a BAIN or FOMOD selection that does not resolve is a hard error
rather than a silent skip, which is the only reason these were all caught.

**Every BAIN row so far has had at least one subpackage spelled differently from
the guide.** Worth treating `mudcrab inspect` as mandatory before writing one.

## Incomplete

### Part 20 row 1a — an unmentioned third folder

> *"Install manually & deselect the Textures and Meshes folders."*

The archive has a third top-level folder, `Docs/`, holding 48 screenshots and a
readme. The guide does not mention it; your Oracle does not have it either. We
excluded it, on the grounds that an instruction about which folders to drop was
not written to preserve a screenshot gallery.

### Part 19 — the row count is not what a first read gives

Part 19 looks like 20 rows. It has **29**: rows 21-29 sit below a long block of
prose and a FOMOD answer list, and are easy to miss entirely. Only the Oracle's
31 folders caught it.

## Ambiguous in a way no amount of care resolves

### "1st main file" when the files are not versions of each other

Part 19 row 22, *Coop's Mudcrab Remake*, says **"(1st main file)"**. That page
hosts **two main files that are not versions of one another** — one for MOO
users, one for people without it. "1st" is a position in a list Nexus is free to
reorder, naming a functional choice the guide never states.

A version drift is at least reasonable-about: the guide meant an older file,
here is a newer one, and `diff` flags it. This is not that. Pick the other one
and you get a working install of the wrong mod, with nothing anywhere saying so.

The same shape applies to every "main file only" / "optional file only" /
"1st main file" selector on a page with more than one of that kind. There is no
fix inside mudcrab; the only answer is to read the page and record *which* file
in words, which is now done for row 22.

## Transcription defects in our copy of the guide

`mofam-source.md` mangles Part 23 row 1's INI block into mojibake
(`set dcvars.ini_DodgeKeyCode______ to__ 42`). The keys and values survive; only
the alignment is lost. Noted because the plan already flagged this file as an
agent transcription with known defects, and this is another.
