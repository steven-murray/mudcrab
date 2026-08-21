use flate2::read::GzDecoder;
use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};
use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use tar::Archive as TarArchive;
use zip::ZipArchive;

pub trait ArchiveExtractor {
    fn name(&self) -> &'static str;
    fn can_handle(&self, source: &Path) -> bool;
    fn extract(
        &self,
        source: &Path,
        target_root: &Path,
        filters: &ArchiveFilters,
    ) -> anyhow::Result<usize>;
}

pub fn extract_with_builtins(
    source: &Path,
    target_root: &Path,
    filters: &ArchiveFilters,
) -> anyhow::Result<usize> {
    let extractors: [&dyn ArchiveExtractor; 4] = [
        &ZipExtractor,
        &TarGzExtractor,
        &TarExtractor,
        &SystemArchiveExtractor,
    ];

    for extractor in extractors {
        if extractor.can_handle(source) {
            tracing::debug!(format = extractor.name(), source = %source.display(), "using archive extractor");
            return extractor.extract(source, target_root, filters);
        }
    }

    anyhow::bail!(
        "unsupported archive format for {} (supports .zip, .tar, .tar.gz, .tgz, .rar, .7z)",
        source.display()
    )
}

pub struct ArchiveFilters {
    include: Option<GlobSet>,
    exclude: Option<GlobSet>,
    /// Exact entry names to keep, when the caller already knows precisely which
    /// entries it wants. Not expressible as globs: Oblivion file names contain
    /// brackets (`Harvest [Flora] - DLCVileLair.esp`), which any pattern match
    /// reads as a character class.
    wanted: Option<BTreeSet<String>>,
}

impl ArchiveFilters {
    pub fn new(include: &[String], exclude: &[String]) -> anyhow::Result<Self> {
        Ok(Self {
            include: compile_globset(include)?,
            exclude: compile_globset(exclude)?,
            wanted: None,
        })
    }

    /// Keep exactly these entries, named as `list_archive_paths` reports them.
    pub fn for_entries(wanted: &BTreeSet<String>) -> Self {
        Self {
            include: None,
            exclude: None,
            wanted: Some(wanted.clone()),
        }
    }

    /// Filters for matching against a staged tree rather than an archive.
    ///
    /// Two differences from `new`, both because the patterns come from a guide
    /// describing folders on disk rather than from archive-entry names:
    ///
    /// * `/` is a real separator, so `textures/rocks/*.dds` means the files
    ///   directly in that folder. Under globset's default a bare `*` also
    ///   matches `/`, so that pattern swallowed `textures/rocks/underwater/`
    ///   whole -- ten files the guide explicitly says to keep. Nothing failed;
    ///   only the Oracle diff noticed.
    /// * matching is case-insensitive, because staged directories are folded to
    ///   lowercase while guides spell folders the way the archive does.
    ///
    /// `**` still crosses separators, which is what `expand_directory_pattern`
    /// relies on to turn a folder name into the folder and all of its contents.
    pub fn new_for_staged_tree(include: &[String], exclude: &[String]) -> anyhow::Result<Self> {
        Ok(Self {
            include: compile_staged_globset(include)?,
            exclude: compile_staged_globset(exclude)?,
            wanted: None,
        })
    }

    pub fn should_extract(&self, path: &str) -> bool {
        if let Some(wanted) = &self.wanted
            && !wanted.contains(path)
        {
            return false;
        }

        let include_match = self.include.as_ref().map(|set| set.is_match(path)).unwrap_or(true);
        if !include_match {
            return false;
        }

        let excluded = self
            .exclude
            .as_ref()
            .map(|set| set.is_match(path))
            .unwrap_or(false);
        !excluded
    }
}

pub fn normalize_archive_path(path: &Path) -> anyhow::Result<String> {
    // On Linux, `\` is a valid filename character and is NOT treated as a path
    // separator by `Path::components()`. Windows-created archives commonly use
    // `\` as the directory separator in entry names, so we normalise to `/`
    // before component-level parsing to avoid extracting everything into a
    // single flat file whose name contains literal backslashes.
    let backslash_normalised;
    let path: &Path = if path.as_os_str().as_encoded_bytes().contains(&b'\\') {
        backslash_normalised = PathBuf::from(path.to_string_lossy().replace('\\', "/"));
        &backslash_normalised
    } else {
        path
    };

    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(seg) => parts.push(seg.to_string_lossy().to_string()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!("unsafe archive path component in {}", path.display());
            }
        }
    }

    Ok(parts.join("/"))
}

struct ZipExtractor;

impl ArchiveExtractor for ZipExtractor {
    fn name(&self) -> &'static str {
        "zip"
    }

    fn can_handle(&self, source: &Path) -> bool {
        lower_name(source).ends_with(".zip") || probe_magic(source, &[0x50, 0x4B, 0x03, 0x04])
    }

    fn extract(
        &self,
        source: &Path,
        target_root: &Path,
        filters: &ArchiveFilters,
    ) -> anyhow::Result<usize> {
        let file = std::fs::File::open(source)
            .map_err(|err| anyhow::anyhow!("failed to open {}: {err}", source.display()))?;
        let mut zip = ZipArchive::new(file)
            .map_err(|err| anyhow::anyhow!("failed to read zip archive {}: {err}", source.display()))?;

        let mut extracted = 0usize;
        for idx in 0..zip.len() {
            let mut entry = zip.by_index(idx).map_err(|err| {
                anyhow::anyhow!("failed to read zip entry {idx} from {}: {err}", source.display())
            })?;

            let Some(entry_path) = entry.enclosed_name().map(|p| p.to_path_buf()) else {
                continue;
            };

            let normalized = normalize_archive_path(&entry_path)?;
            if normalized.is_empty() || !filters.should_extract(&normalized) {
                continue;
            }

            let destination = target_root.join(&normalized);

            if entry.is_dir() {
                std::fs::create_dir_all(&destination).map_err(|err| {
                    anyhow::anyhow!("failed to create {}: {err}", destination.display())
                })?;
                continue;
            }

            if let Some(parent) = destination.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", parent.display()))?;
            }

            let mut out = std::fs::File::create(&destination)
                .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", destination.display()))?;
            std::io::copy(&mut entry, &mut out).map_err(|err| {
                anyhow::anyhow!(
                    "failed to extract zip entry to {}: {err}",
                    destination.display()
                )
            })?;

            extracted += 1;
        }

        Ok(extracted)
    }
}

struct TarGzExtractor;

impl ArchiveExtractor for TarGzExtractor {
    fn name(&self) -> &'static str {
        "tar.gz"
    }

    fn can_handle(&self, source: &Path) -> bool {
        let name = lower_name(source);
        name.ends_with(".tar.gz") || name.ends_with(".tgz") || probe_magic(source, &[0x1f, 0x8b])
    }

    fn extract(
        &self,
        source: &Path,
        target_root: &Path,
        filters: &ArchiveFilters,
    ) -> anyhow::Result<usize> {
        let file = std::fs::File::open(source)
            .map_err(|err| anyhow::anyhow!("failed to open {}: {err}", source.display()))?;
        let gz = GzDecoder::new(file);
        let mut archive = TarArchive::new(gz);
        extract_tar_entries(&mut archive, target_root, filters, source)
    }
}

struct TarExtractor;

impl ArchiveExtractor for TarExtractor {
    fn name(&self) -> &'static str {
        "tar"
    }

    fn can_handle(&self, source: &Path) -> bool {
        lower_name(source).ends_with(".tar")
    }

    fn extract(
        &self,
        source: &Path,
        target_root: &Path,
        filters: &ArchiveFilters,
    ) -> anyhow::Result<usize> {
        let file = std::fs::File::open(source)
            .map_err(|err| anyhow::anyhow!("failed to open {}: {err}", source.display()))?;
        let mut archive = TarArchive::new(file);
        extract_tar_entries(&mut archive, target_root, filters, source)
    }
}

fn extract_tar_entries<R: Read>(
    archive: &mut TarArchive<R>,
    target_root: &Path,
    filters: &ArchiveFilters,
    source: &Path,
) -> anyhow::Result<usize> {
    let mut extracted = 0usize;

    for entry_result in archive.entries().map_err(|err| {
        anyhow::anyhow!("failed to iterate tar entries in {}: {err}", source.display())
    })? {
        let mut entry = entry_result
            .map_err(|err| anyhow::anyhow!("failed reading tar entry in {}: {err}", source.display()))?;

        let entry_path: PathBuf = entry.path().map_err(|err| {
            anyhow::anyhow!("failed to resolve tar entry path in {}: {err}", source.display())
        })?
        .to_path_buf();

        let normalized = normalize_archive_path(&entry_path)?;
        if normalized.is_empty() || !filters.should_extract(&normalized) {
            continue;
        }

        if entry.header().entry_type().is_dir() {
            continue;
        }

        let destination = target_root.join(&normalized);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", parent.display()))?;
        }

        entry.unpack(&destination).map_err(|err| {
            anyhow::anyhow!(
                "failed to unpack tar entry to {}: {err}",
                destination.display()
            )
        })?;

        extracted += 1;
    }

    Ok(extracted)
}

/// Extract only the named entries, keeping their archive paths.
///
/// `wanted` holds entry names exactly as [`list_archive_paths`] reports them.
/// This is what a [`LayoutPlan`](crate::config::install::layout::plan::LayoutPlan)
/// buys: the plan already knows which entries survive, so the other 3798 files
/// in a 3854-file texture pack never have to be written out to be thrown away.
///
/// Falls back to extracting everything when a format cannot do it selectively.
/// The caller copies out of the result by plan either way, so the fallback is
/// slower, never wrong.
pub fn extract_entries(
    source: &Path,
    target_root: &Path,
    wanted: &BTreeSet<String>,
) -> anyhow::Result<usize> {
    if wanted.is_empty() {
        return Ok(0);
    }

    let filters = ArchiveFilters::for_entries(wanted);

    // The native extractors already decide per entry, so restricting them is
    // just a narrower filter -- and it saves the write, which is the cost that
    // matters.
    for extractor in [
        &ZipExtractor as &dyn ArchiveExtractor,
        &TarGzExtractor,
        &TarExtractor,
    ] {
        if extractor.can_handle(source) {
            return extractor.extract(source, target_root, &filters);
        }
    }

    if SystemArchiveExtractor.can_handle(source) {
        match system_extract_entries(source, target_root, wanted) {
            Ok(count) => return Ok(count),
            Err(err) => tracing::warn!(
                source = %source.display(),
                error = %err,
                "selective extraction failed; falling back to extracting the whole archive"
            ),
        }
        return SystemArchiveExtractor.extract(source, target_root, &filters);
    }

    anyhow::bail!(
        "unsupported archive format for {} (supports .zip, .tar, .tar.gz, .tgz, .rar, .7z)",
        source.display()
    )
}

/// Ask 7z for a named subset, straight into `target_root`.
///
/// 7z only. `bsdtar` reads its `-T` member list as fnmatch patterns, so
/// `Harvest [Flora] - DLCVileLair.esp` becomes a character class and matches
/// nothing -- and it reports that as "not found in archive", which is one
/// diagnostic away from looking like a corrupt download. `-spd` turns 7z's own
/// wildcard handling off for the same reason.
///
/// Solid archives still decompress in full internally; what is saved is the
/// writing out, which for this list means 93 MB instead of 8.8 GB.
fn system_extract_entries(
    source: &Path,
    target_root: &Path,
    wanted: &BTreeSet<String>,
) -> anyhow::Result<usize> {
    let src = source
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("non-UTF8 source path: {}", source.display()))?;
    let dst = target_root
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("non-UTF8 target path: {}", target_root.display()))?;

    // A member list runs to thousands of paths, well past what a command line
    // will carry, so it goes in a file. Its own directory, so a half-written
    // list can never be mistaken for archive content.
    let list_dir = crate::util::fs::system_staging_dir_for(source, target_root)?;
    std::fs::create_dir_all(&list_dir)
        .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", list_dir.display()))?;
    let list_path = list_dir.join("entries.txt");

    let result = (|| -> anyhow::Result<usize> {
        let mut listing = wanted.iter().cloned().collect::<Vec<_>>().join("\n");
        listing.push('\n');
        std::fs::write(&list_path, listing)
            .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", list_path.display()))?;
        let list_arg = format!(
            "@{}",
            list_path
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("non-UTF8 list path: {}", list_path.display()))?
        );

        let status = Command::new("7z")
            .args([
                "x", src, &format!("-o{dst}"), "-y", "-spd", "-scsUTF-8", &list_arg,
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_err(|err| {
                anyhow::anyhow!("7z is not available to extract {}: {err}", source.display())
            })?;
        if !status.success() {
            anyhow::bail!("7z failed (exit {status}) extracting {}", source.display());
        }

        // Confirm the subset really arrived. An entry the tool spelled
        // differently to the listing -- a separator, an encoding -- would
        // otherwise be a file missing from the mod rather than a failure, and
        // the caller can still get it right by extracting everything.
        let missing = wanted
            .iter()
            .filter(|entry| !target_root.join(entry).exists())
            .count();
        if missing > 0 {
            anyhow::bail!(
                "7z extracted {} of {} requested entries from {}",
                wanted.len() - missing,
                wanted.len(),
                source.display()
            );
        }

        Ok(wanted.len())
    })();

    let _ = std::fs::remove_dir_all(&list_dir);
    result
}

// ── Archive entry listing (no extraction) ────────────────────────────────────

/// Return all file paths inside `source` without extracting anything.
/// Paths are normalised to forward-slash relative strings (same as extraction).
pub fn list_archive_paths(source: &Path) -> anyhow::Result<Vec<String>> {
    let name = lower_name(source);

    // ZIP — use zip crate's entry iterator
    if name.ends_with(".zip") || probe_magic(source, &[0x50, 0x4B, 0x03, 0x04]) {
        let file = std::fs::File::open(source)
            .map_err(|err| anyhow::anyhow!("failed to open {}: {err}", source.display()))?;
        let mut zip = ZipArchive::new(file)
            .map_err(|err| anyhow::anyhow!("failed to read zip archive {}: {err}", source.display()))?;
        let mut out = Vec::with_capacity(zip.len());
        for idx in 0..zip.len() {
            let entry = zip.by_index(idx).map_err(|err| {
                anyhow::anyhow!("failed to read zip entry {idx} from {}: {err}", source.display())
            })?;
            if entry.is_dir() {
                continue;
            }
            let Some(entry_path) = entry.enclosed_name().map(|p| p.to_path_buf()) else {
                continue;
            };
            let normalized = normalize_archive_path(&entry_path)?;
            if !normalized.is_empty() {
                out.push(normalized);
            }
        }
        return Ok(out);
    }

    // .tar.gz / .tgz — read header only (no decompression of file bodies)
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") || probe_magic(source, &[0x1f, 0x8b]) {
        let file = std::fs::File::open(source)
            .map_err(|err| anyhow::anyhow!("failed to open {}: {err}", source.display()))?;
        let gz = GzDecoder::new(file);
        return list_tar_paths(TarArchive::new(gz), source);
    }

    // .tar — headers only
    if name.ends_with(".tar") {
        let file = std::fs::File::open(source)
            .map_err(|err| anyhow::anyhow!("failed to open {}: {err}", source.display()))?;
        return list_tar_paths(TarArchive::new(file), source);
    }

    // .rar / .7z — use system tool to list
    if name.ends_with(".rar")
        || name.ends_with(".7z")
        || probe_magic(source, &[0x52, 0x61, 0x72, 0x21])
        || probe_magic(source, &[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C])
        || is_self_extracting(source)
    {
        return list_system_archive_paths(source);
    }

    anyhow::bail!(
        "unsupported archive format for {} (supports .zip, .tar, .tar.gz, .tgz, .rar, .7z, \
         and self-extracting .exe installers)",
        source.display()
    )
}

fn list_tar_paths<R: Read>(mut archive: TarArchive<R>, source: &Path) -> anyhow::Result<Vec<String>> {
    let mut out = Vec::new();
    for entry_result in archive.entries().map_err(|err| {
        anyhow::anyhow!("failed to iterate tar entries in {}: {err}", source.display())
    })? {
        let entry = entry_result
            .map_err(|err| anyhow::anyhow!("failed to read tar entry in {}: {err}", source.display()))?;
        if entry.header().entry_type().is_dir() {
            continue;
        }
        let entry_path: PathBuf = entry.path()
            .map_err(|err| anyhow::anyhow!("failed to read tar entry path in {}: {err}", source.display()))?
            .to_path_buf();
        let normalized = normalize_archive_path(&entry_path)?;
        if !normalized.is_empty() {
            out.push(normalized);
        }
    }
    Ok(out)
}

/// List a rar or 7z with a system tool.
///
/// `7z l -slt` first, because it marks directories explicitly. `bsdtar -tf`
/// cannot: it prints one path per line and the only clue is a trailing slash,
/// which plenty of archives do not write -- `Arena Poster_0_44088.rar` stores
/// `textures`, `textures/architecture` and `textures/architecture/imperialcity`
/// as bare paths. That cost nothing while installs walked the extracted tree,
/// and became a directory in a file list the moment they stopped.
fn list_system_archive_paths(source: &Path) -> anyhow::Result<Vec<String>> {
    let src = source
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("non-UTF8 source path: {}", source.display()))?;

    if let Ok(output) = Command::new("7z").args(["l", "-slt", src]).output()
        && output.status.success()
    {
        let listed = parse_7z_listing(&String::from_utf8_lossy(&output.stdout))?;
        // Belt and braces. Both listing bugs found so far were a directory
        // presented as a file, and both were silent until an install tried to
        // copy one -- so the structural check runs even where the tool is
        // supposed to have said.
        return Ok(drop_directory_entries(listed));
    }

    let output = Command::new("bsdtar")
        .args(["-t", "-f", src])
        .output()
        .map_err(|err| {
            anyhow::anyhow!(
                "no system tool (7z, bsdtar) available to list {}: {err}",
                source.display()
            )
        })?;
    if !output.status.success() {
        anyhow::bail!("bsdtar failed listing {}", source.display());
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.ends_with('/') {
            continue;
        }
        let normalized = normalize_archive_path(Path::new(trimmed))?;
        if !normalized.is_empty() {
            out.push(normalized);
        }
    }

    Ok(drop_directory_entries(out))
}

/// Drop entries that other entries sit inside, which only a directory can be.
///
/// The salvage for a lister that cannot say. It cannot see an *empty*
/// directory written without a trailing slash, so it is the fallback rather
/// than the rule.
fn drop_directory_entries(paths: Vec<String>) -> Vec<String> {
    let prefixes: BTreeSet<&str> = paths
        .iter()
        .flat_map(|path| {
            path.match_indices('/')
                .map(|(idx, _)| &path[..idx])
                .collect::<Vec<_>>()
        })
        .collect();

    paths
        .iter()
        .filter(|path| !prefixes.contains(path.as_str()))
        .cloned()
        .collect()
}

/// Parse `7z l -slt`: a header block, a `----------` rule, then one blank-line
/// separated block per entry.
///
/// The header carries a `Path =` of its own -- the archive's absolute path --
/// so collecting before the rule yields the archive as its own first entry.
/// Whether a 7z attribute field marks a directory.
///
/// The field is Windows attribute letters -- `A`, `RD`, `HSA` -- optionally
/// followed by a unix mode after whitespace. `D` is the directory flag and can
/// sit anywhere in the letters: `OOO Enhanced - Resources` writes its folders
/// `RD`, which a `starts_with("D")` test reads as a file. That archive also
/// omits the `Folder` line entirely, so this is the only thing that says.
fn is_directory_attribute(field: &str) -> bool {
    field
        .split_whitespace()
        .next()
        .is_some_and(|flags| flags.contains('D'))
}

fn parse_7z_listing(text: &str) -> anyhow::Result<Vec<String>> {
    let mut out = Vec::new();
    let mut current_path: Option<String> = None;
    let mut current_is_dir = false;
    let mut in_entries = false;

    for line in text.lines() {
        let line = line.trim();
        if !in_entries {
            in_entries = line.starts_with("----------");
            continue;
        }
        if let Some(rest) = line.strip_prefix("Path = ") {
            current_path = Some(rest.to_string());
            current_is_dir = false;
        } else if let Some(rest) = line.strip_prefix("Attributes = ") {
            current_is_dir |= is_directory_attribute(rest);
        } else if line == "Folder = +" {
            current_is_dir = true;
        } else if line.is_empty() {
            if let Some(path) = current_path.take()
                && !current_is_dir
            {
                let normalized = normalize_archive_path(Path::new(&path))?;
                if !normalized.is_empty() {
                    out.push(normalized);
                }
            }
            current_is_dir = false;
        }
    }
    // Flush a final entry when the output does not end in a blank line.
    if let Some(path) = current_path
        && !current_is_dir
    {
        let normalized = normalize_archive_path(Path::new(&path))?;
        if !normalized.is_empty() {
            out.push(normalized);
        }
    }

    Ok(out)
}

// ── System extractor (rar, 7z, and any format not handled natively) ─────────

struct SystemArchiveExtractor;

impl ArchiveExtractor for SystemArchiveExtractor {
    fn name(&self) -> &'static str {
        "system"
    }

    fn can_handle(&self, source: &Path) -> bool {
        let name = lower_name(source);
        name.ends_with(".rar")
            || name.ends_with(".7z")
            || probe_magic(source, &[0x52, 0x61, 0x72, 0x21]) // Rar!
            || probe_magic(source, &[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]) // 7z
            || is_self_extracting(source)
    }

    fn extract(
        &self,
        source: &Path,
        target_root: &Path,
        filters: &ArchiveFilters,
    ) -> anyhow::Result<usize> {
        let stage = crate::util::fs::system_staging_dir_for(source, target_root)?;
        let result = system_extract_to(source, &stage)
            .and_then(|()| crate::util::fs::copy_filtered_tree(&stage, target_root, filters));
        let _ = std::fs::remove_dir_all(&stage);
        result
    }
}

/// A self-extracting installer: a Windows executable with an archive stapled on.
///
/// Standard for mods old enough to predate the mod managers -- Bank of Cyrodiil
/// (2006) is distributed as a zip holding one, and expects the user to run it.
/// Nothing has to be run: both `bsdtar` and `7z` open these directly, scanning
/// past the executable for the archive signature. Recognising the file is the
/// whole of the support needed.
///
/// The `MZ` check is what keeps this cheap. It is the DOS header every PE
/// carries, so a single 2-byte read rejects everything that is not a Windows
/// executable before any subprocess starts; only a real `.exe` costs a `7z l`.
/// Asking 7z rather than trusting the extension is deliberate -- most `.exe`
/// files are not archives, and claiming them here would turn "this is not an
/// installer" into an extraction failure much further down.
fn is_self_extracting(source: &Path) -> bool {
    if !probe_magic(source, &[0x4D, 0x5A]) {
        return false;
    }
    let Some(src) = source.to_str() else {
        return false;
    };

    Command::new("7z")
        .args(["l", "-slt", src])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn probe_magic(source: &Path, magic: &[u8]) -> bool {
    let Ok(mut file) = std::fs::File::open(source) else {
        return false;
    };
    let mut buf = vec![0u8; magic.len()];
    file.read_exact(&mut buf).map(|_| buf == magic).unwrap_or(false)
}


fn system_extract_to(source: &Path, staging: &Path) -> anyhow::Result<()> {
    let src = source
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("non-UTF8 source path: {}", source.display()))?;
    let dst = staging
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("non-UTF8 staging path: {}", staging.display()))?;

    // Try bsdtar first (handles rar, 7z, zip, tar via libarchive)
    if let Ok(status) = Command::new("bsdtar")
        .args(["-x", "-f", src, "-C", dst])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        && status.success()
    {
        return Ok(());
    }

    // Fall back to 7z
    let status = Command::new("7z")
        .args(["x", src, &format!("-o{dst}"), "-y"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|err| {
            anyhow::anyhow!(
                "no supported system tool (bsdtar, 7z) available to extract {}: {err}",
                source.display()
            )
        })?;

    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("7z failed (exit {status}) extracting {}", source.display())
    }
}

fn compile_globset(patterns: &[String]) -> anyhow::Result<Option<GlobSet>> {
    if patterns.is_empty() {
        return Ok(None);
    }

    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = Glob::new(pattern)
            .map_err(|err| anyhow::anyhow!("invalid glob pattern '{}': {err}", pattern))?;
        builder.add(glob);
    }

    Ok(Some(
        builder
            .build()
            .map_err(|err| anyhow::anyhow!("failed to build globset: {err}"))?,
    ))
}

fn compile_staged_globset(patterns: &[String]) -> anyhow::Result<Option<GlobSet>> {
    if patterns.is_empty() {
        return Ok(None);
    }

    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = GlobBuilder::new(pattern)
            .literal_separator(true)
            .case_insensitive(true)
            .build()
            .map_err(|err| anyhow::anyhow!("invalid glob pattern '{}': {err}", pattern))?;
        builder.add(glob);
    }

    Ok(Some(
        builder
            .build()
            .map_err(|err| anyhow::anyhow!("failed to build globset: {err}"))?,
    ))
}

fn lower_name(source: &Path) -> String {
    source
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_include_and_exclude_patterns() {
        let filters = ArchiveFilters::new(&["Data/**".to_string()], &["Data/*.tmp".to_string()])
            .expect("globset should compile");

        assert!(filters.should_extract("Data/test.txt"));
        assert!(!filters.should_extract("Docs/readme.txt"));
        assert!(!filters.should_extract("Data/skip.tmp"));
    }
}
#[cfg(test)]
mod listing_tests {
    use super::*;

    #[test]
    fn the_seven_zip_header_is_not_an_entry() {
        // The header block carries a `Path =` of its own. Collecting from the
        // top of the output makes the archive its own first entry -- and since
        // that path is absolute, `normalize_archive_path` rejects it and the
        // whole listing fails.
        let text = "\
Listing archive: /downloads/Mod.7z

--
Path = /downloads/Mod.7z
Type = 7z
Solid = +

----------
Path = textures/a.dds
Folder = -
Attributes = A

Path = textures
Folder = +
Attributes = D
";
        assert_eq!(parse_7z_listing(text).expect("parse"), ["textures/a.dds"]);
    }

    #[test]
    fn a_read_only_directory_is_still_a_directory() {
        // `OOO Enhanced - Resources` writes `Attributes = RD` and no `Folder`
        // line at all, so the attribute letters are the only signal.
        assert!(is_directory_attribute("D"));
        assert!(is_directory_attribute("RD"));
        assert!(is_directory_attribute("RD_ drwxr-xr-x"));
        assert!(!is_directory_attribute("A"));
        assert!(!is_directory_attribute("A_ -rw-r--r--"));

        let text = "\
----------
Path = meshes/kdLucas
Size = 0
Attributes = RD

Path = meshes/kdLucas/thing.nif
Attributes = A
";
        assert_eq!(
            parse_7z_listing(text).expect("parse"),
            ["meshes/kdLucas/thing.nif"]
        );
    }

    #[test]
    fn a_directory_written_without_a_trailing_slash_is_still_a_directory() {
        // `Arena Poster_0_44088.rar` writes its directories bare, which bsdtar
        // reports indistinguishably from files.
        let listed = vec![
            "textures/architecture/imperialcity/poster.dds".to_string(),
            "textures/architecture/imperialcity".to_string(),
            "textures/architecture".to_string(),
            "textures".to_string(),
        ];
        assert_eq!(
            drop_directory_entries(listed),
            ["textures/architecture/imperialcity/poster.dds"]
        );
    }

    /// A self-extracting installer is a Windows executable with an archive
    /// after it, which is what this fixture is: an `MZ` header, some filler,
    /// then a real 7z stream. `Bank of Cyrodiil 1-11.exe` (2006) is the shape
    /// this stands in for.
    #[test]
    fn a_self_extracting_installer_is_recognised_as_an_archive() {
        let dir = tempfile::tempdir().expect("temp dir");
        let inner = dir.path().join("inner.7z");
        let payload = dir.path().join("a.txt");
        std::fs::write(&payload, b"hi").expect("payload");
        let made = Command::new("7z")
            .args([
                "a",
                "-t7z",
                inner.to_str().unwrap(),
                payload.to_str().unwrap(),
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        if !made.map(|status| status.success()).unwrap_or(false) {
            eprintln!("skipping: no usable 7z on this machine");
            return;
        }

        let mut bytes = b"MZ".to_vec();
        bytes.extend(std::iter::repeat_n(0u8, 512));
        bytes.extend(std::fs::read(&inner).expect("inner archive"));
        let sfx = dir.path().join("installer.exe");
        std::fs::write(&sfx, &bytes).expect("sfx fixture");

        assert!(is_self_extracting(&sfx));
        assert!(SystemArchiveExtractor.can_handle(&sfx));
        assert_eq!(list_archive_paths(&sfx).expect("listable"), ["a.txt"]);
    }

    /// Most `.exe` files are not archives, and claiming them here would turn a
    /// wrong `inner_archive` into a confusing extraction failure much later.
    #[test]
    fn an_executable_that_is_not_an_archive_is_not_claimed() {
        let dir = tempfile::tempdir().expect("temp dir");
        let plain = dir.path().join("plain.exe");
        let mut bytes = b"MZ".to_vec();
        bytes.extend((0u16..1024).map(|value| value as u8));
        std::fs::write(&plain, &bytes).expect("fixture");

        assert!(!is_self_extracting(&plain));
        assert!(!SystemArchiveExtractor.can_handle(&plain));
    }

    #[test]
    fn a_file_sharing_a_name_with_no_children_survives() {
        let listed = vec!["readme".to_string(), "meshes/a.nif".to_string()];
        assert_eq!(
            drop_directory_entries(listed),
            ["readme", "meshes/a.nif"],
            "only a path other paths sit inside is a directory"
        );
    }
}
