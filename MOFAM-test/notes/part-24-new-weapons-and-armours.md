# Part 24 — New Weapons & Armours

12 guide rows (row 7 splits into a–e), 16 mods. **8 of 16 identical**, and every
difference is either a Part 36 merge source or a guide step the Oracle did not
take.

This is also the section the conflict machinery has been waiting for since
Part 9: row 8 brings Colourful Clothing, OOO Enhanced Resources' second partner.

## The OOO Enhanced follow-up finally ran, and it is exact

Guide 11's deferred step — *"hide the files that conflict with Colourful
Clothing and WAC, delete them, then pack the remaining textures into
OOO Enhanced.bsa"* — is now stated as a relationship, not a path list:

```toml
[[mods.actions]]
action = "file_prune"
conflicts_with = [ ...five WAC mods..., "Colourful Clothing - Collection - Seamless OCOv2" ]
except = ["meshes/realswords/nord/chainmailm1.nif"]
```

**It deleted exactly 1725 files, and they are exactly the 1725 in
`ooo-enhanced-conflict-hidden-files.txt`** — the list read off the Oracle by
hand when Part 9 was built. Nothing missing, nothing spurious.

The single computed extra is `chainmailm1.nif`, which Steven decided to keep, so
it is carved out by name. `except` is new: a conflict relationship is a
statement about two mods and occasionally one file inside it is a deliberate
exception, and listing it keeps the statement honest instead of degrading it
back into a path list. An `except` naming a file the selection did *not* pick is
an error, so a carve-out cannot outlive its reason.

That closes the loop the design opened: the Oracle-derived list stops being the
mechanism and becomes the test, and it passes.

## Where our OOO Enhanced differs from the Oracle's, and why ours is right

Three differences, all understood:

1. **3580 loose textures, only in the Oracle.** The Oracle packed its textures
   into `OOO Enhanced.bsa` **and kept the loose copies**. MO2 gives loose files
   precedence over archives regardless of priority, so every file in that
   archive is shadowed by the very file it holds — the BSA is inert. Ours packs
   and prunes, which is the only reading under which the instruction does
   anything.
2. **One loose `meshes/RealSwords/Thumbs.db`.** MO2 strips Windows thumbnail
   caches on install; mudcrab does not. Excluded on the row.
3. **The BSA's bytes.** Ours is 1.899 GB against 1.748 GB for the same 3580
   files, because BSArch deduplicates identical payloads and mudcrab does not.
   Known and recorded since Part 5.

The guide says pack the remaining **textures**, and both BSAs hold 3580 textures
and no meshes — so on that point the two agree exactly.

## Our BSAs differ from BSArch's by one header field

Weapons of Morrowind is the clean case: same 82 entries, same 176,975,853 bytes,
different hash. The whole difference is at offset `0x0C`, the archive-flags word.

| | archiveFlags |
|---|---|
| ours | `0x003` — has folder names, has file names |
| BSArch | `0x783` — those two, plus bits 7, 8, 9 and 10 |

Bits 0 and 1 are what Oblivion reads to find anything at all. The rest are
"retain" hints that vanilla Oblivion ignores, and bits 8–10 are not defined for
the v103 archives Oblivion loads.

**Not matched deliberately.** Copying flag bits whose meaning we cannot state
would be cargo-culting a byte pattern into a file format, and the contents are
verified identical. Recorded so that "the BSA differs" stops being mysterious:
for OOO Enhanced it is payload dedup and size, and for everything else it is
this one word.

Also worth noting the guide says to name it *"Weapons of Morrowind"* and the
plugin is `Weapons Of Morrowind.esp`. Ours follows the guide's casing and the
Oracle follows the plugin's. Oblivion matches BSA to plugin case-insensitively,
so both load.

## An ergonomic trap worth naming

`exclude = ["**/thumbs.db"]` caught **4 of 14**. The archive spells four of them
lowercase and ten `Thumbs.db`, and **archive-side globs are case-sensitive while
staged-tree ones are not**. One pattern that looks like it covers the case
silently covers less than a third of it.

That asymmetry was already on the deferred list; this is the first time it has
cost anything. Both spellings are listed on the row.

## The rest of the section

- **Row 1** says to pack Weapons of Morrowind into a BSA and delete the loose
  folders. Steven has since done so, and the two archives hold **the same 82
  files at the same total size** — but differ in one header word, see below.
- **Row 4** says *hide* `JBlades!.esp`; the Oracle **deleted** it. Ours hides,
  per the guide, so we have a `.mohidden` the Oracle lacks entirely.
- **Row 7c** (Thorn Addon) was missing from the Oracle; Steven has since
  installed it. The guide's *"Once installed delete
  tbskGuardsFeaturesThornAddon.esp"* has not been done there, though — rows 7b
  and 7d got the same instruction and were followed. Ours prunes it, so this
  mod now differs by that one plugin.
- **Row 6** is `[QAC]`, so `tbskGuardsFeatures.esp` differs by cleaning, like
  every other QAC row.
- Four Part 36 merge-source plugins.

## `extract_bsa` already deletes what it unpacked

Row 8 says to delete the archive *and* the esp after extracting. `extract_bsa`
removes the archive itself — leaving it would mean a later `pack_bsa` folds the
old archive into the new one — so only the plugin needed pruning. The first
draft pruned both and failed on the missing BSA, which is the action being
right and the modlist being redundant.
