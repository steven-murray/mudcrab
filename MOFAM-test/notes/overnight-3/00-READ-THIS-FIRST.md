# Third overnight run — start here

**Parts 25 and 26a are built and verified, and the Unique Forts merge with
them.** 70 mods added, every difference accounted for.

| Part | rows | mods | identical | notes file |
|---|---|---|---|---|
| 25 Arthmoor's Towns | 9 | 9 | 0 | `part-25-arthmoors-towns.md` |
| 26a New & Modified Locations | 57 | 59 | **35** | `part-26a-new-and-modified-locations.md` |
| 36 Unique Forts merge (SP7) | 2 | 2 | 1 | same file, last section |

Part 25 shows zero identical and that is expected, not a failure: all eight
villages pack a BSA, and a BSA that holds the same files as yours is still a
different file. The tool now says which — see report 3.

**Full-list state: 373 of 730 compared are byte-for-byte identical**, up from
328 when the run started. 212 mods are from sections not yet built. 456 tests,
clippy clean, working tree clean.

## Read this first, though

Part 26a filled the load order — **254 plugins of Oblivion's 255** — and nothing
else fitted. Rather than stop, I built the Unique Forts merge, which is the
plan's SP7 and, on reading the guide, needed nothing pulled out of order: its
consistency patch is Part 36's own row 3. The load order is back to **245**.

That is the one call tonight I would most like you to look at, and it is
`04-deferred.md` item **D0**.

I then tried the same trick on Part 26b and it does not work — **D0b**. Its
merge's consistency patch needs a master from Part 28, two sections ahead, so
the merge refused to build. I reverted the section rather than leave it half
done; 22 of its mod folders were already written to disk before that and I have
left them alone, since removing things under `~/Games` is your call.

## The four reports

1. `01-oracle-disagreements.md` — one new one: Urasek's voice replacement
2. `02-guide-problems.md` — eight, including a row that names a folder that does
   not exist
3. `03-structural-decisions.md` — six changes to mudcrab, three in the BSA writer
4. `04-deferred.md` — five things needing your call, D0 first

## The one result worth knowing

**Every packed mod in the list now matches your Oracle's BSA exactly in size,
file list and payload bytes.** All 16 of them, including OOO's 947 MB and OOO
Enhanced's 1.75 GB. That came from finding three things wrong in mudcrab's BSA
writer — one of which was a decision I had explicitly made the wrong way in
Part 24 and which the corpus overturned. Report 3, sections 3.1–3.3.

## How much to trust this

Both section reviews found something real. The Part 25 review found a
correctness bug I had introduced hours earlier — an `except` carve-out silently
ignored when two selectors overlapped — and confirmed it with a test rather than
asserting it. It also caught a note citing a file that did not exist, and
writing that file out proved one of my own doc comments had overclaimed. Those
are in report 3 under "What the reviews caught", because they are the best
evidence for how much to trust everything else here.
