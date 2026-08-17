# Report 2 — where the guide is unreliable, ambiguous or self-contradictory

The MOFAM guide is a prose walkthrough written for a human driving MO2 by hand.
Compiling it turns up places where it is wrong, means something other than what
it says, or leaves a decision unstated. Each of these cost a rebuild or nearly
did.

---

## Names that do not match the mods

The single most expensive category, because a wrong name silently matches
nothing rather than failing.

| the guide says | the mod is actually called | cost |
|---|---|---|
| AI Enhanced - Colourful Clothing - Upperclass + Middleclass | `AI Enhanced - Colorful Clothing - Upperclass` **and** `- Middleclass` — two mods, American spelling | Part 9 conflict pass came up short |
| Waalx's Animals and Creatures | `WAC Waalx Animals & Creatures` | same |
| 01 Maskar's Oblivion Overhaul INI Files | `01 Maskar's Oblivion Overhaul patch and INI files` — and it ships a **plugin** the short name hides | Part 10, caught by `inspect` |
| Brunbek Yellow Multi-Colour (Recommended) | **Brumbek** Yellow Multi-**Color** (Recommended) | Part 17 AWLS |
| Blue-purple (Recommended) | Blue-**P**urple (Recommended) | Part 17 AWLS |
| More Colours (Recommended) | More Col**o**rs (Recommended) | Part 17 AWLS |
| Cava Obscura - Filter Patch for Mods.esp | `...Filter Patch **F**or Mods.esp` | Part 12 |

**Counter-example worth recording**: `Colourful Clothing - Collection - Seamless
OCOv2` *is* the mod's exact name, British spelling and suffix included. Having
been caught out twice, I assumed a third error and left 926 files unexplained
until Part 10 checked the actual mod list. **The guide being wrong twice is not
evidence it is wrong a third time.**

---

**The pattern**: a human clicking the nearest visible match never notices any of
these. An exact-match selector fails on every one. Nine occurrences so far, and
they are the single most common way this guide breaks a compiler.

## The guide contradicting itself

- **Part 11 rows 7/8** — row 7 titles the mod `OUT Dungeons`; row 8 says *"ensure
  the bsa naming matches '**OUT - Dungeons**'"*, with a hyphen. Only the
  `.bsa`/`.esp` stems matching **each other** is load-bearing, so either works.
  Followed the Oracle's unhyphenated choice.

---

## Instructions whose real precondition is mentioned only in passing

- **"Enable Parsing of Archives"** (Part 9 #11). Stated once, almost as an
  aside, and it decides whether the whole step works. WAC ships its assets
  inside a BSA, so without archive parsing MO2's conflict tabs show nothing
  against it and two of the guide's three conflict categories look inapplicable.
  A first pass found only the clothing conflicts for exactly this reason.

---

## A guide instruction that needs two archives to satisfy

**Part 16 rows 1 and 32** install mod 54904 twice under different names. The
guide says **BETA1**; the current release is **BETA2**, which ships as **two
archives** (Part 1 and Part 2) with the city-gate textures split across both.
Row 32's six paths cannot all be satisfied from either archive alone.

Nothing in the guide says the mod comes in parts, because it did not when the
guide was written.

## Version pinning left implicit

- The guide frequently says only **"the top file on the page"**, which was true
  in March 2025 and is not now. Every Oracle archive dated after 2025-03 is
  flagged POST-GUIDE by `diff` rather than silently accepted.
- **Part 8 (MOO)** pins OLD FILES 4.9.4.2 explicitly — and it matters: 5.x drops
  a setting the guide sets.
- **Part 9 (OOO Enhanced)** names "5.3 - PreRelease & 5.3b Resources". The two
  halves must match or you get a build neither the guide nor anyone else ran.

---

## Steps that describe MO2 mechanics rather than intent

These are the ones a compiler has to translate rather than execute.

- **"Copy their contents to this empty mod"** (Part 11 #1-6) — means *one mod
  assembled from five archives*, not five mods plus a copy step.
- **"Install manually & deselect everything except…"** (Part 10 #1) — an
  `include` filter.
- **"Open the Conflicts tab & hide the files that…"** — an authorial decision
  about which mod wins, expressed as a sequence of clicks. See report 3.
- **"The mod has been packaged incorrectly"** (Part 11 #12), followed by four
  steps creating folders and dragging files — a `target_subdir`.

---

## Ambiguity that needed a judgment call

- **Part 11 #27**: *"textures > rocks > everything **except** underwater
  folder"*. Expressed by naming what to delete, since under `textures/rocks`
  there is exactly one other directory and 28 loose files. An "except" rule
  would have been more machinery for less clarity.


## Where the guide turned out to be right and I assumed otherwise

Recorded because the failure mode is mine, not the guide's, and it has now
happened twice.

- **Part 9's `Colourful Clothing - Collection - Seamless OCOv2`** is the mod's
  exact name. Having caught the guide out on two names already, I assumed a
  third error and left 926 files unexplained until Part 10 checked the actual
  mod list.
- **Part 16's Cheydinhal city gate** looked like a guide error — the path is not
  in the BETA2 Part 2 archive. It is in Part 1. The guide names six real files;
  the Oracle only has five of them.

**Two errors in a document are not evidence of a third.** Check the artefact,
not the pattern.
