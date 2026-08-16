//! BSA gate against the real MOFAM install.
//!
//! Skipped unless `MUDCRAB_MOFAM_ROOT` points at an MO2 instance, so a checkout
//! without the game still runs green:
//!
//!   MUDCRAB_MOFAM_ROOT=~/Games/Wabbajack/Oblivion/MOFAM-03.25 \
//!     cargo test --test bsa_roundtrip_real -- --nocapture

use mudcrab::bsa::Bsa;
use std::io::Write;
use std::path::{Path, PathBuf};

fn mofam_mods() -> Option<PathBuf> {
    let root = PathBuf::from(std::env::var_os("MUDCRAB_MOFAM_ROOT")?);
    let mods = root.join("mods");
    mods.is_dir().then_some(mods)
}

fn collect_archives(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_archives(&path, out);
            continue;
        }
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_ascii_lowercase();
        let name = name.strip_suffix(".mohidden").unwrap_or(&name);
        if name.ends_with(".bsa") {
            out.push(path);
        }
    }
}

/// A sink that compares what is written against a reference, so a 2 GiB
/// archive does not need a second 2 GiB buffer to be checked byte for byte.
struct CompareSink<'a> {
    expected: &'a [u8],
    at: usize,
    /// Offset of the first difference, if any.
    diff: Option<usize>,
}

impl Write for CompareSink<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.diff.is_none() {
            let available = self.expected.len().saturating_sub(self.at);
            let n = buf.len().min(available);
            if let Some(at) = buf[..n]
                .iter()
                .zip(&self.expected[self.at..self.at + n])
                .position(|(a, b)| a != b)
            {
                self.diff = Some(self.at + at);
            } else if buf.len() > available {
                self.diff = Some(self.expected.len());
            }
        }
        self.at += buf.len();
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn round_trips_every_real_archive_byte_for_byte() {
    let Some(mods) = mofam_mods() else {
        eprintln!("skipping: set MUDCRAB_MOFAM_ROOT to run against the real install");
        return;
    };

    let mut paths = Vec::new();
    collect_archives(&mods, &mut paths);
    paths.sort();
    assert!(!paths.is_empty(), "found no archives under {}", mods.display());

    let mut checked = 0usize;
    let mut total_files = 0usize;
    let mut compressed_files = 0usize;
    let mut compressed_archives = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for path in &paths {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) => {
                failures.push(format!("{}: read failed: {err}", path.display()));
                continue;
            }
        };

        let parsed = match Bsa::parse(&bytes) {
            Ok(parsed) => parsed,
            Err(err) => {
                failures.push(format!("{}: parse failed: {err}", path.display()));
                continue;
            }
        };

        total_files += parsed.file_count();
        if parsed.compressed_by_default() {
            compressed_archives += 1;
        }

        // Every compressed payload must actually inflate. Re-deflating would
        // not reproduce the original stream, so the round-trip below replays
        // the stored bytes; this is what proves they were understood.
        let mut decompress_failures = 0usize;
        for (folder, file) in parsed.files() {
            if !file.compressed {
                continue;
            }
            compressed_files += 1;
            let name = file.path_in(folder);
            match file.data(&name) {
                Ok(data) => {
                    let declared = u32::from_le_bytes(
                        file.stored_bytes()[0..4].try_into().unwrap(),
                    ) as usize;
                    if data.len() != declared {
                        decompress_failures += 1;
                        if decompress_failures == 1 {
                            failures.push(format!(
                                "{}: '{name}' inflated to {} bytes, declared {declared}",
                                path.display(),
                                data.len()
                            ));
                        }
                    }
                }
                Err(err) => {
                    decompress_failures += 1;
                    if decompress_failures == 1 {
                        failures.push(format!("{}: {err}", path.display()));
                    }
                }
            }
        }

        let mut sink = CompareSink {
            expected: &bytes,
            at: 0,
            diff: None,
        };
        if let Err(err) = parsed.write_to(&mut sink) {
            failures.push(format!("{}: write failed: {err}", path.display()));
            continue;
        }

        if let Some(at) = sink.diff {
            failures.push(format!(
                "{}: bytes differ at offset {at} ({} written vs {} original)",
                path.display(),
                sink.at,
                bytes.len()
            ));
            continue;
        }
        if sink.at != bytes.len() {
            failures.push(format!(
                "{}: length differs ({} written vs {} original)",
                path.display(),
                sink.at,
                bytes.len()
            ));
            continue;
        }
        if decompress_failures > 0 {
            continue;
        }
        checked += 1;
    }

    eprintln!(
        "round-tripped {checked}/{} archives byte-for-byte \
         ({total_files} files, {compressed_files} compressed payloads, \
         {compressed_archives} compressed-by-default archives)",
        paths.len()
    );

    assert!(
        failures.is_empty(),
        "{} of {} archives failed:\n  {}",
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

/// The hash is the part of the format that fails silently: a wrong hash makes
/// Oblivion ignore the asset rather than reject the archive.
///
/// The round-trip above already proves the hashes exactly, because the writer
/// recomputes every one of them from the name and the bytes still match. This
/// checks the same thing from a different angle -- records are stored sorted by
/// hash, and a wrong hash function would not reproduce the corpus's ordering --
/// so a hashing regression reports itself as such rather than as an opaque
/// "bytes differ at offset N".
#[test]
fn recomputed_hashes_reproduce_the_real_record_ordering() {
    let Some(mods) = mofam_mods() else {
        eprintln!("skipping: set MUDCRAB_MOFAM_ROOT to run against the real install");
        return;
    };

    let mut paths = Vec::new();
    collect_archives(&mods, &mut paths);
    paths.sort();

    let mut folder_names = 0usize;
    let mut file_names = 0usize;
    let mut problems: Vec<String> = Vec::new();

    for path in &paths {
        let bytes = std::fs::read(path).expect("read");
        let Ok(parsed) = Bsa::parse(&bytes) else {
            problems.push(format!("{}: parse failed", path.display()));
            continue;
        };

        let hashes: Vec<u64> = parsed
            .folders
            .iter()
            .map(|folder| mudcrab::bsa::hash_folder_name(&folder.name))
            .collect();
        folder_names += hashes.len();
        if hashes.windows(2).any(|pair| pair[0] > pair[1]) {
            problems.push(format!("{}: folder hashes are not ascending", path.display()));
        }

        for folder in &parsed.folders {
            let hashes: Vec<u64> = folder
                .files
                .iter()
                .map(|file| mudcrab::bsa::hash_file_name(&file.name))
                .collect();
            file_names += hashes.len();
            if hashes.windows(2).any(|pair| pair[0] > pair[1]) {
                problems.push(format!(
                    "{}: file hashes in '{}' are not ascending",
                    path.display(),
                    folder.name
                ));
            }
        }
    }

    eprintln!(
        "hashed {folder_names} folder names and {file_names} file names across {} archives",
        paths.len()
    );
    assert!(
        problems.is_empty(),
        "{} ordering problems:\n  {}",
        problems.len(),
        problems.iter().take(20).cloned().collect::<Vec<_>>().join("\n  ")
    );
}
