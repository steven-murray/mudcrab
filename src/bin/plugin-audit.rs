//! Report TES4 schema gaps across a tree of plugins.
//!
//! Turns "is the field table finished?" into a concrete worklist: parses every
//! plugin it finds and reports each `(record, field)` pair the schema does not
//! describe, with a count and an example of where it occurs.
//!
//!     cargo run --bin plugin-audit -- ~/Games/.../MOFAM-03.25/mods
//!     cargo run --bin plugin-audit -- <dir> --plugin "Unique Forts Fort Aurus.esp"

use mudcrab::plugin::schema;
use mudcrab::plugin::Plugin;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

struct Gap {
    count: usize,
    example_plugin: String,
    detail: String,
}

fn main() -> std::process::ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(root) = args.next() else {
        eprintln!("usage: plugin-audit <dir> [--plugin NAME]...");
        return std::process::ExitCode::from(2);
    };

    let mut only: Vec<String> = Vec::new();
    while let Some(arg) = args.next() {
        if arg == "--plugin" {
            if let Some(name) = args.next() {
                only.push(name.to_ascii_lowercase());
            }
        }
    }

    let mut paths = Vec::new();
    collect(Path::new(&root), &mut paths);
    paths.sort();

    if !only.is_empty() {
        paths.retain(|path| {
            let name = file_name_lower(path);
            let name = name.strip_suffix(".mohidden").unwrap_or(&name);
            only.iter().any(|wanted| wanted == name)
        });
    }

    if paths.is_empty() {
        eprintln!("no plugins found under {root}");
        return std::process::ExitCode::from(2);
    }

    let mut gaps: BTreeMap<String, Gap> = BTreeMap::new();
    let mut parsed = 0usize;
    let mut records = 0usize;
    let mut unreadable = 0usize;

    for path in &paths {
        let plugin = match Plugin::read(path) {
            Ok(plugin) => plugin,
            Err(err) => {
                eprintln!("  ! {}: {err}", path.display());
                unreadable += 1;
                continue;
            }
        };
        parsed += 1;

        for record in plugin.records() {
            records += 1;
            for error in schema::audit(record) {
                let entry = gaps.entry(error.gap_key()).or_insert_with(|| Gap {
                    count: 0,
                    example_plugin: file_name_lower(path),
                    detail: error.to_string(),
                });
                entry.count += 1;
            }
        }
    }

    println!("parsed {parsed} plugins ({records} records), {unreadable} unreadable");

    if gaps.is_empty() {
        println!("no schema gaps: every record and field is described.");
        return std::process::ExitCode::SUCCESS;
    }

    println!("\n{} schema gap(s), most frequent first:\n", gaps.len());
    let mut ordered: Vec<(&String, &Gap)> = gaps.iter().collect();
    ordered.sort_by_key(|(_, gap)| std::cmp::Reverse(gap.count));

    for (key, gap) in &ordered {
        println!("{key}  x{}  (e.g. {})", gap.count, gap.example_plugin);
    }

    println!("\n--- detail for the first gap ---\n{}", ordered[0].1.detail);
    std::process::ExitCode::from(1)
}

fn file_name_lower(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase()
}

fn collect(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
            continue;
        }
        let name = file_name_lower(&path);
        let name = name.strip_suffix(".mohidden").unwrap_or(&name);
        if name.ends_with(".esp") || name.ends_with(".esm") {
            out.push(path);
        }
    }
}
