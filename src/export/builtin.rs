//! The exporters that ship inside the binary, from `exporters/` in the repo.
//!
//! They are ordinary exporter folders, embedded: `--eject` writes one back out
//! as a starting point, and a folder can name one as its `base`.

/// `json` is built in too, but it writes the document rather than templates.
pub const JSON: &str = "json";

macro_rules! exporter_files {
    ($($exporter:literal => [$($file:literal),* $(,)?]),* $(,)?) => {
        &[$($(($exporter, $file, include_str!(concat!("../../exporters/", $exporter, "/", $file))),)*)*]
    };
}

/// (exporter, path inside it, contents).
const FILES: &[(&str, &str, &str)] = exporter_files! {
    "nexus" => ["exporter.toml", "guide.j2", "entry.j2", "section.j2", "mod.j2", "macros.j2", "symbols.j2"],
    "github-pages" => ["exporter.toml", "_config.yml.j2", "front-matter.j2", "index.md.j2", "section-page.md.j2", "load-order.md.j2"],
    "markdown" => ["exporter.toml", "guide.md.j2", "entry.md.j2", "section.md.j2", "mod.md.j2", "macros.md.j2", "symbols.j2"],
};

/// Template exporter names, in the order they were added.
pub fn names() -> impl Iterator<Item = &'static str> {
    let mut seen: Vec<&'static str> = Vec::new();
    for (name, _, _) in FILES {
        if !seen.contains(name) {
            seen.push(name);
        }
    }
    std::iter::once(JSON).chain(seen)
}

/// The built-in exporter called `spec`, if there is one.
pub fn name(spec: &str) -> Option<&'static str> {
    FILES
        .iter()
        .map(|(name, _, _)| *name)
        .find(|name| *name == spec)
}

pub fn file(exporter: &str, path: &str) -> Option<&'static str> {
    FILES
        .iter()
        .find(|(name, file, _)| *name == exporter && *file == path)
        .map(|(_, _, text)| *text)
}

pub fn files(exporter: &str) -> impl Iterator<Item = (&'static str, &'static str)> + '_ {
    FILES
        .iter()
        .filter(move |(name, _, _)| *name == exporter)
        .map(|(_, path, text)| (*path, *text))
}
