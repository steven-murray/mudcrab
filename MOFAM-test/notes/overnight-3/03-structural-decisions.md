# 3. Structural decisions

Six changes to mudcrab. Three of them are in the BSA writer, and they came out
of one observation: every packed mod in the list differed from your Oracle, and
"sha256 differs" was not an explanation.

## 3.1 The BSA writer stores a repeated payload once

BSArch points several file records at the same offset when their content is
identical. The format allows it — a record carries its own size, and the reader
already handled archives shaped that way. mudcrab stored one copy per record.

Feldscar was 22% larger for this, Urasek two thirds. It is not just disk: it was
the whole reason a packed mod's size never matched yours.

**Every packed mod in the list now matches your Oracle's BSA exactly in size,
file list and payload bytes** — 16 archives, including OOO's 947 MB and OOO
Enhanced's 1.75 GB.

## 3.2 Archives declare the flags real archives declare

mudcrab wrote `archiveFlags = 0x003` — folder names, file names, nothing else.

In Part 24 I decided that matching BSArch's `0x783` would be "cargo-culting a
byte pattern into a file format", and left it. **That was wrong, and the corpus
says so.** Of the 74 BSAs on this machine, all 74 set bits 9 and 10, and all 18
of Bethesda's own set bit 8 as well. `0x703` is the plain uncompressed shape
they share. mudcrab's archives were the only ones anywhere with all three clear.

Now written as `0x703`. Deliberately **not** copied: `0x10` and `0x80`, which
genuinely vary between real archives — `0x80` is BSArch's habit and Bethesda
sets it on one archive of eighteen. Writing a bit real files disagree about
would be a guess; writing one they agree on is not.

The tally is checked in as `notes/bsa-header-flags.csv` so the claim is
inspectable rather than remembered.

## 3.3 A `.lip` file is a voice, not a sound

The *file* flags say which asset kinds an archive can serve, and Oblivion
consults them — an archive that under-declares silently serves nothing of that
kind. mudcrab declared a `.lip` file as **both** a sound and a voice, on the
reasoning that over-declaring is the safe direction.

`Oblivion - Voices1.bsa` holds 16603 `.mp3` and 16595 `.lip` files and declares
voices only. `Oblivion - Sounds.bsa` is 1533 `.wav` files and declares sounds.
Bethesda separates them cleanly, so the rule was simply wrong — and it made
every voiced mod's archive claim a kind it does not hold. Fixed, and the file
flags now match your Oracle everywhere except Reedstand, where **ours is more
accurate**: it holds a `video/` file and declares miscellaneous, as
`Oblivion - Misc.bsa` does for the same kind of content.

## 3.4 `diff` explains a BSA difference instead of hashing it

Two archives can hold the same files, byte for byte, and not be the same file —
the payload region need not follow record order. `diff` now says so:

    Feldscar.bsa  (same size 96749607 B, sha256 f1c21d38d1c5 vs 835b07757eb7)
      same 1849 files, same payloads; archive flags 0x0703 vs 0x0793

That line is the difference between a section costing an hour of investigation
and costing a glance. It is also what found 3.3 — the file-flags mismatch was
invisible until the tool started printing it.

## 3.5 `file_prune` can say "everything except X"

The guide says "delete every esp except X" constantly. It was previously
written as a list of all the others, which goes stale the moment the archive
gains a file. `except` now applies to glob patterns as well as to
`conflicts_with`, so the row says what the guide says.

An `except` no selector picks is still an error, so a carve-out cannot outlive
its reason.

## 3.6 `validate` enforces the 255-plugin limit

Oblivion indexes a plugin's FormIDs with one byte. Past 255 the game does not
complain — it loads the first 255 and the rest are simply absent, which looks
like a mod that failed to install.

This is engine knowledge, not MOFAM knowledge, so it lives in `validate`. With
Part 26a in, the list stands at **254**. It matters now.

## 3.7 Two ergonomics fixes

- **A `manual:` descriptor supplies its own `file_name`.** `manual:Feldscar.7z`
  can only be `Feldscar.7z`; writing it twice was redundancy that had to be kept
  in sync. Part 25's sixteen archives were reported "must be downloaded by hand"
  while sitting in a search path.
- **Wrapper descent now applies to archives containing a plugin.** Part 26a row
  53 wraps its plugins in `SIUnmarkedLocations [updated]/`, named after neither
  the mod nor `Data`. The descent that handles this existed but was unreachable
  whenever an archive held a plugin, because plugin classification ran first and
  bailed. Sibling wrappers each holding a plugin are still rejected.

## What the reviews caught

Both section reviews found something real.

**Part 25** — a bug I had just introduced. `file_prune` credited each `except`
entry to one selector, so a file both selectors reached was credited to the
patterns and then deleted by the conflict pass, which runs first. Silently: with
nothing left unaccounted for, there was nothing to complain about. The reviewer
wrote a test, confirmed it, and reverted cleanly. Also caught a doc comment
glued to the one below it, and a note citing a file that did not exist — writing
that file out found my own doc comment had overclaimed (bit 8 is set by 69 of
74, not all 74).

**Part 26a** — see the section notes.
