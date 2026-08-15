use flate2::read::GzDecoder;
use globset::{Glob, GlobSet, GlobSetBuilder};
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
}

impl ArchiveFilters {
    pub fn new(include: &[String], exclude: &[String]) -> anyhow::Result<Self> {
        Ok(Self {
            include: compile_globset(include)?,
            exclude: compile_globset(exclude)?,
        })
    }

    pub fn should_extract(&self, path: &str) -> bool {
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
    {
        return list_system_archive_paths(source);
    }

    anyhow::bail!(
        "unsupported archive format for {} (supports .zip, .tar, .tar.gz, .tgz, .rar, .7z)",
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

fn list_system_archive_paths(source: &Path) -> anyhow::Result<Vec<String>> {
    let src = source
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("non-UTF8 source path: {}", source.display()))?;

    // bsdtar -tf prints one path per line
    if let Ok(output) = Command::new("bsdtar").args(["-t", "-f", src]).output() {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            let mut out = Vec::new();
            for line in text.lines() {
                let p = Path::new(line.trim());
                if line.trim().ends_with('/') {
                    continue; // directory entry
                }
                let normalized = normalize_archive_path(p)?;
                if !normalized.is_empty() {
                    out.push(normalized);
                }
            }
            return Ok(out);
        }
    }

    // Fall back to `7z l -slt` which emits "Path = ..." lines
    let output = Command::new("7z")
        .args(["l", "-slt", src])
        .output()
        .map_err(|err| anyhow::anyhow!("no system tool (bsdtar, 7z) available to list {}: {err}", source.display()))?;

    if !output.status.success() {
        anyhow::bail!("7z failed listing {}", source.display());
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut out = Vec::new();
    let mut current_path: Option<String> = None;
    let mut current_is_dir = false;

    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Path = ") {
            current_path = Some(rest.to_string());
            current_is_dir = false;
        } else if line.starts_with("Attributes = D") || line == "Folder = +" {
            current_is_dir = true;
        } else if line.is_empty() {
            if let Some(path) = current_path.take() {
                if !current_is_dir {
                    let normalized = normalize_archive_path(Path::new(&path))?;
                    if !normalized.is_empty() {
                        out.push(normalized);
                    }
                }
            }
            current_is_dir = false;
        }
    }
    // flush final entry if file didn't end with blank line
    if let Some(path) = current_path {
        if !current_is_dir {
            let normalized = normalize_archive_path(Path::new(&path))?;
            if !normalized.is_empty() {
                out.push(normalized);
            }
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
    }

    fn extract(
        &self,
        source: &Path,
        target_root: &Path,
        filters: &ArchiveFilters,
    ) -> anyhow::Result<usize> {
        let stage = system_staging_dir(source)?;
        let result = system_extract_to(source, &stage)
            .and_then(|()| copy_filtered_staging(&stage, target_root, filters));
        let _ = std::fs::remove_dir_all(&stage);
        result
    }
}

fn probe_magic(source: &Path, magic: &[u8]) -> bool {
    let Ok(mut file) = std::fs::File::open(source) else {
        return false;
    };
    let mut buf = vec![0u8; magic.len()];
    file.read_exact(&mut buf).map(|_| buf == magic).unwrap_or(false)
}

fn system_staging_dir(source: &Path) -> anyhow::Result<PathBuf> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let name = source
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("archive");
    let dir = std::env::temp_dir().join(format!("mudcrab-sys-{name}-{stamp}"));
    std::fs::create_dir_all(&dir)
        .map_err(|err| anyhow::anyhow!("failed to create staging dir {}: {err}", dir.display()))?;
    Ok(dir)
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
    {
        if status.success() {
            return Ok(());
        }
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

fn copy_filtered_staging(
    staging: &Path,
    target: &Path,
    filters: &ArchiveFilters,
) -> anyhow::Result<usize> {
    let mut count = 0usize;
    copy_staging_recursive(staging, staging, target, filters, &mut count)?;
    Ok(count)
}

fn copy_staging_recursive(
    current: &Path,
    root: &Path,
    target: &Path,
    filters: &ArchiveFilters,
    count: &mut usize,
) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(current)
        .map_err(|err| anyhow::anyhow!("failed to read staging dir {}: {err}", current.display()))?
    {
        let entry = entry.map_err(|err| anyhow::anyhow!("failed to read dir entry: {err}"))?;
        let path = entry.path();
        let ft = entry
            .file_type()
            .map_err(|err| anyhow::anyhow!("file type error for {}: {err}", path.display()))?;

        if ft.is_dir() {
            copy_staging_recursive(&path, root, target, filters, count)?;
            continue;
        }
        if !ft.is_file() {
            continue;
        }

        let rel = path
            .strip_prefix(root)
            .map_err(|err| anyhow::anyhow!("path strip error: {err}"))?;
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if !filters.should_extract(&rel_str) {
            continue;
        }

        let dest = target.join(rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", parent.display()))?;
        }
        std::fs::copy(&path, &dest)
            .map_err(|err| anyhow::anyhow!("failed to copy {} to {}: {err}", path.display(), dest.display()))?;
        *count += 1;
    }
    Ok(())
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