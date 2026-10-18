# mudcrab

A declarative modlist compiler and installer for TES4: Oblivion, written in Rust
and independent of platform.

**I've fought mudcrabs more fearsome than you!**

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
BAIN installers and a fixed 244-plugin load order — from a single TOML file, on
Linux, with no GUI tool involved except the final Wrye Bash step. It was
verified mod-by-mod against a hand-built reference instance, played, and then
installed again from scratch on a fresh machine from the documentation alone.
The list itself is kept privately: it transcribes someone else's guide.

See [docs/roadmap.md](docs/roadmap.md) for what is planned next, and
[docs/known-issues.md](docs/known-issues.md) for what to look out for.

## What it covers

Today, mudcrab is for **Oblivion** (TES4) with **Mod Organizer 2**, and that is
all that has been run:

- **Game:** Oblivion, tested with the Steam GOTY edition. Its plugin and BSA code is TES4's; nothing else is
  read or written. Nothing in the pipeline above that is tied to one game, but
  no other has been tried.
- **Mod manager:** MO2, a portable instance. mudcrab writes the instance --
  mods, profile, load order -- and never touches the game folder; files that
  belong there go through the Root Builder MO2 plugin.
- **Platform:** the full pipeline has been run on **Linux**, with the game, MO2
  and xEdit under Proton. Windows binaries are built and pass the full test
  suite, but a whole install has not been run on Windows yet.
- **Nexus:** downloading through the Nexus API needs a premium account (Nexus's
  rule). Without one, `download` opens each file's page in your browser and
  picks the file up as you download it; everything else, including
  identifying archives, works on any account.

Reports from setups other than these are especially welcome: see
[CONTRIBUTING.md](CONTRIBUTING.md).

## Getting it

Prebuilt binaries for Linux and Windows are attached to each
[release](https://github.com/steven-murray/mudcrab/releases). Unpack and run —
there is nothing to install. The GUI is there too: an AppImage for Linux and
the Steam Deck, and an installer or a zip for Windows (see
[docs/gui.md](docs/gui.md)).

The Linux build needs glibc 2.34 or newer (Ubuntu 22.04, Debian 12, Fedora 35
and later), and nothing else installed.

If you only want the merge engine, that is all you need: `merge` and
`new-merge` read plugin files and write one, with no configuration, no external
tools and no API key. See
[docs/merge-standalone.md](docs/merge-standalone.md).

## Documentation

The documentation is published as a site at
**https://steven-murray.github.io/mudcrab/**, built from `docs/` with
[mdBook](https://rust-lang.github.io/mdBook/) (`mdbook serve` previews it).

| | |
| --- | --- |
| [docs/getting-started.md](docs/getting-started.md) | **Start here**: installing a modlist, from nothing to a playable game |
| [docs/authoring.md](docs/authoring.md) | Writing a modlist: a worked example, from an empty file to an install |
| [docs/merge-standalone.md](docs/merge-standalone.md) | Using the merge engine on its own, as a zMerge replacement |
| [docs/usage.md](docs/usage.md) | Command reference, and what each action does |
| [docs/modlist-format.md](docs/modlist-format.md) | The TOML format |
| [docs/exporters.md](docs/exporters.md) | Turning a modlist back into a guide: Nexus BBCode, Markdown, a GitHub Pages site |
| [docs/mo2-output-structure.md](docs/mo2-output-structure.md) | What gets written into an MO2 instance |
| [docs/troubleshooting.md](docs/troubleshooting.md) | The error messages you are likely to see, and what to do |
| [docs/known-issues.md](docs/known-issues.md) | Limitations, and what to do about them |
| [docs/roadmap.md](docs/roadmap.md) | What is planned next: composability and a GUI, and what is still open |

## The pipeline

```bash
mudcrab compile  modlist.toml   --output build/compiled.json
mudcrab query    build/compiled.json --output build/plan.json --headless
mudcrab download build/plan.json --downloads ~/Games/OblivionModDownloads
mudcrab install  build/plan.json --downloads ~/Games/OblivionModDownloads --mo2-instance-dir ~/mo2/MyList
```

1. **compile** — check the source list's own consistency (names, load order,
   conditions) and write it out for the next steps.
2. **query** — ask the user any questions the list declares, producing a plan
   specific to their answers.
3. **download** — fetch (or find locally) every archive the plan needs.
4. **install** — unpack, apply actions, build merges, write the MO2 instance.

Authoring commands sit alongside it: `inspect` an archive to see its layout and
installer options, `add` a mod to a list without disturbing its comments,
`identify` an archive by hash, `conflicts` to see what two mods both provide,
`diff` to compare a built instance against a reference, `check` and `validate`,
and `export` to write the list back out as a guide people can follow by hand.
`mudcrab --help` lists them all.

## Licence

MIT or Apache-2.0, at your option. See [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE).

## Requirements

- `bsdtar` and `7z` on `PATH`, for `.7z` and `.rar`. Everything else is decoded
  in-process.
- For `qac`: xEdit, configured via `tools.toml` (`mudcrab setup-tools` writes a
  template). It runs headlessly, under Proton or Wine on Linux.
- For Nexus sources: your API key, in `tools.toml` (`setup-tools` asks) or
  `NEXUS_API_KEY`.

## Development

```bash
cargo fmt
cargo test
cargo clippy --all-targets -- -D warnings
```

All three must pass, and CI checks them on every push, with the toolchain
pinned in `rust-toolchain.toml`. [CONTRIBUTING.md](CONTRIBUTING.md) has the
rest: how to report a modlist that installs wrongly, and the conventions the
code keeps -- chief among them that it is heavily commented, deliberately.
