# mudcrab

A platform-independent, declarative modlist compiler and installer for TES4: Oblivion,
written in Rust.

**[Documentation[(https://steven-murray.github.io/mudcrab/)**

**I've fought mudcrabs more fearsome than you!**

## What is mudcrab?

You write a modlist as a TOML file: where each mod comes from, how to unpack it,
what to do to it afterwards, and what order it all loads in. mudcrab compiles
that, downloads the archives, installs them, applies the edits, builds the
merges, and writes a ready-to-use Mod Organizer 2 instance.

It is **not** a mod manager and does not replace MO2. It is closer to
Wabbajack — a way to install someone else's curated list — with one huge difference:

**A Wabbajack install is data with no relationships in it.** The files are
there, but nothing records why a mod was included, what it was patched against,
or which other mods a change would disturb.

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

See [docs/roadmap.md](docs/roadmap.md) for what is planned next, and
[docs/known-issues.md](docs/known-issues.md) for what to look out for.

## What it covers

Today, mudcrab is for **Oblivion** (TES4) with **Mod Organizer 2**, and that is
all that has been run:

- **Game:** Oblivion, tested with the Steam GOTY edition. In the future
  we will try to support other TES games as well.
- **Mod manager:** MO2, a portable instance. mudcrab writes the instance --
  mods, profile, load order -- and never touches the game folder; files that
  belong there go through the Root Builder MO2 plugin. Using mod managers other
  than MO2 is something we'd like to support later. The design of mudcrab
  is reasonably generic so it's not tied too strongly to MO2, but there are places
  where it currently is required.
- **Platform:** the full pipeline has been run on **Linux** and **Windows**.
- **Nexus:** downloading through the Nexus API needs a premium account (Nexus's
  rule). Without one, `download` opens each file's page in your browser and
  picks the file up as you download it; everything else, including
  identifying archives, works on any account.

Reports from setups other than these are especially welcome: see
[CONTRIBUTING.md](CONTRIBUTING.md).

## Getting it

Prebuilt binaries for Linux and Windows can be downloaded from the
[release](https://github.com/steven-murray/mudcrab/releases) page.

The Linux build needs glibc 2.34 or newer (Ubuntu 22.04, Debian 12, Fedora 35
and later).

## Licence

MIT or Apache-2.0, at your option. See [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE).

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
