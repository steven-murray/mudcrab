# Contributing to mudcrab

Bug reports, modlists that install wrongly, and pull requests are all welcome.
mudcrab is young and has so far been run against one large list (see the
readme's scope), so a report from a different setup is worth more than it
looks.

## Reporting a modlist that installs wrongly

This is the most useful kind of report, and the hardest to act on without the
right attachments. Open an issue with the **"A modlist installs wrongly"**
template and include:

- **`mudcrab --version`**, your OS, and MO2's version.
- **The plan**: the JSON `mudcrab query` wrote (`--output plan.json`). It holds
  every decision the install acted on -- archives, layouts, installer choices,
  actions, the load order -- with your answers to the list's questions already
  applied. If the list is public, a link to it as well.
- **The log** of the failing run. Every run writes one, and a failed run prints
  `full log: <path>` as its last line (`~/.local/state/mudcrab/logs/` on Linux,
  `%LOCALAPPDATA%\mudcrab\logs` on Windows).
- **What is wrong, as specifically as you can.** If you have an instance built
  another way (by hand, or with Wabbajack) to compare against, `mudcrab diff`
  output for the mod in question says exactly which files differ; see
  [docs/usage.md](docs/usage.md#diff).
- **The archive**, by name and where it came from (a Nexus file id is ideal).
  Please do not attach mod archives themselves.

Before attaching anything, check it for secrets. A plan and a log never contain
your Nexus API key, but `tools.toml` and your shell environment might, and logs
contain your local paths.

## Other bugs and ideas

Use the **bug report** or **idea** template. For an idea, the problem it solves
matters more than the shape of the solution: several of mudcrab's features
ended up looking quite different from the first suggestion.

## Pull requests

Build and check with:

```bash
cargo fmt
cargo test
cargo clippy --all-targets -- -D warnings
```

All three must pass; CI runs them on every push and pull request. The toolchain
is pinned in `rust-toolchain.toml` and rustup picks it up by itself, so what
passes locally passes in CI. The minimum supported Rust version is
`rust-version` in `Cargo.toml`, also checked by CI.

A few conventions the code relies on:

- **Comments explain why.** Most of mudcrab works around undocumented
  behaviour of a twenty-year-old file format or of MO2; the comment saying why
  a line is the way it is tends to be the most valuable thing in the file.
  Match the density of the code around your change.
- **No blanket `#[allow]`.** Fix the lint, or suppress it narrowly with a
  reason.
- **Tests for behaviour.** A bug fix comes with a test that fails without it.
  Tests must not need network access, an API key or real mod archives;
  `tests/support/` builds small plugins and BSAs in memory.
- **User-facing changes update the docs** in `docs/` (usage, the modlist
  format, known issues) in the same pull request.
- **A modlist's TOML is the user's.** Commands that edit one (`add`, `move`)
  splice lines rather than re-emitting parsed TOML, so comments and layout
  survive. Keep it that way.

By contributing you agree that your work is licensed as the project is: MIT or
Apache-2.0, at the user's option.
