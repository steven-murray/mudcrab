# mudcrab

A declarative modlist compiler and installer for TES4: Oblivion, written in Rust
and independent of platform.

**I've seen mudcrabs more fearsome than you!**

## What mudcrab is

You write a modlist as a TOML file: where each mod comes from, how to unpack it,
what to do to it afterwards, and what order it all loads in. mudcrab compiles
that, downloads the archives, installs them, applies the edits, builds the
merges, and writes a ready-to-use Mod Organizer 2 instance.

It is **not** a mod manager and does not replace MO2. It is closer to
Wabbajack — a way to install someone else's curated list — with one difference
that drives the whole design.

**A Wabbajack install is data with no relationships in it.** The files are
there, but nothing records why a mod was included, what it was patched against,
or which other mods a change would disturb. So the two things people most often
want to do afterwards are the two hardest: change a setting you did not choose,
and add a mod that needs patches from a dozen rows you would now have to
reinstall by hand.

A mudcrab modlist is the list *and* the reasoning, in one file you can read,
diff and send to someone. Because it is declarative and compiled rather than
executed top to bottom, the relationships are visible to the tool as well as to
you.

### Does it work?

Yes, on a hard case. mudcrab reproduces
[MOFAM](https://www.nexusmods.com/oblivion/mods/52949) — a 40-part, ~700-mod
Oblivion guide with six plugin merges, BSA repacking, xEdit cleaning, FOMOD and
BAIN installers and a fixed 242-plugin load order — from a single TOML file, on
Linux, with no GUI tool involved except the final Wrye Bash step. It was
verified mod-by-mod against a hand-built reference instance and then played.
See [MOFAM-test](MOFAM-test/) for that case study.

It is **not yet packaged for other people to use**. See
[docs/roadmap.md](docs/roadmap.md) for what stands between here and that, and
[docs/known-issues.md](docs/known-issues.md) for what will bite you meanwhile.

## Documentation

| | |
| --- | --- |
| [docs/usage.md](docs/usage.md) | Command reference, and what each action does |
| [docs/modlist-format.md](docs/modlist-format.md) | The TOML format |
| [docs/mo2-output-structure.md](docs/mo2-output-structure.md) | What gets written into an MO2 instance |
| [docs/known-issues.md](docs/known-issues.md) | Limitations, and what to do about them |
| [docs/roadmap.md](docs/roadmap.md) | What is planned before a release |

## The pipeline

```bash
mudcrab compile  modlist.toml   --output build/compiled.json
mudcrab query    build/compiled.json --output build/plan.json --headless
mudcrab download build/plan.json --cache .mudcrab-cache
mudcrab install  build/plan.json --cache .mudcrab-cache --mo2-instance-dir ~/mo2/MyList
```

1. **compile** — validate the source list and resolve what can be resolved.
2. **query** — ask the user any questions the list declares, producing a plan
   specific to their answers.
3. **download** — fetch (or find locally) every archive the plan needs.
4. **install** — unpack, apply actions, build merges, write the MO2 instance.

Authoring commands sit alongside it: `inspect` an archive to see its layout and
installer options, `add` a mod to a list without disturbing its comments,
`identify` an archive by hash, `conflicts` to see what two mods both provide,
`diff` to compare a built instance against a reference, `check` and `validate`.
`mudcrab --help` lists them all.

## Requirements

- `bsdtar` and `7z` on `PATH`, for `.7z` and `.rar`. Everything else is decoded
  in-process.
- For `qac`: xEdit, configured via `tools.toml` (`mudcrab setup-tools` writes a
  template). It runs headlessly, under Proton or Wine on Linux.
- For Nexus sources: `NEXUS_API_KEY` in the environment.

## Development

```bash
cargo test
cargo clippy --all-targets -- -D warnings
```

Both must pass. There is no CI yet, so this is convention rather than
automation — [roadmap Phase E](docs/roadmap.md#phase-e--publishing-mechanics).

The codebase is heavily commented, deliberately: most of what mudcrab does is
work around some undocumented behaviour of a twenty-year-old file format or of
MO2, and the comment explaining why a line is the way it is tends to be the most
valuable thing in the file. Keep that up. No blanket `#[allow]` — fix the lint,
or suppress it narrowly with a reason.
