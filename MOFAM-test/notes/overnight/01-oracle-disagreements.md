# Report 1 — where our build does not match the Oracle

Every difference `mudcrab diff` reports, by kind. Grouped because most recur in
every section and only the last group needs a decision from Steven.

Status key: **[systemic]** expected and understood, no action wanted;
**[decide]** needs Steven; **[open]** not yet explained.

---

## Systemic — the same four causes account for most lines

### S1. Part 36 merge-source plugins are hidden in the Oracle, active here
**[systemic]**

The Oracle hides a plugin once it has been merged. Our build has no merges yet,
so those plugins are still active. Every occurrence has been checked against
`mofam.merges.toml` and is a genuine merge source. Resolves itself when Part 36
is built.

Counts so far: Part 7 ×10, Part 8 ×2, Part 9 ×2, Part 10 ×3, Part 11 ×4.

### S2. BSA byte size — mudcrab does not deduplicate payloads **[systemic]**

BSArch points two identical records at one copy; mudcrab stores a payload per
record. **Contents are identical every time** — same file count, same folder
count, same asset-kind flags — and this is checked per BSA rather than assumed.
Costs about 2%.

Occurrences: Part 5 (Evenstars), Part 9 (Oscuro's), Part 11 (all five).

### S3. Dummy plugins differ by 54 bytes, on purpose **[systemic]**

Ours 139 B, the Oracle's 85 B. Both are a bare TES4 header with no records.
The Oracle's carries `CNAM "nmcdyer"`; ours carries `CNAM "mudcrab"` plus an
`SNAM` description. Attributing mudcrab's output to a person would be wrong, so
this stays.

### S4. Files beside the data folder are dropped **[decide — small]**

When an archive holds `Data/` (or a wrapper plus `Data/`), unwrapping takes the
data folder as the mod root and root-level siblings never come across. MO2 keeps
them. Always documentation — readmes, `obmm_BSA_settings.jpg` — so nothing the
engine reads.

Four occurrences: Part 9 (1 readme), Part 11 (Improved Doors and Flora,
Improved Trees and Flora, HD Photorealistic Ivy).

*Worth fixing rather than re-explaining each section, but it is a change to
auto-layout's contract, so it is pinned. See report 4.*

---

## Deliberate divergences — we are right, the Oracle is not

### D1. Maskar's Oblivion Overhaul 4.9.4.2, not 5.0.5 **[decided]**

The guide pins OLD FILES 4.9.4.2; the Oracle runs 5.0.5. Steven chose the
guide. Deciding evidence: 5.x dropped `ini_levelscaling_npc_overridden`, so one
of the guide's 37 INI settings would silently do nothing there. All 37 verified
present in 4.9.4.2 before authoring and read back after install. This mod
therefore diffs wholesale.

### D2. Guide rows Part 11 #1-5 have no mod of their own **[decided]**

They are one mod, `OUT Essentials`, built from five archives. The Oracle's five
folders hold nothing but a `meta.ini` — the guide's own optional cleanup, done
in January. `diff` reporting them missing from ours is correct.

### D3. NightSkies Overhaul — the Oracle skipped a subpackage the guide names
**[decide]**  *Part 12 row 14*

The guide lists five BAIN subpackages, ending `05 - OVERLAY - Aurora - 2k`. The
Oracle's `meta.ini` records four and its folder has no
`textures/sky/overlay.dds`. Not an ambiguity — the guide names it explicitly.
Guide followed, so we have one file the Oracle does not.

**Your call**: add the aurora overlay to the Oracle, or tell me you left it out
deliberately.

### D4. Drifting mist — the guide parks a plugin the Oracle left active
**[decide — cosmetic]**  *Part 12 row 4*

Guide: *"Once installed move drifting mist.esp to the Optional folder."* The
Oracle left it at the mod root. Row 5 ships its own corrected
`drifting mist.esp` at higher priority, so both builds behave identically; the
guide's version just says so explicitly instead of relying on priority order.
Guide followed.

### D5. Installer leftovers — the Oracle is inconsistent with itself
**[decide — cosmetic]**  *Part 12 row 19*

Cava Obscura is a manual install and the Oracle kept neither its `ReadMe.txt`
nor its `omod conversion data/` directory. Part 11's Harvest Flora, also a
manual install, **kept** its `omod conversion data/`. The guide says nothing
either way.

We keep them, consistently — the only rule a compiler can follow. Four lines of
diff on this mod.

### D6. `bUseRefractionShader` — the Oracle never applied it
**[decide]**  *Part 13, closing instruction*

> *"using MO2's Ini Editor, search for bUseRefractionShader and set it to 0.
> This fixes a visual bug with the Oblivion gates."*

The Oracle's `oblivion.ini` still has `=1`. Ours is `=0`, under `[Display]`.

**This one matters beyond itself**: `diff` compares mod folders, not INIs, so
every guide instruction of this kind is invisible to our main verification tool.
Part 13 reported 6 of 6 identical *and* had a setting the Oracle lacks — not a
contradiction, just two different measurements. An audit of every INI edit in
the modlist against the Oracle's profile is queued for the end of the run.

---

## Resolved — recorded because the reasoning matters

- **Part 7 Khajiit head**: the Oracle removed the standard head instead of the
  Nuska variant. Guide followed; Steven has since repaired the Oracle.
- **Part 9 EVE / Seamless cup size**: guide says C-Cup in both rows, the Oracle
  had E-Cup in one. Guide followed; Oracle reinstalled to match.
- **Part 9 OOO Enhanced version pair**: `--from-oracle` gave 5.33 against 5.3b
  resources. 5.33 has since been pulled from Nexus, so the guide's pair is the
  only coherent option.
- **Part 11 BSA packing**: the Oracle had skipped four of five `[DP]` packs.
  Steven has since packed them; all five now match on contents.

**The guide has won every disagreement it has had with the Oracle so far: four
for four.**

---

## Oracle defects found (not our build's problem, but Steven's to fix)

### O1. `OOO Enhanced.bsa` was packed one level too deep — **fixed by Steven**

Stored every path as `textures\textures\…`, so all 3580 textures in it were
unreachable by the engine. Confirmed against a known-good BSA from the same
install. Repacked correctly on 2026-08-17.

### O2. Reinstalling a mod clears its `.mohidden` plugins

Reinstalling `OOO Enhanced` cleared eight hidden plugins, all Prebash merge
sources, so the Oracle now loads both the merge and its sources until they are
re-hidden. Steven has re-hidden these. **The general point stands for any Oracle
mod reinstalled after its merge was built.**
