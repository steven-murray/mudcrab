//! Likely values for `tools.toml`, found by looking where they usually are.
//!
//! `setup-tools` offers these as defaults; nothing here is trusted without the
//! person choosing it. Each search is shallow and bounded, so a home folder
//! full of games costs a moment, not a minute.

use std::path::{Path, PathBuf};

/// A found value and a few words on where it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub path: PathBuf,
    pub why: String,
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Steam library folders that may hold `compatdata` prefixes.
fn steam_roots() -> Vec<PathBuf> {
    let Some(home) = home() else {
        return Vec::new();
    };
    let mut roots: Vec<PathBuf> = [
        ".local/share/Steam",
        ".steam/steam",
        ".var/app/com.valvesoftware.Steam/.local/share/Steam",
    ]
    .iter()
    .map(|r| home.join(r))
    .filter(|p| p.join("steamapps").is_dir())
    .collect();
    // `.steam/steam` is usually a link to `.local/share/Steam`.
    let mut seen = Vec::new();
    roots.retain(|root| {
        let real = std::fs::canonicalize(root).unwrap_or_else(|_| root.clone());
        let new = !seen.contains(&real);
        seen.push(real);
        new
    });
    roots
}

/// A non-Steam game added to Steam: what Steam calls it, and what it runs.
#[derive(Debug, Clone, PartialEq)]
pub struct Shortcut {
    pub app_id: u64,
    pub name: String,
    pub exe: String,
}

/// Steam's non-Steam shortcuts, from every user's `shortcuts.vdf`.
pub fn shortcuts() -> Vec<Shortcut> {
    let mut out = Vec::new();
    for root in steam_roots() {
        let Ok(users) = std::fs::read_dir(root.join("userdata")) else {
            continue;
        };
        for user in users.flatten() {
            if let Ok(bytes) = std::fs::read(user.path().join("config/shortcuts.vdf")) {
                out.extend(parse_shortcuts(&bytes));
            }
        }
    }
    out
}

/// Binary VDF, read just far enough: each entry's `appid` (a 32-bit integer)
/// and the `AppName` and `Exe` strings after it. Key case varies between
/// Steam versions.
fn parse_shortcuts(bytes: &[u8]) -> Vec<Shortcut> {
    fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack
            .windows(needle.len())
            .position(|w| w.eq_ignore_ascii_case(needle))
    }
    fn string_after(bytes: &[u8], key: &[u8]) -> Option<String> {
        let mut needle = vec![0x01];
        needle.extend_from_slice(key);
        needle.push(0);
        let start = find(bytes, &needle)? + needle.len();
        let end = bytes[start..].iter().position(|b| *b == 0)? + start;
        Some(String::from_utf8_lossy(&bytes[start..end]).into_owned())
    }
    let mut out = Vec::new();
    let mut at = 0;
    while let Some(found) = find(&bytes[at..], b"\x02appid\x00") {
        let id_at = at + found + 7;
        let Some(raw) = bytes.get(id_at..id_at + 4) else {
            break;
        };
        let app_id = u64::from(u32::from_le_bytes(raw.try_into().unwrap()));
        // The entry runs to the next `appid`.
        let rest = &bytes[id_at + 4..];
        let entry_end = find(rest, b"\x02appid\x00").unwrap_or(rest.len());
        let entry = &rest[..entry_end];
        out.push(Shortcut {
            app_id,
            name: string_after(entry, b"appname").unwrap_or_default(),
            exe: string_after(entry, b"exe")
                .unwrap_or_default()
                .trim_matches('"')
                .to_string(),
        });
        at = id_at + 4;
    }
    out
}

fn runs_mo2(shortcut: &Shortcut) -> bool {
    shortcut
        .exe
        .to_ascii_lowercase()
        .ends_with("modorganizer.exe")
}

/// Files named `names` (case-insensitively) under `root`, at most `depth`
/// folders down.
fn find_files(root: &Path, names: &[&str], depth: usize, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            if depth > 0 {
                find_files(&path, names, depth - 1, out);
            }
        } else if names.iter().any(|name| {
            entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case(name)
        }) {
            out.push(path);
        }
    }
}

/// Downloads folders of the MO2 instances under `roots`, and of those Steam
/// runs (`shortcuts`).
pub fn downloads_dirs(roots: &[PathBuf], shortcuts: &[Shortcut]) -> Vec<Candidate> {
    let instances = mo2_instances(roots, shortcuts);
    let mut out: Vec<Candidate> = Vec::new();
    for instance in &instances {
        let instance = instance.as_path();
        let Some(dir) = crate::config::mo2::instance_download_dir(instance) else {
            continue;
        };
        if !dir.is_dir() || out.iter().any(|c| c.path == dir) {
            continue;
        }
        let name = instance
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        out.push(Candidate {
            path: dir,
            why: format!("MO2 instance \"{name}\" downloads here"),
        });
    }
    out
}

/// MO2 instance folders: those under `roots` (by their `ModOrganizer.ini`),
/// then those Steam runs (`shortcuts`).
pub fn mo2_instances(roots: &[PathBuf], shortcuts: &[Shortcut]) -> Vec<PathBuf> {
    let mut inis = Vec::new();
    for root in roots {
        find_files(root, &["ModOrganizer.ini"], 4, &mut inis);
    }
    inis.sort();
    let mut instances: Vec<PathBuf> = inis
        .iter()
        .filter_map(|ini| ini.parent().map(Path::to_path_buf))
        .collect();
    // An instance Steam runs but that has not written its settings yet.
    for shortcut in shortcuts.iter().filter(|s| runs_mo2(s)) {
        if let Some(dir) = Path::new(&shortcut.exe).parent()
            && dir.is_dir()
            && !instances.iter().any(|i| i == dir)
        {
            instances.push(dir.to_path_buf());
        }
    }
    instances
}

/// Proton prefixes (`compatdata/<id>/pfx`), most recently used first, with
/// non-Steam shortcuts -- how MO2 is usually run, e.g. as Jackify sets it
/// up -- ahead of games.
pub fn wine_prefixes() -> Vec<Candidate> {
    let known = shortcuts();
    let mut found: Vec<(u8, std::time::SystemTime, Candidate)> = Vec::new();
    for root in steam_roots() {
        let Ok(entries) = std::fs::read_dir(root.join("steamapps/compatdata")) else {
            continue;
        };
        for entry in entries.flatten() {
            let id = entry.file_name().to_string_lossy().into_owned();
            let Ok(number) = id.parse::<u64>() else {
                continue;
            };
            let pfx = entry.path().join("pfx");
            if !pfx.join("drive_c").is_dir() || number == 0 {
                continue;
            }
            let used = std::fs::metadata(entry.path().join("pfx.lock"))
                .or_else(|_| std::fs::metadata(&pfx))
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            // MO2's own prefixes first, then other shortcuts (Steam numbers
            // them above 2^31), then Oblivion's, then the rest.
            let (rank, why) = match known.iter().find(|s| s.app_id == number) {
                Some(s) if runs_mo2(s) => (3, format!("\"{}\": runs {}", s.name, s.exe)),
                Some(s) => (2, format!("\"{}\": runs {}", s.name, s.exe)),
                None if number > (1 << 31) => (2, format!("non-Steam shortcut {id}")),
                None if number == 22330 => (1, "Oblivion's own prefix".to_string()),
                None => (0, format!("Steam app {id}")),
            };
            found.push((rank, used, Candidate { path: pfx, why }));
        }
    }
    found.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    found.into_iter().map(|(_, _, c)| c).collect()
}

/// The Proton a prefix was last run with, from the prefix's `config_info`,
/// then the Proton builds Steam knows about.
pub fn proton_builds(prefix: Option<&Path>) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = Vec::new();
    if let Some(info) = prefix
        .and_then(Path::parent)
        .and_then(|dir| std::fs::read_to_string(dir.join("config_info")).ok())
    {
        // Lines after the first are paths inside the build: `<build>/files/...`.
        let build = info.lines().skip(1).find_map(|line| {
            line.split_once("/files/")
                .map(|(build, _)| PathBuf::from(build))
        });
        if let Some(build) = build
            && build.join("proton").is_file()
        {
            out.push(Candidate {
                path: build.join("proton"),
                why: "the Proton that prefix was last run with".to_string(),
            });
        }
    }
    let mut dirs: Vec<PathBuf> = steam_roots()
        .iter()
        .flat_map(|root| {
            [
                root.join("steamapps/common"),
                root.join("compatibilitytools.d"),
            ]
        })
        .collect();
    dirs.push(PathBuf::from("/usr/share/steam/compatibilitytools.d"));
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut builds: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path().join("proton"))
            .filter(|p| p.is_file())
            .collect();
        builds.sort();
        for proton in builds.into_iter().rev() {
            if !out.iter().any(|c| c.path == proton) {
                out.push(Candidate {
                    path: proton,
                    why: "installed Proton build".to_string(),
                });
            }
        }
    }
    out
}

/// xEdit executables under `roots` and inside `prefix`.
pub fn xedit_exes(roots: &[PathBuf], prefix: Option<&Path>) -> Vec<Candidate> {
    let names = ["TES4Edit.exe", "xEdit.exe", "xEdit64.exe"];
    let mut found = Vec::new();
    for root in roots {
        find_files(root, &names, 5, &mut found);
    }
    if let Some(prefix) = prefix {
        find_files(&prefix.join("drive_c"), &names, 5, &mut found);
    }
    found.sort();
    found.dedup();
    found
        .into_iter()
        .map(|path| Candidate {
            why: "xEdit found here".to_string(),
            path,
        })
        .collect()
}

/// Steam's library folders under each of `steam_roots`: the root itself and
/// every `path` its `libraryfolders.vdf` lists.
fn steam_libraries(steam_roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut libraries: Vec<PathBuf> = Vec::new();
    for root in steam_roots {
        libraries.push(root.clone());
        let text =
            std::fs::read_to_string(root.join("steamapps/libraryfolders.vdf")).unwrap_or_default();
        for line in text.lines() {
            let line = line.trim();
            if !line.starts_with("\"path\"") {
                continue;
            }
            let value = line["\"path\"".len()..].trim().trim_matches('"');
            libraries.push(PathBuf::from(value.replace("\\\\", "\\")));
        }
    }
    let mut seen = Vec::new();
    libraries.retain(|lib| {
        let real = std::fs::canonicalize(lib).unwrap_or_else(|_| lib.clone());
        let new = !seen.contains(&real);
        seen.push(real);
        new
    });
    libraries
}

/// Oblivion's game folder (the one holding `Oblivion.exe`): Steam's install,
/// in any library, then any found under `roots` (a GOG copy, say).
pub fn game_dirs(steam_roots: &[PathBuf], roots: &[PathBuf]) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = Vec::new();
    let mut push = |path: PathBuf, why: String| {
        let real = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if path.join("Oblivion.exe").is_file()
            && !out
                .iter()
                .any(|c| std::fs::canonicalize(&c.path).unwrap_or_else(|_| c.path.clone()) == real)
        {
            out.push(Candidate { path, why });
        }
    };
    for library in steam_libraries(steam_roots) {
        let manifest = std::fs::read_to_string(library.join("steamapps/appmanifest_22330.acf"));
        let Ok(manifest) = manifest else { continue };
        let installdir = manifest
            .lines()
            .map(str::trim)
            .find_map(|line| line.strip_prefix("\"installdir\""))
            .map(|value| value.trim().trim_matches('"').to_string())
            .unwrap_or_else(|| "Oblivion".to_string());
        push(
            library.join("steamapps/common").join(installdir),
            "Steam".to_string(),
        );
    }
    for root in roots {
        let mut exes = Vec::new();
        find_files(root, &["Oblivion.exe"], 4, &mut exes);
        for exe in exes {
            if let Some(dir) = exe.parent() {
                push(dir.to_path_buf(), format!("found under {}", root.display()));
            }
        }
    }
    out
}

/// Every Steam install found on this machine, for [`game_dirs`].
pub fn steam_install_roots() -> Vec<PathBuf> {
    let mut roots = steam_roots();
    if cfg!(windows) {
        for base in ["C:\\Program Files (x86)\\Steam", "C:\\Program Files\\Steam"] {
            let base = PathBuf::from(base);
            if base.join("steamapps").is_dir() {
                roots.push(base);
            }
        }
    }
    roots
}

/// Where to look for games and tools: `~/Games` and `~/Downloads`, as present.
pub fn search_roots() -> Vec<PathBuf> {
    let Some(home) = home() else {
        return Vec::new();
    };
    ["Games", "games", "Downloads"]
        .iter()
        .map(|d| home.join(d))
        .filter(|p| p.is_dir())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_mo2_downloads_folders_and_xedit_under_a_root() {
        let root = tempfile::tempdir().unwrap();
        let instance = root.path().join("Lists/My List");
        std::fs::create_dir_all(instance.join("downloads")).unwrap();
        std::fs::write(instance.join("ModOrganizer.ini"), "[General]\n").unwrap();
        let tools = root.path().join("tools/xEdit 4.1");
        std::fs::create_dir_all(&tools).unwrap();
        std::fs::write(tools.join("TES4Edit.exe"), b"MZ").unwrap();

        let roots = [root.path().to_path_buf()];
        let downloads = downloads_dirs(&roots, &[]);
        assert_eq!(downloads.len(), 1);
        assert_eq!(downloads[0].path, instance.join("downloads"));
        assert!(downloads[0].why.contains("My List"));
        assert_eq!(xedit_exes(&roots, None)[0].path, tools.join("TES4Edit.exe"));
    }

    #[test]
    fn finds_the_game_in_any_steam_library_and_under_a_root() {
        let temp = tempfile::tempdir().unwrap();
        let steam = temp.path().join("Steam");
        let library = temp.path().join("Other Library");
        std::fs::create_dir_all(steam.join("steamapps")).unwrap();
        std::fs::write(
            steam.join("steamapps/libraryfolders.vdf"),
            format!(
                "\"libraryfolders\"\n{{\n\t\"0\"\n\t{{\n\t\t\"path\"\t\t\"{}\"\n\t}}\n}}\n",
                library.display()
            ),
        )
        .unwrap();
        let game = library.join("steamapps/common/Oblivion GOTY");
        std::fs::create_dir_all(&game).unwrap();
        std::fs::write(game.join("Oblivion.exe"), b"MZ").unwrap();
        std::fs::write(
            library.join("steamapps/appmanifest_22330.acf"),
            "\"AppState\"\n{\n\t\"installdir\"\t\t\"Oblivion GOTY\"\n}\n",
        )
        .unwrap();
        let games = temp.path().join("Games");
        let gog = games.join("GOG/Oblivion");
        std::fs::create_dir_all(&gog).unwrap();
        std::fs::write(gog.join("Oblivion.exe"), b"MZ").unwrap();

        let found = game_dirs(&[steam], &[games.clone(), library.clone()]);
        let paths: Vec<_> = found.iter().map(|c| c.path.clone()).collect();
        // The Steam copy once, though the root search also reaches it.
        assert_eq!(paths, vec![game, gog]);
        assert_eq!(found[0].why, "Steam");
    }

    #[test]
    fn reads_steam_shortcuts_names_and_programs() {
        let mut vdf = b"\x00shortcuts\x00\x000\x00".to_vec();
        vdf.extend_from_slice(b"\x02appid\x00");
        vdf.extend_from_slice(&3_598_207_893u32.to_le_bytes());
        vdf.extend_from_slice(b"\x01AppName\x00Mod Organizer 2\x00");
        vdf.extend_from_slice(b"\x01Exe\x00\"/g/Fresh/ModOrganizer.exe\"\x00\x08");
        vdf.extend_from_slice(b"\x001\x00\x02appid\x00");
        vdf.extend_from_slice(&42u32.to_le_bytes());
        vdf.extend_from_slice(b"\x01appname\x00Game\x00\x01exe\x00/g/game.exe\x00\x08\x08");
        let found = parse_shortcuts(&vdf);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].app_id, 3_598_207_893);
        assert_eq!(found[0].name, "Mod Organizer 2");
        assert_eq!(found[0].exe, "/g/Fresh/ModOrganizer.exe");
        assert!(runs_mo2(&found[0]) && !runs_mo2(&found[1]));
    }

    #[test]
    fn a_prefix_names_the_proton_it_was_run_with() {
        let steam = tempfile::tempdir().unwrap();
        let build = steam.path().join("compatibilitytools.d/proton-x");
        std::fs::create_dir_all(build.join("files/lib")).unwrap();
        std::fs::write(build.join("proton"), b"#!/bin/sh").unwrap();
        let app = steam.path().join("compatdata/3620478085");
        std::fs::create_dir_all(app.join("pfx/drive_c")).unwrap();
        std::fs::write(
            app.join("config_info"),
            format!("Proton-X\n{}/files/share/fonts/\n", build.display()),
        )
        .unwrap();
        let found = proton_builds(Some(&app.join("pfx")));
        assert_eq!(found[0].path, build.join("proton"));
    }
}
