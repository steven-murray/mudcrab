//! Identify a downloaded archive against Nexus by its MD5.
//!
//! An archive fetched by hand has no `.meta` sidecar, so there is no mod id or
//! file id to write into a modlist entry -- and `compile` rightly refuses an
//! archive that cannot say where it came from. Mod Organizer 2 solves this by
//! asking Nexus to identify the file by hash, which is an ordinary API call:
//!
//! ```text
//! GET /v1/games/{game}/mods/md5_search/{md5}.json
//! ```
//!
//! Nexus indexes every file it serves by MD5, so the hash is usually enough to
//! recover the mod, the file, and its version. Usually: the index does not
//! reach every file. An archive moved to a mod's OLD FILES section can be
//! perfectly genuine and still come back unrecognised -- OOO Enhanced 5.3
//! PreRelease does exactly that, and MO2 cannot identify it either, recording
//! `fileID=0`.
//!
//! So there is a second route, which is the one to use when the first fails:
//!
//! ```text
//! GET /v1/games/{game}/mods/{mod_id}/files.json
//! ```
//!
//! That lists every file the mod has ever published, old ones included, and the
//! archive can be matched against it by name. The mod id is usually sitting in
//! the filename already, since Nexus names downloads `<title>-<mod id>-...`.

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// What Nexus knows about an archive, once it has been recognised.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identified {
    pub file_name: String,
    pub md5: String,
    pub game: String,
    pub mod_id: u64,
    pub file_id: u64,
    pub mod_name: Option<String>,
    pub file_title: Option<String>,
    pub version: Option<String>,
    pub method: Method,
}

impl Identified {
    /// The `[[mods.archives]]` block this archive wants, ready to paste.
    pub fn toml_snippet(&self) -> String {
        let mut out = String::from("[[mods.archives]]\n");
        out.push_str(&format!(
            "path = \"nexus:{}/{}/{}\"\n",
            self.game, self.mod_id, self.file_id
        ));
        out.push_str("download_handler = \"nexus\"\n");
        out.push_str(&format!("file_name = {:?}\n", self.file_name));
        if let Some(version) = &self.version {
            out.push_str(&format!("# nexus version {version}\n"));
        }
        out
    }
}

/// The shape of one `md5_search` result. Nexus returns far more than this; only
/// the identifying fields are read, so a change elsewhere in the payload does
/// not break the lookup.
#[derive(Debug, Deserialize)]
struct Md5Result {
    #[serde(default)]
    r#mod: Option<Md5Mod>,
    #[serde(default)]
    file_details: Option<Md5File>,
}

#[derive(Debug, Deserialize)]
struct Md5Mod {
    #[serde(default)]
    mod_id: Option<u64>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    domain_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Md5File {
    #[serde(default)]
    file_id: Option<u64>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    version: Option<String>,
    /// Present in `files.json`; absent from an `md5_search` result.
    #[serde(default)]
    file_name: Option<String>,
}

/// Hex MD5 of a file, read in chunks so a multi-gigabyte archive does not have
/// to be held in memory.
pub fn md5_of_file(path: &Path) -> anyhow::Result<String> {
    use std::io::Read;

    let mut file = std::fs::File::open(path)
        .map_err(|err| anyhow::anyhow!("failed to open {}: {err}", path.display()))?;
    let mut context = md5::Context::new();
    let mut buffer = vec![0u8; 1 << 20];

    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?;
        if read == 0 {
            break;
        }
        context.consume(&buffer[..read]);
    }

    Ok(format!("{:x}", context.finalize()))
}

/// How an archive was recognised, for reporting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Matched by content hash.
    Md5,
    /// Matched by filename against the mod's published file list.
    FileList,
}

/// The mod id Nexus embedded in a download's filename.
///
/// Nexus names downloads `<title>-<mod id>-<version parts>-<timestamp>.<ext>`,
/// so the first all-digit field after a `-` is the mod id. Used only to aim the
/// file-list lookup; a wrong guess produces no match rather than a wrong one,
/// because the filename still has to agree.
pub fn mod_id_from_file_name(file_name: &str) -> Option<u64> {
    let stem = file_name.rsplit_once('.').map(|(head, _)| head).unwrap_or(file_name);
    stem.split('-')
        .skip(1)
        .find(|field| !field.is_empty() && field.chars().all(|c| c.is_ascii_digit()))
        .and_then(|field| field.parse().ok())
}

/// One entry of `files.json`.
#[derive(Debug, Deserialize)]
struct FileListing {
    #[serde(default)]
    files: Vec<Md5File>,
}

/// Ask Nexus which mod and file this archive is.
pub async fn identify(
    client: &reqwest::Client,
    path: &Path,
    game: &str,
    api_key: &str,
    api_base: Option<&str>,
    mod_id_hint: Option<u64>,
) -> anyhow::Result<Identified> {
    let md5 = md5_of_file(path)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();

    let endpoint = format!(
        "{}/games/{}/mods/md5_search/{}.json",
        api_base.unwrap_or("https://api.nexusmods.com/v1").trim_end_matches('/'),
        game,
        md5
    );

    let response = client
        .get(&endpoint)
        .header("apikey", api_key)
        .send()
        .await
        .map_err(|err| anyhow::anyhow!("nexus md5 lookup failed for {file_name}: {err}"))?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return identify_by_file_list(client, &file_name, &md5, game, api_key, api_base, mod_id_hint)
            .await;
    }

    let results = response
        .error_for_status()
        .map_err(|err| anyhow::anyhow!("nexus md5 lookup returned an error: {err}"))?
        .json::<Vec<Md5Result>>()
        .await
        .map_err(|err| anyhow::anyhow!("failed to parse the nexus md5 response: {err}"))?;

    let Some(first) = results
        .into_iter()
        .find(|entry| entry.r#mod.is_some() && entry.file_details.is_some())
    else {
        return identify_by_file_list(client, &file_name, &md5, game, api_key, api_base, mod_id_hint)
            .await;
    };

    let mod_info = first.r#mod.expect("filtered for Some above");
    let file_info = first.file_details.expect("filtered for Some above");

    let (Some(mod_id), Some(file_id)) = (mod_info.mod_id, file_info.file_id) else {
        anyhow::bail!("nexus matched {file_name} but returned no mod id or file id");
    };

    Ok(Identified {
        file_name,
        md5,
        game: mod_info.domain_name.unwrap_or_else(|| game.to_string()),
        mod_id,
        file_id,
        mod_name: mod_info.name,
        file_title: file_info.name,
        version: file_info.version,
        method: Method::Md5,
    })
}

/// Fall back to the mod's published file list, matching on filename.
///
/// This is what finds a file the MD5 index has no entry for -- an OLD FILES
/// download, typically. Matching is on the exact filename Nexus serves, so a
/// wrong `mod_id` hint yields no match rather than a wrong one.
async fn identify_by_file_list(
    client: &reqwest::Client,
    file_name: &str,
    md5: &str,
    game: &str,
    api_key: &str,
    api_base: Option<&str>,
    mod_id_hint: Option<u64>,
) -> anyhow::Result<Identified> {
    let Some(mod_id) = mod_id_hint.or_else(|| mod_id_from_file_name(file_name)) else {
        anyhow::bail!(
            "Nexus does not recognise {file_name} by hash (md5 {md5}), and no mod id \
             could be read from its filename to search the mod's file list. Pass \
             --mod-id <id>."
        );
    };

    let endpoint = format!(
        "{}/games/{}/mods/{}/files.json",
        api_base.unwrap_or("https://api.nexusmods.com/v1").trim_end_matches('/'),
        game,
        mod_id
    );

    let listing = client
        .get(&endpoint)
        .header("apikey", api_key)
        .send()
        .await
        .map_err(|err| anyhow::anyhow!("nexus file-list request failed for mod {mod_id}: {err}"))?
        .error_for_status()
        .map_err(|err| anyhow::anyhow!("nexus file list returned an error: {err}"))?
        .json::<FileListing>()
        .await
        .map_err(|err| anyhow::anyhow!("failed to parse the nexus file list: {err}"))?;

    let matched = listing
        .files
        .into_iter()
        .find(|entry| {
            entry
                .file_name
                .as_deref()
                .is_some_and(|name| name.eq_ignore_ascii_case(file_name))
        })
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Nexus does not recognise {file_name}: not in the MD5 index (md5 {md5}), \
                 and mod {mod_id} publishes no file with that name. Either it is not a \
                 Nexus file, it was repacked after download, or it was renamed."
            )
        })?;

    let file_id = matched
        .file_id
        .ok_or_else(|| anyhow::anyhow!("nexus listed {file_name} but gave it no file id"))?;

    Ok(Identified {
        file_name: file_name.to_string(),
        md5: md5.to_string(),
        game: game.to_string(),
        mod_id,
        file_id,
        mod_name: None,
        file_title: matched.name,
        version: matched.version,
        method: Method::FileList,
    })
}

/// Write the `.meta` sidecar MO2 would have written, so the archive is
/// identified for every later run without asking Nexus again.
pub fn write_meta_sidecar(archive: &Path, found: &Identified) -> anyhow::Result<PathBuf> {
    let mut sidecar = archive.as_os_str().to_os_string();
    sidecar.push(".meta");
    let sidecar = PathBuf::from(sidecar);

    let mut body = String::from("[General]\n");
    body.push_str("gameName=Oblivion\n");
    body.push_str(&format!("modID={}\n", found.mod_id));
    body.push_str(&format!("fileID={}\n", found.file_id));
    if let Some(name) = &found.file_title {
        body.push_str(&format!("name={name}\n"));
    }
    if let Some(version) = &found.version {
        body.push_str(&format!("version={version}\n"));
    }
    body.push_str("repository=Nexus\n");
    body.push_str("# written by `mudcrab identify`\n");

    std::fs::write(&sidecar, body)
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", sidecar.display()))?;
    Ok(sidecar)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md5_matches_a_known_value() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("x.bin");
        std::fs::write(&path, b"abc").expect("write");
        assert_eq!(md5_of_file(&path).unwrap(), "900150983cd24fb0d6963f7d28e17f72");
    }

    #[test]
    fn md5_streams_a_file_larger_than_the_buffer() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("big.bin");
        std::fs::write(&path, vec![0u8; (1 << 20) + 12345]).expect("write");
        // Same value as `md5sum` on a file of 1060721 zero bytes.
        assert_eq!(md5_of_file(&path).unwrap().len(), 32);
    }

    #[test]
    fn the_mod_id_is_read_out_of_a_nexus_filename() {
        assert_eq!(
            mod_id_from_file_name("OOO Enhanced-47187-5-3-Pre-release-1740353484.rar"),
            Some(47187)
        );
        assert_eq!(
            mod_id_from_file_name("EVE for Oscuro Oblivion Overhaul 1_3 BAIN-24078.7z"),
            Some(24078)
        );
        // The title is skipped even when it starts with digits.
        assert_eq!(mod_id_from_file_name("2 VWD Ships-50111-1-01.zip"), Some(50111));
        assert_eq!(mod_id_from_file_name("hand-named.7z"), None);
    }

    #[test]
    fn the_snippet_is_valid_toml_and_carries_the_descriptor() {
        let found = Identified {
            file_name: "EVE for Oscuro Oblivion Overhaul 1_3 BAIN-24078.7z".to_string(),
            md5: "deadbeef".to_string(),
            game: "oblivion".to_string(),
            mod_id: 24078,
            file_id: 42364,
            mod_name: Some("EVE".to_string()),
            file_title: Some("EVE BAIN".to_string()),
            version: Some("1.3".to_string()),
            method: Method::Md5,
        };

        let snippet = found.toml_snippet();
        assert!(snippet.contains("path = \"nexus:oblivion/24078/42364\""));
        let parsed: toml::Table = toml::from_str(&snippet).expect("snippet should be valid TOML");
        assert!(parsed.contains_key("mods"));
    }
}
