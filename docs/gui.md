# Installing with the GUI

**Preview.** The GUI is new and not yet in a release; build it from source
(below) to try it. Everything it does is one of the commands in
[Getting started](getting-started.md), which stay the complete way to use
mudcrab.

The GUI's install mode reads a list and installs it from one window:

1. **Open a list**: its `.toml` file. Its guide is on the left -- sections,
   each mod with its notes and plugins, and the load order -- read-only.
2. **Answer its questions**, if it asks any. The list's defaults are chosen;
   your answers are remembered for next time.
3. **Say where things go**: the MO2 instance to install into, the downloads
   folder, and the game folder. The game folder is found as `game-dir` finds
   it, and the downloads folder comes from `tools.toml` when it is set there;
   choose either to use another.
4. **Install.** The steps run one after another -- plan, download, check,
   install -- each showing what it is doing, what it writes, and the command
   that does the same from a terminal (with a button to copy it).

   Archives that need a click (a free Nexus account, or a file the list says
   to fetch by hand) are listed with a button to open each page; click the
   download button there, and each file is picked up as it arrives, including
   from your browser's downloads folder, which it is moved out of. The install
   carries on once all have arrived.

   If `check` finds a problem, the run stops there, with the findings listed,
   as the command would.
5. **Still to do by hand**: the list's steps that no tool can take, such as
   building the Bashed Patch in Wrye Bash. Each ticks itself when it is done
   (when the plugin it makes appears); the rest you tick yourself.

As with `install`, close MO2 before installing: it writes its own copy of the
profile back when it exits.

The GUI keeps its own settings -- recent lists, and where each list was
installed -- in `gui.toml` beside `tools.toml`, never in the list.

## Building it

You need Rust, Node.js, and on Linux WebKitGTK 4.1 (`webkit2gtk-4.1` on Arch,
`libwebkit2gtk-4.1-dev` on Debian and Ubuntu). From the repository:

```bash
cd gui
npm install
npm run build
cd ..
cargo build --release -p mudcrab-gui --features custom-protocol
```

```bash
target/release/mudcrab-gui path/to/list.toml
```

The list is optional; without it the GUI starts on its own page.
