//! Modlist files included from git repositories.
//!
//! An include can name a file in a repository rather than on disk:
//!
//! ```text
//! git+<url>?tag=<tag>#<path in the repository>
//! git+<url>?rev=<commit>#<path>
//! git+<url>?branch=<branch>#<path>
//! ```
//!
//! The repository is cloned once into mudcrab's cache (`git/` under
//! [`cache_root`]) and each commit used is unpacked beside it, read only.
//! From then on the included file is a file on disk like any other, so what
//! it includes in turn, its docs file and its images are found relative to
//! it, inside the same commit.
//!
//! Which commit a `tag` or `branch` means is recorded in the list's lock file
//! (`<list>.lock`, next to it) the first time it is resolved, and read from
//! there after, so a list resolves to the same mods until `mudcrab update`
//! moves it on. With the lock and the cache in place nothing is fetched, and
//! with `--offline` nothing ever is.
//!
//! git itself does the work, as 7-Zip does for archives: there is no git
//! library to build. In a Flatpak it is the host's.

use crate::util::fs::cache_root;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};

/// What `git+...` names: a file at a commit of a repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitSource {
    pub url: String,
    pub reference: Reference,
    /// The file, from the repository's top.
    pub path: String,
}

/// Which commit: a tag or a commit, as a list should; or a branch's newest,
/// for a component still being written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reference {
    Tag(String),
    Rev(String),
    Branch(String),
}

impl Reference {
    /// As the include and the lock file write it: `tag=v3`.
    pub fn key(&self) -> String {
        match self {
            Reference::Tag(tag) => format!("tag={tag}"),
            Reference::Rev(rev) => format!("rev={rev}"),
            Reference::Branch(branch) => format!("branch={branch}"),
        }
    }

    /// What `git rev-parse` resolves it to a commit from.
    fn revision(&self) -> String {
        match self {
            Reference::Tag(tag) => format!("refs/tags/{tag}^{{commit}}"),
            Reference::Rev(rev) => format!("{rev}^{{commit}}"),
            Reference::Branch(branch) => format!("refs/heads/{branch}^{{commit}}"),
        }
    }
}

impl std::fmt::Display for GitSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "git+{}?{}#{}", self.url, self.reference.key(), self.path)
    }
}

/// `include` as a git source: `None` when it is a path, an error when it is
/// a `git+` source written wrongly.
pub fn parse(include: &str) -> Option<anyhow::Result<GitSource>> {
    let rest = include.strip_prefix("git+")?;
    Some(parse_source(rest).map_err(|err| {
        anyhow::anyhow!(
            "{include}: {err}; a git include is git+<url>?tag=<tag>#<file> (or rev=<commit>, \
             or branch=<branch>)"
        )
    }))
}

fn parse_source(rest: &str) -> anyhow::Result<GitSource> {
    let (rest, path) = rest
        .rsplit_once('#')
        .ok_or_else(|| anyhow::anyhow!("it names no file (#<file>)"))?;
    let path = path.trim_start_matches('/');
    anyhow::ensure!(!path.is_empty(), "it names no file (#<file>)");
    let (url, query) = rest
        .rsplit_once('?')
        .ok_or_else(|| anyhow::anyhow!("it names no tag, commit or branch"))?;
    anyhow::ensure!(!url.is_empty(), "it names no repository");
    let (key, value) = query
        .split_once('=')
        .ok_or_else(|| anyhow::anyhow!("{query:?} is not tag=, rev= or branch="))?;
    anyhow::ensure!(!value.is_empty(), "{key}= is empty");
    let reference = match key {
        "tag" => Reference::Tag(value.to_string()),
        "rev" => Reference::Rev(value.to_string()),
        "branch" => Reference::Branch(value.to_string()),
        other => anyhow::bail!("{other}= is not tag=, rev= or branch="),
    };
    Ok(GitSource {
        url: url.to_string(),
        reference,
        path: path.to_string(),
    })
}

static OFFLINE: AtomicBool = AtomicBool::new(false);
static CACHE: OnceLock<PathBuf> = OnceLock::new();

/// Never fetch: resolve git includes from the lock file and the cache alone,
/// and say which cannot be.
pub fn set_offline(offline: bool) {
    OFFLINE.store(offline, Ordering::Relaxed);
}

fn offline() -> bool {
    OFFLINE.load(Ordering::Relaxed)
}

/// Keep repositories in `dir` instead of the user's cache. For tests; the
/// first call wins.
pub fn set_cache_dir(dir: PathBuf) {
    let _ = CACHE.set(dir);
}

/// Where repositories and their commits are kept.
pub fn cache_dir() -> PathBuf {
    CACHE
        .get()
        .cloned()
        .unwrap_or_else(|| cache_root().join("git"))
}

/// A repository's folder in the cache: its name, and a hash of its URL so
/// two repositories of one name do not meet.
fn repo_dir(url: &str) -> PathBuf {
    use sha2::{Digest, Sha256};
    let name = url
        .trim_end_matches('/')
        .trim_end_matches(".git")
        .rsplit(['/', '\\', ':'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("repo");
    let name: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let digest = Sha256::digest(url.as_bytes());
    let hash: String = digest[..6].iter().map(|b| format!("{b:02x}")).collect();
    cache_dir().join(format!("{name}-{hash}"))
}

/// A file of a commit unpacked in the cache, described for a reader:
/// `oblivion-common@3f2a1c0:patches.toml`. `None` for any other file.
pub fn describe(path: &Path) -> Option<String> {
    let rest = path.strip_prefix(cache_dir()).ok()?;
    let mut parts = rest.components();
    let repo = parts.next()?.as_os_str().to_string_lossy().into_owned();
    let commit = parts.next()?.as_os_str().to_string_lossy().into_owned();
    if commit == "repo.git" {
        return None;
    }
    let name = repo
        .rsplit_once('-')
        .map_or(repo.as_str(), |(name, _)| name);
    let file = parts
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/");
    Some(format!("{name}@{}:{file}", &commit[..commit.len().min(7)]))
}

/// The unpacked commit `path` is in, if it is in one.
pub fn checkout_of(path: &Path) -> Option<PathBuf> {
    let rest = path.strip_prefix(cache_dir()).ok()?;
    let mut parts = rest.components();
    let repo = parts.next()?;
    let commit = parts.next()?;
    Some(cache_dir().join(repo).join(commit))
}

/// Refuse to edit `path` when it is a file from a git include: the cache is
/// not where it is changed.
pub fn ensure_editable(path: &Path) -> anyhow::Result<()> {
    match describe(path) {
        Some(file) => anyhow::bail!(
            "that is in {file}, which the list includes from a git repository: change it there, \
             and `mudcrab update` brings the change in"
        ),
        None => Ok(()),
    }
}

/// The lock file: which commit each git include resolved to.
#[derive(Debug, Default, Clone)]
pub struct Lock {
    /// Resolve from this lock and the cache alone: [`set_offline`], when the
    /// lock was read.
    pub offline: bool,
    path: Option<PathBuf>,
    entries: Vec<Locked>,
    changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Locked {
    pub url: String,
    /// `tag=v3`, `rev=...`, `branch=main`.
    #[serde(rename = "ref")]
    pub reference: String,
    pub commit: String,
}

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct LockFile {
    #[serde(default)]
    git: Vec<Locked>,
}

/// The lock file of the list at `list`: `<its name>.lock`, beside it.
pub fn lock_path(list: &Path) -> PathBuf {
    list.with_extension("lock")
}

impl Lock {
    /// The lock file of the list at `list`, or an empty one.
    pub fn read(list: &Path) -> anyhow::Result<Self> {
        let path = lock_path(list);
        let entries = match std::fs::read_to_string(&path) {
            Ok(text) => {
                toml::from_str::<LockFile>(&text)
                    .map_err(|err| anyhow::anyhow!("failed to parse {}: {err}", path.display()))?
                    .git
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(err) => anyhow::bail!("failed to read {}: {err}", path.display()),
        };
        Ok(Self {
            offline: offline(),
            path: Some(path),
            entries,
            changed: false,
        })
    }

    pub fn entries(&self) -> &[Locked] {
        &self.entries
    }

    fn get(&self, url: &str, reference: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|entry| entry.url == url && entry.reference == reference)
            .map(|entry| entry.commit.as_str())
    }

    fn set(&mut self, url: &str, reference: &str, commit: &str) {
        self.forget(url, reference);
        self.entries.push(Locked {
            url: url.to_string(),
            reference: reference.to_string(),
            commit: commit.to_string(),
        });
        self.entries
            .sort_by(|a, b| (&a.url, &a.reference).cmp(&(&b.url, &b.reference)));
        self.changed = true;
    }

    /// Drop what `url` at `reference` resolved to, so the next load resolves
    /// it afresh: what `update` does.
    pub fn forget(&mut self, url: &str, reference: &str) {
        let before = self.entries.len();
        self.entries
            .retain(|entry| !(entry.url == url && entry.reference == reference));
        self.changed |= self.entries.len() != before;
    }

    /// Write the lock file, if anything in it changed.
    pub fn save(&mut self) -> anyhow::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if !self.changed {
            return Ok(());
        }
        let body = toml::to_string(&LockFile {
            git: self.entries.clone(),
        })?;
        let text = format!(
            "# Which commit each git include of this list resolved to. Written by mudcrab:\n\
             # `mudcrab update` moves it on. Keep it with the list.\n\n{body}"
        );
        crate::config::edit::write_atomic(path, &text)?;
        self.changed = false;
        Ok(())
    }
}

/// A git include, resolved: the file on disk, and the commit it is from.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub file: PathBuf,
    pub commit: String,
}

/// The file `source` names, on disk: the commit the lock says, or what its
/// reference means now (recorded in the lock), unpacked in the cache.
pub fn resolve(source: &GitSource, lock: &mut Lock) -> anyhow::Result<Resolved> {
    let key = source.reference.key();
    let repo = repo_dir(&source.url);
    let commit = match lock.get(&source.url, &key) {
        Some(commit) => commit.to_string(),
        None => {
            let commit = find_commit(source, &repo, lock.offline)?;
            lock.set(&source.url, &key, &commit);
            commit
        }
    };
    let checkout = repo.join(&commit);
    if !checkout.is_dir() {
        unpack(source, &repo, &commit, &checkout, lock.offline)?;
    }
    let file = crate::config::loader::normalize(&checkout.join(&source.path));
    anyhow::ensure!(
        file.starts_with(&checkout),
        "{source}: the file is outside the repository"
    );
    anyhow::ensure!(
        file.is_file(),
        "{source}: commit {} has no file {}",
        short(&commit),
        source.path
    );
    Ok(Resolved { file, commit })
}

fn short(commit: &str) -> &str {
    &commit[..commit.len().min(7)]
}

/// What `source`'s reference means now, fetching first when the repository
/// was cloned before. Only for a reference the lock does not hold, so a list
/// resolved once is not fetched for again.
fn find_commit(source: &GitSource, repo: &Path, offline: bool) -> anyhow::Result<String> {
    let bare = repo.join("repo.git");
    if offline {
        return rev_parse(&bare, &source.reference.revision()).ok_or_else(|| {
            anyhow::anyhow!(
                "{source} is not in the lock file, and offline it cannot be looked up; run once \
                 without --offline"
            )
        });
    }
    if bare.is_dir() {
        // A commit given in full is there or not; a tag or branch may have
        // moved since the clone.
        if let Reference::Rev(_) = source.reference
            && let Some(commit) = rev_parse(&bare, &source.reference.revision())
        {
            return Ok(commit);
        }
        fetch(&source.url, &bare)?;
    } else {
        clone(&source.url, repo)?;
    }
    rev_parse(&bare, &source.reference.revision()).ok_or_else(|| {
        let what = match &source.reference {
            Reference::Tag(tag) => format!("no tag {tag}"),
            Reference::Rev(rev) => format!("no commit {rev}"),
            Reference::Branch(branch) => format!("no branch {branch}"),
        };
        anyhow::anyhow!("{source}: the repository has {what}")
    })
}

/// Unpack `commit` into `checkout`, fetching it first if the clone lacks it.
fn unpack(
    source: &GitSource,
    repo: &Path,
    commit: &str,
    checkout: &Path,
    offline: bool,
) -> anyhow::Result<()> {
    let bare = repo.join("repo.git");
    let have = |bare: &Path| rev_parse(bare, &format!("{commit}^{{commit}}")).is_some();
    if !bare.is_dir() || !have(&bare) {
        anyhow::ensure!(
            !offline,
            "{source}: commit {} is not in the cache, and offline it cannot be fetched",
            short(commit)
        );
        if bare.is_dir() {
            fetch(&source.url, &bare)?;
        } else {
            clone(&source.url, repo)?;
        }
        anyhow::ensure!(
            have(&bare),
            "{source}: the repository has no commit {} (the lock file's); `mudcrab update` \
             resolves it again",
            short(commit)
        );
    }
    let output = git(&bare)
        .args(["archive", "--format=tar", commit])
        .output()
        .map_err(not_installed)?;
    anyhow::ensure!(
        output.status.success(),
        "git archive {}: {}",
        short(commit),
        String::from_utf8_lossy(&output.stderr).trim()
    );
    // Unpacked beside its final name and renamed into place, so a half
    // unpacked commit is never taken for a whole one.
    let partial = checkout.with_extension(format!("partial-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&partial);
    std::fs::create_dir_all(&partial)
        .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", partial.display()))?;
    tar::Archive::new(output.stdout.as_slice())
        .unpack(&partial)
        .map_err(|err| anyhow::anyhow!("failed to unpack {}: {err}", short(commit)))?;
    match std::fs::rename(&partial, checkout) {
        Ok(()) => Ok(()),
        // Another run unpacked it first.
        Err(_) if checkout.is_dir() => {
            let _ = std::fs::remove_dir_all(&partial);
            Ok(())
        }
        Err(err) => Err(anyhow::anyhow!(
            "failed to move {} into place: {err}",
            checkout.display()
        )),
    }
}

fn clone(url: &str, repo: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(repo)
        .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", repo.display()))?;
    let bare = repo.join("repo.git");
    let partial = repo.join(format!("repo.partial-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&partial);
    let output = crate::util::host::on_host(git_command())
        .args(["clone", "--bare", "--quiet", "--", url])
        .arg(&partial)
        .output()
        .map_err(not_installed)?;
    anyhow::ensure!(
        output.status.success(),
        "git clone {url}: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    if std::fs::rename(&partial, &bare).is_err() {
        let _ = std::fs::remove_dir_all(&partial);
        anyhow::ensure!(
            bare.is_dir(),
            "failed to move the clone of {url} into place"
        );
    }
    Ok(())
}

fn fetch(url: &str, bare: &Path) -> anyhow::Result<()> {
    let output = git(bare)
        .args([
            "fetch",
            "--quiet",
            "--force",
            "--tags",
            "--",
            url,
            "+refs/heads/*:refs/heads/*",
        ])
        .output()
        .map_err(not_installed)?;
    anyhow::ensure!(
        output.status.success(),
        "git fetch {url}: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );
    Ok(())
}

fn rev_parse(bare: &Path, revision: &str) -> Option<String> {
    if !bare.is_dir() {
        return None;
    }
    let output = git(bare)
        .args(["rev-parse", "--verify", "--quiet", revision])
        .output()
        .ok()?;
    let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (output.status.success() && !commit.is_empty()).then_some(commit)
}

fn git_command() -> Command {
    let mut command = crate::util::process::console("git");
    // Never stop to ask for a password: a private repository fails, saying so.
    command.env("GIT_TERMINAL_PROMPT", "0");
    command
}

/// git, in the repository at `bare`.
fn git(bare: &Path) -> Command {
    let mut command = git_command();
    command.arg("--git-dir").arg(bare);
    crate::util::host::on_host(command)
}

fn not_installed(err: std::io::Error) -> anyhow::Error {
    if err.kind() == std::io::ErrorKind::NotFound {
        anyhow::anyhow!(
            "git is not installed, and this list includes files from git repositories; install \
             it (https://git-scm.com) and try again"
        )
    } else {
        anyhow::anyhow!("failed to run git: {err}")
    }
}

/// What `update` would change: each git include moved to another commit,
/// and the mods that differ for it.
#[derive(Debug, Default)]
pub struct Update {
    pub moved: Vec<Move>,
    /// Mod ids the list gains, loses, or whose definition changes.
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub changed: Vec<String>,
}

#[derive(Debug)]
pub struct Move {
    pub id: String,
    pub source: String,
    pub from: Option<String>,
    pub to: String,
}

/// Resolve the list's git includes afresh -- those with the ids in `only`,
/// or all -- and say what changes. The lock to save, if the change is
/// wanted, comes back with it.
pub fn plan_update(list: &Path, only: &[String]) -> anyhow::Result<(Update, Lock)> {
    use crate::config::loader::{Texts, load_locked};
    let mut lock = Lock::read(list)?;
    let before = load_locked(list, false, &Texts::new(), &mut lock)?;
    let chosen: Vec<_> = before
        .includes
        .iter()
        .filter_map(|included| {
            let source = parse(&included.source)?.ok()?;
            (only.is_empty() || only.contains(&included.id)).then_some((included, source))
        })
        .collect();
    for id in only {
        anyhow::ensure!(
            chosen.iter().any(|(included, _)| &included.id == id),
            "the list has no git include '{id}'"
        );
    }
    for (_, source) in &chosen {
        lock.forget(&source.url, &source.reference.key());
    }
    let after = load_locked(list, false, &Texts::new(), &mut lock)?;

    let mut update = Update::default();
    for (included, _) in &chosen {
        let now = after
            .includes
            .iter()
            .find(|other| other.id == included.id && other.by == included.by)
            .and_then(|other| other.commit.clone());
        if let Some(to) = now
            && Some(&to) != included.commit.as_ref()
        {
            update.moved.push(Move {
                id: included.id.clone(),
                source: included.source.clone(),
                from: included.commit.clone(),
                to,
            });
        }
    }
    let definitions = |list: crate::config::schema::SourceModlist| -> anyhow::Result<
        indexmap::IndexMap<String, serde_json::Value>,
    > {
        let compiled = crate::config::compiler::compile(list)?;
        Ok(compiled
            .mods
            .into_iter()
            .map(|mut entry| {
                // Its commit is in its origin; what matters is the rest.
                entry.origin = None;
                (entry.id.clone(), serde_json::to_value(entry).unwrap_or_default())
            })
            .collect())
    };
    let (old, new) = (definitions(before)?, definitions(after)?);
    for (id, definition) in &new {
        match old.get(id) {
            None => update.added.push(id.clone()),
            Some(was) if was != definition => update.changed.push(id.clone()),
            Some(_) => {}
        }
    }
    update.removed = old
        .keys()
        .filter(|id| !new.contains_key(*id))
        .cloned()
        .collect();
    Ok((update, lock))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_git_include_is_parsed_and_written_back() {
        let source = parse("git+https://github.com/someone/common?tag=v3#parts/core.toml")
            .unwrap()
            .unwrap();
        assert_eq!(source.url, "https://github.com/someone/common");
        assert_eq!(source.reference, Reference::Tag("v3".to_string()));
        assert_eq!(source.path, "parts/core.toml");
        assert_eq!(
            source.to_string(),
            "git+https://github.com/someone/common?tag=v3#parts/core.toml"
        );
        assert!(parse("parts/core.toml").is_none());
    }

    #[test]
    fn a_git_include_without_a_file_or_reference_is_refused() {
        for bad in [
            "git+https://example.com/r?tag=v1",
            "git+https://example.com/r#a.toml",
            "git+https://example.com/r?tags=v1#a.toml",
            "git+https://example.com/r?tag=#a.toml",
        ] {
            let err = parse(bad).unwrap().unwrap_err().to_string();
            assert!(err.contains("git+<url>?tag="), "{bad}: {err}");
        }
    }
}
