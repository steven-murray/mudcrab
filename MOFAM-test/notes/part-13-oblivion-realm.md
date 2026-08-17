# Part 13 (Oblivion Realm)

Six rows, six Oracle folders.

**Status: 6 of 6 byte-for-byte identical. The first section with no differences
at all.**

The one thing `diff` cannot see: the section's closing instruction is an
Oblivion.ini edit, and **the Oracle did not apply it**.

## The Oracle never set `bUseRefractionShader`

> *"Lastly, using MO2's Ini Editor, search for **bUseRefractionShader** and set
> it to 0. This fixes a visual bug with the Oblivion gates."*

The Oracle's own `oblivion.ini` still has `bUseRefractionShader=1`. Ours is now
0, under `[Display]`.

Worth dwelling on: `diff` compares **mod folders**, not INIs, so a whole class
of guide instruction is invisible to our main verification tool. Six of six
identical and a missed setting are not in tension — they are measuring different
things. Every INI instruction from here on needs checking by hand against the
Oracle's profile, not inferred from a clean diff.

## Three of six archives are not fetchable

Rows 2 and 3 are tesall.ru downloads (`[MI]` in the guide), and row 4 is a Nexus
mod whose file id MO2 never recorded. All three are now `manual:` descriptors
resolved from the local archives.

Row 4 is worth flagging separately: `Oblivion Caves retexture 2K` **is** on
Nexus as mod 47407, and only the file id is missing, so this is a lookup rather
than a genuinely unfetchable file. Marked `TODO(steven)` in the TOML.

## Two bugs this section found

### `mudcrab add` could not express "I know the mod, not the file"

It errored out on a `modid` with no `fileid`, which is right when someone is
sitting there to answer it and useless in an unattended run — the archive is
already on disk and the build can proceed. `--manual` now records what is known,
flags what is not, and carries on. Its TODO names the mod id, so the follow-up
is a lookup with a starting point rather than a search.

### Another false POST-GUIDE, one guard short

Part 10 taught `classify_guide_age` to ignore `nexusLastModified` when
`modid=0`. `Oblivion Landskape` has **`modid=7`** — not zero, not a real Nexus
id either, just MO2 bookkeeping — so it sailed past that check and a
hand-downloaded tesall.ru archive was reported as newer than the guide.

The fix is the sharper rule, and the one that should have been used first: the
date is evidence only if MO2 recorded an actual Nexus **file**, which means a
non-zero `fileid` as well as a non-zero `modid`. Third occurrence of this same
false positive; this version keys on the thing that actually implies provenance
rather than on a proxy for it.

## Layouts

Nothing declared. All six are plain auto-detected data folders, including the
two `[MI]` rows whose archives wrap everything in `Data/`. Their root-level
`ReadmeRU.txt` and `.url` files are dropped by unwrapping and are absent from
the Oracle too, so this is the one place where the known "files beside the data
folder" difference happens to agree.

Row 4's *"delete the Meshes folder given we'll use the following mod"* is a
plain `file_prune`; row 5 supplies the replacements.
