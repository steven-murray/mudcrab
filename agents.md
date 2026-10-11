# Working in this repo

## Checks

```bash
cargo fmt
cargo test
cargo clippy --all-targets -- -D warnings
```

All three must pass before any change lands (CI runs `cargo fmt --check`). No
blanket `#[allow]`.

The toolchain is pinned in `rust-toolchain.toml`, and rustup uses it here and
in CI alike, so a check that passes locally passes there. Moving to a newer
Rust is a change of its own: bump the pin, run the three, commit what moves
(a whole-tree reformat goes in `.git-blame-ignore-revs`).

## Comments

The codebase is heavily commented on purpose. Most of what mudcrab does is work
around undocumented behaviour of a twenty-year-old file format, of MO2, or of a
modding tool, and the comment saying *why* a line is the way it is is usually
worth more than the line. Match that density. When a decision rests on evidence
rather than on reasoning, put the evidence in `docs/design/` and cite it.

## The MOFAM workspace

The development case study -- a real ~700-mod list -- lives in its own
**private** repository, `steven-murray/mofam-test`, checked out beside this one
(`../mofam-test`). It is private because it transcribes someone else's guide;
nothing from it belongs in this repository, which is public. See its README.
To run anything there:

```bash
export GAME_DIR=/home/steven/.local/share/Steam/steamapps/common/Oblivion
export NEXUS_API_KEY="$(cat ../mofam-test/input/api-key.txt)"   # gitignored; never commit it
../mofam-test/scripts/run-full.sh --section "<name>"
```

The pipeline's last stage is `mudcrab diff` against a reference instance. **Its
output is the point**: every difference has to be explained, not glanced at. A
clean diff means "both sides did the same thing", which is not the same as
"both sides did the right thing" — where the guide and the reference disagree,
follow the guide and say so.

### Adding a section to `mofam.full.toml`

Work through that Part of `mofam-test/input/mofam-source.md` and, per row:

- decide how the mod is best represented — one mod, several archives, or several
  mods; `mudcrab inspect` the archive before guessing at its layout;
- pin the Nexus mod and file id where there is one;
- find the existing archive in the local downloads folder and give the entry its
  `file_name`, so `--archive-search-path` resolves it offline;
- lower-case any directory the entry creates: the Linux VFS is case-sensitive
  and MO2's is not.

Report which rows are clear, which have no archive, and which are ambiguous.
BAIN and FOMOD layouts are both handled.
