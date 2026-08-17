# Overnight run — start here

**Parts 12 through 17 are built and verified. 83 mods, 70 byte-for-byte
identical against the Oracle, every difference accounted for.**

| Part | mods | identical | differing | notes |
|---|---|---|---|---|
| 12 Weather & Lighting | 20 | 14 | 6 | `part-12-weather-and-lighting.md` |
| 13 Oblivion Realm | 6 | **6** | 0 | `part-13-oblivion-realm.md` |
| 14 ENB & ORC | 3 | 2 | 1 | ENB pinned — `part-14-enb-and-orc.md` |
| 15 Interior Retextures | 13 | 12 | 1 | `part-15-interior-retextures.md` |
| 16 Town & City | 36 | 32 | 3 (+1 extra) | `part-16-town-and-city-retextures.md` |
| 17 AWLS | 5 | 4 | 1 | `part-17-awls.md` |

380 tests, clippy clean, working tree clean. LOOT removed from the build.

Every section was reviewed by a separate agent before moving on, and those
reviews caught six real errors — listed at the bottom of this file, because they
are the best evidence for how much to trust the rest.

## The four reports

1. **`01-oracle-disagreements.md`** — every way our build differs from yours.
   Most are systemic and expected; **ten need a decision from you**, marked
   `[decide]`.
2. **`02-guide-problems.md`** — where the guide is wrong, ambiguous, or
   self-contradictory. The headline: **nine mod/option names in the guide do not
   match the actual mods**, which is the single most common way it breaks a
   compiler.
3. **`03-structural-decisions.md`** — decisions taken about mudcrab itself, each
   with the alternative rejected.
4. **`04-deferred.md`** — twelve things pinned for you rather than improvised.

## If you read only three things

**1. `diff` cannot see INI edits.** Part 13 came out 6-of-6 identical *and* had
a guide instruction your Oracle never applied (`bUseRefractionShader=0`). Not a
contradiction — `diff` compares mod folders only. An audit of every `ini_set`
against your profile INIs is queued (report 4, D9). Until it runs, a clean
section diff proves less than it looks like.

**2. `meta.ini` answers a narrower question than it appears to.**
`installationFile` records the *last* archive installed into a folder, not all
of them — Part 16's T4UTXL is built from two archives and your Oracle names one,
which is why building from it gave 5 files against your 11. `modid` gets written
even for ModDB and MediaFire downloads. `--from-oracle` is a scaffold; the
archive is the authority.

**3. The AWLS row (Part 17) is the one real judgement call waiting for you.**
25 files differ because your installer answers and the guide's disagree, and MO2
does not record FOMOD choices anywhere, so yours cannot be read back. It is a
look-at-the-screen decision.

## What the reviews caught

Six errors I would otherwise have shipped, all now fixed:

- a BAIN miscount (Part 12)
- an overstated claim that Oblivion loads a BSA *only* via a matching plugin
  stem — `Oblivion.ini`'s `SArchiveList` is the other route, and the same
  overstatement was in a code docstring
- a false claim that two ENB folders were the same archive; they are v0500 and
  v0181, which is the choice guide row 1 leaves open
- **`ORC.esp` sitting loose and unhidden in our build**, which I had written up
  as "absent" — a real behaviour gap, now fixed, general case pinned
- an archive described as having three data folders when it has four
- a loose claim about how many Part 15 mods needed layout declarations

One review finding was itself wrong (it claimed no tests covered the new
`ini_set` logic; two do and pass), so the reviews are not infallible either.
