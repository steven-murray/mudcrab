# mudcrab-gui

The GUI: a Svelte 5 + TypeScript page (`src/`) in a Tauri 2 window
(`src-tauri/`), over mudcrab's library. Install mode only, so far; see
[docs/design/gui.md](../docs/design/gui.md) for the plan and
[docs/gui.md](../docs/gui.md) for what it does.

```bash
npm install
npm run tauri dev -- -- path/to/list.toml   # Vite dev server, reloads on edit
npm run check                               # svelte-check and TypeScript
cargo test -p mudcrab-gui                   # the commands, end to end
```

- **`src-tauri/src/lib.rs`**: the commands the page calls, one per step of
  the pipeline. Each calls mudcrab's library as the CLI does and returns the
  command that does the same; generic over Tauri's runtime so
  `tests/install_flow.rs` drives them on the mock runtime.
- **`src-tauri/src/sink.rs`**: mudcrab's events (`mudcrab::events`) and
  progress bars, emitted to the page.
- **`src/lib/api.ts`**: the commands and events, typed.
- **The list's prose** is someone else's Markdown: rendered with `marked`,
  sanitised with DOMPurify, links opened in the browser.

It is a member of the workspace but not a default one, so `cargo build` and
`cargo test` at the root build only the CLI.
