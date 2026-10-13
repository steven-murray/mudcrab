# mudcrab

**A declarative modlist compiler and installer for TES4: Oblivion.**

*I've fought mudcrabs more fearsome than you!*

You write a modlist as a TOML file: where each mod comes from, how to unpack
it, what to do to it afterwards, and what order it all loads in. mudcrab checks
that, downloads the archives, installs them, applies the edits, builds the
merges, and writes a ready-to-use Mod Organizer 2 instance.

It is not a mod manager, and does not replace MO2. It is closer to Wabbajack --
a way to install someone else's curated list -- with one difference: a mudcrab
modlist is the list *and* the reasoning, in one file you can read, diff and
send to someone. Change a setting, add a mod, follow a fix: the relationships
are visible to you and to the tool.

## Where to start

- **Installing a list someone made?** [Getting started](getting-started.md)
  takes you from nothing to a playable game. If something goes wrong,
  [Troubleshooting](troubleshooting.md) has the messages you are likely to see.
- **Writing a list?** [Writing a modlist](authoring.md) builds one from an
  empty file, and [the modlist format](modlist-format.md) lists every key.
- **Just want plugin merging?** [The merge engine on its own](merge-standalone.md)
  replaces zMerge with no configuration at all.
- **Looking up a command?** [Commands](usage.md).

## Does it work?

On a hard case. mudcrab reproduces
[MOFAM](https://www.nexusmods.com/oblivion/mods/52949) -- a 40-part, ~700-mod
Oblivion guide with six plugin merges, BSA repacking, xEdit cleaning, FOMOD and
BAIN installers and a fixed 244-plugin load order -- from a single TOML file,
on Linux, with no GUI tool involved except the final Wrye Bash step. It has
been installed from scratch on a fresh machine from these docs, and played.

Today mudcrab is for Oblivion with MO2. The whole pipeline has been run on
Linux, with the game, MO2 and xEdit under Proton; Windows builds pass the full
test suite, but a whole install has not been run on Windows yet.

## Getting it

Prebuilt binaries for Linux and Windows are attached to each
[release](https://github.com/steven-murray/mudcrab/releases). Unpack and run:
there is nothing to install. The source, issues and the
[contributing guide](https://github.com/steven-murray/mudcrab/blob/main/CONTRIBUTING.md)
are on [GitHub](https://github.com/steven-murray/mudcrab).
