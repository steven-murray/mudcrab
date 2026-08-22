# Working in this repo

## Checks

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

Both must pass before any change lands. No blanket `#[allow]`.

## Comments

The codebase is heavily commented on purpose. Most of what mudcrab does is work
around undocumented behaviour of a twenty-year-old file format, of MO2, or of a
modding tool, and the comment saying *why* a line is the way it is is usually
worth more than the line. Match that density. When a decision rests on evidence
rather than on reasoning, put the evidence in `docs/design/` and cite it.

## The MOFAM workspace

`MOFAM-test/` holds a real ~700-mod list used as the development case study; see
its README. To run anything there:

```bash
export GAME_DIR=/home/steven/.local/share/Steam/steamapps/common/Oblivion
export NEXUS_API_KEY="$(cat MOFAM-test/input/api-key.txt)"   # gitignored; never commit it
./MOFAM-test/scripts/run-full.sh --section "<name>"
```

The pipeline's last stage is `mudcrab diff` against a reference instance. **Its
output is the point**: every difference has to be explained, not glanced at. A
clean diff means "both sides did the same thing", which is not the same as
"both sides did the right thing" — where the guide and the reference disagree,
follow the guide and say so.

### Adding a section to `mofam.full.toml`

Work through that Part of `input/mofam-source.md` and, per row:

- decide how the mod is best represented — one mod, several archives, or several
  mods; `mudcrab inspect` the archive before guessing at its layout;
- pin the Nexus mod and file id where there is one;
- find the existing archive in the local downloads folder and give the entry its
  `file_name`, so `--archive-search-path` resolves it offline;
- lower-case any directory the entry creates: the Linux VFS is case-sensitive
  and MO2's is not.

Report which rows are clear, which have no archive, and which are ambiguous.
BAIN and FOMOD layouts are both handled.
