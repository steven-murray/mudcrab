use crate::config::archive_store::{self, ArchiveRef};
use crate::config::filter::ModFilter;
use crate::config::schema::PersonalizedPlan;
use reqwest::Client;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct DownloadSettings {
    /// The downloads folder: archives under their real names, with MO2
    /// `.meta` files. See `archive_store`.
    pub cache_dir: PathBuf,
    pub retry: u32,
    pub nexus_api_key: Option<String>,
    pub nexus_api_base: Option<String>,
    /// Which mods to fetch. Empty means the whole plan.
    pub filter: ModFilter,
    /// Read-only directories that may already hold the archives, tried in order
    /// before anything goes over the network. See `archive_store::resolve`.
    pub archive_search_paths: Vec<PathBuf>,
    /// How many archives to fetch at once. At least 1.
    pub parallel: usize,
}

#[derive(Debug)]
enum DownloadOutcome {
    /// Already in the downloads folder, or linked in from a search path.
    Cached(PathBuf),
    Downloaded(PathBuf),
}

/// An archive the download pass could not fetch, and why.
#[derive(Debug, Clone)]
pub struct Unfetched {
    pub mod_id: String,
    /// The archive's name, or its source when it has none.
    pub label: String,
    /// `nexus:...`, `manual:...`, a URL or a path.
    pub source: String,
    pub file_name: Option<String>,
    /// Size and MD5, when the list records them.
    pub content: Option<(u64, String)>,
    /// Where and how to get it by hand, when the list says.
    pub page: Option<String>,
    pub note: Option<String>,
    pub error: String,
}

impl Unfetched {
    /// The error, then the list's own word on getting the archive by hand.
    pub fn describe(&self) -> String {
        match how_to_get(self.page.as_deref(), self.note.as_deref()) {
            Some(how) => format!("{}\n{how}", self.error),
            None => self.error.clone(),
        }
    }
}

/// What the list says about fetching an archive by hand, indented to sit under
/// the message about it; `None` when it says nothing.
pub(crate) fn how_to_get(page: Option<&str>, note: Option<&str>) -> Option<String> {
    let mut lines = Vec::new();
    if let Some(page) = page {
        lines.push(format!("    get it from: {page}"));
    }
    if let Some(note) = note {
        lines.push(format!("    {}", note.trim()));
    }
    (!lines.is_empty()).then(|| lines.join("\n"))
}

/// Fetch every archive the plan needs; fail, naming each, if any could not be.
pub async fn download_all(
    plan: &PersonalizedPlan,
    settings: &DownloadSettings,
) -> anyhow::Result<()> {
    let unfetched = download_report(plan, settings).await?;
    if !unfetched.is_empty() {
        let items: Vec<crate::events::Item> = unfetched
            .iter()
            .map(|u| {
                crate::events::Item::new(
                    crate::events::Text::new()
                        .plain(format!("{} (", u.mod_id))
                        .name(&u.label)
                        .plain(format!("): {}", u.describe())),
                )
                .about(&u.mod_id)
            })
            .collect();
        for item in &items {
            tracing::info!(item = %item.text, "not downloaded");
        }
        crate::events::section(crate::events::Tone::Problem, "Not downloaded", items);
        anyhow::bail!(
            "{} archive(s) could not be downloaded, listed above",
            unfetched.len()
        );
    }
    Ok(())
}

/// Fetch every archive the plan needs, and return the ones that could not be,
/// for a caller that has another way to get them (`download --browser`).
pub async fn download_report(
    plan: &PersonalizedPlan,
    settings: &DownloadSettings,
) -> anyhow::Result<Vec<Unfetched>> {
    std::fs::create_dir_all(&settings.cache_dir).map_err(|err| {
        anyhow::anyhow!(
            "failed to create cache directory {}: {err}",
            settings.cache_dir.display()
        )
    })?;

    let client = Client::new();
    let mut downloaded = 0usize;
    let mut skipped = 0usize;
    // Accumulated rather than propagated, exactly as `check` does. A section of
    // a 700-mod list routinely contains more than one dead link, and stopping
    // at the first one turns finding them into one round trip each.
    let mut errors: Vec<Unfetched> = Vec::new();

    // Collected first, then fetched `settings.parallel` at a time. Two jobs for
    // one archive (a file several mods use) are run once: the second would
    // only race the first to the same name.
    let mut jobs: Vec<Job<'_>> = Vec::new();
    for mod_entry in &plan.mods {
        if !settings.filter.matches(&mod_entry.section, &mod_entry.id) {
            skipped += 1;
            continue;
        }

        for archive in &mod_entry.archives {
            if !archive.build.is_empty() {
                for (layer_index, layer) in archive.build.iter().enumerate() {
                    // Build layers carry no `file_name`, so there is nothing to
                    // match a local copy against; they always take the network path.
                    jobs.push(Job {
                        mod_id: &mod_entry.id,
                        label: format!("build layer {} ({})", layer_index + 1, layer.path),
                        path: &layer.path,
                        file_name: layer.file_name.as_deref(),
                        content: None,
                        page: None,
                        note: None,
                        handler: layer.download_handler.as_deref(),
                    });
                }
            } else {
                let path = archive.path.as_deref().unwrap_or_default();
                jobs.push(Job {
                    mod_id: &mod_entry.id,
                    label: archive.file_name.as_deref().unwrap_or(path).to_string(),
                    path,
                    file_name: archive.file_name.as_deref(),
                    content: archive.size.zip(archive.md5.as_deref()),
                    page: archive.download_page.as_deref(),
                    note: archive.download_note.as_deref(),
                    handler: archive.download_handler.as_deref(),
                });
            }
        }
    }

    let mut seen = std::collections::HashSet::new();
    jobs.retain(|job| seen.insert((job.path, job.file_name)));

    use futures_util::stream::{self, StreamExt};
    let bar = crate::events::progress(jobs.len() as u64, "archives");
    let results: Vec<_> = stream::iter(jobs)
        .map(|job| {
            let client = &client;
            let bar = bar.clone();
            async move {
                let result = download_with_retry(
                    client,
                    job.path,
                    job.file_name,
                    job.content,
                    job.handler,
                    settings,
                )
                .await;
                bar.inc(1);
                bar.set_message(job.mod_id);
                (job, result)
            }
        })
        .buffer_unordered(settings.parallel.max(1))
        .collect()
        .await;

    bar.finish();
    let mut fetched = 0usize;
    for (job, result) in results {
        match result {
            Ok(outcome) => {
                downloaded += 1;
                if matches!(outcome, DownloadOutcome::Downloaded(_)) {
                    fetched += 1;
                }
                log_download_outcome(job.mod_id, job.path, &outcome);
            }
            Err(err) => errors.push(Unfetched {
                mod_id: job.mod_id.to_string(),
                label: job.label.clone(),
                source: job.path.to_string(),
                file_name: job.file_name.map(str::to_string),
                content: job.content.map(|(size, md5)| (size, md5.to_string())),
                page: job.page.map(str::to_string),
                note: job.note.map(str::to_string),
                error: err.to_string(),
            }),
        }
    }
    // Completion order is arbitrary with more than one in flight; the report
    // should not be.
    errors.sort_by(|a, b| (&a.mod_id, &a.label).cmp(&(&b.mod_id, &b.label)));

    crate::events::status(
        if errors.is_empty() {
            crate::events::Tone::Good
        } else {
            crate::events::Tone::Warning
        },
        crate::events::Text::new()
            .plain(format!("{downloaded} archive(s) ready in "))
            .dim(settings.cache_dir.display().to_string())
            .plain(format!(" ({fetched} downloaded now)"))
            .plain(if errors.is_empty() {
                String::new()
            } else {
                format!("; {} not downloaded", errors.len())
            }),
    );
    // Emitted before the bail, as in `check`: the run that fails is the run
    // whose summary is worth having, because it is the one naming dead links.
    tracing::info!(
        downloaded,
        failed = errors.len(),
        skipped_by_filter = skipped,
        scope = %settings.filter.describe(),
        "download phase completed"
    );

    Ok(errors)
}

/// One archive (or build layer) to fetch.
struct Job<'a> {
    mod_id: &'a str,
    /// Which archive of the mod, for error messages.
    label: String,
    path: &'a str,
    file_name: Option<&'a str>,
    /// Size and MD5, for finding a renamed copy.
    content: Option<(u64, &'a str)>,
    page: Option<&'a str>,
    note: Option<&'a str>,
    handler: Option<&'a str>,
}

/// Resolution order for one archive: the downloads folder, then the read-only
/// search paths, then the network. The first two never touch the network,
/// which is the whole point -- a large modlist is mostly archives the machine
/// already has.
async fn download_with_retry(
    client: &Client,
    path: &str,
    file_name: Option<&str>,
    content: Option<(u64, &str)>,
    handler: Option<&str>,
    settings: &DownloadSettings,
) -> anyhow::Result<DownloadOutcome> {
    let downloads = &settings.cache_dir;
    let mut archive = ArchiveRef::new(path, file_name);
    archive.content = content;

    if let Some(found) =
        archive_store::resolve(downloads, &settings.archive_search_paths, &archive)?
    {
        return Ok(DownloadOutcome::Cached(found));
    }

    // A manual archive is one no automated fetch can reach -- a login-walled
    // forum host, a dead link, a file the author distributes by hand. The step
    // above is the only one that can ever satisfy it, so fail here rather than
    // falling into the retry loop: retrying a fetch that does not exist just
    // delays the same message three times over.
    if resolve_handler(path, handler) == "manual" {
        let wanted = archive.describe();
        let mut searched = vec![downloads.clone()];
        searched.extend(settings.archive_search_paths.iter().cloned());
        let similar = similar_archives(wanted, &searched);
        let hint = if similar.is_empty() {
            String::new()
        } else {
            format!(
                "\nSimilarly named files in the search paths -- if one is this archive \
                 re-uploaded under a new name, set its file_name to match:\n{}",
                similar
                    .iter()
                    .map(|found| format!("  {}", found.display()))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        };
        anyhow::bail!(
            "archive '{}' must be downloaded by hand -- it is not fetchable from any \
             automated source. Put it in the downloads folder ({}) or one of the \
             --archive-search-path directories and re-run. Searched: {}{hint}",
            wanted,
            downloads.display(),
            describe_search_paths(&settings.archive_search_paths),
        );
    }

    std::fs::create_dir_all(downloads)
        .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", downloads.display()))?;
    // Fetched under a scratch name, then renamed to the archive's real name
    // once the server has said what that is.
    let partial = downloads.join(format!(".mudcrab-{}.part", scratch_name(path)));
    let attempts = settings.retry.max(1);

    for attempt in 1..=attempts {
        let result = download_once(client, path, handler, &partial, settings).await;
        match result {
            Ok(server_name) => {
                let mut name = final_name(&archive, server_name.as_deref(), downloads);
                // A name with no archive extension cannot be unpacked by
                // extension; the file's own magic bytes say what it is.
                if !has_archive_extension(&name) {
                    name.push_str(archive_store::detect_archive_ext(&partial));
                }
                let final_path = downloads.join(name);
                std::fs::rename(&partial, &final_path).map_err(|err| {
                    anyhow::anyhow!("failed to move download to {}: {err}", final_path.display())
                })?;
                archive_store::write_meta(&final_path, &archive)?;
                archive_store::note_written(downloads);
                return Ok(DownloadOutcome::Downloaded(final_path));
            }
            // Asking again cannot change "no key" or "not premium".
            Err(err) if err.downcast_ref::<NoRetry>().is_some() => {
                let _ = std::fs::remove_file(&partial);
                return Err(err);
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
                let _ = std::fs::remove_file(&partial);
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

fn has_archive_extension(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [
        ".7z", ".zip", ".rar", ".tar.gz", ".gz", ".tar", ".esp", ".esm", ".bsa",
    ]
    .iter()
    .any(|ext| lower.ends_with(ext))
}

/// A filesystem-safe token for a source, for the scratch file only.
fn scratch_name(source: &str) -> String {
    source
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// What a freshly downloaded archive is called: the declared `file_name`, else
/// what the server called it, else something derived from the source. If that
/// name is already taken -- by a different file, since lookup missed -- a
/// suffix keeps both, as MO2 does.
fn final_name(archive: &ArchiveRef<'_>, server_name: Option<&str>, downloads: &Path) -> String {
    let base = archive
        .file_name
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .or_else(|| server_name.map(str::to_string))
        .or_else(|| archive.expected_name().map(str::to_string))
        .unwrap_or_else(|| scratch_name(archive.source));
    if !downloads.join(&base).exists() {
        return base;
    }
    let (stem, ext) = match base.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() => (stem.to_string(), format!(".{ext}")),
        _ => (base.clone(), String::new()),
    };
    (1..)
        .map(|n| format!("{stem} ({n}){ext}"))
        .find(|candidate| !downloads.join(candidate).exists())
        .expect("an unused name exists")
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
        // Unreachable in the normal flow: download_with_retry bails on manual
        // before the retry loop. Stated anyway so a future caller that reaches
        // download_once directly gets the same answer rather than "unsupported".
        "manual" => anyhow::bail!(
            "archive {} is marked manual and cannot be fetched automatically",
            path
        ),
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
    if path.starts_with("manual:") {
        return "manual";
    }
    if path.starts_with("http://") || path.starts_with("https://") {
        return "http";
    }

    "local"
}

/// The search paths as the user would have to type them, for error messages.
///
/// An empty list is its own diagnosis: the archive is unfetchable *and* nothing
/// was configured to look for it in, which is a different mistake from "looked
/// and did not find it" and should not read the same.
fn describe_search_paths(paths: &[std::path::PathBuf]) -> String {
    if paths.is_empty() {
        return "(no --archive-search-path was given)".to_string();
    }

    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

fn log_download_outcome(mod_id: &str, source: &str, outcome: &DownloadOutcome) {
    match outcome {
        DownloadOutcome::Cached(p) => {
            tracing::info!(
                mod_id,
                source,
                destination = %p.display(),
                "archive ready in the downloads folder"
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

async fn download_http(
    client: &Client,
    url: &str,
    destination: &Path,
) -> anyhow::Result<Option<String>> {
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

/// A failure no retry can fix: the answer will be the same next time.
#[derive(Debug)]
struct NoRetry(String);

impl std::fmt::Display for NoRetry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for NoRetry {}

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
    // Most people will meet one of the two failures below before anything
    // else, so each says what to do instead: fetch the file by hand, which
    // every later stage then finds by name.
    let by_hand = format!(
        "download it by hand from https://www.nexusmods.com/{game}/mods/{mod_id}?tab=files&file_id={file_id} \
         into the downloads folder (or a folder given with --archive-search-path)"
    );
    let api_key = settings.nexus_api_key.as_deref().ok_or_else(|| {
        NoRetry(format!(
            "no Nexus API key (NEXUS_API_KEY, or nexus_api_key in tools.toml), so Nexus \
             cannot be asked for this file; {by_hand}"
        ))
    })?;

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

    let response = client
        .get(&endpoint)
        .header("apikey", api_key)
        .send()
        .await
        .map_err(|err| anyhow::anyhow!("nexus api request failed: {err}"))?;
    match response.status() {
        reqwest::StatusCode::FORBIDDEN => {
            return Err(NoRetry(format!(
                "Nexus gives download links to premium accounts only; {by_hand}"
            ))
            .into());
        }
        reqwest::StatusCode::UNAUTHORIZED => {
            return Err(NoRetry(
                "Nexus refused the API key: check NEXUS_API_KEY or nexus_api_key in \
                 tools.toml (your key is at https://next.nexusmods.com/settings/api-keys)"
                    .to_string(),
            )
            .into());
        }
        _ => {}
    }
    let links = response
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
        && let Some(name) = filename_from_content_disposition(value)
    {
        return Some(name);
    }

    response
        .url()
        .path_segments()
        .and_then(|mut segments| segments.next_back())
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
        if let Some(raw) = part.strip_prefix("filename=")
            && let Some(name) = sanitize_filename_component(raw.trim_matches('"'))
        {
            return Some(name);
        }
    }
    None
}

/// A server-supplied filename, reduced to a bare name. Kept as the server
/// spelled it -- spaces and all, after percent-decoding -- because it becomes
/// the archive's name in a folder MO2 shows the user.
fn sanitize_filename_component(value: &str) -> Option<String> {
    let decoded = percent_decode(value);
    let basename = decoded
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(&decoded)
        .trim();
    if basename.is_empty()
        || basename == "."
        || basename == ".."
        || basename.chars().any(|c| c.is_control())
    {
        return None;
    }
    Some(basename.to_string())
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(byte) = u8::from_str_radix(&value[i + 1..i + 3], 16)
        {
            out.push(byte);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| value.to_string())
}

/// Files in the search paths whose names look like `wanted` with a different
/// version: the usual shape of a host re-uploading an archive under a new name.
///
/// Names are compared on their words, ignoring the extension and any word with
/// a digit in it (`2`, `v2`, `1beta`), since those are what a new version
/// changes. Only the best-scoring candidates are offered. A candidate must
/// share more than half of `wanted`'s words -- half lets a lone "Oblivion"
/// match half a downloads folder -- including one of three letters or more.
/// At most three, closest first.
pub(crate) fn similar_archives(wanted: &str, search_paths: &[PathBuf]) -> Vec<PathBuf> {
    fn words(name: &str) -> std::collections::BTreeSet<String> {
        let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
        stem.split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|word| !word.is_empty() && !word.chars().any(|c| c.is_ascii_digit()))
            .map(str::to_ascii_lowercase)
            .collect()
    }

    let target = words(wanted);
    if target.is_empty() {
        return Vec::new();
    }

    let mut scored: Vec<(usize, PathBuf)> = Vec::new();
    for directory in search_paths {
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            // MO2 and mudcrab sidecars, not archives.
            if name.ends_with(".meta") || name.ends_with(".orig-name") || !entry.path().is_file() {
                continue;
            }
            let shared: Vec<String> = words(&name).intersection(&target).cloned().collect();
            if shared.len() * 2 > target.len() && shared.iter().any(|word| word.len() >= 3) {
                scored.push((shared.len(), entry.path()));
            }
        }
    }
    let best = scored.iter().map(|(score, _)| *score).max().unwrap_or(0);
    scored.retain(|(score, _)| *score == best);
    scored.sort_by(|a, b| a.1.cmp(&b.1));
    scored.into_iter().take(3).map(|(_, path)| path).collect()
}

/// The real filenames of a Nexus mod's files, by file id.
pub async fn nexus_file_names(
    client: &Client,
    api_base: Option<&str>,
    api_key: &str,
    game: &str,
    mod_id: &str,
) -> anyhow::Result<std::collections::HashMap<String, String>> {
    let mod_id: u64 = mod_id
        .parse()
        .map_err(|_| anyhow::anyhow!("nexus mod id {mod_id} is not a number"))?;
    let files =
        crate::config::identify::nexus_files(client, api_base, api_key, game, mod_id).await?;
    Ok(files
        .into_iter()
        .filter_map(|file| Some((file.file_id.to_string(), file.file_name?)))
        .collect())
}

#[derive(Debug, serde::Deserialize)]
struct NexusDownloadLink {
    #[serde(default, alias = "URI")]
    uri: Option<String>,
}

fn parse_nexus_spec(source: &str) -> anyhow::Result<(&str, &str, &str)> {
    let value = source.strip_prefix("nexus:").ok_or_else(|| {
        anyhow::anyhow!("invalid nexus source, expected nexus:<game>/<mod_id>/<file_id>")
    })?;
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

#[cfg(test)]
mod similar_archive_tests {
    use super::similar_archives;

    #[test]
    fn a_renamed_version_is_suggested_and_unrelated_files_are_not() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "Oblivion Trees v2.7z",
            "Oblivion Trees v2.7z.meta",
            "Oblivion Character Overhaul V2.7z",
            "Unrelated Mod 1.0.zip",
            "Oblivion.zip",
        ] {
            std::fs::write(dir.path().join(name), b"").unwrap();
        }
        let found = similar_archives("Oblivion Trees.7z", &[dir.path().to_path_buf()]);
        let names: Vec<_> = found
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["Oblivion Trees v2.7z"]);
    }

    #[test]
    fn a_name_of_only_numbers_suggests_nothing() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("2024.zip"), b"").unwrap();
        assert!(similar_archives("2023.zip", &[dir.path().to_path_buf()]).is_empty());
    }
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
        let (game, mod_id, file_id) =
            parse_nexus_spec("nexus:skyrimspecialedition/1234/5678").expect("source should parse");

        assert_eq!(game, "skyrimspecialedition");
        assert_eq!(mod_id, "1234");
        assert_eq!(file_id, "5678");
    }

    #[test]
    fn resolves_nexus_handler_from_source_shape() {
        let explicit = CompiledArchive {
            size: None,
            md5: None,
            download_page: None,
            download_note: None,
            condition: None,
            path: Some("https://www.nexusmods.com/skyrimspecialedition/mods/1".to_string()),
            file_name: None,
            download_handler: None,
            layout: None,
            data_folder: None,
            target_subdir: None,
            inner_archive: None,
            bain_subpackages: vec![],
            fomod_selections: vec![],
            build: vec![],
            include: vec![],
            exclude: vec![],
            game_root_files: vec![],
        };
        assert_eq!(
            resolve_handler(
                explicit.path.as_deref().unwrap_or_default(),
                explicit.download_handler.as_deref()
            ),
            "nexus"
        );

        let scheme = CompiledArchive {
            size: None,
            md5: None,
            download_page: None,
            download_note: None,
            condition: None,
            path: Some("nexus:skyrimspecialedition/1/2".to_string()),
            file_name: None,
            download_handler: None,
            layout: None,
            data_folder: None,
            target_subdir: None,
            inner_archive: None,
            bain_subpackages: vec![],
            fomod_selections: vec![],
            build: vec![],
            include: vec![],
            exclude: vec![],
            game_root_files: vec![],
        };
        assert_eq!(
            resolve_handler(
                scheme.path.as_deref().unwrap_or_default(),
                scheme.download_handler.as_deref()
            ),
            "nexus"
        );
    }

    #[test]
    fn resolves_manual_handler_from_scheme_and_from_explicit_setting() {
        assert_eq!(resolve_handler("manual:WACv_1beta.7z", None), "manual");
        assert_eq!(resolve_handler("WACv_1beta.7z", Some("manual")), "manual");

        // A bare filename with no handler is still `local`; `manual` has to be
        // asked for, or every unresolvable path would claim to need a human.
        assert_eq!(resolve_handler("WACv_1beta.7z", None), "local");
    }

    #[tokio::test]
    async fn manual_archive_missing_from_every_search_path_names_the_file_and_the_paths() {
        let temp = tempfile::tempdir().expect("temp dir");
        let cache = temp.path().join("cache");
        let searched = temp.path().join("downloads");
        std::fs::create_dir_all(&cache).expect("cache dir");
        std::fs::create_dir_all(&searched).expect("search dir");

        let settings = DownloadSettings {
            cache_dir: cache.clone(),
            archive_search_paths: vec![searched.clone()],
            ..Default::default()
        };

        let err = download_with_retry(
            &Client::new(),
            "manual:WACv_1beta.7z",
            Some("WACv_1beta.7z"),
            None,
            None,
            &settings,
        )
        .await
        .expect_err("a manual archive that is not on disk cannot succeed");

        let message = err.to_string();
        // The file name, not the descriptor: it is what the user has to go and
        // find, and it is what lookup matches on.
        assert!(message.contains("WACv_1beta.7z"), "{message}");
        assert!(
            message.contains(&searched.display().to_string()),
            "{message}"
        );
        // No retry wrapper -- a hand-fetched file will not appear on attempt 2.
        assert!(!message.contains("attempt(s)"), "{message}");
    }

    #[test]
    fn search_path_description_separates_none_configured_from_none_matched() {
        assert!(describe_search_paths(&[]).contains("no --archive-search-path"));
        assert_eq!(
            describe_search_paths(&[PathBuf::from("/a"), PathBuf::from("/b")]),
            "/a, /b"
        );
    }

    #[tokio::test]
    async fn a_free_account_is_asked_once_not_once_per_retry() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(
                "/v1/games/oblivion/mods/1234/files/5678/download_link.json",
            ))
            .respond_with(ResponseTemplate::new(403))
            .expect(1)
            .mount(&server)
            .await;

        let temp = tempdir().expect("temp dir should be created");
        let settings = DownloadSettings {
            cache_dir: temp.path().to_path_buf(),
            retry: 3,
            nexus_api_key: Some("test-key".to_string()),
            nexus_api_base: Some(format!("{}/v1", server.uri())),
            filter: ModFilter::default(),
            archive_search_paths: Vec::new(),
            parallel: 1,
        };
        let err = download_with_retry(
            &Client::new(),
            "nexus:oblivion/1234/5678",
            None,
            None,
            None,
            &settings,
        )
        .await
        .expect_err("a 403 fails");
        assert!(!err.to_string().contains("attempt"), "{err}");
    }

    #[tokio::test]
    async fn a_free_account_is_told_where_to_download_by_hand() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(
                "/v1/games/oblivion/mods/1234/files/5678/download_link.json",
            ))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;

        let temp = tempdir().expect("temp dir should be created");
        let settings = DownloadSettings {
            cache_dir: temp.path().to_path_buf(),
            retry: 1,
            nexus_api_key: Some("test-key".to_string()),
            nexus_api_base: Some(format!("{}/v1", server.uri())),
            filter: ModFilter::default(),
            archive_search_paths: Vec::new(),
            parallel: 4,
        };
        let client = Client::new();
        let err = download_nexus(
            &client,
            "nexus:oblivion/1234/5678",
            &temp.path().join("x"),
            &settings,
        )
        .await
        .expect_err("a 403 fails");
        let message = err.to_string();
        assert!(message.contains("premium"), "{message}");
        assert!(
            message.contains("https://www.nexusmods.com/oblivion/mods/1234?tab=files&file_id=5678"),
            "{message}"
        );
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
            filter: ModFilter::default(),
            archive_search_paths: Vec::new(),
            parallel: 4,
        };

        let plan = PersonalizedPlan {
            guide: None,
            schema_version: 1,
            name: "Nexus Test".to_string(),
            responses: std::collections::HashMap::new(),
            mod_order: vec!["core".to_string()],
            selected_mods: vec!["core".to_string()],
            actions: Vec::new(),
            post_install_actions: vec![],
            mo2_modlist_entries: vec![],
            mods: vec![PersonalizedMod {
                id: "core".to_string(),
                enabled: true,
                origin: None,
                oracle_name: None,
                section: Vec::new(),
                mod_type: None,
                merge: None,
                plugins: Vec::new(),
                archives: vec![CompiledArchive {
                    size: None,
                    md5: None,
                    download_page: None,
                    download_note: None,
                    condition: None,
                    path: Some("nexus:skyrimspecialedition/1234/5678".to_string()),
                    file_name: None,
                    download_handler: Some("nexus".to_string()),
                    layout: None,
                    data_folder: None,
                    target_subdir: None,
                    inner_archive: None,
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
            inactive_plugins: vec![],
            manual_steps: vec![],
            mo2: None,
        };

        download_all(&plan, &settings)
            .await
            .expect("download should succeed");

        // Saved under the server's name, with the .meta MO2 would write.
        let payload = std::fs::read(temp.path().join("archive.bin"))
            .expect("the archive should be saved under its real name");
        assert_eq!(payload, b"nexus-payload");
        let meta = std::fs::read_to_string(temp.path().join("archive.bin.meta"))
            .expect("a .meta should be written beside it");
        assert!(meta.contains("gameName=SkyrimSE"), "{meta}");
        assert!(
            meta.contains("modID=1234") && meta.contains("fileID=5678"),
            "{meta}"
        );
        let leftovers: Vec<_> = std::fs::read_dir(temp.path())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name != "archive.bin" && name != "archive.bin.meta")
            .collect();
        assert!(
            leftovers.is_empty(),
            "no scratch files left behind: {leftovers:?}"
        );

        // A second run finds it by its ids and does not fetch again.
        download_all(&plan, &settings)
            .await
            .expect("second download should find the archive");
        assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 2);
    }

    #[tokio::test]
    async fn nexus_descriptor_requires_api_key() {
        let temp = tempdir().expect("temp dir should be created");
        let settings = DownloadSettings {
            cache_dir: temp.path().to_path_buf(),
            retry: 1,
            nexus_api_key: None,
            nexus_api_base: None,
            filter: ModFilter::default(),
            archive_search_paths: Vec::new(),
            parallel: 4,
        };

        let plan = PersonalizedPlan {
            guide: None,
            schema_version: 1,
            name: "Nexus Missing Key".to_string(),
            responses: std::collections::HashMap::new(),
            mod_order: vec!["core".to_string()],
            selected_mods: vec!["core".to_string()],
            actions: Vec::new(),
            post_install_actions: vec![],
            mo2_modlist_entries: vec![],
            mods: vec![PersonalizedMod {
                id: "core".to_string(),
                enabled: true,
                origin: None,
                oracle_name: None,
                section: Vec::new(),
                mod_type: None,
                merge: None,
                plugins: Vec::new(),
                archives: vec![CompiledArchive {
                    size: None,
                    md5: None,
                    download_page: None,
                    download_note: None,
                    condition: None,
                    path: Some("nexus:skyrimspecialedition/1234/5678".to_string()),
                    file_name: None,
                    download_handler: Some("nexus".to_string()),
                    layout: None,
                    data_folder: None,
                    target_subdir: None,
                    inner_archive: None,
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
            inactive_plugins: vec![],
            manual_steps: vec![],
            mo2: None,
        };

        let unfetched = download_report(&plan, &settings).await.unwrap();
        assert_eq!(unfetched.len(), 1);
        assert!(
            unfetched[0].error.contains("NEXUS_API_KEY"),
            "{}",
            unfetched[0].error
        );
        let err = download_all(&plan, &settings)
            .await
            .expect_err("missing key should fail");
        assert!(err.to_string().contains("could not be downloaded"), "{err}");
    }
}
