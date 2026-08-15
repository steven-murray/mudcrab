When running any mudcrab script in MOFAM-test, use the following env vars:

    - GAME_DIR = /home/steven/.local/share/Steam/steamapps/common/Oblivion
    - NEXUS_API_KEY = read from `MOFAM-test/input/api-key.txt` (gitignored; never commit it)

e.g. `export NEXUS_API_KEY="$(cat MOFAM-test/input/api-key.txt)"`

When asked to add a new MOFAM section to the mofam.full.toml, go through the specified Part of mofam-source.md and:

* Interpret the best way to represent each mod in our mofam.full.toml, including  mod id and file id if they are nexus mods.
* Identify the existing archive in /home/steven/Games/mod-organizer-2-oblivion/modorganizer2/downloads that corresponds to this mod and adopt it into our cache (remember we have the adopt-mo2-downloads.py script available to help).
* When adopting (renaming) an archive into our cache format, make sure to use
  the format with the extensions included (e.g. .7z and .zip)
* Report which mods are clear, which may not have archives, and which are difficult to interpret.
* Remember that we understand BAIN layouts and FOMOD layouts