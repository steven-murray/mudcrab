//! One list installed into two profiles of one MO2 instance, with different
//! answers: a mod both install alike is one folder both use, and one they
//! install differently gets a folder each. Neither install changes a folder
//! the other profile uses.

use assert_cmd::Command;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::tempdir;
use zip::write::FileOptions;

fn mudcrab() -> Command {
    let mut command = Command::cargo_bin("mudcrab").unwrap();
    let state = std::env::temp_dir().join("mudcrab-test-state");
    command
        .env("XDG_STATE_HOME", &state)
        .env("LOCALAPPDATA", &state);
    command
}

fn zip(path: &Path, files: &[(&str, &str)]) {
    let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
    for (name, text) in files {
        zip.start_file(*name, FileOptions::<'_, ()>::default())
            .unwrap();
        zip.write_all(text.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
}

struct Setup {
    dir: tempfile::TempDir,
    compiled: PathBuf,
}

impl Setup {
    fn new() -> Self {
        let dir = tempdir().unwrap();
        let d = dir.path();
        std::fs::create_dir_all(d.join("game")).unwrap();
        std::fs::write(d.join("game/Oblivion.ini"), "[General]\n").unwrap();
        std::fs::create_dir_all(d.join("downloads")).unwrap();
        zip(
            &d.join("downloads/shared.zip"),
            &[("Data/shared.txt", "same for all")],
        );
        zip(&d.join("downloads/core.zip"), &[("Data/core.txt", "core")]);
        zip(
            &d.join("downloads/big.zip"),
            &[("Data/big.txt", "the big textures")],
        );
        // `core` takes its second archive only with the big answer, so the
        // two answers install it differently; `shared` is the same either way.
        let list = d.join("list.toml");
        std::fs::write(
            &list,
            "name = \"Profiles\"\n\n[inputs.big]\nquery = \"Big?\"\ntype = \"bool\"\ndefault = false\n\n\
             [[mods]]\nid = \"shared\"\narchives = [{ path = \"manual:shared.zip\" }]\n\n\
             [[mods]]\nid = \"core\"\narchives = [\n    { path = \"manual:core.zip\" },\n    \
             { path = \"manual:big.zip\", if = \"big\" },\n]\n\n\
             [[manual-steps]]\ntitle = \"Build the Bashed Patch\"\nsteps = \"Run Wrye Bash.\"\n\
             done_when_plugin = \"Bashed Patch, 0.esp\"\n",
        )
        .unwrap();
        let compiled = d.join("compiled.json");
        mudcrab()
            .arg("compile")
            .arg(&list)
            .arg("-o")
            .arg(&compiled)
            .assert()
            .success();
        Self { dir, compiled }
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.dir.path().join(rel)
    }

    /// Install into `profile` with `big` answered so.
    fn install(&self, profile: &str, big: bool) {
        let plan = self.path(&format!("plan-{profile}-{big}.json"));
        mudcrab()
            .arg("query")
            .arg(&self.compiled)
            .args(["--headless", "--set", &format!("big={big}"), "-o"])
            .arg(&plan)
            .assert()
            .success();
        mudcrab()
            .arg("install")
            .arg(&plan)
            .arg("--downloads")
            .arg(self.path("downloads"))
            .arg("--mo2-instance-dir")
            .arg(self.path("mo2"))
            .args(["--profile-name", profile])
            .arg("--game-dir")
            .arg(self.path("game"))
            .assert()
            .success();
    }

    fn modlist(&self, profile: &str) -> Vec<String> {
        let mut lines: Vec<String> =
            std::fs::read_to_string(self.path(&format!("mo2/profiles/{profile}/modlist.txt")))
                .unwrap()
                .lines()
                .map(str::to_string)
                .collect();
        lines.sort();
        lines
    }

    fn has(&self, rel: &str) -> bool {
        self.path("mo2/mods").join(rel).exists()
    }
}

#[test]
fn a_mod_installed_differently_gets_a_folder_per_profile() {
    let s = Setup::new();
    s.install("Lean", false);
    s.install("Big", true);

    // One folder for the mod both install alike.
    assert!(s.has("shared/shared.txt"));
    assert!(!s.has("shared [Big]"));
    // The lean profile's core is as it was; the big one has its own.
    assert!(s.has("core/core.txt") && !s.has("core/big.txt"));
    assert!(s.has("core [Big]/core.txt") && s.has("core [Big]/big.txt"));
    assert_eq!(s.modlist("Lean"), ["+core", "+shared"]);
    assert_eq!(s.modlist("Big"), ["+core [Big]", "+shared"]);

    // Installing each again changes nothing of the other's.
    s.install("Lean", false);
    assert!(!s.has("core/big.txt"));
    s.install("Big", true);
    assert!(s.has("core [Big]/big.txt"));
    assert_eq!(s.modlist("Big"), ["+core [Big]", "+shared"]);
}

#[test]
fn a_profile_whose_answers_come_to_match_shares_the_folder() {
    let s = Setup::new();
    s.install("Lean", false);
    s.install("Big", true);
    // Big changes its mind: its core is now exactly the lean profile's.
    s.install("Big", false);
    assert_eq!(s.modlist("Big"), ["+core", "+shared"]);
    assert!(s.has("core/core.txt") && !s.has("core/big.txt"));
}

#[test]
fn a_profile_changing_a_shared_mod_moves_to_its_own_folder() {
    let s = Setup::new();
    s.install("Lean", false);
    s.install("Other", false);
    assert_eq!(s.modlist("Other"), ["+core", "+shared"]);
    // Other now wants the big core; the lean profile still uses `core`.
    s.install("Other", true);
    assert!(s.has("core/core.txt") && !s.has("core/big.txt"));
    assert!(s.has("core [Other]/big.txt"));
    assert_eq!(s.modlist("Other"), ["+core [Other]", "+shared"]);
    assert_eq!(s.modlist("Lean"), ["+core", "+shared"]);
}

impl Setup {
    /// `steps` for `profile`, and what it printed.
    fn steps(&self, profile: &str) -> String {
        let plan = self.path(&format!("plan-{profile}-false.json"));
        let out = mudcrab()
            .arg("steps")
            .arg(&plan)
            .arg("--mo2-instance-dir")
            .arg(self.path("mo2"))
            .args(["--profile-name", profile])
            .arg("--game-dir")
            .arg(self.path("game"))
            .output()
            .unwrap();
        assert!(out.status.success());
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        )
    }
}

#[test]
fn a_bashed_patch_in_overwrite_moves_into_the_profiles_own_mod() {
    let s = Setup::new();
    s.install("Lean", false);
    s.install("Other", false);
    // Wrye Bash, run from MO2 in the lean profile, writes into overwrite/.
    let overwrite = s.path("mo2/overwrite");
    std::fs::create_dir_all(overwrite.join("Docs")).unwrap();
    std::fs::write(overwrite.join("Bashed Patch, 0.esp"), "TES4 patch").unwrap();
    std::fs::write(overwrite.join("Docs/Bashed Patch, 0.txt"), "what went in").unwrap();

    let said = s.steps("Lean");
    assert!(said.contains("Moved"), "{said}");
    assert!(said.contains("1 of 1 step(s) done"), "{said}");
    assert!(!overwrite.join("Bashed Patch, 0.esp").exists());
    assert!(s.has("Bashed Patch, 0 [Lean]/Bashed Patch, 0.esp"));
    assert!(s.has("Bashed Patch, 0 [Lean]/Docs/Bashed Patch, 0.txt"));
    let first = |profile: &str| {
        std::fs::read_to_string(s.path(&format!("mo2/profiles/{profile}/modlist.txt")))
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .to_string()
    };
    assert_eq!(first("Lean"), "+Bashed Patch, 0 [Lean]");

    // An install keeps it in the mod list, first.
    s.install("Lean", false);
    assert_eq!(first("Lean"), "+Bashed Patch, 0 [Lean]");
    // It is the lean profile's patch, not the other's.
    assert!(s.steps("Other").contains("0 of 1 step(s) done"));
    assert_ne!(first("Other"), "+Bashed Patch, 0 [Lean]");
}
