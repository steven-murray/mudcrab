# Troubleshooting

The messages you are most likely to see, what each means, and what to do.
They are grouped by the command that prints them; the text in each heading is
how the message begins.

Every run writes its full log and, when it fails, ends by naming it:

```text
full log: /home/you/.local/state/mudcrab/logs/install-20261012-050631.log
```

(`%LOCALAPPDATA%\mudcrab\logs` on Windows.) The terminal shows the error and
what caused it; the log has every step before it. `-v` shows the full log on
the terminal as the run goes. Limitations that are already known, with
workarounds, are in [known-issues.md](known-issues.md).

## Setting up

### `no downloads folder: pass --downloads <DIR>, or set downloads_dir in tools.toml`

mudcrab has not been told where archives live. Set `downloads_dir` in
`tools.toml` once (`mudcrab setup-tools <list>` writes the file, in the place
every command looks), or pass `--downloads` each time. See
[getting-started.md](getting-started.md#1-tell-mudcrab-where-things-live).

### `no game folder: pass --game-dir or set GAME_DIR`

An install into an MO2 instance takes the game folder from MO2's own settings
(`gamePath` in `ModOrganizer.ini`). This means that setting is missing, or
names the folder by a Wine drive other than `Z:`, which mudcrab cannot see
into. Open MO2 once and check it found the game, or pass `--game-dir`.

### `failed to locate Oblivion.ini for game directory ...`

The game has not written its `Oblivion.ini` yet. Start Oblivion once from its
own launcher, so it does, then install again. mudcrab copies it into the MO2
profile and makes its edits there; the original is never changed.

### `no system tool (7z, bsdtar) available to ...`

`.7z` and `.rar` archives are unpacked by `7z` or `bsdtar`, and neither is on
your `PATH`. Install either (on Linux, `p7zip` or `libarchive`; on Windows,
7-Zip, and add its folder to `PATH`).

### `... qac action requires [tes4edit] configuration in tools.toml`

The list cleans plugins with xEdit, and `tools.toml` does not say where xEdit
is. Run `mudcrab setup-tools <list>` and fill in the `[tes4edit]` section (on
Linux, also `[wine]`).

## Writing or changing a list

`validate` and `compile` check the list's own consistency before anything is
downloaded (see [usage.md](usage.md#compile) for what that covers).

### `mod X declares plugin Y that is missing from global plugins load order`

The mod says it ships a plugin, but the top-level `plugins` load order does not
list it. Add it to `plugins` where it belongs -- or let `mudcrab add` do it,
with `--load-after <plugin>` or `--load-last`.

### `Y is not in the load order yet; say where with --load-after <plugin> or --load-last`

From `add`: the same thing, caught before anything is written. mudcrab never
guesses a plugin's position, because a plugin in the wrong place fails
silently in game.

### ``mod X: its `if` asks about input 'Y', which [inputs] does not declare``

A condition names a question the list does not ask -- usually a typo. Declare
it under `[inputs]`, or fix the name.

### `FOMOD selection for step 'S' group 'G' references unknown option 'O'. Available options: ...`

A `fomod_selections` entry names an option the installer does not have. The
message lists the ones it does; `mudcrab inspect <archive>` shows the whole
installer. Often the mod was updated and an option renamed.

### `BAIN subpackage 'P' was not found in ... Available top-level directories: ...`

As above, for a BAIN package: `bain_subpackages` names a folder the archive
does not have.

## Downloading

### `no Nexus API key (NEXUS_API_KEY, or nexus_api_key in tools.toml) ...`

Give mudcrab your API key, from your Nexus account's
[API keys page](https://next.nexusmods.com/settings/api-keys): `setup-tools`
stores it in `tools.toml` (with `--force` if you already have one, or add
`nexus_api_key = "..."` to the file yourself), or set `NEXUS_API_KEY`. Or
download the file from the link given.

### `Nexus gives download links to premium accounts only; download it by hand from ...`

Nexus only lets premium accounts download through its API. Run
`mudcrab download <plan>` in a terminal: it opens each file's page and picks
the file up as you download it. Or open the link, download the file into the
downloads folder, and run `download` again: mudcrab finds it by name.
Everything else works on a free account.

### `Nexus refused the API key: check NEXUS_API_KEY or nexus_api_key in tools.toml`

The key is wrong or has been revoked. Copy it again from the API keys page.
`NEXUS_API_KEY`, when set, is used instead of the one in `tools.toml`.

### Pages open in an unfamiliar browser window (Linux)

The browser was started with a different `XDG_CONFIG_HOME` from usual, so it
opened a fresh profile. `download` warns when it is set. Unset it for that
command.

### `archive 'A' must be downloaded by hand -- it is not fetchable from any automated source`

The list marks this archive `manual:`, because no download link can reach it
(a forum host behind a login, say). Where the list says where and how to get
it, that follows the message (`get it from: ...`); `download` opens that page
for you.
If the message lists *similarly named files*, one of them may be the same
archive, renamed by its host since the list was written: if so, set the
entry's `file_name` to that name. (An entry that records the archive's `size`
and `md5` finds a renamed copy by itself; `add --archive --manual` writes
them.)

## Checking

### `A is not in the downloads folder (...) or any search path`

From `check`: the archive has not been downloaded yet. The message says
whether `mudcrab download` can fetch it or it is a hand download, and names
similarly named files on disk if there are any -- the commonest cause is a mod
updated since the list was written, so the file you have is a different
version. Either get the version the list names, or (if you are the list's
author) update the entry.

### `archive is not readable (...)`

The file is there but cannot be opened: usually a download that stopped
partway. Delete it and download it again.

## Installing

### `Mod Organizer 2 has <instance> open (pid ...). Close it first`

MO2 writes its own copy of the profile back when it exits, which would undo
the install. Close MO2 and install again. (On Windows mudcrab cannot yet tell
whether MO2 is open, so close it first anyway.)

### The install finished, but MO2 shows the plugins in the wrong order

MO2 applies the load order when it loads the profile. Restart MO2 after an
install, or switch to another profile and back.

### I changed the list, but `install` says the mod is unchanged

A mod is reinstalled when its entry changes. If what changed is mudcrab itself
(a newer version installs differently), `install --force` reinstalls
everything in scope; combine it with `--only <mod id>` to redo one mod.

### `this plugin uses Oblivion Magic Extender (OBME): ...`

From a merge: one of the plugins being merged uses OBME, which mudcrab cannot
merge safely. Leave that plugin out of the merge's `sources` and load it on
its own.

## Playing

### Settings the list changes have no effect in game

The list's INI changes are made to the MO2 profile's own `Oblivion.ini`, and
MO2 only uses that file when the profile's "Use profile-specific Game INI
Files" is on. `install` turns it on; if the profile was changed in MO2 since,
check it under Profiles.

### Problems on Linux (black menus, missing water, ENB)

Running under Proton has its own fixes; see
[known-issues.md](known-issues.md#linux--proton).

## Still stuck

[CONTRIBUTING.md](../CONTRIBUTING.md#reporting-a-modlist-that-installs-wrongly)
says what to attach to a report so it can be acted on: the plan, the log, and
`mudcrab diff` output if you have a reference instance to compare against.
