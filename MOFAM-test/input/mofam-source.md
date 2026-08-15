
*Please do spend a few moments to read the* [Prologue](https://www.nexusmods.com/oblivion/articles/44565)*to grasp the background, intent & brief overview of the final-build. *Certain key-technical, thematic & practical advice is given to shed deeper light on both the installation & final output. Given its broad scope and wide array of mods & differing authors' input, I affectionately perceive MOFAM to be a celebration of the Nexus & modding community, and five years of my life well-spent & richly-rewarded.**

**
**

But how to sum this up for you in a nutshell...

What differentiates MOFAM from other guides is it's purely **MO2-centric**: utilising its tools, additional plugins, and more modern techniques to maximise the value & potential of the load order with improved handling & QoL features. It is also the resulting (and ongoing!) work of all my 5 years of modding the game, similarly to my other guide.

## Welcome to **MOFAM**

***Voted by you! Thankyou! ***

## Part 1 - SETUP
*I use the term '**Advanced**' in this mod title as I'm hoping & assuming this isn't your first rodeo with MO2, or indeed modding Beth titles. It is of paramount importance to have confidence in the basic toolset such as xEdit & MO2, as well as a solid grasp of core IT-literacy skills. My Discord is available for adhoc help & support if needed.*

*Some important points to illustrate:*

- An install of both a **portable** instance of [MO2](https://www.nexusmods.com/skyrimspecialedition/mods/6194?tab=files) & the official Steam GOTY Version of the game, with all DLC. I've never had the GOG version of the game so can only offer versioning advice around the Steam GOTY. For MO2 I simply extract in the root of a drive, away from the OS (e.g. D:\MO2-FO4).
- You will notice from the screengrab above I like to have **executables** as **shortcuts**. I've also highlighted the two headers which are most commonly used for creating separators, empty mods, quick access to MO2's folders, etc.
- A fairly commonly used practise in MO2 is **merging mods** in the left pane that come from the same download page (such as Main Mod + Update + Hotfix). I **avoid** this approach completely. Firstly it's terribly easy to forget that you *have* done (unless a note is added to the mod e.g. 'Merged Hotfix'). Secondly, it tends to brain-burp MO2's versioning system when you check for updates. It's better to *understand* what the hotfix or update included & provides improved *visibility* in the longer term (until the mod itself updates to deprecate the previous hotfix or update). Avoiding this common approach encourages & ensures basic **traceability** & **version control** management.
- To enable a more aesthetically pleasing UI I recommend a dark mode such as any of the **'vs15 Dark...'** options in Settings \> Themes. I also recommend disabling the 'Log' from being visible at the bottom of the GUI as this rarely provides useful information.
- Right click any of the **headers** in the left pane and ensure Conflicts, Flags, Content, Version, Installation & Priority are enabled.
- Within MO2's Modify Executables ensure the following are downloaded & create **executables** including the info below.
- I like to have the tools in **chronological** order of typical usage from left to right. This can be done by drag-and-dropping the executable
  names up & down in the MO2 Modify Executables popup menu.

*MO2 EXECUTABLE SHORTCUTS*

1. [Oblivion](https://www.nexusmods.com/oblivion/mods/37952?tab=files)  *Follow [this](https://github.com/ModOrganizer2/modorganizer/wiki/Running-Oblivion-OBSE-with-MO2) short tutorial on how to ensure [OBSE](https://www.nexusmods.com/oblivion/mods/37952?tab=files) & MO2 work properly. Check 'Force Load Libraries' is enabled and in the Modify Executables menu, select "Add Row" in the Configure Libraries window. Click the "..." button next to your new row and ensure "obse_1_2_416.dll" is in the name box. As of 2.4.4 of MO2 this process is automatic but it's good to check. I'd strongly recommend firing up the game & verifying this is running prior to proceeding with the guide. You can do this by simply starting a new game, invoking the console & typing 'getobseversion'. At time of writing the guide is using version 22.11.*

2. Oblivion Launcher. *This is added by default, however take note it can revert your Oblivion.ini if selected accidentally. Always have a **backup** of this file at the ready once it's configured (we create a 'Backup' folder soon for our vanilla DLC plugins so now would be a good time to create it in game root). I actually right click & **remove** mine to prevent this from happening.*

3. [LOOT](https://www.nexusmods.com/site/mods/439) *Many would argue true modders don't need automation for plugin-sorting, however it's still a hugely useful tool to run to verify which plugins need QAC'ing. This is the principle purpose of it for us. Add the argument **--game="Oblivion"** to ensure it's pointed to the correct title.*

4. [BSArch Pro](https://www.nexusmods.com/fallout4/mods/63243?tab=files) *A hugely effective tool of which we'll use to pack certain mods throughout the installation. Users of CAO, Archive2 etc will be right at home.*

5. [Wrye Bash](https://www.nexusmods.com/site/mods/591) *The tool that I'm sure all of us used when initially modding the game. Hugely capable, respected, and relied upon for years. I simply download the standalone executable & add to the game's root. **NOTE**: We are now using & referencing version **311** that went Live 05/23. Later versions of this on W11 & MO2 2.5 cause python errors.*

6. [xEdit](https://www.nexusmods.com/oblivion/mods/11536) *Create an empty mod in MO2 called 'TES4Edit Cache' & enable the mod for all profiles in use by also enabling 'Create files in mod
instead of overwrite' within the Modify Executables menu, and selecting the TES4Edit Cache mod from the accompanying dropdown.*

7. [xEdit QAC](https://www.nexusmods.com/oblivion/mods/11536) *As above, point the executable to TES4EditQuickAutoClean.exe*. *Whilst there are certain MO2 plugins that automate cleaning, they've proved unreliable for 32bit titles so we're doing it the old fashioned way here.*

8. [zEdit](https://github.com/z-edit/zedit/releases/tag/0.6.6.1) *Use this particular version as I've found the latest version intermittently unreliable for merging, which is the principle reason we're using it - zMerge. Within the Merge Settings menu, ensure the Merge Output Path is your \mods folder. Given we will also use the now widely-adopted MO2 plugins for managing the merged plugins & profiles, ensure 'Disable plugins' and 'Disable Mods' are disabled in the Integrations.*

9. [slowLODGen](https://www.nexusmods.com/oblivion/mods/54553?tab=description) *The successor to xLODGEN, slowpard's new tool has revolutionised both the quality & performance for the game. As I posted on his modpage, possibly the most important mod to hit Oblivion portal in years. Set up the bat file as an executable as per the description page ensuring you also edit the yaml.*

We will also have some uses of [BAE](https://www.nexusmods.com/skyrimspecialedition/mods/974?tab=files) so make sure you have it installed.

*MO2 PLUGINS*

- We will use **merge plugins hide** plugin so install this to the plugins folder of MO2. Essential for both syncing the load order
  across profiles & managing merged plugins. [Here](https://github.com/deorder/mo2-plugins/releases/tag/1.2) is the link. Restart MO2 & verify that in the Plugins menu, hide-type is 'optional' (note this is case-sensitive) and the other option 'true'. The latest version we've found to have issues with its present naming convention, so once installed, rename it to 'merge-plugins' otherwise MO2 (2.5) will throw an error at boot.

- A new plugin to hit Nexus for the sole purpose of QoL improvements for the merging process we run later is called [Prepare merge](https://www.nexusmods.com/skyrimspecialedition/mods/47791); download and add to the Plugins folder of MO2. A great series of walkthroughs are provided [here](https://www.youtube.com/channel/UCbjg6D8oJotlkxM6QQ5ePQw) if you're new to the tool (although the videos are for SSE, the processes are identical).
- Ensure 'Use profile-specific Game INI Files' & 'Automatic Archive Invalidation' in the MO2 Profiles menu are **deselected**.

**

MO2 CONFLICT MANAGEMENT

*Given we make full use of archiving & asset-management (i.e. 'left pane) within MO2, it is crucial to have the setting 'Enable Parsing of Archives' enabled in MO2's settings \> Workarounds menu. Archived assets within mods are italicised & loose are displayed normally (if this process is new to you).*

* TAGS/SUFFIXES ** **Please take note tags/suffixes are applied to a number of installs*:

- \[**QAC**\]: This indicates the plugin requires running through **Q**uick **A**uto **C**lean (shortcutted above)
- \[**DP**\]: For several mods we run them through BSArchPro, however a [Dummy Plugin](https://www.nexusmods.com/oblivion/mods/52949?tab=files) will also be needed to connect the created archive where one does not exist already. This is also installed as part of the final Utilities separator for ease of reference, so have it on your desktop during the installation on the two occasions it's required.
- \[**MI**\] indicates the Data directory is incorrectly set & requires a **M**anual **I**nstall. Simply right click the Data folder and select 'Set as \ directory'. In some instances, further edits are required & these are explained on a case-by-case basis. As a reminder, within MO2 this is what is performed, highlighted bottom left in the images in the spoiler below.




*Now we have MO2 up & running, before we start this adventure there are two tools we also need to run on the base game to improve things.*

1. [4GB RAM Patcher](https://www.nexusmods.com/oblivion/mods/45576). Download & run the tool against the Oblivion.exe *only*. Note it's not required if you're using the GOG version of the game, however I'm on Steam so need to use it. I also install this as a mod in MO2 in the Utilities separator, for safekeeping.

2. [Oblivion BSA Decompressor](https://www.nexusmods.com/oblivion/mods/49652) A relatively new tool from zilav, this genuinely makes a difference. **IMPORTANT**! Note this requires the DLC plugins to be present in the game root, hence we run it now prior to the next step of cleaning the DLCs.

*DLC & PLUGIN CLEANING*

- Copy and paste the vanilla DLC esp's from the game's Data folder to a folder of your choosing (e.g. 'Backups'). Leave Oblivion.esm alone.
- Using LOOT, reference the plugins that require QAC'ing. Allow the operations to perform one by one.
- Create the first separator naming it 1 - CLEANED MASTERS. You'll notice each 'Part' of the install is a separator, so this helps keeping our
  numbering & referencing consistent.
- Create an empty mod in MO2. Cut and paste the cleaned DLC esp's from the Data folder into this mod. I simply call mine 'Clean ESM' (yes, I know, they're not esm's; it's an old habit!) This also negates the unsightly 'Unmanaged' entries on the left pane in MO2.

* BETHINI*

[Bethini](https://www.nexusmods.com/oblivion/mods/46440?tab=files) is also hugely useful to run. Many of you in my Discord & other guide will already know automating ini's just makes me nervous, but given DoubleYou's also a Contributor to my server, my anxiety is alleviated somewhat!

Regardless, this is very tricky to broadly recommend to everyone. However, one thing we all share in common, is the fact the Oblivion engine is notoriously unforgiving & rarely benefits from whatever overpriced GPU nVidia throws at us.

Due to this, I set Recommended Tweaks, High Preset, disable AA, and leave everything at default within Bethini's tabs. Whenever there's a specific parameter to change arising from a mod install, I'll highlight this as we proceed.

There are however some recommended tweaks we can make now to mitigate save-file corruption. Apply the following using MO2's Ini editor:
bSaveOnInteriorExteriorSwitch**=**0
bSaveOnRest**=**0
bSaveOnTravel**=**0
bSaveOnWait**=**0
bAllowScriptedAutosave=0



Part 2 - OBSE PLUGINS
1. OBSE 22.11 Ini

*With OBSE Installed in root as per Setup, I install the Data part of the OBSE download here.*

2. [Add Actor Values](https://www.nexusmods.com/oblivion/mods/33248)

*Right click Oblivion \> Data and select Set as Data Directory. Deselect AddActorValues_example.esp, and then expand the Plugins folder & deselect 'AddActorValues_CS.dll.
*
3. [AveSithis Engine Fixes](https://www.nexusmods.com/oblivion/mods/53911)
4. [Base Object Swapper](https://www.nexusmods.com/oblivion/mods/53872)

*A staple in FO4 & Skyrim modding, we now have the hugely powerful BOS within Oblivion's portal.*

5. [Better Auto-Walk OBSE](https://www.nexusmods.com/oblivion/mods/49105) (main file only)
6. [Blue's Engine Fixes](https://www.nexusmods.com/oblivion/mods/52700)
7. [Blockhead](https://www.nexusmods.com/oblivion/mods/43752)
8. [ConScribe](https://www.nexusmods.com/oblivion/mods/26510)
9. [Console Ignores Player](https://www.nexusmods.com/oblivion/mods/52721) (main file only)
10. [Console Numpad Support OBSE](https://www.nexusmods.com/oblivion/mods/49490) (main file only)
11. [Console Paste Support](https://www.nexusmods.com/oblivion/mods/49104)
12. [Crash Logger Improved](https://www.nexusmods.com/oblivion/mods/54527)(OLD FILES 1.6.0)
13. [Engine Bug Fixes](https://www.nexusmods.com/oblivion/mods/47085?tab=files) (main file only) \[**MI**\]
14. [Enchantment Cost Multiplier](https://www.nexusmods.com/oblivion/mods/50462)
15. [Faster Sleep Wait](https://www.nexusmods.com/oblivion/mods/50517) (main file only)
16. [Fractional Magic Damage](https://www.nexusmods.com/oblivion/mods/37717)

*Note the mod is not setup properly. Right click \ and select Create Directory. Call this 'OBSE', right click this once more & select Create Directory, naming it 'Plugins'. Lastly, drag the .dll into this folder & install.*

17. [Instant Continue Button](https://www.nexusmods.com/oblivion/mods/49545) (2nd main file)
18. [Jump While Blocking](https://www.nexusmods.com/oblivion/mods/49495)
19. [List Missing Mods on Load](https://www.nexusmods.com/oblivion/mods/52717?tab=files) (1st main file)
20. [Map Menu Doesn't Click While Dragging OBSE](https://www.nexusmods.com/oblivion/mods/50537)
21. [Menu Alt-Tab Crash Fix](https://www.nexusmods.com/oblivion/mods/47954)
22. [MenuQue - OBSE Plugin](https://www.nexusmods.com/oblivion/mods/32200?tab=files) (version 16.0beta)

*Install manually, right click Data & select Set as Data Directory. Within OBSE \> Plugins, deselect OBSE_Kyoma_MenuQue.dll & install.*

23. [Message Logger](https://www.nexusmods.com/oblivion/mods/45870)\[**MI**\]
24. [No Combat Music](https://www.nexusmods.com/oblivion/mods/52723)

*Oblivion's OST is timeless in my opinion & does not need modifying. However, its one caveat is the flawed implementation & handling of the combat music, hence we use this mod to counterract this.*

25. [No Inventory on Alt-tab](https://www.nexusmods.com/oblivion/mods/52716?tab=files)
26. [No Lockpick Activate](https://www.nexusmods.com/oblivion/mods/52719?tab=files)
27. [OBL Mod Limit Fix](https://www.nexusmods.com/oblivion/mods/50066?tab=files)
28. [OBSE -Elys- Universal Silent Voice](https://www.nexusmods.com/oblivion/mods/16622) (version 93)

*Similarly to mod 10, right click \ and select Create Directory. Call this 'OBSE', right click this once more & select Create Directory, naming it 'Plugins'. Lastly, drag all 4 files into this folder & install.*

29. [Oblivion Display Tweaks](https://www.nexusmods.com/oblivion/mods/50348?tab=files)

*Once installed, open the Ini files tab of MO2 and make the following edits:*

**bFPSFix** = 0
**iMaxFPSTolerance** = 120

*Note the second parameter is set due to our cap of 60fps applied in Oblivion Reloaded Combined installed later.

I also highly recommend playing in Borderless Windowed mode given this plugin safely provides that with the other OBSE plugins installed here as well, so disable bFull Screen in your Oblivion.ini via the Ini Editor in the Tools header of MO2.*

30. [Oblivion Priority](https://www.nexusmods.com/skyrimspecialedition/mods/50129?tab=files) (OLD FILES: Version 3.0.0)

*Note this mod is from the Skyrim portal, so download manually from the webpage & cut/paste to your download folder afterwards. Additionally, open the mod once installed & set the URL under 'Use Custom URL' in the Nexus Info tab.*

31. [Oblivion Magic Extender](https://www.nexusmods.com/oblivion/mods/31981)

*Install manually, then right click the Oblivion \> Data folder & select Set as Data Directory. **Disable** all the **\_example.esp**'s & **OBME_CS.dll**.

*32. [RuntimeEditorIDs](https://www.nexusmods.com/oblivion/mods/40132)
33. [SkyBSA](https://www.nexusmods.com/oblivion/mods/49568)

*Hugely important mod that also provides the Archive Invalidation function, hence we have this disabled in the Profiles menu of MO2.*

34. [WalkBlessed OBSE Plugin (diagonal move)](https://www.nexusmods.com/oblivion/mods/49067?tab=files) (main file only)
35. [Skyrim Camera for Walkblessed](https://www.nexusmods.com/oblivion/mods/51244)

## Part 3 - UNOFFICIAL PATCHES
1. [Unofficial Oblivion Patch](https://www.nexusmods.com/oblivion/mods/5296?tab=files) (main file only)

*Once installed delete UOP Vampire Aging & Face Fix.esp.*

2. [Unofficial Shivering Isles Patch](https://www.nexusmods.com/oblivion/mods/10739)
3. [Unofficial Oblivion DLC Patches](https://www.nexusmods.com/oblivion/mods/9969)4. [Unofficial Oblivion Tree Patch](https://www.nexusmods.com/oblivion/mods/53079)
5. [Unofficial Shivering Isles Tree Patch](https://www.nexusmods.com/oblivion/mods/53101)


## Part 4 - TWEAKS AND FIXES
1. [MigMaster Script Resources](https://www.nexusmods.com/oblivion/mods/45875)

*Once installed delete SafeCloningFunction - Filter.esp*

2. [Migck's Miscellaneous fixes tweaks and additions](https://www.nexusmods.com/oblivion/mods/42658)

*Be sure to read the modpage as this mod houses some very useful, as the separator suggests, tweaks and fixes! Once installed however make the following edit in MigMiscellanea.ini under the Misc section toward the bottom:*

**set zzzMigckQ.bBetterSkillup to 0** (simply change the boolean setting from 1 to 0)

3. [SB Weapon Drop Fix](https://www.nexusmods.com/oblivion/mods/50223?tab=files)

4. [SB - Skill Bonus Items (Fix for Skeleton Key - Gray Princes Training - and Night Mother's Blessing)](https://www.nexusmods.com/oblivion/mods/52984?tab=files) (2nd main file, 'unbreakable')
5. [Vile Lair DLC - Tweaks and Fixes](https://www.nexusmods.com/oblivion/mods/52213?tab=description)
6. [Clickable Magic Gate](https://www.nexusmods.com/oblivion/mods/51375)
7. [Market District Landscape Fix and Imperial City Landscape Fix](https://www.nexusmods.com/oblivion/mods/50770?tab=files) (1st main file only)
8. [Thieves Den Barter For Upgrades](https://www.nexusmods.com/oblivion/mods/48226?tab=files)
9. [No Annoying Conjurer Attack (Spell Tomes DLC fix)](https://www.nexusmods.com/oblivion/mods/47452?tab=files)
10. [Goblin Tribes Fixed](https://www.nexusmods.com/oblivion/mods/52252)
11. [Less Maddening Shivering Isles Fetch Quests -- Place Matrices and Oddities in World](https://www.nexusmods.com/oblivion/mods/52621)
12. [Lava Does Fire Damage](https://www.nexusmods.com/oblivion/mods/43046)
13. [Bibliophilia](https://www.nexusmods.com/oblivion/mods/51251)
14. [Knights of the Nine - Improved Infamy System](https://www.nexusmods.com/oblivion/mods/50195?tab=files) (main file only)
15. [Guard Infamy Greeting Fix](https://www.nexusmods.com/oblivion/mods/52249)
16. [Collection of Cleaned - Updated - Fixed - UOP Compatible Mods](https://www.nexusmods.com/oblivion/mods/52833) (Ayleid Well Message Instead of Messagebox - UOP Compatible)

*Once installed delete ChapelMessageNotBox.esp.*

17. [DLC Lore Books](https://www.nexusmods.com/oblivion/mods/46715)

*Within the BAIN Package installer, select 00 Merged only.*

18. [Creature Fix Compendium](https://www.nexusmods.com/oblivion/mods/50100)
19. [Mesh Improvement Project](https://www.nexusmods.com/oblivion/mods/44501?tab=files) (main file only)

*Within the BAIN Package Installer, select just 00 Core.*

20. [No Havoc Objects](https://www.nexusmods.com/oblivion/mods/46593) (all four files, install separately)
21. [Walk through Oblivion Gates](https://www.nexusmods.com/oblivion/mods/54076)
22. [Locked Fighters Guild Doors Bug Fix](https://www.nexusmods.com/oblivion/mods/53345)
23. [Uriel Septim Audio Cleanup](https://www.nexusmods.com/oblivion/mods/54428)
24. [Skingrad Statue Improved](https://www.nexusmods.com/oblivion/mods/47055)
25. [Leyawiin Statue Base Mesh Fix](https://www.nexusmods.com/oblivion/mods/54381)
26. [UOP Talos Bridge Collision Fix](https://www.nexusmods.com/oblivion/mods/53283)
27. [Minotaur Horn Drop Fix](https://www.nexusmods.com/oblivion/mods/54893?tab=description) (default version)
28. [Mind your head - signs repositioned](https://www.nexusmods.com/oblivion/mods/52551)

## Part 5 - LOD
1. [Evenstars Colourwheel LOD Update](https://www.nexusmods.com/oblivion/mods/42190) \[**DP**\]

*Within the BAIN Package installer, select:

*

- 00 Textures
- 04 Statues and Shrines

*
Once installed, pack the mod using **BSArch** & name both the BSA & your dummy plugin as **Evenstars Colourwheel LOD Update**, then delete the loose meshes & textures folders.*

2. [VWD For Leyawiin](https://www.nexusmods.com/oblivion/mods/50052)
3. [VWD For Townhouses](https://www.nexusmods.com/oblivion/mods/50073?tab=files)
4. [VWD Ships](https://www.nexusmods.com/oblivion/mods/50111) (2 VWD Ships - KatKat74's Textures)
5. [J3 Atlassed VWD 2](https://www.nexusmods.com/oblivion/mods/51732?tab=files) (main file: J3 Atlassed VWD 2 - Cyrodiil - BAIN installer)

*Within the Options menu of the BAIN Wizard Installer, select 'Performance (no rocks)'*

6. [J3 Atlassed VWD 2](https://www.nexusmods.com/oblivion/mods/51732?tab=files) (optional file: J3 Atlassed VWD 2 - Shivering Isles - Bomret's Texture Pack for Shivering Isles)
7. [Bruma Frostcrag Spire LOD](https://www.nexusmods.com/oblivion/mods/49898?tab=files) (optional file)
8. [Landscape LOD Textures by Xerus](https://www.nexusmods.com/oblivion/mods/17300?tab=files)

- Install manually, and expand the Landscape LOD Textures by Xerus \> Shivering Isles folder.
- Next, drag the Textures folder here over the Cyrodiil folder
- Right click the Cyrodiil folder & select Set as \ directory

9. [Imperial City LOD - Performance Edition](https://www.nexusmods.com/oblivion/mods/54402?tab=files) (main file only)

10. (Create Empty Mod) Merged LOD

*You can optionally create an empty mod for running slowLODGen by yourself later, or simply use my download on this modpage installed in the same position.*

## Part 6 - UI & UX IMPROVEMENTS
1. [T4UT - Menus Repolished](https://www.nexusmods.com/oblivion/mods/54904?tab=files)

2. [Link Plus Plus](https://www.nexusmods.com/oblivion/mods/53352?tab=description)
3. [DarnifiedUI FOMOD Conversion](https://www.nexusmods.com/oblivion/mods/50176?tab=posts)

*Read the instructions on the modpage **carefully** - alternatively, within the posts tab of the mod **OBLI33**'s thread (where you can see I posted too) covers the installation in more detail. This process ensures we have DarnifiedUI working as intended within MO2.

Within the fomod, simply accept all default settings (all Select Components enabled; Normal font size & Default Font1 Options; no Custom Options enabled).

Be sure to also make the Oblivion.ini edits otherwise the fonts ingame will be incorrect. I'd recommend firing up the game just to ensure you've done it correctly after this. For ease of reference, they are here:

*
\[Fonts\]
SFontFile_1=Data\Fonts\Kingthings_Regular.fnt
SFontFile_2=Data\Fonts\DarN_Kingthings_Petrock_14.fnt
SFontFile_3=Data\Fonts\DarN_Kingthings_Petrock_16.fnt
SFontFile_4=Data\Fonts\DarN_Oblivion_28.fnt
SFontFile_5=Data\Fonts\Handwritten.fnt*

*

*

Within MO2, I simply called my final mod 'DarNified UI MO2 READY' and copy/pasted to my downloads folder for future safekeeping.

*

**Ultrawide Users only:**
*From within your final output mod, open Menus/loading_menu.xml with an editor of your choice (e.g. Notepad++).*
*
Search for \

Given I'm on a 22:9 monitor, I applied the following edits (note, without the 'This changes the aspect ratio' text of course!):*

``
``
``
` 0 `
``
`  `
` 1000 `
``
``
``
` 2 `
``
``
``
``
` 2 `
``
``
``
``
` 22 ``` 9 `` ``

*So why DarnifiedUI after all these years still? Simply put, Oblivion's UI is timeless. Changing it for more (vanilla) Skyrim-esque layouts is a regression in my opinion, given the UI was designed for console-first. Its elegant aesthetic is kept intact for this build.*

4. [UHD Fonts for Darnified UI](https://www.nexusmods.com/oblivion/mods/49266?tab=files) (main file)
5. [Darnified Animated Menus](https://www.nexusmods.com/oblivion/mods/50071?tab=files)

*Within the BAIN Package Installer, select just 10 - Core Module - Main*

6. [Achievements for Oblivion](https://www.nexusmods.com/oblivion/mods/52923?tab=files)

7. [Dynamic Map](https://www.nexusmods.com/oblivion/mods/35969?tab=files)

*Within the BAIN Package Installer, select:

*

- 00 - Core
- 01 Elven Map

*Once installed, open the INI Files tab in MO2 & apply the following:*

set tnoDM.zoomIn to 264
set tnoDM.zoomOut to 265
set tnoDM.zoomReset to 258

*This ensures the mouse-wheel movements control the zoom.*

8. [VKVII Oblivion Cyrodiil Map](https://www.moddb.com/mods/vkvii-oblivion-cyrodiil-map)

9. [Shivering Isles Map HD](https://www.nexusmods.com/oblivion/mods/45768?tab=description) (first main file)

*Once installed, ensure you add the 13 stated lines in the description page to the bottom of Dynamic Map.ini.*

10. [World Maps for All Worldspaces](https://www.nexusmods.com/oblivion/mods/48914)

11. [Marking the Landmarks](https://www.nexusmods.com/oblivion/mods/48892) \[**QAC**\]
12. [Map Marker Overhaul](https://www.nexusmods.com/oblivion/mods/26389) Within the BAIN Wizard Installer:

**Select Icon Style**: Elven Map Redux Options
**Ini File Installation Wizard**: Default Settings

*Once installed, open Map Marker Overhaul.ini in the INI Files tab & at line 156/7 set '**set tnoMMO.visibleDistance**' to 0.

Quite simply the best map marker mod on Nexus. Remember to read the modpage to grasp its capabilities. I really make use of holding CTRL and using the varying options available such as setting locations as 'Done' as an example.*

13. [Unknown Undiscovered Colored Map Markers](https://www.nexusmods.com/oblivion/mods/50326?tab=files) (Unknown Colored Map Markers for Map Marker Overhaul (MMO))
14. [Loot Menu](https://www.nexusmods.com/oblivion/mods/48027?tab=files) (main file & install Smaller Font separately)
15. [Loot Feed](https://www.nexusmods.com/oblivion/mods/52763) (main file + My Universal Fonts, install separately)

*Both 'Loot' mods are relatively new on the Oblivion modding landscape, and really help take the game's modernity to the next level in terms of UI & UX, proving familarity with modern titles is never a bad thing.*

16. [Follower Status](https://www.nexusmods.com/oblivion/mods/53074)

*Once installed, open Follower Status - Config.ini in the INI Files tab & apply the following:*

`set dsFSQ.iVisibility to 1`

17. [Extended UI](https://www.nexusmods.com/oblivion/mods/50135) (main file only)

*Once installed apply the following edits to ExtendedUI.ini in the INI Files tab of MO2:*

**set ExUI.bEnableSpellFavourites** to 0
**ExUI.bEnableCyclingQuicksave** to 0

18. [QZ Easy Menus Update](https://www.nexusmods.com/oblivion/mods/23404)

*Once installed, open the INI Files tab of MO2 and in QZ Easy Menus.ini, comment out set **EasyMenu.iAltExitKey** by adding a semi-colon at the beginning of the line.*

19. [Display Stats](https://www.nexusmods.com/oblivion/mods/31855)
*Within the BAIN Package Installer, choose 00 CORE + 01 Darnified UI.*

20. [Stats Checker](https://www.nexusmods.com/oblivion/mods/44844)
21. [Pick Me](https://www.nexusmods.com/oblivion/mods/48479?tab=files) (main file only)
22. [Enhanced Hotkeys](https://www.nexusmods.com/oblivion/mods/34735) (main file only)

*Again, another Best-on-Nexus mod in my opinion. In terms of QoL there really are no substitutes. As an example, I like to have my main weapons cycled using 1, a touch-only usage of 2 for healing, cycling destruction spells with 3, and so forth. Get creative as this mod truly is a one-of-a-kind.*

23. [Book Tracker Updated](https://www.nexusmods.com/oblivion/mods/49195?tab=files) (first main file only) \[**MI**\]
24. [Better Enemy Health](https://www.nexusmods.com/oblivion/mods/53605?tab=description)

*Once installed apply the following edits in the config in the INI files tab within MO2:*

**set dsEHMainQ.iEnableExtraBars** to 1**
set dsEHMainQ.iDisableVEH** to 1

25. [Icons for Alchemy Apparatus](https://www.nexusmods.com/oblivion/mods/43782?tab=files) (main file only)
26. [Quest Log Manager](https://www.nexusmods.com/oblivion/mods/32266)

*Within the BAIN Package Installer, choose:

*

- 00 CORE
- 01 Darnified UI

*
*27. [Dot Crosshair](https://www.nexusmods.com/oblivion/mods/47325?tab=files) (smaller version)
28. [Diverse Effect Icons](https://www.nexusmods.com/oblivion/mods/10254)

*Once installed delete every esp.*

29. [Diverse Effect Icons OBSE](https://www.nexusmods.com/oblivion/mods/49511)
30. [Better Letters](https://www.nexusmods.com/oblivion/mods/5392) \[**MI**\] \[**QAC**\]
31. [Better Scroll Background](https://www.nexusmods.com/oblivion/mods/27882)
32. [Vanilla Style Loading Screens Addon](https://www.nexusmods.com/oblivion/mods/50014?tab=files) (optional file)

*Within the BAIN Package Installer, choose 00 CORE*

33. [Upscaled Vanilla Style Loading Screens and MOO Themed Loading Screens - 4k and 2k versions](https://www.nexusmods.com/oblivion/mods/51109?tab=files) (2k - Vanilla Style Loading Screens Addon and Vanilla Style MOO Themed Loading Screens)

*Within the BAIN Package Installer, select both MOO Themed Loading Screens + Vanilla Style Loading Screens

*

## Part 7 - CHARACTER & NPCs
*You'll notice a huge fraction of this part is from Dispensation's guide. I'll freely admit it was a reference tool for this particular section given it's quite simply flawless.*

1a. [Oblivion Character Overhaul version 2](https://www.nexusmods.com/oblivion/mods/44676?tab=files) \[**MI**\] (Main file only)

*During MI, deselect Oblivion_Character_Overhaul.esp*

1b. [Oblivion Character Overhaul - Advanced Edition](https://www.nexusmods.com/oblivion/mods/52010) (Oblivion Character Overhaul - Advanced Edition (OCOv2 Hairstyles)

2. [AI Enhanced - Oblivion Character Overhaul version 2](https://www.nexusmods.com/oblivion/mods/52135)

*Once installed, delete/hide the 'textures\characters\nuska\hair' folder.*

3. [Ragdolls for Oblivion](https://www.nexusmods.com/oblivion/mods/51844) (main file & optional file, install separately)
4. [Light compatible Skeleton](https://www.nexusmods.com/oblivion/mods/49080) (version 5)

- Install manually & right click \ and select Create Directory. Call this 'meshes'.
- Right click this new meshes folder & once more Create (a new) Directory, naming it 'characters'.
- Lastly, drag the \_1stperson & \_male folders into the characters folder, and install.

5. [Seamless - OCOv2](https://www.nexusmods.com/oblivion/mods/45859) (Main file first)

*Once installed, hide or delete:

*

- EVE_ShiveringIslesEasterEggs.esp
- EVE_StockEquipmentReplacer.esp
- Meshes \> characters \> Argonian

*
*6. [Seamless - OCOv2](https://www.nexusmods.com/oblivion/mods/45859) (SR- OCOv2 Nudes)
7. [Seamless - OCOv2](https://www.nexusmods.com/oblivion/mods/45859) (SR- OCOv2 Wider Chin Dunmer)
8. [New Brows for OCO v2](https://www.nexusmods.com/oblivion/mods/48199)
9. [Detailed Mouth for OCOv2](https://www.nexusmods.com/oblivion/mods/47463)[(MO2 link)](https://www.nexusmods.com/oblivion/mods/47463?tab=files&file_id=1000015727&nmm=1)
10. [Oblivion Texture Overhaul - Realistic Mouth ( Teeth plus )](https://www.nexusmods.com/oblivion/mods/51720)

11. [New Eyes for OCO v2](https://www.nexusmods.com/oblivion/mods/46995) (main file)

12. [Beards in Tamriel](https://www.nexusmods.com/oblivion/mods/47071)
13. [NPC Hair Matches Beard - Updated](https://www.nexusmods.com/oblivion/mods/49861) (main file, and also the 'Beards in Tamriel - Optimised Meshes' optional file, install separately)
14. [OCOv2 Male Beard](https://www.nexusmods.com/oblivion/mods/48816)

15. [OCOv2 Uses Merged Teeth](https://www.nexusmods.com/oblivion/mods/48378) \[**MI**\]
16. [Distinct Nord Stubbles for OCOv2](https://www.nexusmods.com/oblivion/mods/50906)17. [Lifelike Eye Normalmaps](https://www.nexusmods.com/oblivion/mods/47467) (1st main file only)
18. [Warpaints scars and face markings for OCO2](https://www.nexusmods.com/oblivion/mods/51511) (main file, then 'Argonian and Khajiit patch' & lastly 'Argonians patch for Seamless mod'; install separately in that order)

19. [For OCOv2 - Reposition Teeth For All Races](https://www.nexusmods.com/oblivion/mods/51625) (main file only)

*Within the BAIN Package Installer, select:

*001 core

20. [OCO 2 glowing nostrils fix](https://www.nexusmods.com/oblivion/mods/46342)
21. [OCOv2 Enhanced Beast Races patch](https://www.nexusmods.com/oblivion/mods/47866?tab=files) (main file) \[**MI**\]*

Once installed, hide or delete:*

- OCOv2 Beast Races Enhanced.esp
- Textures \> Characters \> Argonian \> Female
- Textures \> Characters \> Argonian \> Male
- Textures \> Characters \> Khajiit \> Female
- Textures \> Characters \> Khajiit \> Male
- Textures \> Characters \> Khajiit \> earkhajiit.dds (x2)
- Textures \> Characters \> Nuska \> Khajiit \> headkhajiit (x2 files)
- Meshes \> Characters \> argonian \> headargonian.egt

22. [Claws whiskers and Seamless tails only](https://www.nexusmods.com/oblivion/mods/47866?tab=files) (from above modpage; optional file)

*Once installed, hide or delete:*

- meshes \> characters \> bodyassestoverrides \> perrace \> F \> Argonian_tail.nif

23. [Enhanced Beast Races HGEC Argonian patch](https://www.nexusmods.com/oblivion/mods/51542)
24. [Enhanced Beast Races alternate male Argonians](https://www.nexusmods.com/oblivion/mods/51685)
25. [Improved NPC Faces for OCOv2](https://www.nexusmods.com/oblivion/mods/51971) (main file only)
26. [Oblivion Character Overhaul v2 - DLC Addon](https://www.nexusmods.com/oblivion/mods/52405)
27. [Unused OCOv2 Eyes and DLC Characters Incorporated](https://www.nexusmods.com/oblivion/mods/52052)
28. [OCOv2 Baurus tweak](https://www.nexusmods.com/oblivion/mods/48370) \[**MI**\]
29. [Sirens Deception Beautified](https://www.nexusmods.com/oblivion/mods/42291)
30. [Miscellaneous Patch Collection by Dispensation](https://www.nexusmods.com/oblivion/mods/52874?tab=files)

*Install just: **Oblivion Character Overhaul version 2 patches**.* *I'd also recommend naming the mod this way as will install this download multiple times.*
*
Finally, delete **DispMiscPatch_OCOv2 - VKVII Argonian and Khajiit Patch.esp.***

31. [OCOv2 Eyelash Mesh Fix](https://www.nexusmods.com/oblivion/mods/53752)
32. [Character Customization Expanded](https://www.nexusmods.com/oblivion/mods/53860)

33. [Improved Vanilla Hairs](https://www.nexusmods.com/oblivion/mods/49078?tab=files) (Full textures & Mesh Improvements; install separately)

*I found mod-added hair replacers a little wacky; all we truly need for Hair is a decent uptick in detail.*

## Part 8 - OVERHAULS: MASKAR
*This needs no introduction. Paired with Oscuro's below, this combination truly takes the game to the next level. Many may deem it 'hardcore' however we take certain steps during the guide to mitigate some of the overly harsh challenges the combo provides.

There is a gigantic pdf outlining the mod's features so if not aware of this mod's capabilities, do give it a read.*

1. [Maskar's Oblivion Overhaul](https://www.nexusmods.com/oblivion/mods/42780?tab=files) (OLD FILES 4.9.4.2)

*A fair few ini edits are needed, so I'd recommend opening the mod in explorer to search for them. WIthin the main mod's ini, apply the following:*
`set MOO.ini_levelscaling_reduced to 1`
`set MOO.ini_ability_climb_npc to 0`
`set MOO.ini_dungeon_light to -1`
`set MOO.ini_torch_brightness to -1`
`set MOO.ini_companion_disposition_min to 70`
`set MOO.ini_damage_combat to 1.5`

`set MOO.ini_add_redpanda to 0`

`set MOO.ini_weight_repairhammer to 0`
`set MOO.ini_weight_lockpick to 0`
`set MOO.ini_weight_skeletonkey to 0`
`set MOO.ini_weight_herderscrook to 5`
`set MOO.ini_weight_magnifyingglass to 0`

`set MOO.ini_spell_light to -1`

`set MOO.ini_ability_equipment to 0`

`set MOO.ini_disease_groups to 0`
`set MOO.ini_disease_creatures to 0`
`set MOO.ini_disease_airborne to 0`
`set MOO.ini_disease_shader to 0`
`set MOO.ini_disease_airborne_outdoors to 0`
`set MOO.ini_disease_airborne_dungeon to 0`
`set MOO.ini_disease_airborne_player to 0`
`set MOO.ini_disease_airborne_creature to 0`

`set MOO.ini_compatibility_tools_convert to 1`

`set MOO.ini_ability_heal to 0`
`set MOO.ini_magic_healing_undead to 0`

`set MOO.ini_levelscaling_npc_overridden to 2`

`set MOO.ini_add_bandits to 0`
`set MOO.ini_add_conjurers to 0`
`set MOO.ini_add_necromancers to 0`
`set MOO.ini_add_marauders to 0`
`set MOO.ini_add_mythicdawn to 0`
`set MOO.ini_add_vampires to 0`

`set MOO.ini_add_ancientvampires to 0`
`set MOO.ini_add_outlaws to 0`
`set MOO.ini_add_plunderers to 0`
`set MOO.ini_add_plaguebringers to 0`
`set MOO.ini_add_planesummoners to 0`

*The changes are mostly due to compatibility with other mods, consistency with other aspects of this load order, and increasing stability.

**NOTE**: If updating for 03.25, the changes added are from set.MOO.ini_ability_heal & below.*

2. [MOO - Non-Elder Scrolls Franchise Recolors](https://www.nexusmods.com/oblivion/mods/54665?tab=files)
3. [Hill Giant Eye Fix - Loreless Creatures - MOO](https://www.nexusmods.com/oblivion/mods/54664?tab=files) (2nd main file)

4. [Basic Harvest](https://www.nexusmods.com/oblivion/mods/51833) *Install manually, and right click:

BasicHarvest_FilterPatch_V1.4 \> **01_MOO_DefaultProbabilities** & select Set as \ Directory*

5. [MOO Themed Loading Screens](https://www.nexusmods.com/oblivion/mods/48858?tab=files) (MOO Loading Screens - New Pics Vanilla Style)

*Once installed, delete the textures folder.*

6. [Smaller MOO Backpacks](https://www.nexusmods.com/oblivion/mods/49858?tab=files) (first main file)
7. [Item Description Framework for Maskar's Oblivion Overhaul](https://www.nexusmods.com/oblivion/mods/50670) (install both main files separately)

*Note for MOO Item Description Weapons, create two directories: Menus \> Strings & place the WEPON_strings.xml file in the Strings folder.*

8. [Seamless Equipment - MOO](https://www.nexusmods.com/oblivion/mods/49898?tab=files)
9. [OCOv2 - MOO Patch](https://www.nexusmods.com/oblivion/mods/51379?tab=files) (optional file)
10. [MOBS patch for Maskar's Oblivion Overhaul](https://www.nexusmods.com/oblivion/mods/47473)

## Part 9 - OVERHAULS: OSCURO
*Some may argue Oscuro's is outdated with Maskar's on the landscape. I beg to differ! The ingame 'handshake' between the two is almost seamless, with Maskar's providing features & functionality, using Oscuro's unforgiving yet hugely rewarding rewrite of the game's gameplay mechanics.

Again, if unfamiliar with this combination, rethink your perceptions of the game. Indeed if you've never played Oblivion before, then treat this gameworld lightly! I feel all great RPGs should be punishing at early levels, yet not too unfair to lure the player into a sense of achievement & progression when the proverbial 'barriers' are broken through. There are a whole host of ways to survive & thrive through the early game, and with certain mods & tweaks in this build, the challenges of Oscuro & Maskar's mods are lessened for a more forgiving but still challenging experience.*

1. [OOO - Oscuro's Oblivion Overhaul - Updated](https://www.nexusmods.com/oblivion/mods/46199?tab=files) (Oscuro's Oblivion Overhaul BSA)

Once installed, using [BAE](https://www.nexusmods.com/skyrimspecialedition/mods/974?tab=files), extract the archive to the mod's location. We'll pack the loose sound files here & repack once they're installed.

2. [OOO esps](https://www.nexusmods.com/oblivion/mods/46199?tab=files)
3. [OOO Voice FIles](https://www.nexusmods.com/oblivion/mods/46199?tab=files)

*Open mod 1 & 3 in explorer, and paste the voice files into mod 1. Then repack using BSArchPro saving over the existing archive & delete the loose files remaining in mod 1. Lastly, disable mod 3 given they've been archived.*

4. [OOO - KotN Patch](https://www.nexusmods.com/oblivion/mods/46199?tab=files)

5. [OOO Flavor Text for Extended UI](https://www.nexusmods.com/oblivion/mods/51781)\[**MI**\]
6. [Seamless - Robert Male](https://www.nexusmods.com/oblivion/mods/45864?tab=files) (optional file SE- Robert Male v5.2 all-in-1 Addons

*Within the BAIN Package Installer, select 50 Robert v5 Muscular OOO*

7. [EVE HGEC Eyecandy Variants Expansion](https://www.nexusmods.com/oblivion/mods/24078?tab=files) (EVE for Oscuro Oblivion Overhaul 1_3 BAIN) \[**QAC**\]

*Within the BAIN Package Installer, select:*

- 00 Core
- 10 Equipment Replacer Upperbody - Normal C-Cup
- 15 Equipment Replacer Lowerbody - Normal

8. [Seamless - HGEC Female](https://www.nexusmods.com/oblivion/mods/45858?tab=files) (optional SE- HGEC OOO 24078)

*Within the BAIN Package Installer, select:*

- 00 Core
- 10 Equipment Replacer Upperbody - Normal C-Cup
- 15 Equipment Replacer Lowerbody - Normal

9. [OOO Shivering Isles](https://www.nexusmods.com/oblivion/mods/46508?tab=files) (main file + OCO Compatibility Patch + Exnem and HGEC Models - **install separately**)

*From the main file, move **OOOShiveringIsles_Optional_CrucibleEdits.esp** to the optional folder.*

10. [Visually Improved Staffs for OOO](https://www.nexusmods.com/oblivion/mods/41477)

11. [OOO Enhanced](https://www.nexusmods.com/oblivion/mods/47187?tab=files) (both main files: **5.3 - PreRelease** & **5.3b Resources**; install separately)

*Within the BAIN Package Installer, select:*

- 00 Base
- 10 DLCs
- 11 DLCs - Frostcrag (Vanilla)
- 12 DLCs - Battlehorn Castle (Vanilla)
- 20 Knights of the Nine (OOO Enhanced)
- 30 Shivering Isles (REQUIRES OOO SI)
- 80 Av Latta Magicka

*After several months of liaising with the author, this is a truly great addon for Oscuro & a key part of 03.25's update. Be sure to read the modpage to grasp what's being changed, added & improved upon. Note we will return to this install of the Resources to optimise the build later, a REMINDER! will be added once Colourful Clothing Collection in Part 24 has been concluded.

As a further reminder, ensure 'Enable Parsing of Archives' is enabled as per setup in MO2's Settings.

Post-install steps for OOO Enhanced:

*

- Open the conflicts tab & select 'Providing Mod' within Losing file conflicts
- Select all files under 'AI Enhanced - Colourful Clothing - Upperclass + Middleclass'
- Right click & hide
- Perform this for Colourful Clothing - Collection - Seamless OCOv2
- Select 'Overwritten mods' in the Winning File conflicts tab
- Select all files under 'Waalx's Animals and Creatures'
- Right click & hide
- Select the Filetree tab & Open mod in explorer; search for 'mohidden' & delete the files

*
Lastly, BSArch the textures folder naming it 'OOO Enhanced.bsa' & delete the loose textures folder.

*

## Part 10 - OVERHAULS: WAC
*Waalx's mod touches more than just populating the landscape with beautiful new animals & creatures. A healthy amount of new gear is
also added.*

1. [WAC Waalx Animals & Creatures](https://tesalliance.org/forums/index.php?/files/file/1318-wac-waalx-animals-creatures/&changelog=1175) (Download this file \> WACv_1beta.7z only)

*Install manually & deselect everything except:*

- Waalx Animals & Creatures.esm
- WAC.bsa

*Once installed, rename the WAC BSA file to WACIntegration. This ensures the handshake with the following mod's plugin.*

2. [WAC - Integration](https://www.nexusmods.com/oblivion/mods/51102?tab=files)*

*

- *Within the BAIN Package Installer, select:*

*
*

- 00 Core
- 01 Maskar's Oblivion Overhaul INI Files

3. [HGEC Equipment Replacer for WAC](https://www.nexusmods.com/oblivion/mods/37594?tab=files&file_id=87752&nmm=1)

*Within the BAIN Package Installer, select 00 Data only.*

4. [WAC - Integration - Roberts Conversion](https://www.nexusmods.com/oblivion/mods/53333)
5. [WAC - Integration - HGEC Gauntlets Conversion](https://www.nexusmods.com/oblivion/mods/53357)

## Part 11 - BASELINE TEXTURES
*For users of my MOFAM: FO4 guide, our process with the following OUT mods will be familiar. Create the empty mod 'OUT Essentials' and download & install mods 1-5 whilst copying their contents to this empty mod. All of these are within the Optional downloads, and I simply rename them this way whilst installing them into MO2 for ease of reference.*

1. [Oblivion Upscaled Textures (OUT) - 2x Clothes](https://www.nexusmods.com/oblivion/mods/49351)
2. [Oblivion Upscaled Textures (OUT) - 2x Weapons](https://www.nexusmods.com/oblivion/mods/49351) *(note for this one there are two downloads, pick 'yeeeet' - although they're both seemingly indentical)*
3. [Oblivion Upscaled Textures (OUT) - 2x Armor](https://www.nexusmods.com/oblivion/mods/49351)
4. [Shivering Isles Upscaled Textures (SIUT) - 2x Clothes](https://www.nexusmods.com/oblivion/mods/49645)
5. [Shivering Isles Upscaled Textures (SIUT) - 2x Weapons](https://www.nexusmods.com/oblivion/mods/49645)
6. OUT Essentials \[**DP**\]

*Once all of mods 1-5 have been copied here, pack the mod through BSArch. Name both the bsa & your dummy plugin as '**OUT Essentials**', then delete the loose folders. I also delete mods 1-5 after this step to save on instance space, however that's optional.*

7. [OUT Dungeons](https://www.nexusmods.com/oblivion/mods/49351?tab=files) (Kart_OUT_dungeons_2x)
8. OUT - Dungeons \[**DP**\]

*Similarly to the process for Mods 1-6, pack this mod using BSArch the provided dummy plugin & ensure the bsa naming matches '**OUT - Dungeons**' then delete mod 7 once concluded.*

9. [Really Textured Normal Maps - Updated](https://www.nexusmods.com/oblivion/mods/50088) \[**DP**\]

*Once installed, pack the mod through BSArch. Name both the bsa & your dummy plugin as '**Really Textured Normal Maps**', then delete the loose textures folder.*

10. [Bomret's Texture Pack for Shivering Isles v1 with meshes from USIP](https://www.nexusmods.com/oblivion/mods/46162) \[**DP**\]

*Once installed, pack the mod through BSArch. Name both the bsa & your dummy plugin as '**Bomret's Texture Pack for Shivering Isles v1 with meshes from USIP**', then delete the loose meshes & textures folders.*

11. [DLC Upscaled Textures (DLCUT)](https://www.nexusmods.com/oblivion/mods/49798?tab=files) (kart_DLCUT_2x & kart_KNUT_2x; install **separately**)

12. [AI Powered Landscape Retexture](https://www.nexusmods.com/oblivion/mods/53871) \[**DP**\] *The mod has been packaged incorrectly so during Install:*

- Right click Data & select Create directory
- Name this 'Textures'
- Right click 'Textures' & select Create Directory
- Name this 'Landscape'
- Drag all the files into 'Landscape'

*Once installed, pack the mod through BSArch. Name both the bsa & your dummy plugin as '**AI Powered Landscape Retexture**', then delete the loose textures folder. Superb landscape mod that ticks all the relevant boxes.*

13. [Daydream - Grass Texture Atlas](https://www.nexusmods.com/oblivion/mods/49346) (main file + update, install **separately**)

*Previously to Daydream I used [LowPoly Grass](https://www.nexusmods.com/oblivion/mods/5434) and was thrilled to see this mod accomplishes the same only with improved aesthetics, fitting the following mod as well.*

14. [Let there be Flowers](https://www.nexusmods.com/oblivion/mods/49616?tab=files) (main file only)

*Once installed, using the Ini Editor tool in MO2, ensure '**iMaxGrassTypesPerTexure'** is set to 5 & the two **fGrassEnd/StartDistance** are set thusly:

*
`[Grass]`
`bDrawShaderGrass=1`
`bGrassPointLighting=0`
`fGrassEndDistance=8000.0000`
`fGrassStartFadeDistance=7000.0000`
`fGrassWindMagnitudeMax=125.0000`
`fGrassWindMagnitudeMin=5.0000`
`fTexturePctThreshold=0.3000`
`fWaveOffsetRange=1.7500`
`iGrassDensityEvalSize=2`
`iMaxGrassTypesPerTexure=5`
`iMinGrassSize=80`

15. [Improved Doors and Flora](https://www.nexusmods.com/oblivion/mods/8298?tab=files) (2nd main file)\[**MI**\]
16. [IDF Update](https://www.nexusmods.com/oblivion/mods/8298?tab=files)

17. [Improved Trees and Flora](https://www.nexusmods.com/oblivion/mods/8500?tab=files)\[**MI**\]
18. [Improved Trees and Flora](https://www.nexusmods.com/oblivion/mods/11891) 2 (1st main file)
19. [ITF2Update](https://www.nexusmods.com/oblivion/mods/11891?tab=files)

*Once installed, hide Meshes \> Plants \> bwcattail01 + 02.nif's.*

20. [Arboretum - retexture for a tree-hugging crowd](https://www.nexusmods.com/oblivion/mods/45521?tab=files) (main file only)

*Fantastic update to this mod essentially deprecates the long-time usage of
Enhanced Vegetation. Enjoy richer & more varied trees and foliage
whilst remaining true to the original art direction.*

21. [Oblivion 2020 Retexture Project](https://www.nexusmods.com/oblivion/mods/49933?tab=files) (optional file: 2020 Retexture Project - Tree Shadows)
22. [HD Photorealistic Ivy by greenback12 for Oblivion](https://www.nexusmods.com/oblivion/mods/54754) **\[MI\]** (MIXED)

23. [Harvest Flora](https://www.nexusmods.com/oblivion/mods/2037?tab=files) *Once installed:

*

- Delete the NoMushroomStalks folder
- Run Harvest \[Flora\] - DLCFrostcrag.esp through \[**QAC**\]
- Run Harvest \[Flora\] - Shivering Isles.esp through \[**QAC**\]
- Open Harvest \[Flora\] - DLCFrostcrag.esp in xedit & remove the **Worldspace** group.

24. [Improved Flora Harvest Fix](https://www.nexusmods.com/oblivion/mods/20109)
25. [TreeOpt](https://www.nexusmods.com/oblivion/mods/49150)

26. [Rocks Retexture](https://www.nexusmods.com/oblivion/mods/45881?tab=files) (1k)
27. [2020 Retexture Project - Landscapes and Rocks](https://www.nexusmods.com/oblivion/mods/49933?tab=files) *We use a very small fraction of this mod to focus on the coast-side rocks, so once installed delete the following:*

- meshes

- textures \> dungeons
- textures \> landscape
- textures \> rocks \> everything **except** underwater folder

28. [Nice Ice a.k.a. The Hills Have Ice](https://www.nexusmods.com/oblivion/mods/45342)

*'You call this a baseline?' I hear you cry.

I've found after extended gameplay sessions the engine simply cannot take large-scale 2k retexture baselines, mods with 1000's of loose files & begins to suffer. Mild stutters turn into judders, and with engine memory optimisation from OBSE & ORC (installed later), my focus is **performance**.

You'll be pleasantly surprised how great the game still looks with the various retexture-parts we install.

An important point to share is the **far majority of our retexturing is in 1k**. Only in interiors is 2k used, given how the engine & this Load order handles cell-changes & memory management.*

## Part 12 - WEATHER & LIGHTING
1. [Weather - All Natural](https://www.nexusmods.com/oblivion/mods/18305?tab=files) All Natural - Real Lights.esp: \[**QAC**\]

- Select 'All Natural - Real Lights ONLY'

*Did you read the Notes? Make sure to manually rename the bsa to 'All Natural - Real Lights'!*

2. [Weather - All Natural Real Lights - Candelabra pathgrid fix](https://www.nexusmods.com/oblivion/mods/46854)

3. [NAO - Natural and Atmospheric Oblivion](https://www.nexusmods.com/oblivion/mods/50923)

- Select '0 \[CORE\] Natural and Atmospheric Oblivion' \> Right click 'Data' & set as your directory.

*Enjoy a newer, more performant & richer Weather mod to accompany the new enb we'll install later.*

4. [Atmospheres 2021 - Drifting mist for Vanilla setup](https://www.nexusmods.com/oblivion/mods/51592?tab=files) (optional file)

*Once installed move drifting mist.esp to the Optional folder.*

5. [drifting mist lleyawiin missing house fix](https://www.nexusmods.com/oblivion/mods/51592?tab=files) \[**QAC**\]
6. [Atmospheres 2021 - Additional weather effects](https://www.nexusmods.com/oblivion/mods/51592?tab=files) (optional file)

*These two additions from Atmospheres breathe life into the worldspace with improved placements of mists over the previously used 'Oblivion Fragrance Mists'. Enjoy a more snowy Bruma now with the additional weathers.*

7. [Realistic Aurora In Motion](https://www.nexusmods.com/oblivion/mods/46917)
8. [Falling Leaves](https://www.nexusmods.com/oblivion/mods/44257?tab=files) (1st main file)

*Once installed, delete Falling Leaves - UL LushWoodlands Patch.esp.*

9. [T4UT - Skies Repolished](https://www.nexusmods.com/oblivion/mods/54904)

10. [High Quality Snowflakes by Xelus](https://www.nexusmods.com/oblivion/mods/49898?tab=files)
11. [Moon Replacer](https://www.nexusmods.com/oblivion/mods/46924)
12. [DOWNPOUR - rain retexture](https://www.nexusmods.com/oblivion/mods/45520)

*Within the BAIN Package Installer, select 01 Small only.

*13. [Simple Sunglare with Lens Flare](https://www.nexusmods.com/oblivion/mods/51354?tab=description)
14. [Oblivion NightSkies Overhaul](https://www.nexusmods.com/oblivion/mods/50121?tab=files) (main file only)

*Within the BAIN Installer Package, select:*

- 01 - MESHES - Nebula 1 & 2 + Overlay
- 02 - TEXTURES - Stars - 2k
- 03 - TEXTURES - Nebula 1 - Version 1 - 2k
- 04 - TEXTURES - Nebula 2 - Version 1 (Vanilla 1k)
- 05 - OVERLAY - Aurora - 2k

15. [Atmos4096](https://www.nexusmods.com/oblivion/mods/49978?tab=files) (install DustCloud & GasClouds separately)
16. [Better Rainbows](https://www.nexusmods.com/oblivion/mods/36566?tab=files) \[**QAC**\]
17. [Lights of Oblivion - Road Lanterns](https://www.nexusmods.com/oblivion/mods/46131?tab=files) (Fantasy Mesh)

18. [ILOO - Interior Lighting Oblivion Overhaul](https://www.nexusmods.com/oblivion/mods/53298?tab=description)

*In my mind the spiritual successor for All Natural without the weighty accompaniments it brought.*

19. [Cava Obscura](https://www.nexusmods.com/oblivion/mods/35099?tab=description) (first main file only) \[**MI**)

*During MI, **deselect 'Cava Obscura - Filter Patch for Mods.esp'**. We install the update file that has this updated filter patch within the Filter Patches separator later.
*
*Dungeon lighting can play a huge part in not only aesthetic immersion, but also gameplay-mechanics. Take serious consideration when entering dungeons; acquire Night-eye spells, improve your sneak; torches are not as effective anymore!

There are similar mods that perform this on Nexus however Cava Obscura gets the parameters just right with the added bonus of a filter patch installed later.*

## Part 13 - OBLIVION REALM
*The still-terrifying realm of Oblivion is timeless & exemplifies the game's thematic & aesthetic juxtaposition between the almost utopian Cyrodiil with this hellish nightmare. A simple & I'm sure very familiar suite of mods improve this area of the game, with the excellent Deadlands installed later.*

1. [Oblivion Realm HD](https://www.nexusmods.com/oblivion/mods/43100?tab=description)
2. [Oblivion Landscape](https://tesall.ru/files/modi-dlya-oblivion/reteksturi-i-repleiseri/6235-retekstur-landshafta-obliviona) \[**MI**\]
3. [Oblivion Trees](https://tesall.ru/files/modi-dlya-oblivion/reteksturi-i-repleiseri/7359-derevya-obliviona)\[**MI**\]
4. [Oblivion Caves Retexture](https://www.nexusmods.com/oblivion/mods/47407?tab=files) (2k)

*Once installed, delete the Meshes folder given we'll use the following mod.

*5. [Collection of Fixes by Lazaro - Oblivion Caves retexture 2K Enhanced meshes](https://www.nexusmods.com/oblivion/mods/53031)

6. [Ayleid Glow Maps Minor Fix and some](https://www.nexusmods.com/oblivion/mods/53411?tab=files) (Improved glow maps for Oblivion Realm only)*

Lastly, using MO2's Ini Editor, search for **bUseRefractionShader** and set it to 0. This fixes a visual bug with the Oblivion gates.*

## Part 14 - ENB & OBLIVION RELOADED COMBINED
1. [ENB Series (Oblivion)](http://enbdev.com/download_mod_tesoblivion.htm)

*Extract the d3d9.dll from the wrapper version to the oblivion **root folder**.

**NOTE**: Some users have reported issues with the latest (500) version so use .181 if visual glitches arise.*

2. [CandidENB_Reborn](https://www.nexusmods.com/oblivion/mods/47810?tab=files)

*With the switch to newer weathers in 09.24, a new enb choice was needed. This combination truly takes the visuals to the next level whilst in many instances improving on performance from the previously used Cyrodiil enb.

* *If updating from a previous version of MOFAM: be sure to COMPLETELY uninstall Cyrodiil ENB (including the enbseries.dll).*

3. [CandidENB Tweaked ENBSeries.ini](https://www.nexusmods.com/oblivion/mods/52949?tab=files)

*A tuned variant of the ENB for mod compatibility & enhanced visuals and performance. Simply download & replace the one from above. Adjustments to bloom, adaptation, fire, ambient lighting & overrall clarity have been applied to suit the build.

*

*Note the following mods are the first that are installed within MO2's separator.*

4. **[Oblivion Reloaded Combined](https://www.nexusmods.com/oblivion/mods/51927?tab=files&file_id=1000034808)** (**ORC 180**) *Once installed hide:*

- Textures \> Effects \> TerrainNoise.dds
- Meshes \> Characters \> \_1stPerson \> Skeleton.nif
- Meshes \> Characters \> \_male \> Skeleton.nif

***IMPORTANT**: We have isolated a rare crash that occurs for some users. Once the mod has been installed, fire up the game to the main menu & start a new game; then simply quit. The crash was occurring if the following ini edits were applied before the game had been started with the mod present.

*

- Open up **ORC\Fog\Fog.ini** via the INI Files menu of MO2 & set both \[**World**\] & \[**Interior**\] Amount values to **0.0**.

*This setting fixes a rare bug where the screen can occasionally turn blue when certain magic effects are applied to the player character.

*5. [ORC Ini](https://www.nexusmods.com/oblivion/mods/52949?tab=files)*

I've provided the main Ini as a download to save time. Feel free to reference the changes applied in the Notes tab of the mod.

Oblivion Reloaded Combined is a more performant variant of the mighty Oblivion Reloaded. We use it predominantly for its engine management features & smaller mods it also encompasses, hence the majority of its post-processing effects are disabled in favour of enb.

A personal favourite is the **gravity** setting. Replacing a similar [mod](https://www.nexusmods.com/oblivion/mods/43920) by Elzee, this genuinely makes combat & traversal feel faster & more fluid, fixing the surreal Moon-like gravity of vanilla.*

**Now here comes the bad news**. Since MOFAM went Live we've had some unfortunate reports in my Discord of the mod simply crashing the game for a small subset of users, after the main menu is loaded.

If this occurs for you, then consider an alternative to the mod such as *[Oblivion Stutter Remover](https://www.nexusmods.com/oblivion/mods/23208). There is also an optimised Ini file for it* *[here](https://www.nexusmods.com/oblivion/mods/51468?tab=posts), but take note of the MO2-specific bug, the fix of which is highlighted in the comments section. Do NOT use enboost, as we use ENB in this build & that will render it useless.*

6. [Vanilla Remastered 1k](https://www.nexusmods.com/oblivion/mods/50903?tab=files) (Whiteflame Fix optional file)

*As a last step, within 11.24.1's update we are disabling the FPS cap within ORC's ini & enabling it within the enbseries.ini. Given this step is user-specific, perform the following:*

- Using [this](https://www.nexusmods.com/fallout3/mods/24349?tab=description) modpage's description under \[HAVOK\], apply the relevant setting within your Oblivion.ini (e.g. if 60fps is your max refresh rate, ensure fMaxtime is 0.0166), using MO2's Ini Editor function for ease of use.
- Regardless if your monitor is capable of over 90fps, we have found the best setting to be fMaxTime=0.0111, with a cap of 90fps set in enbseries.ini
- Open the enbseries.ini downloaded from my modpage & under \[LIMITER\], set EnableFPSLimit to true, with the FPSLimit at either 60 or 90 based off your settings applied above.

## Part 15 - INTERIOR RETEXTURES
1. [Gecko's Fort Interior Textures](https://www.nexusmods.com/oblivion/mods/45996)
2. [Gecko's Imperial Dungeon Textures - 2K Parallax](https://www.nexusmods.com/oblivion/mods/46864?tab=description)

3. [High Fantasy Cyrodiil Caves](https://www.nexusmods.com/oblivion/mods/50017)
4. [2020 Retexture Project - Caves](https://www.nexusmods.com/oblivion/mods/49933?tab=description) *Once installed delete every file except:*

- textures \> dungeons \> caves \> cavefungus01\*
- textures \> dungeons \> caves \> cavefungus02\*

*This will leave 5 files remaining; these are the best retextures for the most often-found flora in caves.*

5. [Ruined Ruins](https://www.nexusmods.com/oblivion/mods/44697?tab=files) (both main files, install separately)
6. [AI Enhanced - Ruined Ruins](https://www.nexusmods.com/oblivion/mods/52851)

7. [Bettys Sewer Textures](https://www.nexusmods.com/oblivion/mods/47316) (Wet Version Update1)

8. [VKVII_Oblivion_Cathedrals (medium)](https://www.moddb.com/mods/vkvii-oblivion-cathedrals/downloads/vkvii-oblivion-cathedrals-medium)
9. [Faster Dungeon Doors](https://www.nexusmods.com/oblivion/mods/46889)

10. [HD Cobwebs](https://www.nexusmods.com/oblivion/mods/50488?tab=files)\[**MI**\]
11. [Double Sided Cobwebs](https://www.nexusmods.com/oblivion/mods/46264) \[**MI**\]

*Use the 'Main files \> data' folder as the data directory.*

12. [Improved Candles](https://www.nexusmods.com/oblivion/mods/34410?tab=files) \[**MI**\]

## Part 16 - TOWN AND CITY RETEXTURES
1. [T4UTXL - Architecture_BETA1](https://www.nexusmods.com/oblivion/mods/54904) *Given we install this particular mod twice, during Quick Install name it "**T4UTXL - Architecture_BETA1 - Priory**"*

- Delete everything except Textures \> Architecture \> Priory

*Then delete (with asterisked files denoting ALL variants of the file):*

- Textures \> Architecture \> Priory \> priorydoor01\*
- Textures \> Architecture \> Priory \> weynondoor01\*

2. [Got Wood - Retexture](https://www.nexusmods.com/oblivion/mods/54680)

3. [VKVII Oblivion Castles (Medium)](https://www.moddb.com/mods/vkvii-oblivion-castles)
4. [VKVII Oblivion Imperial City (Medium)](https://www.moddb.com/mods/vkvii-oblivion-imperial-city/downloads/vkvii-oblivion-imperial-city-medium-size)
5. [TD_Tombstones retextures](https://www.nexusmods.com/oblivion/mods/48614) \[**MI**\]

6. [Arena Of Awe - Retexture](https://www.nexusmods.com/oblivion/mods/53052)
7. [Arena Poster](https://www.nexusmods.com/oblivion/mods/25367)
8. [Beautiful Arena Spectator](https://www.nexusmods.com/oblivion/mods/41741)

9. [Cheydinhal Retexture](https://www.nexusmods.com/oblivion/mods/44685)

10. [Leyawiin Woodland](https://www.nexusmods.com/oblivion/mods/46628?tab=files) (1k) \[**MI**\] *Once installed delete:*

- Textures \> Landscape

11. [Leyawiin Woodland floor fix](https://www.nexusmods.com/oblivion/mods/48759?tab=files) (Leyawiin Woodland floor fix -All Natural compatible)

12a. [Improved Chorrol](https://www.nexusmods.com/oblivion/mods/9500?tab=files) (main file Improved Chorrol) \[**MI**\]
12b. [chorrolupdate](https://www.nexusmods.com/oblivion/mods/9500?tab=files)
12c. [chorrol2011](https://www.nexusmods.com/oblivion/mods/9500?tab=files)
13. [Improved Bravil](https://www.nexusmods.com/oblivion/mods/10383?tab=files) (2nd main file then install 1st main file separately)

14. [TD Unique Skingrad](http://www.mediafire.com/file/2f9lwl81th6vnua/TD_Unique_Skingrad.7z/file) \[**MI**\]

*During* \[**MI**\], *deselect TD_Unique_Skingrad.esp. Paste the URL into the 'Use Custom URL' field within the Nexus Info tab for ease of reference.*

15. [TD Unique Anvil](http://www.mediafire.com/file/pxrpq126ixq92s4/TD_Unique_Anvil.7z/file) \[**MI**\]

*During* \[**MI**\]*, deselect TD_Unique_Anvil.esp.* *Paste the URL into the 'Use Custom URL' field within the Nexus Info tab for ease of reference.*

16. [TD Aesthetics](https://tesdiesel.blogspot.com/2015/06/tdaesthetics-of-garbage.html) \[**MI**\]
17. [Farm fence retexture and UV maps](https://www.nexusmods.com/oblivion/mods/47355?tab=files)

*Install manually & deselect the textures folder.*

18. [TD_Hutor - Oblivion farms retexture](https://www.nexusmods.com/oblivion/mods/48613?tab=files) (main file) \[**MI**\]
19. [Collection of Cleaned - Updated - Fixed - UOP Compatible Mods](https://www.nexusmods.com/oblivion/mods/52833?tab=files) (TD_Hutor - Oblivion farms retexture fix)

20. [Kvatch HD](https://www.nexusmods.com/oblivion/mods/45670?tab=files) (main file then update, install separately)

21. [Darooz Bruma](https://www.nexusmods.com/oblivion/mods/41107) *Once installed hide or delete:*

- Textures \> Architecture \> Bruma \> Interioronly \> brudoorupper02\*

22. [Khettienna's Mini-Mods](https://www.nexusmods.com/oblivion/mods/46187?tab=files) (main file)

*Within the (somewhat large) BAIN Package, select the following:*
KMM Chorrol Mages Guild UV Tweaks v1.0
KMM Crucible Sewage Retex v2.0
KMM Garridan's Tears Retex v1.0
KMM Leyawiin Woodland Stonewall Normalmap Fix v1.0
KMM Paint Palette Retex v1.0
KMM SE Beds Improved UV v1.0 Stone
KMM Skingrad Modular Door UV Fix for Mikal33's Improved Doors & Flora v1.0

23. [VKVII Oblivion Sidewalk Stonewall (Medium)](https://www.moddb.com/mods/vkvii-oblivion-sidewalks-stonewalls/downloads)*

*24. [Better Window Reflections](https://www.nexusmods.com/oblivion/mods/46774)
25. [Retextured Inn Signs](https://www.nexusmods.com/oblivion/mods/48576)
26. [Retextured Road Signs](https://www.nexusmods.com/oblivion/mods/48235) (1st main file only)
27. [Signs of Mage Guilde English version](https://www.nexusmods.com/oblivion/mods/25122)

- Once installed move *MageGuild_simbol.esp* to the optional folder.

28. [Signs of Mage Guilde English version - Mergeable](https://www.nexusmods.com/oblivion/mods/52823)

29. [Daedric Statues Improved - 2k Upscaled and Fixed Textures](https://www.nexusmods.com/oblivion/mods/54765)
30. [Statues HD](https://www.nexusmods.com/oblivion/mods/43104)

*Texturing can invoke such differing emotive responses. Allowing this mod to win over previous ones provides a deeper engagement with the worldspace feeling that the world is in perilous decline. Even though a number of these are in 4k resolution no performance impact has been observed; and coupled with Betty's Skingrad statue fix installed earlier enjoy a richer suite of Statues.*

31. [Imperial Roads](https://www.nexusmods.com/oblivion/mods/46744?tab=files) (main file only)

32. [T4UTXL - Architecture_BETA1](https://www.nexusmods.com/oblivion/mods/54904?tab=files) *Given we install this particular mod twice, during Quick Install name it "**T4UTXL - Architecture_BETA1 - City Gates**"

Once installed delete everything except:*

- Textures \> Architecture \> Anvil \> anvilcastledoor01\*
- Textures \> Architecture \> Bravil \> bravilentrancegate01\*
- Textures \> Architecture \> Bruma \> brumacitygate\*
- Textures \> Architecture \> Leyawiin \> leyawiincastledoor\*
- Textures \> Architecture \> Skingrad \> skcastledoorlarge\*

- Textures \> Architecture \> Castle \> Cheydinhal \> cheydinhalcitydoor01\*

*
Finally: One of my oldest eye-sores in the game is fixed - City Gates.*

## Part 17 - ANIMATED WINDOW LIGHTING SYSTEM
1. [AWLS Animated Window Lighting System](https://www.nexusmods.com/oblivion/mods/19628?tab=files) (main file)

**Plugin**: Advanced Smoking Chimneys
**QTP3**: Skip
**RAEVWD**: Skip
**BomretSI**: Install Files
**Options**: Choose a combo pack
**Pick a Complete Texture Pack**: Orange - Brumbeck Recommends
**Cathedral Windows Options**: Orange - Brumbeck Recommends
**Mages Guild Magic Circle Window**: Purple
**Imperial City Temple Windows Options**: Blue
**Shivering Isles Settlements Options**: Brunbek Yellow Multi-Colour (Recommended)
**Shivering Isles Palace Options**: Dual Nature (Recommended)
**Shivering Isles Crucible Options**: Blue-purple (Recommended)
**Shivering Isles Bliss Options**: More Colours (Recommended)

2. [TD and AWLS patch](https://www.nexusmods.com/oblivion/mods/50490?tab=files) (all three files, install separately)

3. [Diverse Chapels Vanilla](https://www.nexusmods.com/oblivion/mods/48732) *Within the BAIN Installer, select:*

- 00 Core
- 10 AWLS Support

## Part 18 - KATKAT'S LOCATION RETEXTURES
*Given they're non-Nexus sourced, use the 'Use Custom URL' function of MO2 within the Nexus Info tab for QoL. Katkat's an incredible retexture artist & we use dedicated separators for her work.*

1. [Katkat's FarmClutters](https://tesall.ru/files/modi-dlya-oblivion/reteksturi-i-repleiseri/7437-fermerskii-instrument) \[**MI**\]
2. [Katkat's White Gold tower](https://tesall.ru/files/modi-dlya-oblivion/reteksturi-i-repleiseri/10205-bashnya-belogo-zolota) \[**MI**\]

3. [Katkat's AYLEID RUINS HD](https://tesall.ru/files/modi-dlya-oblivion/reteksturi-i-repleiseri/9151-aileidskie-ruini-hd) (Install 'Base Metal' \[**MI**\] then 'Unofficial Oblivion Patch Meshes' separately)
4. [Katkat's cloudrulertemple](https://tesall.ru/files/modi-dlya-oblivion/reteksturi-i-repleiseri/9193-khram-povelitelya-oblakov-hd-ot-katkat74) \[**MI**\]
5. [Katkat's wayshrine](https://tesall.ru/files/modi-dlya-oblivion/reteksturi-i-repleiseri/7982-dorozhnie-svyatilishcha-ot-katkat74) \[**MI**\]
6. [Katkat's basements](https://tesall.ru/files/modi-dlya-oblivion/reteksturi-i-repleiseri/7202-podvali-hd) \[**MI**\]
7. [Ships from katkat74 AWLS](https://tesall.ru/files/modi-dlya-oblivion/reteksturi-i-repleiseri/8107-repleis-korablei-ot-katkat74) \[**MI**\]

*Once installed, open the Conflicts tab & hide the files that win over 2 VWD Ships - KatKat74's Textures (4 files)*

8. [English Bloated Float Signs Super Resolution 2k for KatKat74's Ships Retexture](https://www.nexusmods.com/oblivion/mods/51438?tab=description)

9. [Katkat's Bliss](https://tesall.ru/files/modi-dlya-oblivion/reteksturi-i-repleiseri/10189-bliss-hd) \[**MI**\]
10. [Flora Vilverin by Katkat74](https://tesall.ru/files/modi-dlya-oblivion/reteksturi-i-repleiseri/9525-flora-vilverina-ot-katkat74) \[**MI**\]
11. [Katkat'S Waterfall](https://tesall.ru/files/modi-dlya-oblivion/reteksturi-i-repleiseri/8047-vodopadi-ot-katkat74) \[**MI**\]
12. [Katkat's Upper Class Furniture](https://tesall.ru/files/modi-dlya-oblivion/reteksturi-i-repleiseri/7089-repleis-mebeli-visshego-klassa) \[**MI**\]
13. [KatKat74 Upper-class Stool Fix](https://www.nexusmods.com/oblivion/mods/53128) (main file only)
14. [Katkat's VEGETABLE GARDEN](https://tesall.ru/files/modi-dlya-oblivion/reteksturi-i-repleiseri/7143-ogorodistii-ogorod) \[**MI**\]
15. [Katkat's well](https://tesall.ru/files/modi-dlya-oblivion/reteksturi-i-repleiseri/7980-kolodets-ot-katkat74) \[**MI**\]

## Part 19 - CREATURE & ANIMAL RETEXTURES
1. [Mythic Creatures](https://www.nexusmods.com/oblivion/mods/29569?tab=description)\[**MI**\]
2. [Big Liz's Big Textures for Big Lizards (Clannfear and Daedroth) - Clannfear Reptilian 2K](https://www.nexusmods.com/oblivion/mods/54407?tab=files)
3. [Big Liz's Big Textures for Big Lizards (Clannfear and Daedroth) - Daedroth Reptilian 2K](https://www.nexusmods.com/oblivion/mods/54407?tab=files)

4. [Xivilaization Revolution](https://www.nexusmods.com/oblivion/mods/54392?tab=description) (2k)
5. [Mythic Animals](https://www.nexusmods.com/oblivion/mods/29638?tab=files) (low res)\[**MI**\]
6. [Mythic Ghosts and Goblins](https://www.nexusmods.com/oblivion/mods/36000)

*Once installed delete the Alt Ghost Texture (Rags) folder.
*
7. [Mythic Madness](https://www.nexusmods.com/oblivion/mods/38251)\[**MI**\]

8. [Beautiful Creatures - Spriggan](https://www.nexusmods.com/oblivion/mods/43285)
9. [Beautiful Creatures - Spider Daedra](https://www.nexusmods.com/oblivion/mods/43297?tab=files) (main file only)

10. [Improved Flame Atronachs](https://www.nexusmods.com/oblivion/mods/45356?tab=files)

*Install manually, and right click the flame atronach replacer \> Data folder & select Set as \ Directory.*

11. [Better minotaurs](https://www.nexusmods.com/oblivion/mods/49425)

12. [Unique Liches](https://www.nexusmods.com/oblivion/mods/54384?tab=files) (both main & optional files, install **separately**)
13. [Better Lorgren Benirus](https://www.nexusmods.com/oblivion/mods/51263?tab=files) (Better Lorgren Benirus - No Staff Edit)

14. [Diablo-like Goblins](https://www.nexusmods.com/oblivion/mods/48477?tab=files) (Goblin Aesthetics Tweak - Vanilla)

15. [Mehrunes Dagon Retex by themythofstrider](https://www.nexusmods.com/oblivion/mods/29314?tab=files) (optional file only)\[**MI**\]

16. [Ducks and Swans for Cyrodiil](https://www.nexusmods.com/oblivion/mods/45275) (main file) \[**QAC**\]
17. [Diverse Ducks and Swans](https://www.nexusmods.com/oblivion/mods/46526)

*This mod requires installing twice. For each separate install, right click the Data folder in each of the 'ducks' and 'swans' folders & set as \ directory, calling the installs 'Diverse Ducks and Swans - Ducks' & 'Diverse Ducks and Swans - Swans' respectively.

*18. [More Butterflies](https://www.nexusmods.com/oblivion/mods/46785?tab=files) (main file then Update; install **separately**)

19. [Simple Horse Utilities](https://www.nexusmods.com/oblivion/mods/51197?tab=files) (1st main file)
20. [Coop's TW3 Oblivion Horse Replacer](https://www.nexusmods.com/oblivion/mods/53323) Within the fomod select the following:

- **Misc**: Feathering + Simple Horse Utilities Patch + Armored Legion Horses
- **ArmoredManeFix**: MergeablePatch
- **KlenPatch**: None
- **Bodies**: Shaggy Horses
- **Horns**: None

*Absolutely superb horse-mod from Coop - take note WAC & OOO's Legion Horses are patched in as part of our prebash-merge created later.*

21. [Coops Deer and Mountain Lion Revamp](https://www.nexusmods.com/oblivion/mods/53640?tab=files) (2k) Within the fomod select the following:

- **Buck**: Dark
- **Doe**: Solid
- **MtnLion**: Green
- **MOO**: MOO

*
*22. [Coop's Mudcrab Remake](https://www.nexusmods.com/oblivion/mods/53435?tab=files) (1st main file) Within the fomod select the following:
*
*

- **Pattern**: New
- **Shine**: Muted
- **Misc**: None

*
*23. [Coop's Vanilla Wolf Revamp 2K](https://www.nexusmods.com/oblivion/mods/53384?tab=files)
24. [Coop's MOO Wolf Revamp 2K](https://www.nexusmods.com/oblivion/mods/53384?tab=files)
*
You can tell I'm a fan of Coop's work. Give her a kudos.

Similarly to the previous Katkat part, given they're non-Nexus sourced, use the 'Use Custom URL' function of MO2 within the Nexus Info tab for QoL.*

25. [Katkat's Sheep](https://tesall.ru/files/modi-dlya-oblivion/sushchestva-monstri-pitomtsi/8770-baranistie-barani) \[**MI**\]
26. [Katkat's slaughterfish](https://tesall.ru/files/modi-dlya-oblivion/sushchestva-monstri-pitomtsi/9073-riba-ubiitsa-sk) \[**MI**\]
27. [Katkat's dog](https://tesall.ru/files/modi-dlya-oblivion/sushchestva-monstri-pitomtsi/8672-sobakus-vulgaris) \[**MI**\]
28. [Katkat's bear](https://tesall.ru/files/modi-dlya-oblivion/sushchestva-monstri-pitomtsi/9025-medvedi-cdpr) \[**MI**\]
29. [Katkat's boar](https://tesall.ru/files/modi-dlya-oblivion/sushchestva-monstri-pitomtsi/8797-raskabanevshie-kabani) \[**MI**\]

## Part 20 - WEAPONS ARMOUR & CLOTHING IMPROVEMENTS
1a. [Weapon Improvement Project](https://www.nexusmods.com/oblivion/mods/43852?tab=files) (main file only)

*Install manually & deselect the Textures and Meshes folders.*

1b. [Weapon Improvement Project](https://www.nexusmods.com/oblivion/mods/43852?tab=files) (optional file, Town Guard Shields - Preview)
1c. [Weapon Improvement Project - fixes (NO ESP)](https://mega.nz/file/lGwSjY4K#Nn_RX412Fq8ObbvNJlpSU2ti00aoyZHSwI9Zc3rAkBs)
2. [Knights of the Nine_Weapon Improvement Project Patch](https://www.nexusmods.com/oblivion/mods/46313?tab=files) (1st main file)

3. [HiRes Iron Armor](https://www.nexusmods.com/oblivion/mods/30386)
4. [HiRes Iron Weapons](https://www.nexusmods.com/oblivion/mods/30357)

5. [Visually Improved Staffs](https://www.nexusmods.com/oblivion/mods/38438)

*Within the BAIN Package installer, select:*

- 00 Core
- 01 Optional glowing
- 02 Hrormirs Ice Staff
- 03 Staff of Indarys

6. [Vanilla Gear Redux](https://www.nexusmods.com/oblivion/mods/45241?tab=files) (1st main file)
7. [Vanilla Gear Redux-Seamless OCOv2 fix](https://www.nexusmods.com/oblivion/mods/50858?tab=files) (1st main file, 'Reasonable')
8. [Patch for Vanilla Gear Redux Reasonable Flavor - Clipping and fpv fixes](https://www.nexusmods.com/oblivion/mods/51851?tab=files) (main file)

*Within the BAIN Package Installer, select 00 Core only.*

9. [Blue Darker Glass - Armor and Weapons](https://www.nexusmods.com/oblivion/mods/52978)
10. [Amber Enhancements](https://www.nexusmods.com/oblivion/mods/18111?tab=files) (Amber Bow Replacer + Amber Sword replacer, install separately)
11. [Mythic Amber Armor](https://www.nexusmods.com/oblivion/mods/37955)\[**MI**\]
12. [Madness Armor and Weapons Retex by TheMythofStrider](https://www.nexusmods.com/oblivion/mods/29286)\[**MI**\]
13. [Half-off Clavicusvile Mask remodel replacer](https://www.nexusmods.com/oblivion/mods/51113)\[**MI**\]
14. [Gray Cowl of Nocturnal Reshaped](https://www.nexusmods.com/oblivion/mods/50545)
15. [Goblin Totem Staff Icon](https://www.nexusmods.com/oblivion/mods/41058)\[**MI**\]
16. [Cutlass Retexture](https://www.nexusmods.com/oblivion/mods/51704)\[**MI**\]
17. [Banes Steel Helm Replacer](https://www.nexusmods.com/oblivion/mods/36219)

18. [Closed Iron Helmet](https://www.nexusmods.com/oblivion/mods/40791)\[**MI**\]
19. [Closed Ebony Helmet](https://www.nexusmods.com/oblivion/mods/40829)\[**MI**\]

20. [Insanitys Ebony Sword Replacer](https://www.nexusmods.com/oblivion/mods/43718)
21. [Insanitys Umbra Sword Replacer](https://www.nexusmods.com/oblivion/mods/43762)

22. [Magical Arrows](https://www.nexusmods.com/oblivion/mods/43666)
23. [Initial Glow Redux](https://www.nexusmods.com/oblivion/mods/46961)
24. [SI Mania Clothing Fix](https://www.nexusmods.com/oblivion/mods/52202)
25. [Retextured Rings](https://www.nexusmods.com/oblivion/mods/47742)

26. [Vanilla Amulets fix for HGEC](https://www.nexusmods.com/oblivion/mods/47583)
27. [Retextured Amulets](https://www.nexusmods.com/oblivion/mods/47734)
28*.*[Sanguine Rose Retexture](https://www.nexusmods.com/oblivion/mods/48529)*

*

## Part 21 - CLUTTER & MISCELLANEOUS RETEXTURES
1. [T4UT - CLUTTER_BETA1](https://www.nexusmods.com/oblivion/mods/54904?tab=files) *During Quick Install, name the mod **T4UT - CLUTTER_BETA1 - Farmhouse & Vinyard** for ease of reference. Once installed delete everything except:*

- Textures \> Clutter \> Vinyard
- Textures \> Clutter \> Farmhouse

2. [Improved Fruits Vegetables and Meats](https://www.nexusmods.com/oblivion/mods/10487?tab=files)
3. [IFVMUpdate](https://www.nexusmods.com/oblivion/mods/10487?tab=files)

*Once installed, open the Conflicts tab & hide the 5 winning mesh conflicts over Katkat's Vegetable Garden.*

4. [Sweet Rolls - A Sweet Roll Replacer](https://www.nexusmods.com/oblivion/mods/52645)
5. [Garlic - A Garlic Replacer](https://www.nexusmods.com/oblivion/mods/52670)
6. [EGO - Nirnroot Retexture](https://www.nexusmods.com/oblivion/mods/53574)

7. [TIBs Compact Quivers - Thinner Arrow Holders](https://www.nexusmods.com/oblivion/mods/45111?tab=files) *Install the optional files separately:*

- TIBs Compact Quivers - Manual Install
- TIBs Compact Quivers - Manual Install - SI
- TIBs Compact Quivers - Manual Install - Bonus Textures

8. [The Good China - Retexture](https://www.nexusmods.com/oblivion/mods/54636)

9. [TD Lower Clutter](https://www.mediafire.com/file/y4hpcsfplpqmp80/TD_Lower_Clutter.7z/file) \[**MI**\]

*Paste the URL into the 'Use Custom URL' field within the Nexus Info tab for ease of reference.*

10. [Improved Skulls and Bones and Ironwork](https://www.nexusmods.com/oblivion/mods/36075?tab=files) (1st main file only)
11. [Book Jackets Oblivion High Res BAIN](https://www.nexusmods.com/oblivion/mods/50033?tab=files) (main file only)

*Within the BAIN Package Installer, select:*

- 00 Core Assets
- 01c Core Book Jackets ESP - Filter Version (Maximum Compatibility with Knights and UOP)

12. [Book Jackets KOTN](https://www.nexusmods.com/oblivion/mods/27521)
13. [Book Jackets KOTN HD update](https://www.nexusmods.com/oblivion/mods/49307)
14. [Book Jackets for Misc DLC](https://www.nexusmods.com/oblivion/mods/49260)
15. [TD Alternative Book Covers](http://www.mediafire.com/file/zdm8rceqe70sgla/TD_Alternative_Books_Covers.7z/file) \[**MI**\]

*Paste the URL into the 'Use Custom URL' field within the Nexus Info tab for ease of reference.*

16. [Better book pages](https://www.nexusmods.com/oblivion/mods/47352)

17. [HiRes Silver and Gold Clutter](https://www.nexusmods.com/oblivion/mods/38546?tab=files) (both main & optional files, install separately)

18. [VKVII OBLIVION MAGES GUILD CLUTTER](https://www.moddb.com/mods/vkvii-oblivion-mages-guild-clutter/downloads/vkvii-oblivion-mages-guild-clutter)

19. [Alluring Wine Bottles with Real Glass](https://www.nexusmods.com/oblivion/mods/46789)
20. [Alluring Wine Bottles with Real Glass - Loose Files Patch for Mods](https://www.nexusmods.com/oblivion/mods/52171)

21. [Retextured Potions](https://www.nexusmods.com/oblivion/mods/49068)

*Within the BAIN Package Installer, select 00 Core only.

*22. [Paintings Variation](https://www.nexusmods.com/oblivion/mods/46482?tab=files) (both main file & optional, install separately)
23. [High-Res Varla and Welkynd Texture Replacer](https://www.nexusmods.com/oblivion/mods/36814)

*Within the BAIN Package Installer, select:*

- 00 High-Res Varla & Welkynd Textures
- 01 Fixed UV Meshes w. 2x UV Scaling
- 02 More Meshes w. New Iron Tex & 2x UV Scaling

24. [Particled Ayleid Stones](https://www.nexusmods.com/oblivion/mods/47245)25. [HiRes Hay Bail Textures](https://www.nexusmods.com/oblivion/mods/27138)

26. [Darooz Upperclass clutter](https://www.nexusmods.com/oblivion/mods/41108)
27. [Darooz artsupplies](https://www.nexusmods.com/oblivion/mods/41122)
28. [Modryn Oreyn Renaissance Master](https://www.nexusmods.com/oblivion/mods/51434)

29. [Kat's Actually Decent Enviroment Map](https://www.nexusmods.com/oblivion/mods/48856)
30. [Luna's Ironwood Nut Retex](https://www.nexusmods.com/oblivion/mods/49242)

*Install manually, and right click the LunasIronwoodNutRetex \> **Data** folder & select Set as \ Directory.
*
31. [Azuras Star retexture](https://www.nexusmods.com/oblivion/mods/50362?tab=files) (main file only)
32. [HD Septim](https://www.nexusmods.com/oblivion/mods/49017)

**Main**: Textures auto-selected
**Meshes**: Normal Size Mesh

33. [Little Baron Flower Pot Makeover - patched and adjusted](https://www.nexusmods.com/oblivion/mods/50792?tab=files) (Little Baron Flower Pot Makeover - patched and adjusted)

34. [Reb's Average Misc Junkyard](https://www.nexusmods.com/oblivion/mods/53617)

*A great suite of texturing that covers some neglected crucial extras. Installing separately, include:*

- Reb's Average Misc Guild Stuff 1k
- Reb's Average Misc Alchemy Replacers 1k
- Reb's Average Misc Misc Junk 1k
- Reb's Average Misc Tents Replacer 1k
- Reb's Average Misc DLC Stuff 1k
- Reb's Average Misc Mage Stuff 1k

35. [Savilla's Stone Enhanced](https://www.nexusmods.com/oblivion/mods/52713) *In the BAIN Installer, select:*

- 01 - Marbled Style - Alternative

## Part 22 - EFFECTS
1. [Alternate ghost effect](https://www.nexusmods.com/oblivion/mods/47401)
2. [Improved Fires and Flames](https://www.nexusmods.com/oblivion/mods/38061?tab=files) (Improved Fires and Flames - Performance)

*Once installed, open the conflicts tab & hide the 2 winning texture files over Katkat's Ayleid Ruins HD mod.*

3. [Smoking Firesources BETA - Project Ambience](https://www.nexusmods.com/oblivion/mods/46476?tab=files) *Install in this order, separately:*

- Smoking Firesources beta - Project Ambience - LANTERN PACK for VANILLA

*Once installed, hide:* *Meshes \> Lights \> IronLampHangingShort01Fake.nif*

- Smoking Firesources beta - Project Ambience - CANDLES PACK
- Hotfix for Candles pack
- Smoking Firesources beta - Project Ambience - Torch MIP version

4. [IMPROVED Fire Spell Animation](https://www.nexusmods.com/oblivion/mods/9058)\[**MI**\]

*We ironically previously used this MA's reference mod, however this alternate variant plays more nicely with the recent ENB-switch in 09.24's update.*

5. [Better Summon Undead Effect](https://www.nexusmods.com/oblivion/mods/10418)*

*

## Part 23 - COMBAT & MAGIC

*

*
1. [Dynamic Oblivion Combat](https://www.nexusmods.com/oblivion/mods/49873) *Once installed, apply the following edits in Dynamic Oblivion Combat.ini:*

`set dcvars.ini_DodgeKeyCode������ to�� 42`
`set dcvars.ini_NPCdodgePercent������ to�� 50`
`set dcvars.ini_NPCflankPercent������ to�� 50`
`set dcvars.ini_NPCDisarmToKOratio�� to��� 10`

*We previously used Combat Additions however a critical mod conflict was found that essentially broke OCRAFT (the crafting framework we make expanded use of later). Reverting back to Dynamic Oblivion Combat wasn't a difficult choice, given its a less script-heavy combat mod & I used it for years.*

2. [De-Nock Arrows xOBSE](https://www.nexusmods.com/oblivion/mods/52143)

3. [Better Blood](https://www.nexusmods.com/oblivion/mods/24448?tab=files) (main file only)

*Once installed delete SkycaptainsBloodTime.esp.*

4. [Better Blood skin decal fix](https://www.nexusmods.com/oblivion/mods/46344?tab=files#)5. [Normal Map for Better Blood](https://www.nexusmods.com/oblivion/mods/50547)
6. [StarXs Vampire Deaths](https://www.nexusmods.com/oblivion/mods/25613) \[**MI**\]

*Once installed delete StarX Vampire Deaths.esp & StarX Vampire Deaths.esm.*

7. [StarX Vampire death Improved Extended Edition](https://www.nexusmods.com/oblivion/mods/50480?tab=files)

8. [Av Latta Magicka - Oblivion Magic Overhaul](https://www.nexusmods.com/oblivion/mods/49096?tab=files) (main file only)

*Once installed, open the mod's INI Files tab & apply the following to Av Latta Magicka.ini's Restoration section:*

`set almQ.bDisableREHEShader to 1 `
*I've tried all the major Magic mods & this one's 'the keeper' in my opinion. Not only are balanced & thoughtful spells a key part of it, but also the milestones of each of the magic spells have been redone to great effect.*

9. [Miscellaneous Patch Collection for Mods by Dispensation](https://www.nexusmods.com/oblivion/mods/52874?tab=files) (Install just Av Latta Magicka - Migcks Misc Elemental Fists Poison Patch, and name the mod this way)

10. [Get rid of small Souls - Empty Soulgems](https://www.nexusmods.com/oblivion/mods/49753?tab=files) (both main file & optional, install separately)
11. [Better Traps](https://www.nexusmods.com/oblivion/mods/47713)

12. [De Rerum Dirennis - Alchemy Overhaul](https://www.nexusmods.com/oblivion/mods/53020)

*A streamlined, lightweight overhaul to Alchemy, proving Skyrim did one or two things better in this aspect of the game.*

13. [Enemy Spell Variety](https://www.nexusmods.com/oblivion/mods/53501)

## Part 24 - NEW WEAPONS & ARMOURS
*A very slim section here, given the huge amount of new weapons and armours are mostly provided in the overhauls installed earlier. However, these are too good to miss & improve upon some key aspects.*

1. [Weapons Of Morrowind](https://www.nexusmods.com/oblivion/mods/45440?tab=files) (1st main file)

*Once installed, pack the mod using BSArch & be sure to name your BSA as 'Weapons of Morrowind'. Then delete the loose Textures & Meshes folders.*

2. [Unique Artifacts for Unique People](https://www.nexusmods.com/oblivion/mods/49871?tab=files)
3. [Unique Artifacts for Unique People - Patches](https://www.nexusmods.com/oblivion/mods/49871?tab=files)

*Within the BAIN Package Installer, select just:*

02 Extended UI Weapon Backstory Descriptions

4. [Jaysus Blades](https://www.nexusmods.com/oblivion/mods/18529?tab=files) (main file & hide **Jblades!.esp**)
5. [Jaysus Blades Plugin fixes for Ultimate Leveling](https://www.nexusmods.com/oblivion/mods/53167?tab=files)

*We found certain weapons to be far too OP & also cause issues with Ultimate Levelling, so this new patch addresses these issues.*

6. [Local Guards Features](https://www.nexusmods.com/oblivion/mods/45517?tab=files) \[**QAC**\]

*Install manually, right click the Local Guards Features \> **Data** folder & Set as \ directory*

7a. [Local Guards Features Unofficial Patch](https://www.nexusmods.com/oblivion/mods/49896)7b. [Local Guards Features - Kvatch Addon](https://www.nexusmods.com/oblivion/mods/48779)

*Once installed delete tbskGuardsFeaturesKvatchAddon.esp.*

7c. [Local Guards Features - Thorn Addon](https://www.nexusmods.com/oblivion/mods/49043)

*Once installed delete* *tbskGuardsFeaturesThornAddon.esp.*

7d. [Local Guards Features - White Stallion Addon](https://www.nexusmods.com/oblivion/mods/50482)

*Once installed delete* *tbskGuardsFeaturesWhiteStallionAddon.esp.*

7e. [Local Guards Features - Gaius Prentus Addon](https://www.nexusmods.com/oblivion/mods/52015?tab=files) (Local Guards Features - Merged Vanilla Addons (Bashed Patch Mergeable))

8. [Colorful Clothing - Collection](https://www.nexusmods.com/oblivion/mods/53708?tab=files) - (Seamless OCOv2 main file) *Once installed, perform the following:*

- Using BAE, extract the archive to the mod's install location
- Delete the archive & Colorful Clothing - Collection.esp

***REMINDER**!! Revisit OOO Enhanced Resources & perform the post-install steps.*
9. [AI Enhanced - Colorful Clothing - Middleclass](https://www.nexusmods.com/oblivion/mods/52163?tab=files) (1k)
10. [AI Enhanced - Colorful Clothing - Upperclass](https://www.nexusmods.com/oblivion/mods/52165?tab=files) (1k)

11. [Unused Magic Items Integrated](https://www.nexusmods.com/oblivion/mods/49404)
12. [Travelling Equipment - Cloaks and Backpacks](https://www.nexusmods.com/oblivion/mods/50205?tab=files) (main file only)

## Part 25 - ARTHMOOR'S TOWNS
*Almost a staple in many Skyrim mod-builds, Arthmoor's towns are similarly of a seamless high quality within Oblivion as well.

**IMPORTANT**: As of 10.24's update we've sourced VA files via a different mod-guide so it's crucial you're confident with BSArch Pro.
For each mod I've made them a & b - **open both a & b once installed via explorer** to easily perform the archiving with BSArch Pro.
I also highly recommend ensuring 'Use Custom URL' is enabled in the Nexus tab of the mods with the URLs pasted there.*

1a. [Feldscar](https://www.afkmods.com/index.php?/files/file/249-feldscar/)
1b. [Feldscar_-_VA](https://www.mediafire.com/file/ffsy9na6xx2rtba/Feldscar_-_VA.7z/file)

- Drag the meshes, sound & textures folder into BSArch Pro from 1a
- Drag the sound folder from 1b into BSArch Pro & select Replace All
- Pack the mod ensuring the archive within 1a is named 'Feldscar' then delete the now-loose meshes, sound & textures folders & disable mod 1b.

2a. [Frostcrag Village](https://www.afkmods.com/index.php?/files/file/250-frostcrag-village/)
2b. [Frostcrag_Village_-_VA](https://www.mediafire.com/file/cvsdi6bja9gvi4t/Frostcrag_Village_-_VA.7z/file)

- Drag the meshes & textures folder into BSArch Pro from 2a
- Drag the sound folder from 2b into BSArch Pro
- Pack the mod ensuring the archive within 2a is named 'Frostcrag Village' then delete the now-loose meshes & textures folders & disable mod 2b.

3a. [Gottshaw Village](https://www.afkmods.com/index.php?/files/file/251-gottshaw-village/)
3b. [Gottshaw_Village_-_VA](https://www.mediafire.com/file/ko2oz4q3zxtqird/Gottshaw_Village_-_VA.7z/file)

- Drag the meshes & textures folder into BSArch Pro from 3a
- Drag the sound folder from 3b into BSArch Pro
- Pack the mod ensuring the archive within 3a is named 'Gottshaw Village' then delete the now-loose meshes & textures folders & disable mod 3b.

4a. [Molapi](https://www.afkmods.com/index.php?/files/file/252-molapi/)
4b. [Molapi_-_VA](https://www.mediafire.com/file/9ox0y79v4742bxv/Molapi_-_VA.7z/file)

- Drag the meshes, sound & textures folder into BSArch Pro from 4a
- Drag the sound folder from 4b into BSArch Pro & select Replace All
- Pack the mod ensuring the archive within 4a is named 'Molapi' then delete the now-loose meshes, sound & textures folders & disable mod 4b.

5a. [Reedstand](https://www.afkmods.com/index.php?/files/file/255-reedstand/)
5b. [Reedstand_-_VA](https://www.mediafire.com/file/xv1jon6va21br2l/Reedstand_-_VA.7z/file)

- Drag the meshes, sound & textures folder into BSArch Pro from 5a
- Drag the sound folder from 4b into BSArch Pro
- Pack the mod ensuring the archive within 5a is named 'Reedstand' then delete the now-loose meshes, sound & textures folders & disable mod 5b.

6a. [Sutch Village](https://www.afkmods.com/index.php?/files/file/256-sutch-village/)
6b. [Sutch Village - VA](https://mega.nz/file/nhIhRaQT#sU6lHo3-saa-cHENjC64sMnP3TONW0OQlKmFzsIRH30)

- Drag the meshes, sound & textures folder into BSArch Pro from 6a
- Drag the sound folder from 6b into BSArch Pro
- Pack the mod ensuring the archive within 6a is named 'Sutch Village' then delete
  the now-loose meshes, sound & textures folders & disable mod 6b.

7a. [Urasek](https://www.afkmods.com/index.php?/files/file/257-urasek/)
7b. [Urasek_VA](https://www.mediafire.com/file/ed7bubxp6ll5j63/Urasek_VA.7z/file)

- Drag the meshes & textures folders into BSArch Pro from 7a
- Drag the sound folder from 7b into BSArch Pro & select Replace All
- Pack the mod ensuring the archive within 7a is named 'Urasek' then delete the now-loose meshes, sound & textures folders & disable mod 7b.

8a. [Vergayun](https://www.afkmods.com/index.php?/files/file/258-vergayun/)
8b. [Vergayun_-_VA](https://www.mediafire.com/file/z6fed24j8zsm2aa/Vergayun_-_VA.7z/file)

- Drag the meshes & textures folder into BSArch Pro from 8a
- Drag the sound folder from 8b into BSArch Pro
- Pack the mod ensuring the archive within 8a is named 'Vergayun' then delete the now-loose meshes & textures folders & disable mod 8b.

*You can double-check this has all been performed correctly by verfiying within the Plugins tab of MO2 that each of the above plugins have the archive-flag associated to them. It's a potentially long-winded step however performance is our focus hence having 1000's of loose files archived is always preferable.*

9. [Miscellaneous Patch Collection by Dispensation](https://www.nexusmods.com/oblivion/mods/52874?tab=files)

*Select just **'Compatibility Patches for Arthmoor's Mods**' & name the mod this way. Once installed, delete every esp except:*

- DispMiscPatch_Ducks and Swans - Reedstand Patch.esp

Part 26a - NEW & MODIFIED LOCATIONS
*You'll notice this part being particularly large. Whilst we're well balanced with not going overboard on retexturing, this allows a broader yet still conservative intake of new locations to discover without detrimentally affecting the engine.
*

1. [Improved Fighters Guild ENG](https://www.nexusmods.com/oblivion/mods/53987)
2. [Improved Mages Guild ENG](https://www.nexusmods.com/oblivion/mods/54006)

*Enjoy a more vibrant & elegant feel to the guilds; akin to JK's mods for Skyrim, but with some added quest tweaks.*

3. [Improved Fighters Guild ENG OOO and All Natural Patch](https://www.nexusmods.com/oblivion/mods/54336)
4. [Improved Mages Guild ENG OOO and ALL NATURAL PATCH](https://www.nexusmods.com/oblivion/mods/54348)
5. [Improved Fighters Guild ENG - Town Guard Shields Preview Patch](https://www.nexusmods.com/oblivion/mods/54287?tab=description)

*For those in my Discord, durbinh's a familiar local-legend. Give him a kudos.*

6. [ImpeREAL City - Unique Districts](https://www.nexusmods.com/oblivion/mods/19589?tab=files) (ImpeREAL City - Unique Districts - All The Districts - Merged) \[**QAC**\]

*Once installed, open the mod in xEdit & perform the following:*

- Delete: Light \> xx010F43 CityStreetlightWaterfrontDistrict01
- Delete: Worldspace \> 0000003C Tamriel

*This removes the Waterfront aspect from the merge, given it's excess on performance.*

7. [The Imperial Waters](https://www.nexusmods.com/oblivion/mods/25804) \[**MI**\] \[**QAC**\]

*During MI, right click Files & select Set as \ Directory, and disable The Imperial Water - BETTER CITIES.esp plugin.*

8. [Ice's Waterfront Tunnel](https://www.nexusmods.com/oblivion/mods/51749?tab=files)

*This trio of mods for the Imperial City act as replacers for Better Cities (as of 07.23 version of the guide), proving far more performant whilst still retaining the core aesthetics.*

9. [Add some flavor - Roadside inns](https://www.nexusmods.com/oblivion/mods/53257)
10. [Add some flavor - priories](https://www.nexusmods.com/oblivion/mods/53149)
11. [Miscellaneous Patch Collection by Dispensation](https://www.nexusmods.com/oblivion/mods/52874)

*Within the BAIN Package Installer, select just Add Some Flavor mods & call the mod 'Miscellaneous Patch Collection by Dispensation - Add Some Flavor mods' for ease of reference.

Of the 3 plugins we only require **DispMiscPatch_AddSomeFlavorRoadsideInns - GottshawVillage Patch.esp** so delete the other two.*

12. [ImpeREAL Empire - Unique Castles](https://www.nexusmods.com/oblivion/mods/22446)
13. [ImpeREAL Castles - Skingrad Patch](https://www.nexusmods.com/oblivion/mods/41114?tab=files) (1st main file)

14. [County Gates](https://www.nexusmods.com/oblivion/mods/50778?tab=files) (first main file only)
15. [County Gates - Town Guard Shields Preview Patch](https://www.nexusmods.com/oblivion/mods/53034?tab=posts)

16. [Arena Champion's Villa](https://www.nexusmods.com/oblivion/mods/50321)\[**MI**\]
17. [Knightly Orders for Cities](https://www.nexusmods.com/oblivion/mods/49934?tab=files) (1st main file)

18. [Unique Landscapes - River Ethe](https://www.nexusmods.com/oblivion/mods/17330)
29. [Unique Landscapes - Panther River](https://www.nexusmods.com/oblivion/mods/20332)
20. [Unique Landscapes - Imperial Isle](https://www.nexusmods.com/oblivion/mods/9531)
21. [Unique Landscapes - Brena River Ravine](https://www.nexusmods.com/oblivion/mods/23573)
22. [Unique Landscapes - Ancient Yews](https://www.nexusmods.com/oblivion/mods/11458)
23. [Unique Landscapes - Rolling Hills](https://www.nexusmods.com/oblivion/mods/10768?tab=files)
24. [Unique Landscapes - Cloudtop Mountains](https://www.nexusmods.com/oblivion/mods/16677)

*A beautiful selection of my favourites from the Unique Landscapes suite. Performant, lightweight & aesthetically stunning - albeit with some minor tweaks addressed in the patching later. Key sections of the map have been overhauled for a fresh & engaging experience.*

25. [The Hesu Mod Collection](https://www.afkmods.com/index.php?/files/file/2344-the-hesu-mod-collection/) The Valenwood Mine

*Install manually & right click Hesu Mods \> **HESU The Valenwood Mine v1.2** & select Set as \ directory. Id suggest naming the mod as per the title here for ease of reference.

*26. [The Hesu Mod Collection](https://www.afkmods.com/index.php?/files/file/2344-the-hesu-mod-collection/) Skyrim temple

*Install manually & right click Hesu Mods \> **HESU Skyrim Temple v1.2** & select Set as \ directory.* *Id suggest naming the mod as per the title here for ease of reference.*

28. [The Hesu Mod Collection](https://www.afkmods.com/index.php?/files/file/2344-the-hesu-mod-collection/) Smoke Town

*Install manually & right click Hesu Mods \> **HESU Smoke Town v1.1** & select Set as \ directory.* *Id suggest naming the mod as per the title here for ease of reference.*

29. [Legion Forester Outposts Revisited](https://www.nexusmods.com/oblivion/mods/51512) (first main file)

- Install manually, and expand Legion Forester Outposts Revisited
- Expand 01 Diversity Addons \> Drag LFO - All Races Addon.esp to the 00 Core folder
- Expand 02 Local Guards Features Patch \> Drag LFO - Local Guards Features Patch.esp to the 00 Core folder
- Right click 00 Core & select Set as \ directory.

30. [Legion Forester Outposts Revisited](https://www.nexusmods.com/oblivion/mods/51512) (optional file: New OCO Eyes for All Races Addon)

31. [Nobody Goes into the Mountains but Hunters](https://www.nexusmods.com/oblivion/mods/49092?tab=files) (main file only)
32. [Nobody Goes into the Mountains but Hunters - UL Compilation Compatible](https://www.nexusmods.com/oblivion/mods/49092?tab=files) \[**QAC**\]

33. [Better Dungeons](https://www.nexusmods.com/oblivion/mods/40392?tab=files) (Main files Better Dungeons + Better Dungeons BSA, install **separately**)[](https://www.nexusmods.com/oblivion/mods/52558)
34. [Bruma Guild Reconstructed](https://www.afkmods.com/index.php?/files/file/264-bruma-guild-reconstructed/)

*Whilst Better Forts merged version was previously used, a fair fraction of the forts exhibited excess performance hits. Think of the below as the 'Best Of...' that are all lightweight implementation & we create our own merge later to retrieve plugin-space.*

35. [Better Fort Aurus](https://www.nexusmods.com/oblivion/mods/50682)
36. [Better Fort Doublecross](https://www.nexusmods.com/oblivion/mods/51325)
37. [Better Fort Facian](https://www.nexusmods.com/oblivion/mods/51464)
38. [Better Fort Hastrel](https://www.nexusmods.com/oblivion/mods/50538)
39. [Better Fort Irony](https://www.nexusmods.com/oblivion/mods/51841)
40. [Better Fort Naso](https://www.nexusmods.com/oblivion/mods/50586)
41. [Better Fort Rayles](https://www.nexusmods.com/oblivion/mods/51148)
42. [Better Fort Redman](https://www.nexusmods.com/oblivion/mods/51444)
43. [Better Fort Teleman](https://www.nexusmods.com/oblivion/mods/51594)
44. [Better Fort Vlastarus](https://www.nexusmods.com/oblivion/mods/52603)

46. [Glowing Stones](https://www.nexusmods.com/oblivion/mods/43331) \[**QAC**\]

47a. [Reworked Posts](https://www.nexusmods.com/oblivion/mods/47223?tab=files) (Reworked Post - Carved Letters)

47a. [Patch for Arthmoor Villages and Reworked Posts](https://www.nexusmods.com/oblivion/mods/47299?tab=files) (Reworked Posts and Reedstand Village Patch)
47b. [Patch for Arthmoor Villages and Reworked Posts](https://www.nexusmods.com/oblivion/mods/47299?tab=files) (Reworked Posts and Gottshaw Village Patch)
47c. [Patch for Arthmoor Villages and Reworked Posts](https://www.nexusmods.com/oblivion/mods/47299?tab=files) (Reworked Posts and Sutch Village Patch)

48. [Dagger_Data](https://www.nexusmods.com/oblivion/mods/53539?tab=files) (main file only) *A few post-install steps are required.*

- Using BAE, extract both archives to the mod's folder & delete both archives & Dagger_Data.esp once this is performed.

*Delete everything except:*

- Dagger_Data.esm

- Meshes \> Dag \> Architecture
- Meshes \> Dag \> Dungeons
- Meshes \> Dag \> clutter

- Textures \> dag
- Textures \> landscape
- Textures \> plants

49. [The Chorrol Graveyard Overhaul](https://www.nexusmods.com/oblivion/mods/53786)
50. [Cheydinhal Cemetery Overhaul](https://www.nexusmods.com/oblivion/mods/53797) \[**QAC**\]
51. [Gogan's Family Cemetery](https://www.nexusmods.com/oblivion/mods/53768) \[**QAC**\]
52. [Better Odiil Farm](https://www.nexusmods.com/oblivion/mods/52638) \[**QAC**\]

53. [SI Unmarked Locations](https://www.nexusmods.com/oblivion/mods/51169)

*Once installed delete SI Unmarked Locations without markers.esp*

54. [SI New Sheoth Outskirts](https://www.nexusmods.com/oblivion/mods/51232)
55. [SI Driftdwell](https://www.nexusmods.com/oblivion/mods/52324)
56. [SI Whispersins](https://www.nexusmods.com/oblivion/mods/51623)

57. [Deadlands](https://www.nexusmods.com/oblivion/mods/50437?tab=files) (main file only)

PART 26b - TOWN & CITY EXTRAS (TACE) MERGE
*Better Cities was a firm-favourite for years however due to its heavyweight implementation we had to remove it a while back. Yet I still miss it. With the above separator we've touched on many aspects of the worldmap, yet a new separator for 03.25 focuses solely on the Cities. Enjoy a one-plugin merge of all of the below (save the crucial merge-patch installed later), encompassing what I perceive as 'Better Cities Lite'.*

1. [Add some flavor - Talos Bridge](https://www.nexusmods.com/oblivion/mods/52658) (1st main file)
2. [Enhanced Cyrodiil - Cities](https://www.nexusmods.com/oblivion/mods/47205)

*Install manually, and right click Enhanced Cyrodiil - Cities \> **Standard** folder & select Set as \ directory.
*
3. [Add some flavor - city gates - without IC](https://www.nexusmods.com/oblivion/mods/52564?tab=files)

4. [Gardens of Cyrodiil - Castle Courtyards](https://www.nexusmods.com/oblivion/mods/54179?tab=files) - Cheydinhal Castle Courtyard
5. [Gardens of Cyrodiil - Cheydinhal Peach Tree Island](https://www.nexusmods.com/oblivion/mods/54050?tab=files) (main file) \[**QAC**\]
6. [Cheydinhal Garden](https://www.nexusmods.com/oblivion/mods/49544)

7. [Gardens of Cyrodiil - Castle Courtyards](https://www.nexusmods.com/oblivion/mods/54179) - Chorrol Castle Courtyard \[**QAC**\]
8. [Chorrol Great Oak Replacer](https://www.nexusmods.com/oblivion/mods/46950%20) \[**MI**\] *Right click **01 Classic Bark** & select Set as Data Directory.*
9. [Gardens of Cyrodiil - Chorrol Park](https://www.nexusmods.com/oblivion/mods/53900) \[**QAC**\]
10. [Chorrol Lower Class Houses](https://www.afkmods.com/index.php?/files/file/858-chorrol-lower-class-houses/) *Within the BAIN Installer, select:*

- 00 Core
- 01 Vanilla

11. [Falling Leaves Chorrol - Project Ambience](https://www.nexusmods.com/oblivion/mods/46462)

12. [People Live Here - Skingrad Enhancement Mod](https://www.nexusmods.com/oblivion/mods/52456?tab=files) (MOO Compatibility main file) \[**QAC**\] *Once QAC has performed, a few wild edits need removing:*

- Worldspace \> 0001C31D \ \> Delete Block -11, 2
- 0001C31D \ \> Block -1, 0 \> Sub-Block -2, 0 \> 0000A7E9 \ \> Delete xx002136
- 0001C31D \ \> Block -1, -1 \> Sub-Block -3, -1 \> Delete xx0020EE

*Save & close xedit.*

13. [Gardens of Cyrodiil - Castle Courtyards](https://www.nexusmods.com/oblivion/mods/54179) - Skingrad Castle Courtyard

14. [Gardens of Cyrodiil - Anvil the city of Dibella](https://www.nexusmods.com/oblivion/mods/54371?tab=files) (main file only) \[**QAC**\]
15. [Anvil Morning Glory](https://www.nexusmods.com/oblivion/mods/19039?tab=description) (1st main file) \[**MI**\] *Right-click Data & set as Data Directory, then deselect every esp except Anvil_MorningGlory_Mixed.* \[**QAC**\]

16. [Falling Rubbish Bravil - Project Ambience](https://www.nexusmods.com/oblivion/mods/46467)

17. [Falling Pollen Leyawiin - Project Ambience](https://www.nexusmods.com/oblivion/mods/46463)

18. [Slightly Different Bruma](https://www.nexusmods.com/oblivion/mods/54442?tab=files) (main file only)
19. [Gardens of Cyrodiil - Bruma Greenhouses](https://www.nexusmods.com/oblivion/mods/54002)

20. [Gardens of Cyrodiil - Knights of the Thorn Lodge](https://www.nexusmods.com/oblivion/mods/54522) \[**QAC**\]

## Part 27 - NEW & MODIFIED NPCs
*Similarly to part 24, this part is quite conservative given there have been a whole host of new NPCs added by the overhauls installed already. Again, these iron out the remaining creases in my mind as to what should have been in vanilla.*

1. [Collection of Cleaned - Updated - Fixed - UOP Compatible Mods](https://www.nexusmods.com/oblivion/mods/52833?tab=files) (More Mythic Dawn Agents - Cleaned)
2. [Vanilla Remastered 1K](https://www.nexusmods.com/oblivion/mods/50903?tab=files) (Very Horny Knights)
3. [Shivering Isles Raiders](https://www.nexusmods.com/oblivion/mods/51291)
4. [Countess](https://www.nexusmods.com/oblivion/mods/49602)
5. [Pinarus Inventius - Actual Hunter](https://www.nexusmods.com/oblivion/mods/48701)

6. [Akatosh Retexture by themythofstrider](https://www.nexusmods.com/oblivion/mods/29321)\[**MI**\]
7. [Tavern Goers 2 - Redux](https://www.nexusmods.com/oblivion/mods/48660?tab=files) (main file, Merged)

8. [Street Vendors of Cyrodiil](https://www.nexusmods.com/oblivion/mods/48143?tab=files) (main file v. 2.91 + optional Street Vendors of Cyrodiil v2.91 - Not in IC; install separately)

9. [Daedric Shrines Prodded With a Stick](https://www.nexusmods.com/oblivion/mods/46930?tab=files) (main file only)
10. [Culus the Mighty](https://www.nexusmods.com/oblivion/mods/48164)

Think of Culus as Dogmeat from Fallout - given Cheydinhal is always my first city to visit for the easy Guild-quests, it's almost a staple for the early game to have him by your side until more fitting followers join you on your journey.

11. [Shivering Isles Trainers (Partial)](https://www.nexusmods.com/oblivion/mods/50718)

## Part 28 - NEW & MODIFIED QUESTS
*Again, you may be thinking this is a very simple list. No huge overhauls such as Kvatch Rebuilt or Knights of the Nine Revelation. The four main drivers on the build for extra quests not only arise from the expansive overhauls already installed, but The Lost Spires (which was a precursor to the groundbreaking 'Legacy of the Dragonborn' in my mind), and The Ayleid Steps, a sprawling adventure with unexpected twists & turns. AFK Weye with Tales of Cyrodiil round things off in a lore-friendly way.

*1. [Progress Tracker - Quest Completionist's Companion](https://www.nexusmods.com/oblivion/mods/53328)
2. [Quest INIs for Progress Tracker](https://www.nexusmods.com/oblivion/mods/53886)

*The mod requires packaging for MO2, so once installed open the mod in explorer & perform the following:*

- Create a new folder called 'ini'
- Within this folder create another called 'progresstracker'
- Cut & paste the 7 ini's from the root into this folder & close explorer
- Right click the mod & select 'Ignore Missing Data'

3. [Progress Tracker Ini Mod Compendium](https://www.nexusmods.com/oblivion/mods/53942) (optional MO2 file)
4. [Progress Tracker - Even more Quest INIs](https://www.nexusmods.com/oblivion/mods/54885)

*The mod requires packaging for MO2, so once installed open the mod in explorer & perform the following:*

- Create a new folder called 'ini'
- Within this folder create another called 'progresstracker'
- Cut & paste the ini's from the root into this folder & close explorer
- Right click the mod & select 'Ignore Missing Data'

*
Feel like you're done? This suite of mods will indicate otherwise!*

5. [Configuration Items Begone](https://www.nexusmods.com/oblivion/mods/53354)

**Options**: Select both 'Apply filter patch for removing configuration items from mods' + 'Add LINK++ Support'
**Options**: Leave 'Don't add factions rating scroll' unselected
**Options**: Select 'Don't add torch hotkey item'

***NOTE**: MO2 may crash upon installing; simply reboot the app & the mod has installed successfully.
*

6. [Oblivion Content Restoration Project](https://www.nexusmods.com/oblivion/mods/45909?tab=files) (main file + optional file, install separately)
7. [Miscellaneous Patch Collection for Mods by Dispensation](https://www.nexusmods.com/oblivion/mods/52874?tab=files) (Install just Oblivion Content Restoration Project Patches, and name the mod this way)

8. [The Lost Spires](https://web.archive.org/web/20210225110856/http://www.lostspires.com/pages/downloads.htm) (The Lost Spires v.14 file only) \[**QAC**\] Alternative download [here](https://www.moddb.com/mods/oblivion-overhaul-mod/downloads/the-lost-spires-v14).
9. [The Lost Spires - Cleaned Up Scribe Store Ruins](https://www.nexusmods.com/oblivion/mods/50146) (main file only)
10. [The Lost Spires - Tweaks and Enhancements](https://www.nexusmods.com/oblivion/mods/51037?tab=files) (LS - Assorted Fixes)
11. [Collection of Cleaned - Updated - Fixed - UOP Compatible Mods](https://www.nexusmods.com/oblivion/mods/52833) (The Lost Spires - NPC AI Addon)
12. [Lost Spires Archaeology Guild Robe fix](https://www.nexusmods.com/oblivion/mods/49343)

13. [Bash-able Quest Delayers](https://www.nexusmods.com/oblivion/mods/25946?tab=description) (Aellis Bashed Delayers v3129C)

14. [The Ayleid Steps](https://www.nexusmods.com/oblivion/mods/16316?tab=files) (main file only)
15. [Voiced Addons Collection for Mods (ElevenLabs)](https://www.nexusmods.com/oblivion/mods/52772) (The Ayleid Steps - Voiced Addon BSA)
16. [The Ayleid Steps - The Guardian's Atlas](https://www.nexusmods.com/oblivion/mods/51923)
17. [The Ayleid Steps - Compatibility Patches](https://www.nexusmods.com/oblivion/mods/47142?tab=files)

18. [A Brotherhood Renewed](https://www.afkmods.com/index.php?/files/file/260-a-brotherhood-renewed/)

*Once installed delete the Sound \> Voice folder.*

19a. [Better Dark Brotherhood Sanctuary](https://www.nexusmods.com/oblivion/mods/22135?tab=files)

*Within the BAIN Package Installer, select:*

00 Core Files (Required)
01 Cobl Version

19b. [Voiced Addons Collection for Mods (ElevenLabs)](https://www.nexusmods.com/oblivion/mods/52772?tab=files) : Better Dark Brotherhood Sanctuary - Voiced Addon

20. [Thievery in the Imperial City - Tweaks and Fixes](https://www.nexusmods.com/oblivion/mods/52785?tab=files) (main file)
21. [Bounty Quests Fixed and Polished](https://www.nexusmods.com/oblivion/mods/48330)

*Once installed delete the Bounty Quests OOO Patch.esp*

22. [Voiced Addons Collection for Mods (ElevenLabs)](https://www.nexusmods.com/oblivion/mods/52772) (Bounty Quests Fixed and Polished - Voiced Addon BSA)

23. [No More Wild Goose Chases - Re-Patched and UOP Fixes Applied](https://www.nexusmods.com/oblivion/mods/49958)
24. [Sinderion's Serendipity - Nirnroot Quest Reward](https://www.nexusmods.com/oblivion/mods/50129)

25. [SM DLC Plugin Refurbish](https://www.nexusmods.com/oblivion/mods/11474)

*Within the BAIN Package Installer, select:*

- 00 Full Lite Plugin
- 01 OBSE Level Plugin
- 02 Compatability Plugins

26. [Voiced Addons Collection for Mods (ElevenLabs)](https://www.nexusmods.com/oblivion/mods/52772) (SM DLC Plugin Refurbish - Voiced Addon)

27. [Fame Based Daedric Quest Requirements](https://www.nexusmods.com/oblivion/mods/42614)

*Once installed delete DaedricRequirementsEASY.esp.*

28. [Shivering Isles - New Dukes](https://www.nexusmods.com/oblivion/mods/50047)

29. [HackDirt The Deep Ones](https://www.nexusmods.com/oblivion/mods/36224)

*Install manually & right click the HackdirtTheDeepOnes3.3 \> **Data** folder and select Set as \ directory.*

30. [The Well of Minlorada](https://www.nexusmods.com/oblivion/mods/38816)
31. [AI Voice Addon for The Well of Minlorada](https://www.nexusmods.com/oblivion/mods/53401)

32. [AFK_Weye](https://www.nexusmods.com/oblivion/mods/22828) *In the BAIN Installer, select:*

- 00 Core
- 01 Cobl

33. [AFK Weye Voices (ElevenLabs)](https://www.nexusmods.com/oblivion/mods/54851) *Once installed, perform the following (similar process for the Arthmoor mods installed earlier):*

- Using BAE, extract AFK_Weye.bsa from the main mod (32) to its install location
- Copy the Sound folder from mod 33 to mod 32, replacing when prompted
- Using BSArch, repack the textures, meshes & sound folder & update the existing bsa (i.e., no need to rename it)
- Delete the loose textures, meshes & sound folders from mod 32
- Disable mod 33

34. [AFK_Weye Manor VWD](https://www.nexusmods.com/oblivion/mods/49322)
35. [Miscellaneous Patch Collection by Dispensation](https://www.nexusmods.com/oblivion/mods/52874?tab=files) - *Select just 'AFK_Weye - Reworked Posts Patch' & name the mod **Miscellaneous Patch Collection by Dispensation - AFK Weye***
36. [AFK_Weye - Typo and Grammatical Patch](https://www.nexusmods.com/oblivion/mods/52800)

37. [Tales of Cyrodiil](https://www.nexusmods.com/oblivion/mods/48792)
38. [Tales of Cyrodiil Voices (ElevenLabs)](https://www.nexusmods.com/oblivion/mods/54825) *Once installed pack the sound folder using BSArch & name the archive 'Tales of Cyrodiil.bsa' & delete the loose Sound folder.*

## Part 29 - ANIMATIONS
*Animation mods in this generation of modding simply made the game look utterly bizarre in this humble Modder's opinion. Below are a simple but staple selection in fixing what was sorely needed.*

1. [Smoother Horse Animations](https://www.nexusmods.com/oblivion/mods/45146)
2. [Faster Horses](https://www.nexusmods.com/oblivion/mods/20265)
3. [Faster Horse Dismount](https://www.nexusmods.com/oblivion/mods/50226)4. [Combat Stance Reanimation](https://www.nexusmods.com/oblivion/mods/43695) \[**MI**\]

*During MI, right click & select **Core** and select Set as \ Directory*

5. [Stylish Jump - Animation Replacer](https://www.nexusmods.com/oblivion/mods/20459) (main file only)

*Install manually & right click the 'Normal' folder and select Set as \ directory.*

6. [Lich - Skeleton Hand-To-Hand Animations](https://www.nexusmods.com/oblivion/mods/48606)
7. [Mehrunes Dagon Walking Animation](https://www.nexusmods.com/oblivion/mods/52126)

*Once installed delete MehrunesDagonWalk.esp*

8. [Wrye Bash Collection of Mergeable Mods](https://www.nexusmods.com/oblivion/mods/52823) (Mehrunes Dagon Walking Animation - Mergeable)
9. [Unique Wolf Animations Restored](https://www.nexusmods.com/oblivion/mods/53042?tab=files&file_id=1000034956) \[**MI**\]

- Expand Unique Wolf Animations Restored
- Expand 01 Patches
- Drag Wolf Animations Restored - MOO Patch.esp into 00 Core
- Right click 00 Core & select Set as \ Directory

10. [NPC Idle Animation Restoration and Additions](https://www.nexusmods.com/oblivion/mods/53184)

## Part 30 - SKILLS & LEVELLING
1. [Ultimate Leveling](https://www.nexusmods.com/oblivion/mods/49134)

*Once installed, open the mod in Explorer & within **Ultimate Leveling for advanced users.ini**, apply the following edits:*
`set ULVL.ini_xp_kill_show_level to 1`

`set ULVL.ini_xp_skill_cap to 200`

`set ULVL.ini_xp_level_mult to 400`
`set ULVL.ini_xp_level_base to 1000`

`set ULVL.ini_xp_skill_level_points_journeyman to 2`
`set ULVL.ini_xp_skill_level_points_expert to 3`

`set ULVL.ini_xp_skill_level_points_minor to 10`

`set ULVL.ini_xp_read_skillbook_minor to 4`
`set ULVL.ini_xp_read_skillbook_major to 8`

`set ULVL.ini_xp_train_minor to 2`
`set ULVL.ini_xp_train_major to 3`

`set ULVL.ini_horseshoe_total to 0`
`set ULVL.ini_UI_horseshoes to 0`

`set ULVL.ini_rested_bonus to 20`

*Lastly, one edit in **Ultimate Leveling.ini**:*

`set ULVL.ini_add_horseshoes to 0`

*This is a gamechanger. Whereas I'm sure we all mastered our own techniques of power levelling in vanilla, this returns us to a more fluid & immersive way of levelling. Note in the Ini settings above, my 'arc' for levelling is acute for early levels & pans off toward 20+. This allows for a more forgiving early-game given we have MOO & OOO hardening the core mechanics.

Each level increases with the amount of XP in such a manner so that by level 20, you'll need ~20k in XP, at level 30, 30k XP, and so on.*

2. [OCRAFT - Oblivion Crafting Framework](https://www.nexusmods.com/oblivion/mods/51796)
3. [OCRAFT - Cobl Glue](https://www.nexusmods.com/oblivion/mods/54430)
4. [OCRAFT - Stations for Sale](https://www.nexusmods.com/oblivion/mods/54541?tab=files) (1st main file)

*A new mod to hit Nexus that allows the player to purchase the key-stations for either the inventory, or within owned houses. Note that weights have been adjusted in CR for a more immersive approach.*

5. [OCRAFT - Compatibility Settings](https://www.nexusmods.com/oblivion/mods/53220?tab=files)

*It's rare if non-existent for me to think 'If only Oblivion had this from Skyrim...' This superb suite of mods are one of the few exceptions
to that rule.*

6. [Fundament](https://www.nexusmods.com/oblivion/mods/41005)

*Once installed delete Bundlement.esp.*

7. [FEA - Fundament Enchanting Addons](https://www.nexusmods.com/oblivion/mods/41553)

*Within the BAIN Package Installer, select just 00 Core.

Once installed, paste the following into **Custom Trainers.ini:***
`set migFeaQ.customTrain to (GetFormFromMod "Oblivion.esm" 02D025)``;Uurwen`
`set migFeaQ.customLevl to 65`
`SetStage migFeaQ 1`
`set migFeaQ.customTrain to (GetFormFromMod "Oblivion.esm" 015EA9)``;Calindil`
`set migFeaQ.customLevl to 40`
`SetStage migFeaQ 1`
`set migFeaQ.customTrain to (GetFormFromMod "Oblivion.esm" 0222B7)``;Contumeliorus Florius`
`set migFeaQ.customLevl to 65`
`SetStage migFeaQ 1`

*Be sure to read the description page if unfamiliar with this mod.*

8. [PSO - Pickpocket Skill Overhaul](https://www.nexusmods.com/oblivion/mods/48118)
9. [Dynamic Training Cost](https://www.nexusmods.com/oblivion/mods/46373)

*Within MigTraining.ini, enable the following parameters by setting them to 1:*
`set migTrainingQ.bDisplaySkillNumbers`
`set migTrainingQ.bTrainSkillAdjust`
`set migTrainingQ.bTrainAttAdjust`
`set migTrainingQ.bTrainerSkillAdjust`
`set migTrainingQ.bTrainDispAdjust`

10. [Auto Update Leveled Items And Spells](https://www.nexusmods.com/oblivion/mods/39635)

*Once installed open Auto Update Leveled Items and Spells.ini in the INI Tab of MO2 & change **Set AULIAS.FotMCostMult** to 0.*

11. [Auto Update Leveled Items And Spells](https://www.nexusmods.com/oblivion/mods/52229?tab=files) - Script Patch (Main file only)

12. [Gent's Level Scaling Overhaul - Alternate Version](https://www.nexusmods.com/oblivion/mods/54700?tab=description)
13. [More Books Teach](https://www.nexusmods.com/oblivion/mods/3075)
14. [Dahyka's Vanilla Racials and Birthsigns Improved](https://www.nexusmods.com/oblivion/mods/47639)
15. [OCOv2 - Race Rebalance Mods Patches](https://www.nexusmods.com/oblivion/mods/49864) - Oblivion Character Overhaul V2 - Dahyka's Vanilla Racials and Birthsigns Improved Patch (OCO Uses Merged Teeth)

## Part 31 - AUDIO & DIALOGUE IMPROVEMENTS
1. [Symphony of Violence - Combat Sound Enhancement](https://www.nexusmods.com/oblivion/mods/13987)
2. [GOSH - Gecko's Oblivion Sound overHaul](https://www.nexusmods.com/oblivion/mods/45214)

*Once installed delete GOSH Region Ambiance.esp*

3. [Vicious Trolls Sound Replacer](https://www.nexusmods.com/oblivion/mods/50294?tab=files)
4. [Diablo-like Goblins](https://www.nexusmods.com/oblivion/mods/48477?tab=files) (Goblin Aesthetics Tweak - Sound Replacer)

5. [Dialogue Tweaks Fixes and Restorations](https://www.nexusmods.com/oblivion/mods/44862) *Once installed:*

- Open Ini Files tab & within Dialog TFR Costs.ini
- set TrespassDialogRestore�� to�� 0

6. [Realistic Player Dialogue Overhaul](https://www.nexusmods.com/oblivion/mods/46243)

7. [miguick Dialogue Tweaks Tweaked Trespassing](https://www.nexusmods.com/oblivion/mods/50891)
8. [Horse Hoof Sounds](https://www.nexusmods.com/oblivion/mods/45112) (main file only)
9. [Consistent Beggar Voices](https://www.nexusmods.com/oblivion/mods/48336)
10. [Nightmarish Oblivion Gate Sounds](https://www.nexusmods.com/oblivion/mods/45609)
11. [Expanded Greetings](https://www.nexusmods.com/oblivion/mods/33979)
12. [Louder Chapel Bells](https://www.nexusmods.com/oblivion/mods/52468)
13. [Voices for Female Dremora NPCs](https://www.nexusmods.com/oblivion/mods/52498)

14. [Swearing Rats](https://www.nexusmods.com/oblivion/mods/52661)

*OK; you might be thinking 'What?' Similarly to Skyrim's [Swearing Mudcrabs](https://www.nexusmods.com/skyrimspecialedition/mods/1951), it injects some humour & sillyness, reminding us we're playing a fantasy game after all. I can understand if you omit it, but it never fails to raise a cheeky grin.*

15. [Your Mother Was a Hamster](https://www.nexusmods.com/oblivion/mods/42960?tab=description) (main file only) *Once installed open the INI Files tab & apply the following to the ini:*

- set aaTauntQuest.aaTauntMult to 1

16. [Female Grunts Replacer](https://www.nexusmods.com/oblivion/mods/54471)
17. [Quieter Dragon Sounds (For MOO)](https://www.nexusmods.com/oblivion/mods/54072?tab=files)
18. [Enhanced Music Overhaul](https://www.nexusmods.com/oblivion/mods/54652) \[**MI**\]
19. [Disable Detect Life During Dialog](https://www.nexusmods.com/oblivion/mods/54942)

## Part 32 - COMMON OBLIVION (COBL)
*Warm fuzzy feelings embrace me with Common Oblivion present. It introduces such wonderful QoL features that blend seamlessly with the gameplay. Don't forget your luggage!*

1. [Cobl](https://www.nexusmods.com/oblivion/mods/21104?tab=files) (main file)

**Stable or Development**: Stable
**Packages**: Tweaks
**Options**: Cobl Tweaks - SI
**Misc**: Nothing

*Once installed, delete every plugin except:*

Cobl Main.esm
Cobl Glue.esp
Cobl Si.esp
Cobl Filter Late MERGE ONLY.esp

2. [TIBs Compact Quivers - Thinner Arrow Holders](https://www.nexusmods.com/oblivion/mods/45111?tab=files) (TIBs Compact Quivers - Manual install - Apachii and COBL)

*Once installed delete the following folders:*

- meshes \> weapons \> Apachii
- meshes \> weapons \> apachiiMale
- textures \> apachii
- textures \> apachiiMale

3. [Khettienna's Mini-Mods](https://www.nexusmods.com/oblivion/mods/46187?tab=files) (KMM Higher-Res Welkynd Textures for Cobl Ayleid Meteoric Weapons)
4. [Cobl Unofficial Patch](https://www.nexusmods.com/oblivion/mods/51517)

*Once installed, delete **Salmo the Baker, Cobl.esp***

5. [Pek COBL Book Jackets - Stand Alone](https://www.nexusmods.com/oblivion/mods/51953)

*Once installed delete PekCOBLBookJackets.esp*

6. [Wrye Bash Collection of Mergeable Mods](https://www.nexusmods.com/oblivion/mods/52823?tab=files) (Pekkas COBL Books Jackets - Mergeable Replacer ESP)
7. [Cobl for DLC Homes](https://www.nexusmods.com/oblivion/mods/53063?tab=files) (main file only)
8. [Legacy of the Champion](https://www.nexusmods.com/oblivion/mods/52047?tab=files) (main file only) *Once installed delete everything except:*

- textures \> custom \> 3bears \> porridge\*.dds (2 files)

*This fixes a missing texture within the COBL ingredients. Kudos as always to Durbinh on the spot. Rename the mod to Legacy of the Champion (Cobl Porridge DDS) for ease of reference.*

## Part 33 - GAMEPLAY & IMMERSIVE EXTRAS
1. [SupreMe Overhaul](https://www.nexusmods.com/oblivion/mods/51073) Once installed delete the sound folder.

*A large mini-merge of the author's work, this particular mod has great benefits. Be sure to download the Consribe Logs MOFAM LINK Settings from my modpage that essentially enables 4 parts of the mod:*

- An improved Bounty & Crime system
- Health Regeneration outside of combat *(spamming Restoration no longer yields XP given we use Ultimate Levelling)*
- Movement & Encumbrance *(this superseded both 'Basic Physical Activities' sprinting mechanic & 'Move While Encumbered' however with added details for the player character)*
- Combat Hide

2. [AutoHaggle](https://www.nexusmods.com/oblivion/mods/52532)

3. [Bank of Cyrodiil](https://www.nexusmods.com/oblivion/mods/3172) *This mod is a little unusual in terms of its installation.*

- Simply press OK as normal & ignore the Continue? MO2 popup.
- Open the mod in explorer & select Bank of Cyrodiil 1-11.exe
- The mod's install location should be defaulted in terms of 'Extract to:' so select 'Extract'
- Optionally delete Bank of Cyrodiil 1-11.exe

4. [Bank of Cyrodiil Voices](https://www.nexusmods.com/oblivion/mods/54289?tab=posts)

*Once installed, perform the following (similar process for the Arthmoor mods installed earlier):*

- Copy the Sound folder from mod 4 to mod 3, replacing when prompted
- Using BSArch, repack the textures, meshes & sound folder & name the archive za_bankmod
- Delete the loose textures, meshes & sound folders from mod 3
- Disable mod 4

5. [Crime has witnesses](https://www.nexusmods.com/oblivion/mods/22894)

*Once installed, open the mod in MO2 & within the INI Files tab, apply the following to kuerteeCrimeHasWitnesses.ini:*

`set kCWWQuest.showWitnesses to 1`
*
Lastly, delete the omod conversion data folder.*

6. [Crime Has Witnesses - Responsibility Tweak](https://www.nexusmods.com/oblivion/mods/33682)
7. [Reznod Mannequins](https://www.nexusmods.com/oblivion/mods/2055)

8. [Put it in its Place - Enhanced Grabbing](https://www.nexusmods.com/oblivion/mods/19847) (main file only)
9. [Take or Equip](https://www.nexusmods.com/oblivion/mods/52733?tab=files) (main file only)

10. [Convenient Open Spells](https://www.nexusmods.com/oblivion/mods/53163)

11a. [Greed Arena (AoG)](https://www.nexusmods.com/oblivion/mods/48277) \[**QAC**\]

*Once installed, open the mod in MO2 & within the INI Files tab, apply the following to GreedArena.ini:
*

`set aogArn.arena24 to 1`
`set aogArn.corpseLoot to 1`

11b. [Voiced Addons Collection for Mods (ElevenLabs)](https://www.nexusmods.com/oblivion/mods/52772) : Greed Arena (AoG) - Voiced Addon Loose

12. [Camping](https://www.nexusmods.com/oblivion/mods/37197)

*Once installed, *delete the omod conversion data folder.**

13. [Note To Self](https://www.nexusmods.com/oblivion/mods/37909)

*Indeed book writing kits are part of MOO but this mod allows the PC to have the capabilities from the get-go.*

14. [Quests make friends](https://www.nexusmods.com/oblivion/mods/51792)\[**MI**\]

15. [Weightless Arrows-Potions-Other Consumables](https://www.nexusmods.com/oblivion/mods/47916)
16. [Weightless Varla and Welkynd stones](https://www.nexusmods.com/oblivion/mods/51520)
17. [Respawning Hollowed Amber Stumps and Madness Ore Deposits](https://www.nexusmods.com/oblivion/mods/48056?tab=files) (main file + optional, install separately)

*Similarly to my other guide for FO4, having the principle items of note within the inventory to have weight (i.e. weapons & armour) without worrying about ingredients, potions & clutter, reduces the tedious gameplay loops involved. We also make some further weight-tweaks in the Bashed patch later, and Maskar's items have similarly been reduced in weight in my CR patch.*

18. [Base Object Swapper Integrations](https://www.nexusmods.com/oblivion/mods/53877?tab=files) *Install separately:*

- Cobl - Food
- OCRAFT - Stations
- OOO - Treasure

19. [AsteriaSennall's MOFAM Fixes](https://www.nexusmods.com/oblivion/mods/52949?tab=files)

*AstreriaSennall's been a great contributor in my Discord for the Oblivion project, so enjoy these finely-tuned tweaks & fixes that I've merged into one download. They include:*

- KatKat Uppertable Fixed
- Really Fixed Havok Training Dummy
- Improved Fighters Guild ENG - Fixes v1.2
- Improved Mages Guild ENG - Fixes v1.4
- Mesh Gap Fixes

20. [Sneak Vignette](https://www.nexusmods.com/oblivion/mods/54862)
21. [Drowning Visuals](https://www.nexusmods.com/oblivion/mods/54926?tab=posts)

Breathe a sigh of relief. You've made it this far, and the end is in sight. Well done! But you didnt think we could throw all this together and hope for the best? Now comes the fun part of installing the remaining patches, conflict-resolving, running Lods & automation.

## Part 34 - FINAL FILTER PATCHES
*Filter patches are a smart & unique aspect of modding Oblivion. Ignore the missing master warnings in MO2 as these are designed for the Bashed Patch to only consume the existing masters in the LO.*

1. [Ragdolls for Oblivion - Filter Patch for Mods](https://www.nexusmods.com/oblivion/mods/52804)
2. [OCOv2 - Filter Patch for Mods](https://www.nexusmods.com/oblivion/mods/51379?tab=files) (main file)
3. [Basic harvest](https://www.nexusmods.com/oblivion/mods/51833?tab=files)

*Install manually & right click BasicHarvest_FilterPatch_V1.4 \> **00_CoreFilterPatch**, and select Set as \ directory.*

4. [MOBSification of MODS - Weapon Balancing](https://www.nexusmods.com/oblivion/mods/45522) (main file)
5. [Miscellaneous Patch Collection for Mods by Dispensation](https://www.nexusmods.com/oblivion/mods/52874?tab=files) *

I'd recommend calling the mod 'Miscellaneous Patch Collection by Dispensation - Filter Patches' as we've installed this download several times already. Select just*:

- OCRAFT Patches

*Finally, delete **DispMiscPatch_OCRAFT - Better Camps Patch** + **DispMiscPatch_RadiantAI NPCs Alive Cyrodiil - OCRAFT Int Patch** esp's.
*
6. [Cava Obscura - Updated Filter Patch for Mods](https://www.nexusmods.com/oblivion/mods/35099?tab=files)
7. [Unique Landscapes - OOO Adaptation](https://www.nexusmods.com/oblivion/mods/48463)

- Using the BAIN installer, select just **01 Separate UL**

## Part 35 - OSCURO'S PATCHES
1. [Improved Chests](https://www.nexusmods.com/oblivion/mods/51815?tab=files) (Improved Chests (OOO Compatible))

*Within the BAIN Installer, select just Improved Chests (OOO Compatible)
*
2. [OOO - Oscuro's Oblivion Overhaul - Updated (Unofficial patch)](https://www.nexusmods.com/oblivion/mods/54069?tab=files) (OOO Update Patch full main file)

*A new addition that refines the epic OOO to further modernised standards. Give the MA a kudos.*

3. [COBL Tweaks - MOFAM Patch](https://www.nexusmods.com/oblivion/mods/52949?tab=files)
4. [Better Dark Brotherhood Sanctuary - OOO Patch](https://www.nexusmods.com/oblivion/mods/48560)
5. [Unique Artifacts for Unique People](https://www.nexusmods.com/oblivion/mods/49871?tab=files) (Unique Artifacts for Unique People - Patches)

*Within the BAIN Package Installer, select just:*

- 01 OOO Patch

6. [Weapons Of Morrowind - OOO (Extended) Patch](https://www.nexusmods.com/oblivion/mods/52307?tab=files)

7. [Bounty Quests Fixed and Polished](https://www.nexusmods.com/oblivion/mods/48330?tab=description)

*Install manually & deselect everything except Bounty Quests OOO Patch.esp.*

8. [Retextured Potions - OOO Patch](https://www.nexusmods.com/oblivion/mods/49256)

9. [Local Guards Features](https://www.nexusmods.com/oblivion/mods/45517)

*Install manually & right click the Local Guards Features \> **Oscuro's Oblivion Overhaul Patch** and select Set as \ directory.
Lastly, name the mod '**Local Guards Features - OOO Patch'***

10. [OOO Spectral Fix](https://www.nexusmods.com/oblivion/mods/52949?tab=files)*
*11. [OOO CoblTweaks Fix](https://www.nexusmods.com/oblivion/mods/52949?tab=files)
12. [Various OOO Adaptations (Arthmoor mods MOFAM Edit)](https://www.nexusmods.com/oblivion/mods/52949?tab=files) *

This is a master-cleaned variant of Saldron's original, to use only the Arthmoor mods we use hence saving a plugin slot as we merge it.*

## Part 36 - zMERGED PLUGINS
- **6** Merges.
- I would **strongly recommend** creating a new MO2 profile for each merge, naming it such as 'MERGE - PREBASH'. To do so simply copy the MOFAM profile from within the Profiles menu & name it as suggested. This will also help display your profiles within MO2 alphabetically.
- Be sure to setup **Integration** settings within zmerge & disabling plugins in Merge **Settings**. We use merge plugins hide for this.
- Merge type is always default, CLOBBER
- **IMPORTANT**! The Bash Tags are provided in my [Conflict Resolution](https://www.nexusmods.com/oblivion/mods/52949?tab=files) mod so please be precise with the naming conventions as they have to match.
- Id also suggest taking this part **slowly** if new to the process. Plenty of help online in the link just below plus my discord has been helping with zmerge for a few years now.
- A new addition is **[Prepare Merge](https://www.nexusmods.com/skyrimspecialedition/mods/47791)** plugin. This is an incredible tool that automates creating the required per-profile Load Order for merging.
- Some basic steps are below however if the process is still proving a headache, Blasphemous Wartotle's videos [here](https://www.youtube.com/channel/UCbjg6D8oJotlkxM6QQ5ePQw) (although made for Lexy's SSE guide, are still relevant) can help.
1 - Ensure all plugins are active in the main MOFAM profile
2 - From the MO2 Tools menu, select Prepare Merge \> Load active profile as base
3 - Navigate to or if new, create (e.g.) MERGE - PREBASH profile within MO2 Profiles menu.
4 - Copy all the plugins from the relevant section on this modpage.
5 - Within Prepare Merge, select Import entries from clipboard
6 - Select Prepare merge in active profile

1. [OPM Consistency Patch](https://www.nexusmods.com/oblivion/mods/52949?tab=files)

**2. OOO Patches Merged**

*Create a new profile called MERGE - OOO and using Prepare Merge, create the following output:*
Oscuro's_Oblivion_Overhaul - Knights of Nine.esp
DLCFrostcrag - OOO Adaptation.esp
DLCHorseArmor - OOO Adaptation.esp
DLCBattlehornCastle - OOO Adaptation.esp
DLCMehrunesRazor - OOO Adaptation.esp
DLCThievesDen - OOO Adaptation.esp
DLCVileLair - OOO Adaptation.esp
OOO Enhanced - Shivering Isles.esp
Arthmoor Villages - OOO Adaptation.esp
BDBS - Oscuro's Oblivion Overhaul.esp
UAUP - OOO Patch.esp
Weapons Of Morrowind - OOO (Extended) Patch.esp
PotionReplacer OOO Patch.esp
tbskGuardsFeatures OOO Patch.esp
OOO Spectral Fix.esp
OOOCoblTweaksFix.esp
OOO_Unoficial-Patch.esp
OOO Enhanced - Av Latta Magica Compatibility Patch.esp
OPM Consistency Patch.esp

- **Name**: OOO Patches Merged
- **Filename**: OOO Patches Merged

3. [UFM Consistency Patch](https://www.nexusmods.com/oblivion/mods/52949?tab=files)

**4. Unique Forts Merged**

*Create a new profile called MERGE - UFM & using Prepare Merge, create the following output:*
Unique Forts Fort Aurus.esp
Unique Forts Fort Doublecross.esp
Unique Forts Fort Facian.esp
Unique Forts Fort Hastrel.esp
Unique Forts Fort Irony.esp
Unique Forts Fort Naso.esp
Unique Forts Fort Rayles.esp
Unique Forts Fort Redman.esp
Unique Forts Fort Teleman.esp
Unique Forts Fort Vlastarus.esp
UFM Consistency Patch.esp

- **Name**: Unique Forts Merged
- **Filename**: Unique Forts Merged

2. [Prebash Merge Consistency Patch](https://www.nexusmods.com/oblivion/mods/52949?tab=files)

*Install this patch as it's included in the following merge. ABSOLUTELY CRUCIAL to have this given core-CR is performed within it.*

**3. Prebash Merge**

*Create a new profile called MERGE - PREBASH and using Prepare Merge, create the following output:
*
Oblivion Citadel Door Fix.esp
UOPTalosBridgeCollisionFix.esp
DLCSpellTomes - Unofficial Patch.esp
Av Latta Magicka - Rebalance Complete.esp
MageGuild_simbol.esp
WeightlessAmmoConsumablesPotions.esp
weightlessstones.esp
aesrespawningamberstumpsandmadnessoredeposits.esp
aesrespawninggoldandsilverveins.esp
DLCVileLair - Tweaks.esp
DLCThievesDen - BarterForUpgrd.esp
FasterHorses.esp
Initial Glow Redux - Creatures.esp
Initial Glow Redux.esp
lGet rid of Small Souls - Soulgem Prices.esp
Bibliophilia.esp
Banes Steel Helm Replacer.esp
ReteroX_ClosedIronHelmet_v1.esp
ReteroX_ClosedEbonyHelmet_v1.esp
Imperial City Landscape Fix.esp
Alternate ghost shader.esp
Oblivion Content Restoration Project -- Knights.esp
DialogTweaksTweakedTrespassing.esp
00 Realistic Player Speech.esp
lVanilla Style Loading Screens Addon.esp
xxReworkedReedstand.esp
xxreworkedgottshaw.esp
xxreworkedSutch.esp
Louder Chapel Bells.esp
PinarusInventiusActualHunter.esp
Unused Magic Items Integrated.esp
Voices for Female Dremora NPCs.esp
The Ayleid Steps - Patches.esp
lLet there be Flowers.esp
GuardInfamyGreetingFix.esp
AyleidWellMessage.esp
GoblinTribesFixed.esp
ExpandedGreetings.esp
tbskGuardsFeaturesMergedVanillaAddons.esp
DLCSpellTomes-No Attack.esp
SM Plugin Refurbish Lite Knights Infamy.esp
Bruma Frostcrag Spire LOD.esp
ORC.esp
MehrunesDagonWalk.esp
Better Traps.esp
DaedricRequirements.esp
OCOv2 Beast Races Enhanced.esp
WAC - HGEC Equipment Replacer.esp
Wolf Animations Restored.esp
The Ayleid Steps - Voiced Addon.esp
Shivering Isles Female Armor Displays.esp
DispMiscPatch_OCOv2 - Adoring Fan No Beard.esp
DispMiscPatch_Ducks and Swans - Reedstand Patch.esp
WAC Integration HGEC Gauntlets Patch.esp
The Lost Spires - NPC AI Addon.esp
Locked Fighters Guild Doors Bug Fix.esp
DispMiscPatch_AddSomeFlavorRoadsideInns - GottshawVillage Patch.esp
ApparatusIcons.esp
Harvest \[Flora\] - DLCFrostcrag.esp
Harvest \[Flora\] - DLCVileLair.esp
Harvest \[Flora\] - Shivering Isles.esp
Female Amulets.esp
more books teach.esp
Book Jackets Oblivion.esp
Knights - Book Jackets.esp
Book Jackets DLC Misc.esp
Alluring Wine Bottles.esp
Better minotaurs.esp
BetterLorgrenBenirus_NoStaffEdit.esp
KingofMisc.esp
BattlehornLich.esp
Weapon Improvement Project.esp
Goblin Totem Staff Icon.esp
PekCOBLBookJackets.esp
Improved Fires and Flames - Increased Sound.esp
Symphony of Violence.esp
Vicious Trolls Sound Attenuator.esp
PotionReplacer.esp
CoopArmoredLegionHorses.esp
DLCHorseArmor - Mane Enabled.esp
ShiveringIslesTrainersPartial.esp
SavillaStoneEnhanced.esp
DispMiscPatch_AFK_Weye - Reworked Posts Patch.esp
AFK_Weye - Typo and Grammatical Patch.esp
Minotaur Horns Fix.esp
Merge Consistency Patch.esp

- **Name**: Prebash Merge
- **Filename**: Prebash Merge

4. [TACE Consistency Patch](https://www.nexusmods.com/oblivion/mods/52949?tab=files)
**
5. TACE Merge
**
*Create a new profile called MERGE - TACE and using Prepare Merge, create the following output:
*
Anvil the city of Dibella.esp
Anvil_MorningGlory_Mixed.esp
Cheydinhal Castle Courtyard.esp
Cheydinhal Peach Tree Island.esp
CheydinhalGarden.esp
Chorrol Castle Courtyard.esp
Chorrol Park.esp
ChorrolGreatOakReplacer.esp
Falling leaves Chorrol.esp
Chorrol LCH.esp
Falling Pollen Leyawiin.esp
Falling Rubbish Bravil.esp
Gardens of Cyrodill-Knights of the Thorn Lodge.esp
Skingrad Castle Courtyard.esp
SkingradDeuglified.esp
Add some flavor - city gates - without IC.esp
Enhanced Cyrodiil - Cities.esp
Add some flavor - Talos bridge.esp
Bruma Greenhouses.esp
Slightly Different Bruma.esp
TACE Consistency Patch.esp

- **Name**: TACE Merge
- **Filename**: TACE Merge

6\. [Land Magic](https://www.nexusmods.com/oblivion/mods/52949?tab=files)**

7. [Late Loaders Consistency Patch](https://www.nexusmods.com/oblivion/mods/52949?tab=files)

8. Late Loaders Merged**

*Create a new profile called MERGE - LATE LOADERS and using Prepare Merge, create the following output:
*
WACIntegration - MOO Patch.esp
All Natural - Real Lights - candelabra pathgrid fix.esp
DispMiscPatch_Av Latta Magicka - Migck Misc Poison Fists Patch.esp
tbskGuardsFeatures_UOP_Patch.esp
LFO - Local Guards Features Patch.esp
DispMiscPatch_OCRP_VGR_Patch.esp
DispMiscPatch_OCRP - Original Brown Leather Armor Restored.esp
Auto Update Leveled Items And Spells - Script Patch.esp
Cobl Glue - Bravil Barrel Fix.esp
OCOv2 - MOO Patch.esp
MOO MOBS Patch.esp
Wolf Animations Restored - MOO Patch.esp
MGE Compatibility.esp
Improved MG Patch.esp
FGE Compatibility.esp
Improved FG Patch.esp
LandMagicPatch.esp
Cava Obscura - Cyrodiil.esp
Cava Obscura - SI.esp
Late Loaders Consistency Patch.esp

**Name**: Late Loaders Merged
**Filename**: Late Loaders Merged

9. [NPC Merge Consistency Patch](https://www.nexusmods.com/oblivion/mods/52949?tab=files)***

**A crucial patch & addition within 11.24 containing just over **1400** NPC's fully CR'd with the build. This was a long time coming on my part, given Wrye Bash's automation capabilities for addressing this crucial record group (within any LO) misses the mark in many instances.**

*****10.** **NPC Merge**

*Create a new profile called MERGE - NPC and using Prepare Merge, create the following output:*
***
***
***

***LFO - All Races Addon.esp
Oblivion_Character_Overhaul_Faces.esp
LFO OCO New Eyes.esp
NPC Hair Matches Beard.esp
OCO uses merged teeth.esp
Improved NPC Faces for OCOv2.esp
OCOv2 - DLC Addon.esp
OCO Unused Eyes and DLC Eyes Incorporated.esp
OOOShiveringIsles_OCO_Patch.esp
Baurus tweak.esp
Siren's Deception Beautified.esp
Unique Artifacts for Unique People - Distribution.esp
Dahyka_OCO_Patch.esp
NPC Merge Consistency Patch.esp***

***

***

*****Name**: NPC Merge
**Filename**: NPC Merge.esp
***
REMINDER**: Ensure the names are correct as the BashTags are provided within the Conflict Resolution mod installed later.

Once navigating back to your main profile & activating the merge, to use **Merge Plugins Hide** to hide all the plugins from the respective merge.

**IMPORTANT**: Delete & rebuild a merge every time. Within Oblivion, when masters update, this can & regularly does create avoidable errors an existing merge in zMerge is updated.

**IMPORTANT:*** *Users of my FO4 guide will know I'm a huge advocate of this, so make a habit of Error-Checking your merges in xEdit just to be sure.
Any \ messages are likely arising if the first IMPORTANT step above is ignored.

When revisiting a merge, it's always good practise to **Sync** the load order from your main MOFAM profile (using **Sync Mod Order** from the Tools dropdown). When rebuilding, ensure the name of the mod updates (e.g. I timestamp if creating an updated version of the same merge) - the plugin name stays the same of course.*

*The far majority of these plugins are initially tagged as 'mergeable' by Wrye Bash. We are simply using a more modern technique to maximise the potential of the load order whilst still focusing on a stable & enjoyable build as the premise.*

## Part 37 - PLUGIN SORTING
Rather than provide complex LOOT rules or step-by-step manual intervention to share the LO prior to Conflict Resolution & Automation, ***some trusted Discord members have confirmed it IS POSSIBLE to paste the the loadorder.txt file into your MO2 profile, close & restart MO2 to have the plugins in order.

***Please be conscious of this edge-case scenario & DOUBLE CHECK the Load Order is correct however.

Download the Load Order from my modpage here to begin.

[LINK TO LOAD ORDER LIBRARY](https://loadorderlibrary.com/lists/mofam-oblivion-2)

*Take note, this is for BEFORE Bashed Patch creation and slowLODGen plugin additions.*

## Part 38 - CONFLICT RESOLUTION & AUTOMATION
1. Bashed Patch

*Prior to running Wrye Bash I recommend opening xEdit & Sort(ing) Masters for the whole Load Order. As of version 311 some extra steps are required, however remember once the tweaks & finer settings are applied, they need not be re-applied on subsequent rebuilds of the Bashed patch.*

**IMPORTANT**! **Before opening Wrye Bash ensure the Conflict Resolution mod below is installed & active, as it contains the Bash Tags for the merges we made earlier.**

*When building the patch, take note of the following:*

- Create an empty mod called 'Bashed Patch' & add the Bash Patches folder from [here](https://www.nexusmods.com/oblivion/mods/52949?tab=files) that contains the configuration.
- Open Wrye Bash through MO2
- Ensure the Bashed Patch is above Conflict Resolution and below NPC Merge.esp
- Select the Edit header \> Active Plugins \> Deactivate All
- Select the Edit header \> Active Plugins \> Activate Non-Mergeable
- Locate Conflict Resolution.esp, OOO Patches Merged.esp + NPC Merge.esp & enable them (selecting the dot in the square icon to change it to a tick)
- Right click Bashed Patch.esp & select Rebuild Patch
- In the 'Deactivate Prior to Patching' popup, deselect both OOO Patches Merged.esp & NPC Merge.esp
- Select Import (base of the UI) & select the file installed from step 1
- Select Build Patch
- Once created, close Wrye Bash & move the plugin from the Overwrite folder & into the mod created in step 1

2. [Conflict Resolution](https://www.nexusmods.com/oblivion/mods/52949?tab=files)

*The final plugin that ties the room together with outstanding Bashed Patch forwards, Bash Tags, tweaks & fixes, and ongoing minor bug fixing. All of its masters are detailed as Requirements on this modpage.
*

## Part 39 - SLOWLODGEN
*The new addition to Nexus Portal for Oblivion, slowLODGen, is a remarkable achievement & one of the most important mods in years. Whereas previously xLODGEN required loose files in the game's root Data folder to avoid stuttering (an MO2/vfs bug), we can now enjoy LOD as was potentially initially envisaged by the developers of the game.

Now that all plugins are in place, with merges created, it's time to run slowLODGen. As of 10.24 it's a very new tool to the Nexus, so in case of individual local issues that may arise during its creation **I've provided the output as a download on my modpage**. However, this shouldn't put you off from using it & catering it to your own LO's.

Run the tool using MO2's executable & the output is created in the **Overwrite** folder in MO2; simply paste the 2x bsa's & plugins to the empty mod created in Part 5, called 'Merged LOD'.

Take note in the log, that the **esm** must be placed in a certain spot - for MOFAM it stated **02** hence if you opt to use my download **place it after Av Latta Magicka.esm** - the dummy plugin is simply there for the archive & is placed at the last position within Part 5's plugins on the right pane **after IC LOD.esp**.

I should mention that Landscape LOD after all my testing has a higher impact on performance than Object. All the extra triangles & draw calls provides very diminishing returns, given we also use a great retex to effectively tick that box.*

## Part 40 - UTILITIES & MO2 ADVICE
***IMPORTANT:** As of 10.24, the key mod here is **Consribe Logs MOFAM LINK Settings**. As per the screengrab most mods need not stay active.*

1. **[4gb Ram Patcher](https://www.nexusmods.com/oblivion/mods/45576)

***Even though we run this during setup, I like to have it here for safekeeping.***

**2. [Dummy ESP](https://www.nexusmods.com/oblivion/mods/52949?tab=files)
3. [FormID Finder](https://www.nexusmods.com/oblivion/mods/16704)
4. [RefScope](https://www.nexusmods.com/oblivion/mods/21862)

*3 & 4 are both optional mods to activate as-and-when you feel it's necessary. They're useful for debugging purposes should the occasion arise.

Don't forget if you're curious about any given item or NPC in the game world, more often than not the console will indicate the formID & the hex-ref can be referenced using LINK's menu (or indeed alt-tabbing to MO2), hence identifying the mod that introduced what was selected.*

5. (Create Empty Modss) OBSE Logs & Inis & Conscribe Logs

*Over time MO2 will create logs & ini's from (e.g.) Blockhead, MenuQue, Piip & SKYBSA in the Overwrite folder. For minor QoL feel free to cut & paste them here.*

6. [Consribe Logs MOFAM LINK Settings](https://www.nexusmods.com/oblivion/mods/52949?tab=files&jump_to_comment=145592928)

*Ensure this is active as indicated below. Once ingame with your player character, simply open the pause menu & select Options \> Mods \> Import Settings. Users of my MOFAM FO4 guide will be reminded of the MOFAM MCM Settings Manager mod, a huge QoL step forward in automating settings for a smoothe transition to gameplay.

A few points to make however:*

- Note that the changes applied are within the Notes tab of the mod
- For non-Ultrawide users, revert the change of Loot Menu's X-parameter from 1500 to 1000

7. (Created in Setup) TES4Edit Cache

*I like to create further separators such as '**TO BE ADDED**', '**TESTING**', '**UPDATING**' & lastly '**DEPRECATED**'.For maintaining your load order & future-proofing QoL, I recommend doing something similar.*

*
*

**UPDATING:

**

- Firstly copy the existing MO2 profile & rename it to the version (e.g. MOFAM MM.YY). Navigate to this profile when starting to Update.
- When updating my Modlist, regardless of game, installing the updated mod using the name + version (so it doesn't overwrite the existing mod it will replace) & placing these into the 'Updating' separator helps organise updating the build considerably easier.
- Once a mod that's been updated is replaced, I then move it to the DEPRECATED separator & add a 'DEP' prefix to the mod's name, as you'll find if you remove the versioning of the mod that's been updated you can't have 2 mods in MO2 named the exact same way.
- *Similarly, for mods I remove, I right click & send to separator \> DEPRECATED. This helps keep your last MO2 profile intact in case of human-error.*

*
*

## Now would also be a good time (using Merge plugins hide) **sync** the load order across all profiles in use. Whenever there is a change in mod order on the left pane, it's good practise to keep all the profiles in use up to date, including the default.

## Congratulations! This has been an epic journey & I hope you've also learned a few new tricks along the way. It's time to jump ingame and enjoy the fruits of your labours. HOWEVER! There are still some important tweaks to make once ingame, so don't forget to incorporate the below for every new game you start.

## Epilogue - INGAME SETTINGS
Here are my game video settings. Note only a handful are required (e.g. HDR, Water Ripples & Anti Aliasing).

Texture Size: Large
Fade Values: 66%
Grass Distance: Full
View Distance: Full
3x Distant settings: On
In & Ext shadows: 3
Self Shadows: On
Shadows on Grass: On
Tree Canopy Shadows: On
Shadow Filtering: High
HDR Lighting: On
Bloom Lighting: Off
Water Detail: High
Water Reflections: On
Water Ripples: Off
Window Reflections: OnBlood Decals: High
Anti Aliasing: Off

**REMINDER!!** Given we use Conscribe as of 10.24, all the LINK settings are automated now so when starting New Game's, take note of the instructions in Part 40 under 'Consribe Logs MOFAM Link Settings'.

## Appendix: KNOWN ISSUES & FINAL THOUGHTS
*This is Oblivion; arguably one of the least forgiving titles to mod. Whereas in this build I've strived to have as little bugs as possible with solid performance & stability, there are still bugs. Honesty's the best policy (especially in software!) so below are the principle known issues:*

- Frame drops during heavy magic-combat if many NPCs are involved
- If you see both the Enchanting & Pickpocket skills with the copy NOT INITIALIZED, save/reload or restart the game.
- Ensure you have a backup of the right pane in MO2 (the icon next to 'Active'). In the rare instance a hard-reset is required due to a crash, the plugin list can reorder itself.
- A number of mods (e.g. SI Whispersins, Driftwell) are unvoiced; this is normal.

*Now let's talk crashes. We all get them. Over the years however the build has improved considerably as I've pulled many offending mods that were purely unstable to work with. With the advent of certain OBSE plugins & our improved toolset, crashes are very few & far-between. It's not unusual for me to go for 20 hours of crash-free-gameplay then randomly get hit by one seemingly randomly. I've found if you truly stress the engine, such as go interior \> exterior \> interior repeatedly during hectic fights, or repeatedly load a save after dying multiple times in the same cell, this can cause instability. If & when you do crash, message logger outputs the last calls in a .txt file & can be quite insightful.

In terms of updates & updating MOFAM: luckily, unlike SSE for example, mods in general within Oblivion don't update at the same furious pace. Only if there is a major overhaul or removal-from-Nexus of a key mastermod will MOFAM need a once-over to resume its lifespan, which I've purposefully built to stand the test of time.

Id also like to highlight to veterans that this is my first large upload to Nexus's Oblivion portal. You may disagree with some of the mod choices, comments, and of course some of the content. I may have 'Master' in my avatar name, but I'm certainly not claiming to be a master of modding. If you're aware of a better process or approach, don't hesitate to share. We're a community.

Naturally, with a game of this scope, not even my 500 hours ingame can cover every plausible scenario. One of the founding principles of QA is no one person can find every bug in a piece of software. If you find problems that aren't down to installation-error or missing certain advice above, let's discuss it in the posts tab or in my Discord.

To sign-off, I'd like to thank all the mod-authors involved, Bethesda for creating the game & modding platform, the Nexus & staff for hosting it all, and last but not least yourselves.

*

I hope you enjoy MOFAM for years to come.

*MasterLix.*

## ACKNOWLEDGEMENTS
*I want to give a huge thankyou & shoutout to all the **Contributors, Server Boosters, Mod Authors** and growing number of **Wastelanders** on my Discord for their continued help, advice & guidance since December 2020 since my Discord went out with MOFAM: FO4. A special shoutout to [durbin](https://www.nexusmods.com/fallout4/users/565819) for kicking me up the arse to get this Live too! Certain members of our modding community within the TES4 Portal such as [Dispensation](https://www.nexusmods.com/oblivion/users/12692318), [Maskar](https://www.nexusmods.com/oblivion/users/1006768), [Mixxa77](https://www.nexusmods.com/oblivion/users/10186610), [DianaTESGotH](https://www.nexusmods.com/oblivion/users/19386019), [CarlosS4444](https://www.nexusmods.com/oblivion/users/707851), Katkat74 & the VKVII team...the list goes on. Amazing contributions that make this (what is now) nostalgic gem of a video game close to our modded hearts. [DarkLadyLexy](https://www.nexusmods.com/skyrimspecialedition/users/45193647#) and [Evertiro](https://lotdplus.com/) & their whole team for the continued work & talent behind their SSE guides, of which were an influence for MOFAM. [Wellden](https://www.nexusmods.com/oblivion/users/15376219) for his now years of friendship online, [Bevilex](https://www.nexusmods.com/oblivion/users/28534185) for getting me into modding from the now-legendary guide, and lastly [OutdatedTV](https://www.nexusmods.com/skyrimspecialedition/users/30790290) for rounding us all up on the [TUCO Guide](https://www.nexusmods.com/skyrimspecialedition/mods/10694) & Modding Team.*

**



**

## You guys rock.

***[If you fancy something similar check out my other guide for Fallout 4, celebrating it's 2nd Year on Nexus.](https://www.nexusmods.com/fallout4/mods/48580/?tab=posts&jump_to_comment=124045392) [See you there!](https://www.nexusmods.com/fallout4/mods/48580/?tab=posts&jump_to_comment=124045392) ***


**

[As a third addition to my suite of guides, try Fallout 3 which went Live 01.24. See you there!](https://www.nexusmods.com/fallout3/mods/25994?tab=description)

