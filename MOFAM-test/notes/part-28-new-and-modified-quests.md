# Part 28 — New & Modified Quests

38 rows, 39 mods. **26 of 39 identical on the first build.**

## The 13 differences

**8 are merge-source hides** — Prebash consumes them and the Oracle has that
merge built: OCRP Knights, both OCRP patches, Lost Spires NPC AI Addon, the
Ayleid Steps voiced addon and its patches, SM Plugin Refurbish Knights Infamy,
Daedric Requirements, the AFK Weye Reworked Posts patch and the AFK Weye typo
patch.

**5 are individual, and each is explained:**

### AFK_Weye — ours is packed, the Oracle's is not

Ours has `AFK_Weye.bsa`; the Oracle has **8424 loose files** and no archive.
This is the row 33 finding: the guide says to unpack the mod's own BSA, overlay
mod 33's voices, repack, and delete the loose folders. In the Oracle the overlay
never happened — BAE wrote `sound/voice/afk_weye.esp/` (367 files) and the copy
wrote `sound/Voice/AFK_Weye.esp/` (7892), which on a case-sensitive filesystem
are two directories, not one — and the repack was never done.

mudcrab folds directory names to lowercase on the way in, so the two land on one
path and genuinely overlay, then `pack_bsa` writes the archive the guide asks
for. **Ours follows the guide; the difference is deliberate.**

### Tales of Cyrodiil Voices — same archive, different byte order

11954 files, identical payloads, `same size 247899942 B`. Layout and the flag
word only, the same class as every other packed mod.

### Configuration Items Begone — one file we cannot produce

`INI Tweaks/Oscuro's_Oblivion_Overhaul.ini` exists only in the Oracle. It is
written by the package's **BAIN wizard script**, which mudcrab has no engine
for; the two subpackages the guide selects give everything else. This is the
first row in the list where a wizard's output is actually missing rather than
merely unused.

### Progress Tracker - Even more Quest INIs — a stray in the Oracle

`ini/progresstracker/meta.ini` exists only in the Oracle, and **the archive does
not ship it**. Left over from the by-hand repackaging the guide used to require.
Ours is right.

## Problems with the guide

**Rows 2 and 4's packaging instructions are obsolete.** Both say to create
`ini/progresstracker/` and move the INIs into it from the archive root. Every
one of the four Progress Tracker archives now ships them already under
`ini/ProgressTracker`, so there is nothing to move and following the
instructions literally would produce `ini/progresstracker/ini/ProgressTracker/`.

**Row 29 names a folder from an older version.** "right click the
HackdirtTheDeepOnes3.3 > Data folder"; the archive here is 3.5 and the folder is
named for that. 3.3 is not on this machine.

## Oracle disagreements

**Row 21.** The guide says "Once installed delete the Bounty Quests OOO
Patch.esp". The Oracle keeps it, as a mod of its own
(`Bounty Quests Fixed and Polished - OOO Patch`). Ours deletes it, per the
guide, so the Oracle has a mod we do not.

Plus the AFK_Weye repack above.

## Version drift

Two post-guide archives: Hackdirt The Deep Ones 3.5 (2025-12-23) and the
Progress Tracker INIs 1.0.4 (2025-05-15). Two unknown-age: the Lost Spires from
the Internet Archive and BrotherhoodRenewed from afkmods, neither carrying a
Nexus file id.
