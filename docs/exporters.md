# Writing an exporter

`mudcrab export` renders a modlist through an *exporter*: a folder holding an
`exporter.toml` and some [Jinja](https://jinja.palletsprojects.com/templates/)
templates (mudcrab uses [minijinja](https://docs.rs/minijinja), which follows
Jinja2 closely). The built-in exporters are folders like this too, compiled into
the binary; see [`exporters/`](https://github.com/steven-murray/mudcrab/tree/main/exporters).

```bash
mudcrab export mofam.toml --to nexus -o guide.bbcode    # a built-in
mudcrab export mofam.toml --to ./my-exporter -o out/    # a folder
mudcrab export mofam.toml --to mine                     # exporters/mine/, next to the modlist
mudcrab export mofam.toml --to nexus --list-options
```

## Three ways to customise

**Set options.** Every exporter declares options with defaults. Set them per
run with `--opt key=value`, or per modlist:

```toml
[export.nexus]
load_order = false
```

**Override one template.** A folder whose `exporter.toml` names a `base`
inherits everything from that built-in and replaces only the templates it
contains. To change how each mod is written in the Nexus guide:

```
my-nexus/
  exporter.toml     name = "my-nexus" / base = "nexus"
  mod.j2            your version
```

The built-in templates include each other by plain name (`{% include "mod.j2" %}`),
and those lookups check your folder first, so your `mod.j2` is used everywhere
the guide writes a mod. A template can also reach a built-in one explicitly as
`builtin/<exporter>/<file>`, to wrap it (`{% include "builtin/nexus/guide.j2" %}`)
or `{% extends %}` it.

**Start from a copy.** `mudcrab export --to nexus --eject my-nexus` writes the
built-in's files into a new folder, to change freely.

## exporter.toml

```toml
name = "my-guide"
description = "What it is for"
base = "nexus"            # optional: inherit from a built-in
output = "file"           # "file": one page, to -o or stdout; "site": a folder of pages
markup = "bbcode"         # what `| markup` turns docs' Markdown into: bbcode, html, markdown
images = "url"            # "url": link images by url; "copy": copy local images into the site

[[pages]]
template = "guide.j2"

# A site renders several pages; `output` is itself a template.
# [[pages]]
# template = "section.md.j2"
# output = "{{ section.anchor }}.md"
# for_each = "section"     # once per top-level section, with `section` set

[options.toc]
default = true            # its type (bool, number, text) is how --opt is read
help = "List the sections first"
```

A `site` exporter's `static/` folder is copied into the output as it stands
(a `_config.yml`, a stylesheet).

## What templates see

| Name | What |
|---|---|
| `doc` | The document: the same thing `--to json` writes. Run that on your modlist to see every field. |
| `options` | Option values after defaults, `[export.<name>]` and `--opt`. Also a global, so macros in imported templates can read it. |
| `exporter` | `name`, `markup`. |
| `section` | The current top-level section, on a `for_each = "section"` page. |

The document's outline:

- `doc.name`, `doc.docs` (the guide's `[docs]`), `doc.inputs` (each with
  `query`, `choices`, `default`, and `answer` when exporting a plan),
  `doc.load_order` (`name`, `active`, `mod`), `doc.mod_count`.
- `doc.entries`: the top level, each with `kind` either `"section"` or `"mod"`.
- A section: `name` (as MO2 shows it), `title`, `path`, `depth`, `anchor`,
  `docs`, `entries`.
- A mod: `id`, `title`, `anchor`, `pages` (each Nexus page its archives come
  from, with `mod_url`), `condition` (`text`, as a reader would say it, and
  `expr`), `docs`, `plugins`, `archives`, `steps`, `merge`.
- An archive: `file_name`, `source`, `nexus` (`file_url`, ids), `url`,
  `condition`, `instructions` (each a `kind` and `text`: installer choices and
  the like, in words), and `raw` (the entry as the modlist has it).
- A step: `action`, `kind`, `text`, `code` (literal text to go with it),
  `condition`, `raw`.
- `doc.kinds`: every kind the guide uses, for a key.
- An entry of kind `"revisit"`: steps of a mod installed earlier that need
  mods installed since (a `conflicts_with` naming a later mod). It carries the
  mod's `id`, `title`, `mod_anchor`, `section` and `steps`, and sits right
  after the last mod those steps need.
- `doc.tools`: Mod Organizer 2 plugins (`type = "mo2-plugin"`), to set up
  before any mod and not part of `entries`.
- `doc.game_settings`: the list's own `[ini]` settings, as steps.
- A condition's `question` is set when the condition is simply "yes to this
  question", so a guide can say "Optional: <question>" instead of the full
  sentence in `text`.

## Symbols

Every instruction and step has a `kind` naming the activity -- `bain`, `fomod`,
`ini`, `qac`, `hide`, `bsa`, ... (the full list is in
`src/export/describe.rs`) -- plus `optional`, `manual` and `merge` for mods
and archives. The built-in exporters mark each with a symbol (🧹 for QAC, ⚙️
for an INI edit) and open with a key listing only the symbols the guide uses.
The table lives in each exporter's `symbols.j2`: override that one file to
change them.

`docs` anywhere has `title`, `summary`, `body`, `footer`, `images`, `links`
(a mod's links appear beside its title, as 🌐, in the built-in exporters), and any other
key the author added: a custom exporter is free to use fields only its own
lists have.

A missing value renders as nothing rather than failing, including down a chain
(`mod.nexus.mod_url` on a mod with no Nexus archive), so templates can test with
a plain `{% if %}`.

## Filters

| Filter | Does |
|---|---|
| `markup` | Docs Markdown into the exporter's `markup` |
| `bbcode`, `html` | The same, into a fixed markup |
| `image_src(place)` | An image's `src`: its `url`, or with `images = "copy"` a copied local file's path in the site. Nothing if the image cannot be used here, with a warning naming `place`. |

Plus everything Jinja2 offers (`default`, `join`, `selectattr`, ...).
