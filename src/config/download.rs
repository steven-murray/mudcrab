use crate::config::schema::PersonalizedPlan;
use reqwest::Client;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct DownloadSettings {
    pub cache_dir: PathBuf,
    pub retry: u32,
    pub nexus_api_key: Option<String>,
    pub nexus_api_base: Option<String>,
}

enum DownloadOutcome {
    Cached(PathBuf),
    Downloaded(PathBuf),
}

pub async fn download_all(plan: &PersonalizedPlan, settings: &DownloadSettings) -> anyhow::Result<()> {
    std::fs::create_dir_all(&settings.cache_dir).map_err(|err| {
        anyhow::anyhow!(
            "failed to create cache directory {}: {err}",
            settings.cache_dir.display()
        )
    })?;

    let client = Client::new();
    let mut downloaded = 0usize;

    for mod_entry in &plan.mods {
        for (archive_index, archive) in mod_entry.archives.iter().enumerate() {
            if !archive.build.is_empty() {
                for (layer_index, layer) in archive.build.iter().enumerate() {
                    let destination = settings.cache_dir.join(build_layer_cache_file_name(
                        &mod_entry.id,
                        archive_index,
                        layer_index,
                        &layer.path,
                    ));
                    let outcome = download_with_retry(
                        &client,
                        &layer.path,
                        layer.download_handler.as_deref(),
                        &destination,
                        settings,
                    )
                    .await?;
                    downloaded += 1;
                    log_download_outcome(&mod_entry.id, &layer.path, &outcome);
                }
            } else {
                let path = archive.path.as_deref().unwrap_or_default();
                let destination = settings
                    .cache_dir
                    .join(cache_file_name(&mod_entry.id, archive_index, path));
                let outcome = download_with_retry(
                    &client,
                    path,
                    archive.download_handler.as_deref(),
                    &destination,
                    settings,
                )
                .await?;
                downloaded += 1;
                log_download_outcome(&mod_entry.id, path, &outcome);
            }
        }
    }

    tracing::info!(downloaded, "download phase completed");
    Ok(())
}

async fn download_with_retry(
    client: &Client,
    path: &str,
    handler: Option<&str>,
    destination: &Path,
    settings: &DownloadSettings,
) -> anyhow::Result<DownloadOutcome> {
    if let Some(existing) = resolve_cache_path(
        destination.parent().unwrap_or_else(|| Path::new(".")),
        destination.file_name().and_then(|name| name.to_str()).unwrap_or_default(),
    ) {
        tracing::info!(source = %path, destination = %existing.display(), "archive already cached, skipping download");
        return Ok(DownloadOutcome::Cached(existing));
    }

    let attempts = settings.retry.max(1);

    for attempt in 1..=attempts {
        let result = download_once(client, path, handler, destination, settings).await;
        match result {
            Ok(original_name) => {
                let actual_dest = if let Some(name) = &original_name {
                    append_extension_from_original_name(destination, name)
                } else {
                    destination.to_path_buf()
                };
                if let Some(name) = original_name {
                    let metadata_path = cache_original_name_path_for_destination(&actual_dest);
                    std::fs::write(&metadata_path, name).map_err(|err| {
                        anyhow::anyhow!(
                            "failed to write download filename metadata {}: {err}",
                            metadata_path.display()
                        )
                    })?;
                }
                return Ok(DownloadOutcome::Downloaded(actual_dest));
            }
            Err(err) if attempt < attempts => {
                tracing::warn!(
                    attempt,
                    attempts,
                    source = %path,
                    error = %err,
                    "download failed, retrying"
                );
            }
            Err(err) => {
                return Err(anyhow::anyhow!(
                    "failed to download {} after {} attempt(s): {err}",
                    path,
                    attempts
                ));
            }
        }
    }

    unreachable!("retry loop always returns");
}

async fn download_once(
    client: &Client,
    path: &str,
    handler: Option<&str>,
    destination: &Path,
    settings: &DownloadSettings,
) -> anyhow::Result<Option<String>> {
    let handler = resolve_handler(path, handler);

    match handler {
        "local" => {
            copy_local(path, destination)?;
            Ok(None)
        }
        "http" => download_http(client, path, destination).await,
        "nexus" => download_nexus(client, path, destination, settings).await,
        other => anyhow::bail!("unsupported download handler {other} for source {}", path),
    }
}

fn resolve_handler<'a>(path: &str, handler: Option<&'a str>) -> &'a str {
    if let Some(handler) = handler {
        return handler;
    }

    if path.starts_with("nexus:") || path.contains("nexusmods.com") {
        return "nexus";
    }
    if path.starts_with("http://") || path.starts_with("https://") {
        return "http";
    }

    "local"
}

fn log_download_outcome(mod_id: &str, source: &str, outcome: &DownloadOutcome) {
    match outcome {
        DownloadOutcome::Cached(p) => {
            tracing::info!(
                mod_id,
                source,
                destination = %p.display(),
                "archive ready from cache"
            );
        }
        DownloadOutcome::Downloaded(p) => {
            tracing::info!(
                mod_id,
                source,
                destination = %p.display(),
                "archive downloaded"
            );
        }
    }
}

fn copy_local(source: &str, destination: &Path) -> anyhow::Result<()> {
    let src = Path::new(source);
    if !src.exists() {
        anyhow::bail!("local source {} does not exist", source);
    }

    std::fs::copy(src, destination).map_err(|err| {
        anyhow::anyhow!(
            "failed to copy local source {} to {}: {err}",
            source,
            destination.display()
        )
    })?;

    Ok(())
}

async fn download_http(client: &Client, url: &str, destination: &Path) -> anyhow::Result<Option<String>> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|err| anyhow::anyhow!("http request failed for {url}: {err}"))?
        .error_for_status()
        .map_err(|err| anyhow::anyhow!("http status error for {url}: {err}"))?;

    let original_name = derive_server_filename(&response);

    let bytes = response
        .bytes()
        .await
        .map_err(|err| anyhow::anyhow!("failed to read response body for {url}: {err}"))?;

    std::fs::write(destination, bytes)
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", destination.display()))?;

    Ok(original_name)
}

async fn download_nexus(
    client: &Client,
    source: &str,
    destination: &Path,
    settings: &DownloadSettings,
) -> anyhow::Result<Option<String>> {
    if source.starts_with("http://") || source.starts_with("https://") {
        return download_http(client, source, destination).await;
    }

    let (game, mod_id, file_id) = parse_nexus_spec(source)?;
    let api_key = settings
        .nexus_api_key
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("NEXUS_API_KEY must be set for nexus downloads"))?;

    let api_base = settings
        .nexus_api_base
        .as_deref()
        .unwrap_or("https://api.nexusmods.com/v1");
    let endpoint = format!(
        "{}/games/{}/mods/{}/files/{}/download_link.json",
        api_base.trim_end_matches('/'),
        game,
        mod_id,
        file_id
    );

    let links = client
        .get(&endpoint)
        .header("apikey", api_key)
        .send()
        .await
        .map_err(|err| anyhow::anyhow!("nexus api request failed: {err}"))?
        .error_for_status()
        .map_err(|err| anyhow::anyhow!("nexus api returned error: {err}"))?
        .json::<Vec<NexusDownloadLink>>()
        .await
        .map_err(|err| anyhow::anyhow!("failed to parse nexus api response: {err}"))?;

    let link = links
        .first()
        .and_then(|entry| entry.uri.as_deref())
        .ok_or_else(|| anyhow::anyhow!("nexus api did not return a download URI"))?;

    download_http(client, link, destination).await
}

fn derive_server_filename(response: &reqwest::Response) -> Option<String> {
    if let Some(value) = response
        .headers()
        .get(reqwest::header::CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
    {
        if let Some(name) = filename_from_content_disposition(value) {
            return Some(name);
        }
    }

    response
        .url()
        .path_segments()
        .and_then(|segments| segments.last())
        .and_then(sanitize_filename_component)
}

fn filename_from_content_disposition(value: &str) -> Option<String> {
    for part in value.split(';').map(str::trim) {
        if let Some(raw) = part.strip_prefix("filename*=") {
            let value = raw.split("''").last().unwrap_or(raw);
            if let Some(name) = sanitize_filename_component(value.trim_matches('"')) {
                return Some(name);
            }
        }
        if let Some(raw) = part.strip_prefix("filename=") {
            if let Some(name) = sanitize_filename_component(raw.trim_matches('"')) {
                return Some(name);
            }
        }
    }
    None
}

fn sanitize_filename_component(value: &str) -> Option<String> {
    let basename = value
        .rsplit('/')
        .next()
        .unwrap_or(value)
        .rsplit('\\')
        .next()
        .unwrap_or(value)
        .trim();

    if basename.is_empty() || basename == "." || basename == ".." || basename.contains('\0') {
        return None;
    }

    let sanitized: String = basename
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();

    if sanitized.is_empty() {
        None
    } else {
        Some(sanitized)
    }
}

pub fn cache_original_name_path(cache_dir: &Path, cache_name: &str) -> PathBuf {
    cache_dir.join(format!("{cache_name}.orig-name"))
}

pub fn resolve_cache_path(cache_dir: &Path, cache_name: &str) -> Option<PathBuf> {
    let exact = cache_dir.join(cache_name);
    if exact.exists() {
        return Some(exact);
    }

    let name_lower = cache_name.to_ascii_lowercase();
    let mut extension_match: Option<PathBuf> = None;
    let entries = std::fs::read_dir(cache_dir).ok()?;
    for entry in entries.flatten() {
        let candidate_name = entry.file_name();
        let candidate_lower = candidate_name.to_string_lossy().to_ascii_lowercase();

        if candidate_lower == name_lower {
            return Some(entry.path());
        }

        // Accept "{cache_name}.{ext}" where ext is all alphanumeric (e.g. "7z", "zip", "rar").
        // This matches files that were renamed post-download to include the archive extension,
        // while excluding sidecar files like "…orig-name".
        if extension_match.is_none() {
            if let Some(suffix) = candidate_lower.strip_prefix(&format!("{name_lower}.")) {
                if !suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_alphanumeric()) {
                    extension_match = Some(entry.path());
                }
            }
        }
    }

    extension_match
}

fn cache_original_name_path_for_destination(destination: &Path) -> PathBuf {
    let mut sidecar = destination.as_os_str().to_os_string();
    sidecar.push(".orig-name");
    PathBuf::from(sidecar)
}

/// If the `destination` path doesn't already end with the extension from `original_name`, rename
/// the file on disk to append it and return the new path.  This ensures cached archives are
/// recognisable by external tools (e.g. `.7z`, `.zip`).  When the source URL already encoded the
/// filename (e.g. `https://…/mod.7z`), `destination` will already have the correct extension and
/// no rename is performed.
fn append_extension_from_original_name(destination: &Path, original_name: &str) -> PathBuf {
    let ext = match Path::new(original_name)
        .extension()
        .and_then(|e| e.to_str())
    {
        Some(e) => e.to_ascii_lowercase(),
        None => return destination.to_path_buf(),
    };

    let current_ext = destination
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());

    if current_ext.as_deref() == Some(ext.as_str()) {
        return destination.to_path_buf();
    }

    let new_path = PathBuf::from(format!("{}.{ext}", destination.display()));
    if let Err(err) = std::fs::rename(destination, &new_path) {
        tracing::warn!(
            from = %destination.display(),
            to = %new_path.display(),
            "failed to rename cached archive to include extension: {err}"
        );
        return destination.to_path_buf();
    }
    new_path
}

#[derive(Debug, serde::Deserialize)]
struct NexusDownloadLink {
    #[serde(default, alias = "URI")]
    uri: Option<String>,
}

fn parse_nexus_spec(source: &str) -> anyhow::Result<(&str, &str, &str)> {
    let value = source
        .strip_prefix("nexus:")
        .ok_or_else(|| anyhow::anyhow!("invalid nexus source, expected nexus:<game>/<mod_id>/<file_id>"))?;
    let mut parts = value.split('/');
    let game = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("nexus source missing game segment"))?;
    let mod_id = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("nexus source missing mod id segment"))?;
    let file_id = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("nexus source missing file id segment"))?;

    if parts.next().is_some() {
        anyhow::bail!("nexus source has too many path segments");
    }

    Ok((game, mod_id, file_id))
}

fn safe_filename(mod_id: &str, archive_index: usize, source: &str) -> String {
    let tail = source.rsplit('/').next().unwrap_or("archive.bin");
    let tail = tail.split('?').next().unwrap_or(tail);

    let sanitized: String = tail
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();

    format!("{}_{}_{}", mod_id, archive_index, sanitized)
}

pub fn cache_file_name(mod_id: &str, archive_index: usize, source: &str) -> String {
    safe_filename(mod_id, archive_index, source)
}

pub fn build_layer_cache_file_name(
    mod_id: &str,
    archive_index: usize,
    layer_index: usize,
    source: &str,
) -> String {
    let safe_id: String = mod_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let tail = source.rsplit('/').next().unwrap_or("archive.bin");
    let tail = tail.split('?').next().unwrap_or(tail);
    let safe_tail: String = tail
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{safe_id}_{archive_index}_layer{layer_index}_{safe_tail}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::CompiledArchive;
    use crate::config::schema::{PersonalizedMod, PersonalizedPlan};
    use tempfile::tempdir;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn parses_nexus_spec() {
        let (game, mod_id, file_id) = parse_nexus_spec("nexus:skyrimspecialedition/1234/5678")
            .expect("source should parse");

        assert_eq!(game, "skyrimspecialedition");
        assert_eq!(mod_id, "1234");
        assert_eq!(file_id, "5678");
    }

    #[test]
    fn resolves_cache_file_case_insensitively() {
        let temp = tempdir().expect("temp dir should be created");
        let existing = temp
            .path()
            .join("Thieves Den Barter for Upgrades_0_1000018326");
        std::fs::write(&existing, b"payload").expect("write cache file");

        let resolved = resolve_cache_path(
            temp.path(),
            "Thieves Den Barter For Upgrades_0_1000018326",
        )
        .expect("cache file should resolve");

        assert_eq!(resolved, existing);
    }

    #[test]
    fn resolves_nexus_handler_from_source_shape() {
        let explicit = CompiledArchive {
            path: Some("https://www.nexusmods.com/skyrimspecialedition/mods/1".to_string()),
            download_handler: None,
            layout: None,
            data_folder: None,
            target_subdir: None,
            bain_subpackages: vec![],
            fomod_selections: vec![],
            build: vec![],
            include: vec![],
            exclude: vec![],
            game_root_files: vec![],
        };
        assert_eq!(resolve_handler(explicit.path.as_deref().unwrap_or_default(), explicit.download_handler.as_deref()), "nexus");

        let scheme = CompiledArchive {
            path: Some("nexus:skyrimspecialedition/1/2".to_string()),
            download_handler: None,
            layout: None,
            data_folder: None,
            target_subdir: None,
            bain_subpackages: vec![],
            fomod_selections: vec![],
            build: vec![],
            include: vec![],
            exclude: vec![],
            game_root_files: vec![],
        };
        assert_eq!(resolve_handler(scheme.path.as_deref().unwrap_or_default(), scheme.download_handler.as_deref()), "nexus");
    }

    #[tokio::test]
    async fn nexus_download_uses_api_link_response_uri_alias() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path(
                "/v1/games/skyrimspecialedition/mods/1234/files/5678/download_link.json",
            ))
            .and(header("apikey", "test-key"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                format!(r#"[{{"URI":"{}/files/archive.bin"}}]"#, server.uri()),
                "application/json",
            ))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/files/archive.bin"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"nexus-payload".to_vec()))
            .mount(&server)
            .await;

        let temp = tempdir().expect("temp dir should be created");
        let settings = DownloadSettings {
            cache_dir: temp.path().to_path_buf(),
            retry: 1,
            nexus_api_key: Some("test-key".to_string()),
            nexus_api_base: Some(format!("{}/v1", server.uri())),
        };

        let plan = PersonalizedPlan {
            schema_version: 1,
            name: "Nexus Test".to_string(),
            responses: std::collections::HashMap::new(),
            mod_order: vec!["core".to_string()],
            selected_mods: vec!["core".to_string()],
            actions: toml::Table::new(),
            post_install_actions: vec![],
            mo2_modlist_entries: vec![],
            mods: vec![PersonalizedMod {
                id: "core".to_string(),
                mod_type: None,
                archives: vec![CompiledArchive {
                    path: Some("nexus:skyrimspecialedition/1234/5678".to_string()),
                    download_handler: Some("nexus".to_string()),
                    layout: None,
                    data_folder: None,
                    target_subdir: None,
                    bain_subpackages: vec![],
                    fomod_selections: vec![],
                    build: vec![],
                    include: vec![],
                    exclude: vec![],
                    game_root_files: vec![],
                }],
                files: vec![],
                actions: vec![],
            }],
            plugins: vec![],
        };

        download_all(&plan, &settings)
            .await
            .expect("download should succeed");

        let files: Vec<_> = std::fs::read_dir(temp.path())
            .expect("cache dir should exist")
            .filter_map(Result::ok)
            .collect();
        let archive = files
            .iter()
            .find(|entry| entry.path().extension().and_then(|ext| ext.to_str()) != Some("orig-name"))
            .expect("archive payload should exist");
        let metadata = files
            .iter()
            .find(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("orig-name"))
            .expect("archive metadata sidecar should exist");

        let payload = std::fs::read(archive.path()).expect("downloaded file should be readable");
        assert_eq!(payload, b"nexus-payload");

        let original_name = std::fs::read_to_string(metadata.path())
            .expect("original filename metadata should be readable");
        assert_eq!(original_name, "archive.bin");
    }

    #[tokio::test]
    async fn nexus_descriptor_requires_api_key() {
        let temp = tempdir().expect("temp dir should be created");
        let settings = DownloadSettings {
            cache_dir: temp.path().to_path_buf(),
            retry: 1,
            nexus_api_key: None,
            nexus_api_base: None,
        };

        let plan = PersonalizedPlan {
            schema_version: 1,
            name: "Nexus Missing Key".to_string(),
            responses: std::collections::HashMap::new(),
            mod_order: vec!["core".to_string()],
            selected_mods: vec!["core".to_string()],
            actions: toml::Table::new(),
            post_install_actions: vec![],
            mo2_modlist_entries: vec![],
            mods: vec![PersonalizedMod {
                id: "core".to_string(),
                mod_type: None,
                archives: vec![CompiledArchive {
                    path: Some("nexus:skyrimspecialedition/1234/5678".to_string()),
                    download_handler: Some("nexus".to_string()),
                    layout: None,
                    data_folder: None,
                    target_subdir: None,
                    bain_subpackages: vec![],
                    fomod_selections: vec![],
                    build: vec![],
                    include: vec![],
                    exclude: vec![],
                    game_root_files: vec![],
                }],
                files: vec![],
                actions: vec![],
            }],
            plugins: vec![],
        };

        let err = download_all(&plan, &settings)
            .await
            .expect_err("missing key should fail");
        assert!(err.to_string().contains("NEXUS_API_KEY"));
    }
}
