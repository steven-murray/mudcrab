# Part 23 — Combat & Magic

13 guide rows, 14 mods (row 10 installs two files from one page). **9 of 14
identical.** Four differences are Part 36 merge sources hidden in the Oracle;
the fifth is a real disagreement about two numbers.

## The Oracle disagrees with the guide on two values

Guide row 1 gives four INI edits for `Dynamic Oblivion Combat.ini`. Two of them
do not match what the Oracle contains:

| setting | guide | Oracle |
|---|---|---|
| `dcvars.ini_DodgeKeyCode` | 42 | 42 |
| `dcvars.ini_NPCdodgePercent` | **50** | **70** |
| `dcvars.ini_NPCflankPercent` | **50** | **70** |
| `dcvars.ini_NPCDisarmToKOratio` | 10 | 10 |

Ours follows the guide, so this file differs by exactly those two lines and
nothing else. **For Steven to settle** — either the guide's numbers are what he
meant and the Oracle drifted, or he tuned them deliberately and the guide is
stale.

Worth noting the guide's transcription is mangled here — `set
dcvars.ini_DodgeKeyCode______ to__ 42` — but only in the whitespace; every key
and value is legible through it.

## Two `ini_set` bugs, both about not vandalising the file

### `set-to` lines were re-rendered, losing columns and comments

`Dynamic Oblivion Combat.ini` is tab-aligned and annotates every line:

```
set dcvars.ini_NPCdodgePercent		to	70	;Default 90
```

Editing it produced `set dcvars.ini_NPCdodgePercent to 50` — right value, and
the author's column and their note about the default both gone. The file lost 82
bytes across four edits.

This is the same mistake the *standard* format made and had fixed twice already
(column alignment, then right-hand-side spacing). `replace_value_in_place`
returned `None` for anything that was not `Standard`, so `set-to` fell through
to rendering a fresh line every time. It now rewrites only the value and leaves
everything either side of it alone.

The old test asserted the *normalising* behaviour — it was called
`ini_set_updates_set_to_assignment_with_arbitrary_spacing`, and the arbitrary
spacing was the input, not something to preserve. Renamed and inverted, with a
second test for the trailing comment.

### A file with no final newline gained one

`Av Latta Magicka.ini` ships without a trailing newline. Rewriting it added one,
so a file we changed one value in differed from the Oracle by two bytes. The
line *ending* was already preserved; whether there was a final one was not.

Both are small. Both are the same principle: setting a value is not a licence to
reformat somebody's file, and a report full of two-byte differences is a report
people stop reading.

## The guide's "Restoration section" is not a section

Guide row 8 says to apply an edit to *"Av Latta Magicka.ini's Restoration
section"*. The file has no `[Restoration]` header — it marks its sections with
comment banners:

```
;============ Restoration =============
set almQ.bTurnRestoration to 1
set almQ.bDisableREHEShader to 1
```

Declaring `section = "Restoration"` therefore found nothing and **appended a new
`[Restoration]` header with a second copy of the key**, leaving the file with the
setting twice and the real one untouched. The key occurs once in the whole file,
so no section is needed at all.

That mudcrab created a section rather than failing is deliberate — Oblivion.ini
genuinely omits sections it has no non-default keys for — but it is worth
recording that the behaviour is wrong-looking in a file whose sections are
cosmetic. Left as it is; the modlist says why in a comment on the row.

## Small things

- Rows 3 and 6 say *delete* rather than hide, and the Oracle's folders have no
  plugin at all, so `file_prune` matches on both sides.
- Row 6 deletes `StarX Vampire Deaths.esp` and `.esm`; row 7 then ships its own
  `StarX Vampire Deaths.esp`, which is the one that loads. Row 6 contributes
  assets only.
- Row 9 installs one folder out of sixteen in a patch collection, so
  `data_folder`. The archive spells it `Migck's` and the guide `Migcks`; the
  Oracle's folder follows the guide, and so does our id.
