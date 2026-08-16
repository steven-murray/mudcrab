//! M2 gate against the real MOFAM install.
//!
//! Skipped unless `MUDCRAB_MOFAM_ROOT` points at an MO2 instance, so a checkout
//! without the game still runs green:
//!
//!   MUDCRAB_MOFAM_ROOT=~/Games/Wabbajack/Oblivion/MOFAM-03.25 \
//!     cargo test --test plugin_roundtrip_real -- --nocapture

use mudcrab::plugin::Plugin;
use std::path::{Path, PathBuf};

fn mofam_mods() -> Option<PathBuf> {
    let root = PathBuf::from(std::env::var_os("MUDCRAB_MOFAM_ROOT")?);
    let mods = root.join("mods");
    mods.is_dir().then_some(mods)
}

/// Every plugin under the mods tree, including `.mohidden` ones (merge sources
/// are hidden after merging, and they are exactly what we need to parse).
fn collect_plugins(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_plugins(&path, out);
            continue;
        }
        let name = path.file_name().unwrap_or_default().to_string_lossy().to_ascii_lowercase();
        let name = name.strip_suffix(".mohidden").unwrap_or(&name);
        if name.ends_with(".esp") || name.ends_with(".esm") {
            out.push(path);
        }
    }
}

#[test]
fn round_trips_every_real_plugin_byte_for_byte() {
    let Some(mods) = mofam_mods() else {
        eprintln!("skipping: set MUDCRAB_MOFAM_ROOT to run against the real install");
        return;
    };

    let mut paths = Vec::new();
    collect_plugins(&mods, &mut paths);
    paths.sort();
    assert!(!paths.is_empty(), "found no plugins under {}", mods.display());

    let mut checked = 0usize;
    let mut compressed_records = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for path in &paths {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) => {
                failures.push(format!("{}: read failed: {err}", path.display()));
                continue;
            }
        };

        let plugin = match Plugin::parse(&bytes) {
            Ok(plugin) => plugin,
            Err(err) => {
                failures.push(format!("{}: parse failed: {err}", path.display()));
                continue;
            }
        };

        compressed_records += plugin.records().filter(|r| r.is_compressed()).count();

        let written = plugin.to_bytes();
        if written != bytes {
            let at = written
                .iter()
                .zip(&bytes)
                .position(|(a, b)| a != b)
                .unwrap_or_else(|| bytes.len().min(written.len()));
            failures.push(format!(
                "{}: bytes differ at offset {at} ({} written vs {} original)",
                path.display(),
                written.len(),
                bytes.len()
            ));
            continue;
        }
        checked += 1;
    }

    eprintln!(
        "round-tripped {checked}/{} plugins byte-for-byte ({compressed_records} compressed records)",
        paths.len()
    );

    assert!(
        failures.is_empty(),
        "{} of {} plugins failed:\n  {}",
        failures.len(),
        paths.len(),
        failures
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}
