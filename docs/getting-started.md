# Getting started: installing a modlist

This page takes you from nothing to a playable game, using a modlist someone
else has written. Writing your own list is a different job; see
[The modlist format](modlist-format.md).

A mudcrab modlist is a `.toml` file (sometimes with a second, `*.docs.toml`,
holding its prose). mudcrab reads it, downloads every archive it names,
installs them into a Mod Organizer 2 instance exactly as the list describes,
and sets up the load order. You end with MO2 ready to launch the game.

## Why several steps, not one button

If you are used to installing mods one at a time in MO2, five commands may
seem like a lot. Each one finishes a different kind of work, and each leaves a
file behind, so a problem shows up early and nothing is ever redone needlessly:

- **`compile` checks the list's own consistency before anything is
  downloaded.** A typo, a reference to a mod that is not there, or a
  contradiction in the list's logic is reported in seconds, not an hour into
  an install. (Problems inside the archives, such as an installer option
  that does not exist, can only be found once they are downloaded.)
- **`query` turns the list plus your answers into a plan**: one file saying
  exactly what will be installed. Your choices live there, not in the list, so
  changing your mind means re-running one quick step. The plan is also what to
  attach if you report a problem.
- **`download` is the slow, network-bound part**, kept apart so it can be
  retried, done partly by hand, or done once for several installs. Everything
  after it works offline.
- **`check` confirms every archive is present and opens**, before a long
  install starts.
- **`install` only acts.** It can be stopped and re-run, and only redoes what
  changed.

The commands run happily back to back, so a script or a shell history entry
makes the whole thing one line after the first time.

## Before you start

You need:

- **Oblivion**, installed and **run once** from its launcher, so it has
  written its `Oblivion.ini`. mudcrab has been tested with the Steam GOTY
  edition.
- **A portable Mod Organizer 2 instance** for Oblivion, used for nothing else.
  mudcrab fills this instance in.
  - On Windows: download MO2, unpack it into its own folder, start it, choose
    "Portable" and point it at the game folder (`mudcrab game-dir` prints it).
  - On Linux, MO2 runs under Proton, and Jackify sets that up: see
    [Setting up MO2 on Linux](#setting-up-mo2-on-linux) below.
- **`bsdtar` and `7z`** on your `PATH`, for `.7z` and `.rar` archives. Most
  Linux distributions have both a package away; on Windows, install 7-Zip and
  add it to `PATH`.
- **mudcrab** itself: download the build for your system from the
  [releases page](https://github.com/steven-murray/mudcrab/releases) and
  unpack it. There is nothing to install.
- **A Nexus Mods account.** With a premium account, mudcrab downloads
  everything itself. Without one, you download the archives by hand (Nexus
  only gives download links to premium accounts) and mudcrab takes it from
  there; see [step 3](#3-download-the-archives).

Some lists need more: xEdit, if the list cleans plugins (`qac` actions), or an
MO2 plugin. Step 1 tells you.

### Setting up MO2 on Linux

1. In [Jackify](https://github.com/Omni-guides/Jackify), open the
   "Additional Tasks" tab and choose "Setup Mod Organizer 2". It installs MO2
   into the folder you give it and adds it to Steam as a non-Steam game. It
   then shows in Jackify's dashboard too.
2. Give it a name you will recognise. Steam calls it "Mod Organizer 2", which
   gets confusing once you have more than one: in your Steam library,
   right-click it, choose Properties, and change the name at the top.
3. Start it from Steam (or Jackify's dashboard). MO2's setup window appears.
4. Choose **Portable**, so the instance lives in its own folder.
5. When it asks for the game, a file window opens. MO2 runs under Proton,
   which sees your Linux files as drive `Z:`, so browsing there is tedious.
   Instead, run

   ```bash
   mudcrab game-dir
   ```

   and paste the `Z:\...` folder it prints into the **File name** box at the
   bottom of the window. (`mudcrab game-dir --plain` prints only that, for
   piping to the clipboard.)

See [Linux / Proton in Known issues](known-issues.md#linux--proton) for what else Linux
needs.

## 1. Tell mudcrab where things live

```bash
mudcrab setup-tools mylist.toml
```

This asks for what the list needs -- the downloads folder (one folder every
archive goes in, shared by every list you install) and, if the list cleans
plugins, xEdit and the Proton prefix to run it in -- offering what it finds:
MO2 instances, Steam's Proton prefixes (named after their Steam shortcuts),
xEdit under `~/Games`. Press Enter to take the first suggestion, a number for
another, or type a path.

It also asks for your Nexus API key, from your Nexus account's
[API keys page](https://next.nexusmods.com/settings/api-keys). A premium
account downloads with it; a free one uses it to name files. Kept in
`tools.toml`, every command finds it; `NEXUS_API_KEY` in the environment, if
set, takes precedence. The file is readable only by you -- do not share it.

It writes `tools.toml` where every mudcrab command looks for it
(`~/.config/mudcrab/`, or `$XDG_CONFIG_HOME/mudcrab/` when that is set;
`%APPDATA%\mudcrab\` on Windows) and says where. The commands below then need
no `--downloads`.

## 2. Compile the list and answer its questions

```bash
mudcrab compile mylist.toml --output compiled.json
mudcrab query compiled.json --output plan.json
```

`compile` checks that the list is consistent -- every name it refers to
exists, every plugin is in the load order, every condition asks a question the
list declares -- and writes it out in the form the later steps read. It does
not look at any archive (you have none yet) or ask anything. `query` asks the
questions the list declares -- an optional mod, a choice between versions -- and writes
`plan.json`: exactly what will be installed, given your answers. Press Enter
to take the default the list's author chose. `--headless` takes every default
without asking.

## 3. Download the archives

```bash
mudcrab download plan.json
```

Every archive is attempted. Re-run it as often as you like: archives already
in the downloads folder are not fetched again.

**Without a premium Nexus account**, or for a file the list says to download
by hand, someone has to click. `download` opens the pages of the files it
cannot download itself in your browser, ten at a time, and picks each file up
as it arrives. (`--no-browser` lists them and stops instead; with no browser
to open, that is what happens.) On each page, click "Slow download" (on
Linux the pages open there): your browser saves the file, and mudcrab offers
to move it into the downloads folder. On Windows, "Mod Manager Download" also
works if MO2 is open: MO2 saves the file straight into the downloads folder.
As each file arrives, the next page opens. If you already have some of the archives --
another MO2 instance's downloads, a Wabbajack folder -- point mudcrab at them
instead: `--archive-search-path <folder>` (repeatable) uses them without
copying or changing them.

When `download` reports nothing missing, check what you have:

```bash
mudcrab check plan.json
```

`check` works on the downloads folder, where `download` has put everything,
and finds everything about the list it can without installing it:

- every archive is there and opens (a download cut short fails here), and
  matches its recorded MD5 if the list gives one;
- `install` could lay each one out: the installer options and BAIN
  subpackages the list chose exist, and the files it names are there;
- the list's edits to each mod's files -- files to delete, hide or move --
  each find what they name, rehearsed on placeholder files exactly as
  `install` would run them;
- every plugin's masters are in the load order, active and earlier; every
  plugin in the load order is installed by some mod; every BSA has an active
  plugin to load it.

It reports every problem together, with what the list says about getting any
archive that is missing. Warnings (a plugin two mods both install, one a mod
installs that the load order leaves out) do not fail it. It changes nothing,
and takes under a minute on a large list. See
[`check` in Commands](usage.md#check) for the details.

## 4. Install

**Close MO2 first.** MO2 writes its profile back when it exits, which would
undo the install. (On Linux mudcrab checks, and refuses to start while MO2
has the instance open; on Windows it cannot tell yet.)

```bash
mudcrab install plan.json --mo2-instance-dir <your MO2 folder>
```

This installs every mod, applies the list's edits (INI settings, files to
leave out, plugins to clean), builds its merged plugins, and writes MO2's
profile and load order. A large list takes a while; a run that stops partway
picks up where it left off.

mudcrab installs into the profile named `Default` (`--profile-name` to choose
another), and from then on **that profile belongs to mudcrab**: its mod list
and plugin list are rewritten from the plan on every install. Keep your own
experiments in a different profile.

The game's own folder is never touched. Settings the list changes in
`Oblivion.ini` go into the profile's own copy, and mudcrab turns on MO2's
"profile-specific game INI files" so they take effect.

**Then do what the install says is left.** A few steps cannot be done for you
-- most often building the **Bashed Patch**, which only Wrye Bash can do, from
its own window. `install` ends by listing each one, under "Still to do by
hand", with the steps to follow, and keeps listing it until it is done. Do
them before you play: the game still starts without them, but not as the
list intends.

To see whether any plugin names a mesh or sound the game will not find,
run `mudcrab assets --mo2-instance-dir <your MO2 folder>`: a report, worth a
look if something shows as a missing-mesh marker in game.

## 5. Play

Start MO2. It applies the load order when it loads the profile, so if it was
open during the install, restart it.

A few things are left to you, because they are choices or happen in a tool's
own window. The list's documentation says which; to read it as a guide, with
every step spelled out:

```bash
mudcrab export mylist.toml --to markdown -o guide.md
```

Typically that means setting up MO2's executables (launching the game through
OBSE, for instance) and any step done in a GUI tool, such as building a Wrye
Bash Bashed Patch.

## Changing your mind

- **A different answer** to one of the list's questions: run `query` again
  (or `query --set name=value`), then `download` and `install`. Only what
  changed is installed again.
- **A new version of the list**: the same three steps after `compile`.
- **Something went wrong**: every run writes a full log and prints its path on
  failure (`full log: ...`). [Troubleshooting](troubleshooting.md) explains
  the messages you are likely to see, and [Known issues](known-issues.md)
  covers what is already known; to report something new, see
  [the contributing guide](https://github.com/steven-murray/mudcrab/blob/main/CONTRIBUTING.md#reporting-a-modlist-that-installs-wrongly).

## Where to next

- [Commands](usage.md): every command and flag, including installing one
  section of a large list at a time (`--section`, `--only`).
- [The modlist format](modlist-format.md): writing or changing a list.
- [Turning a list into a guide](exporters.md): for people who install by
  hand.
