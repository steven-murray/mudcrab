# Part 28 recon — surveyed, not yet built

38 rows. **Every archive is on disk**, including the two manual ones
(`thelostspiresv14.zip` from the Internet Archive, `BrotherhoodRenewed.7z` from
afkmods). Nothing needs fetching.

## The finding: AFK Weye's voice overlay never overlaid

Guide row 33 says:

> - Using BAE, extract AFK_Weye.bsa from the main mod (32) to its install location
> - Copy the Sound folder from mod 33 to mod 32, replacing when prompted
> - Using BSArch, repack the textures, meshes & sound folder & update the existing bsa
> - Delete the loose textures, meshes & sound folders from mod 32
> - Disable mod 33

In the Oracle, `AFK_Weye` contains **two sound trees that differ only in case**:

| path | files |
|---|---|
| `sound/voice/afk_weye.esp/` | 367 |
| `sound/Voice/AFK_Weye.esp/` | 7892 |

The first is what BAE wrote when it unpacked `AFK_Weye.bsa` (BSA paths are
stored lowercase). The second is mod 33's `Sound/Voice/AFK_Weye.esp/`, copied in
with its own casing. On Windows those are one directory and the copy would have
replaced the 367 files as the guide intends. **On this filesystem they are two
directories**, so nothing was replaced — both trees are present, and which file
wins any given path is left to MO2's VFS.

Steps 3 and 4 were also not performed: `AFK_Weye` has no `.bsa` at all, and the
loose `meshes`, `sound` and `textures` folders are still there.

This is the same case-collision hazard Part 25 had, and the reason mudcrab folds
directory names to lowercase on the way into a mod folder: our build will
genuinely overlay, which means our `AFK_Weye` will differ from the Oracle's by
construction — correctly.

## Other rows needing more than an archive entry

| row | what |
|---|---|
| 1–4 | Progress Tracker: the INIs ship at the archive root and have to be moved into `ini/progresstracker/`. `file_move` covers it. The Oracle spells one of them `ini/ProgressTracker` and the other `ini/progresstracker`, so one will differ from ours in case. |
| 5 | Configuration Items Begone — a BAIN **Wizard** script, not subpackages. Needs inspecting before it can be authored. |
| 6 | OCRP main + optional, "install separately" → two mods. |
| 8 | The Lost Spires — manual, `thelostspiresv14.zip`. `[QAC]`. |
| 19a, 25, 32 | BAIN subpackage selections. |
| 21 | Guide says delete `Bounty Quests OOO Patch.esp`; **the Oracle keeps it as its own mod** instead. Divergence to settle. |
| 27 | Delete `DaedricRequirementsEASY.esp`. |
| 29 | `[MI]`, `data_folder = "HackdirtTheDeepOnes3.3/Data"`. |
| 33 | The AFK Weye repack above: extract a BSA, overlay a second mod's sound, repack. |
| 38 | Pack Tales of Cyrodiil's sound folder into `Tales of Cyrodiil.bsa`. The Oracle did do this one. |

## Merge sources in this section

Nine plugins here are hidden in the Oracle because Prebash consumes them: OCRP
Knights, both OCRP patches, Lost Spires NPC AI Addon, the Ayleid Steps voiced
addon and patches, SM Plugin Refurbish Knights Infamy, Daedric Requirements, the
AFK Weye Reworked Posts patch and the AFK Weye typo patch.
