//! Template exporters: a folder of Jinja templates and an `exporter.toml`.
//!
//! An exporter is looked up in layers. A folder naming a `base` built-in finds
//! any template it lacks in that built-in, so overriding one piece of the Nexus
//! exporter -- how a mod is written, say -- is a folder with an `exporter.toml`
//! and a `mod.j2`. Built-in templates include their siblings by plain name, and
//! that lookup goes through the same layers, which is what makes the override
//! reach. A template can also extend a built-in one explicitly, as
//! `builtin/<name>/<file>`, to change a block rather than replace the file.

use super::Document;
use super::builtin;
use super::markup::{self, Markup};
use indexmap::IndexMap;
use minijinja::value::Value;
use minijinja::{Environment, Error, ErrorKind, UndefinedBehavior};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// A built-in exporter this one builds on: everything not set here, and
    /// every template not in this folder, comes from it.
    #[serde(default)]
    pub base: Option<String>,
    #[serde(default)]
    pub output: Option<OutputMode>,
    #[serde(default)]
    pub markup: Option<Markup>,
    #[serde(default)]
    pub images: Option<ImageMode>,
    #[serde(default)]
    pub pages: Vec<Page>,
    #[serde(default)]
    pub options: IndexMap<String, OptionSpec>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputMode {
    /// One file, written to `-o` or stdout.
    File,
    /// A directory of pages, plus the exporter's `static/` files.
    Site,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageMode {
    /// Images are linked by `url`; one with only a local path is left out,
    /// and said so.
    Url,
    /// Local images are copied into the output's `images/`; `url` otherwise.
    Copy,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    pub template: String,
    /// Path inside a site, itself a template (`"{{ section.anchor }}.md"`).
    /// Ignored for single-file output.
    #[serde(default)]
    pub output: Option<String>,
    /// Render once per top-level section, with `section` set.
    #[serde(default)]
    pub for_each: Option<ForEach>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ForEach {
    Section,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OptionSpec {
    pub default: toml::Value,
    #[serde(default)]
    pub help: String,
}

/// Where an exporter's files come from.
#[derive(Debug, Clone)]
enum Layer {
    Dir(PathBuf),
    Builtin(&'static str),
}

impl Layer {
    fn read(&self, file: &str) -> Option<String> {
        match self {
            Layer::Dir(dir) => std::fs::read_to_string(dir.join(file)).ok(),
            Layer::Builtin(name) => builtin::file(name, file).map(str::to_string),
        }
    }

    /// Files under `static/`, as (path inside static, contents).
    fn static_files(&self) -> anyhow::Result<Vec<(String, Vec<u8>)>> {
        match self {
            Layer::Builtin(name) => Ok(builtin::files(name)
                .filter_map(|(path, text)| {
                    path.strip_prefix("static/")
                        .map(|p| (p.to_string(), text.as_bytes().to_vec()))
                })
                .collect()),
            Layer::Dir(dir) => {
                let root = dir.join("static");
                let mut out = Vec::new();
                if root.is_dir() {
                    collect_files(&root, &root, &mut out)?;
                }
                Ok(out)
            }
        }
    }
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_files(root, &path, out)?;
        } else {
            let relative = path
                .strip_prefix(root)?
                .to_string_lossy()
                .replace('\\', "/");
            out.push((relative, std::fs::read(&path)?));
        }
    }
    Ok(())
}

/// An exporter ready to run: its layers, top first, and its settings with
/// everything inherited filled in.
pub struct Exporter {
    pub name: String,
    layers: Vec<Layer>,
    output: OutputMode,
    markup: Markup,
    images: ImageMode,
    pages: Vec<Page>,
    options: IndexMap<String, OptionSpec>,
}

impl Exporter {
    /// `spec` is a built-in name, a path to an exporter folder, or the name of
    /// a folder in `exporters/` next to the modlist.
    pub fn find(spec: &str, modlist_dir: &Path) -> anyhow::Result<Exporter> {
        let as_path = PathBuf::from(spec);
        let local = modlist_dir.join("exporters").join(spec);
        if as_path.join("exporter.toml").is_file() {
            Exporter::from_dir(&as_path)
        } else if let Some(name) = builtin::name(spec) {
            Exporter::from_layers(chain(Layer::Builtin(name))?)
        } else if local.join("exporter.toml").is_file() {
            Exporter::from_dir(&local)
        } else {
            anyhow::bail!(
                "no exporter {spec:?}: not a built-in ({}), not a folder with an exporter.toml, \
                 and not in {}",
                builtin::names().collect::<Vec<_>>().join(", "),
                modlist_dir.join("exporters").display()
            )
        }
    }

    fn from_dir(dir: &Path) -> anyhow::Result<Exporter> {
        Exporter::from_layers(chain(Layer::Dir(dir.to_path_buf()))?)
    }

    fn from_layers(layers: Vec<Layer>) -> anyhow::Result<Exporter> {
        let manifests: Vec<Manifest> = layers
            .iter()
            .map(read_manifest)
            .collect::<anyhow::Result<_>>()?;
        let pick = |get: &dyn Fn(&Manifest) -> bool| manifests.iter().find(|m| get(m));
        let name = manifests[0].name.clone();
        let output = manifests
            .iter()
            .find_map(|m| m.output)
            .unwrap_or(OutputMode::File);
        let markup = manifests
            .iter()
            .find_map(|m| m.markup)
            .unwrap_or(Markup::Markdown);
        let images = manifests
            .iter()
            .find_map(|m| m.images)
            .unwrap_or(ImageMode::Url);
        let pages = pick(&|m| !m.pages.is_empty())
            .map(|m| m.pages.clone())
            .ok_or_else(|| anyhow::anyhow!("exporter {name}: no [[pages]] to render"))?;
        // Base options first, so a folder can change a default or add its own.
        let mut options = IndexMap::new();
        for manifest in manifests.iter().rev() {
            options.extend(manifest.options.clone());
        }
        if output == OutputMode::File && pages.len() != 1 {
            anyhow::bail!("exporter {name}: single-file output renders exactly one page");
        }
        Ok(Exporter {
            name,
            layers,
            output,
            markup,
            images,
            pages,
            options,
        })
    }

    pub fn output_mode(&self) -> OutputMode {
        self.output
    }

    /// The options, their defaults and what they do, for `--list-options`.
    pub fn describe_options(&self) -> String {
        let mut out = String::new();
        for (key, spec) in &self.options {
            out.push_str(&format!("{key} = {}\n", spec.default));
            if !spec.help.is_empty() {
                out.push_str(&format!("    {}\n", spec.help));
            }
        }
        if out.is_empty() {
            out.push_str(&format!("{} has no options\n", self.name));
        }
        out
    }

    /// Option values: declared defaults, then the modlist's `[export.<name>]`,
    /// then `--opt`. A name the exporter does not declare is an error, since a
    /// misspelt option would otherwise do nothing.
    pub fn resolve_options(
        &self,
        from_modlist: Option<&toml::Value>,
        from_cli: &[String],
    ) -> anyhow::Result<toml::Table> {
        let mut values: toml::Table = self
            .options
            .iter()
            .map(|(key, spec)| (key.clone(), spec.default.clone()))
            .collect();
        let known = || self.options.keys().cloned().collect::<Vec<_>>().join(", ");
        if let Some(table) = from_modlist {
            let table = table
                .as_table()
                .ok_or_else(|| anyhow::anyhow!("[export.{}] must be a table", self.name))?;
            for (key, value) in table {
                anyhow::ensure!(
                    values.contains_key(key),
                    "[export.{}] sets {key}, which the exporter does not have (it has: {})",
                    self.name,
                    known()
                );
                values.insert(key.clone(), value.clone());
            }
        }
        for pair in from_cli {
            let (key, raw) = pair
                .split_once('=')
                .ok_or_else(|| anyhow::anyhow!("--opt {pair}: expected key=value"))?;
            let default = values.get(key).ok_or_else(|| {
                anyhow::anyhow!("--opt {key}: no such option (it has: {})", known())
            })?;
            let value = match default {
                toml::Value::Boolean(_) => toml::Value::Boolean(match raw {
                    "true" | "yes" | "1" => true,
                    "false" | "no" | "0" => false,
                    _ => anyhow::bail!("--opt {key}={raw}: {key} is true or false"),
                }),
                toml::Value::Integer(_) => toml::Value::Integer(
                    raw.parse()
                        .map_err(|_| anyhow::anyhow!("--opt {key}={raw}: {key} is a number"))?,
                ),
                _ => toml::Value::String(raw.to_string()),
            };
            values.insert(key.to_string(), value);
        }
        Ok(values)
    }

    /// Render `document` into `output` (a file, a directory, or stdout).
    /// Returns warnings worth telling the user about.
    pub fn render(
        &self,
        document: &Document,
        options: &toml::Table,
        modlist_dir: &Path,
        output: Option<&Path>,
    ) -> anyhow::Result<Vec<String>> {
        let state = Arc::new(Mutex::new(RenderState::default()));
        let mut env = self.environment(state.clone())?;
        let doc = Value::from_serialize(document);
        let options = Value::from_serialize(options);
        // Also a global, so macros in imported templates -- which do not see
        // the caller's context -- can follow options too.
        env.add_global("options", options.clone());
        let exporter = minijinja::context! { name => self.name.clone(), markup => self.markup };

        let mut rendered: Vec<(Option<String>, String)> = Vec::new();
        for page in &self.pages {
            let template = env.get_template(&page.template).map_err(|err| {
                anyhow::anyhow!("exporter {}: {}", self.name, describe_error(&err))
            })?;
            let sections: Vec<Option<Value>> = match page.for_each {
                None => vec![None],
                Some(ForEach::Section) => document
                    .entries
                    .iter()
                    .filter(|entry| matches!(entry, super::Entry::Section(_)))
                    .map(|entry| Some(Value::from_serialize(entry)))
                    .collect(),
            };
            for section in sections {
                let ctx = minijinja::context! {
                    doc => doc.clone(), options => options.clone(), exporter => exporter.clone(), section => section.clone(),
                };
                let text = template.render(&ctx).map_err(|err| {
                    anyhow::anyhow!("exporter {}: {}", self.name, describe_error(&err))
                })?;
                let path = match &page.output {
                    Some(pattern) => Some(env.render_str(pattern, &ctx).map_err(|err| {
                        anyhow::anyhow!("exporter {}: page output: {err}", self.name)
                    })?),
                    None => None,
                };
                rendered.push((path, tidy(&text)));
            }
        }

        let state = std::mem::take(&mut *state.lock().expect("render state"));
        match self.output {
            OutputMode::File => {
                let (_, text) = rendered.pop().expect("one page");
                match output {
                    Some(path) => std::fs::write(path, text).map_err(|err| {
                        anyhow::anyhow!("failed to write {}: {err}", path.display())
                    })?,
                    None => print!("{text}"),
                }
            }
            OutputMode::Site => {
                let dir = output.ok_or_else(|| {
                    anyhow::anyhow!(
                        "exporter {} writes a site: give the directory with -o",
                        self.name
                    )
                })?;
                for (path, text) in rendered {
                    // A page with nothing to say (a closing page for a guide
                    // with no closing text) is not written at all.
                    if text.trim().is_empty() {
                        continue;
                    }
                    let path = path.ok_or_else(|| {
                        anyhow::anyhow!(
                            "exporter {}: a site page needs an `output` path",
                            self.name
                        )
                    })?;
                    write_inside(dir, &path, text.as_bytes())?;
                }
                // Lower layers first, so a folder's own static file wins.
                for layer in self.layers.iter().rev() {
                    for (path, bytes) in layer.static_files()? {
                        write_inside(dir, &path, &bytes)?;
                    }
                }
                for (source, dest) in &state.copies {
                    let bytes = std::fs::read(modlist_dir.join(source))
                        .map_err(|err| anyhow::anyhow!("failed to read image {source}: {err}"))?;
                    write_inside(dir, dest, &bytes)?;
                }
            }
        }
        Ok(state.warnings)
    }

    fn environment(&self, state: Arc<Mutex<RenderState>>) -> anyhow::Result<Environment<'static>> {
        let mut env = Environment::new();
        env.set_undefined_behavior(UndefinedBehavior::Chainable);
        env.set_trim_blocks(true);
        env.set_lstrip_blocks(true);
        env.set_keep_trailing_newline(true);

        let layers = self.layers.clone();
        env.set_loader(move |name| {
            if let Some(rest) = name.strip_prefix("builtin/") {
                let (exporter, file) = rest.split_once('/').unwrap_or((rest, ""));
                return Ok(builtin::name(exporter)
                    .and_then(|n| builtin::file(n, file))
                    .map(str::to_string));
            }
            Ok(layers.iter().find_map(|layer| layer.read(name)))
        });

        let markup = self.markup;
        env.add_filter("markup", move |text: Option<String>| {
            render_markup(text, markup)
        });
        env.add_filter("bbcode", |text: Option<String>| {
            render_markup(text, Markup::Bbcode)
        });
        env.add_filter("html", |text: Option<String>| {
            render_markup(text, Markup::Html)
        });

        let images = self.images;
        env.add_filter(
            "image_src",
            move |image: Value, place: Option<String>| -> Result<Value, Error> {
                let get = |key: &str| -> Option<String> {
                    image
                        .get_attr(key)
                        .ok()
                        .filter(|v| !v.is_undefined() && !v.is_none())
                        .map(|v| v.to_string())
                };
                let (path, url) = (get("path"), get("url"));
                let mut state = state
                    .lock()
                    .map_err(|_| Error::new(ErrorKind::InvalidOperation, "render state"))?;
                match (images, path, url) {
                    (ImageMode::Copy, Some(path), _) => {
                        let dest = format!("images/{}", path.replace(['/', '\\'], "-"));
                        if !state.copies.iter().any(|(_, d)| *d == dest) {
                            state.copies.push((path, dest.clone()));
                        }
                        Ok(Value::from(dest))
                    }
                    (_, _, Some(url)) => Ok(Value::from(url)),
                    (_, path, None) => {
                        state.warnings.push(format!(
                            "{}: image {} has no url, so it is left out",
                            place.unwrap_or_else(|| "an image".to_string()),
                            path.unwrap_or_default()
                        ));
                        Ok(Value::from(()))
                    }
                }
            },
        );
        Ok(env)
    }

    /// Copy a built-in exporter's files into `dir` to edit freely.
    pub fn eject(spec: &str, dir: &Path) -> anyhow::Result<usize> {
        let name = builtin::name(spec)
            .ok_or_else(|| anyhow::anyhow!("{spec:?} is not a built-in exporter"))?;
        if dir.exists() && std::fs::read_dir(dir)?.next().is_some() {
            anyhow::bail!("{} is not empty; eject into a new folder", dir.display());
        }
        let mut count = 0;
        for (path, text) in builtin::files(name) {
            write_inside(dir, path, text.as_bytes())?;
            count += 1;
        }
        Ok(count)
    }
}

#[derive(Default)]
struct RenderState {
    /// Images to copy into a site: (path relative to the modlist, path in the site).
    copies: Vec<(String, String)>,
    warnings: Vec<String>,
}

/// `top` and every built-in it builds on, top first. Built-ins can build on
/// each other too (`github-pages` on `markdown`).
fn chain(top: Layer) -> anyhow::Result<Vec<Layer>> {
    let mut layers = vec![top];
    loop {
        let last = layers.last().expect("never empty");
        let Some(base) = read_manifest(last)?.base else {
            break;
        };
        let name = builtin::name(&base)
            .ok_or_else(|| anyhow::anyhow!("{last:?}: base {base:?} is not a built-in exporter"))?;
        anyhow::ensure!(
            !layers
                .iter()
                .any(|l| matches!(l, Layer::Builtin(n) if *n == name)),
            "{last:?}: base {base:?} builds on itself"
        );
        layers.push(Layer::Builtin(name));
    }
    Ok(layers)
}

fn read_manifest(layer: &Layer) -> anyhow::Result<Manifest> {
    let raw = layer
        .read("exporter.toml")
        .ok_or_else(|| anyhow::anyhow!("{layer:?} has no exporter.toml"))?;
    toml::from_str(&raw).map_err(|err| anyhow::anyhow!("{layer:?}: exporter.toml: {err}"))
}

fn render_markup(text: Option<String>, markup: Markup) -> String {
    text.map(|t| markup::render(&t, markup)).unwrap_or_default()
}

/// A template error with where it happened, which minijinja keeps in the chain.
fn describe_error(err: &Error) -> String {
    let mut out = err.to_string();
    if let Some(detail) = err
        .display_debug_info()
        .to_string()
        .lines()
        .find(|l| l.contains("-->"))
    {
        out.push_str(&format!(" ({})", detail.trim()));
    }
    out
}

/// Templates leave runs of blank lines where conditional blocks rendered
/// nothing; no output format wants more than one.
fn tidy(text: &str) -> String {
    let mut out = String::new();
    let mut blank = 0;
    for line in text.lines() {
        let line = line.trim_end();
        if line.is_empty() {
            blank += 1;
            if blank > 1 {
                continue;
            }
        } else {
            blank = 0;
        }
        out.push_str(line);
        out.push('\n');
    }
    out.trim_start_matches('\n').to_string()
}

/// Write `relative` under `dir`, refusing a path that would land outside it --
/// page names come from templates, which come from whoever wrote the exporter.
fn write_inside(dir: &Path, relative: &str, bytes: &[u8]) -> anyhow::Result<()> {
    let relative = Path::new(relative);
    anyhow::ensure!(
        relative
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_))),
        "refusing to write {} outside the output folder",
        relative.display()
    );
    let path = dir.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, bytes)
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", path.display()))
}

#[cfg(test)]
mod tests {
    #[test]
    fn pages_cannot_be_written_outside_the_output() {
        let dir = tempfile::tempdir().unwrap();
        for bad in ["../escape.md", "/etc/passwd", "a/../../b.md"] {
            assert!(super::write_inside(dir.path(), bad, b"x").is_err(), "{bad}");
        }
        super::write_inside(dir.path(), "a/b.md", b"x").unwrap();
        assert!(dir.path().join("a/b.md").is_file());
    }

    #[test]
    fn runs_of_blank_lines_collapse() {
        assert_eq!(super::tidy("\n\na\n\n\n\nb  \n"), "a\n\nb\n");
    }
}
