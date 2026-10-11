# Writing an exporter

`mudcrab export` renders a modlist through an *exporter*: a folder holding an
`exporter.toml` and some [Jinja](https://jinja.palletsprojects.com/templates/)
templates (mudcrab uses [minijinja](https://docs.rs/minijinja), which follows
Jinja2 closely). The built-in exporters are folders like this too, compiled into
the binary; see [`exporters/`](../exporters/).

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
| `options` | Option values after defaults, `[export.<name>]` and `--opt`. |
| `exporter` | `name`, `markup`. |
| `section` | The current top-level section, on a `for_each = "section"` page. |

The document's outline:

- `doc.name`, `doc.docs` (the guide's `[docs]`), `doc.inputs` (each with
  `query`, `choices`, `default`, and `answer` when exporting a plan),
  `doc.load_order` (`name`, `active`, `mod`), `doc.mod_count`.
- `doc.entries`: the top level, each with `kind` either `"section"` or `"mod"`.
- A section: `name` (as MO2 shows it), `title`, `path`, `depth`, `anchor`,
  `docs`, `entries`.
- A mod: `id`, `title`, `anchor`, `nexus` (`mod_url`), `condition` (`text`, as a
  reader would say it, and `expr`), `docs`, `plugins`, `archives`, `steps`,
  `merge`.
- An archive: `file_name`, `source`, `nexus` (`file_url`, ids), `url`,
  `condition`, `instructions` (installer choices and the like, in words), and
  `raw` (the entry as the modlist has it).
- A step: `action`, `text`, `code` (literal text to go with it), `condition`,
  `raw`.

`docs` anywhere has `title`, `summary`, `body`, `footer`, `images`, `links`, and any other
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
