//! M8 gate: the out-of-scope detectors refuse, and refuse for the right reason.
//!
//! `merge::audit` encodes three scoping decisions that M0 established by
//! measurement. Tests that only checked "the six real merges still pass" would
//! pass just as happily if the detectors were stubbed out. These drive them
//! from the other side: construct the counter-example each assumption was
//! predicated on not existing, and require a refusal.
//!
//! The negative controls matter as much as the positives. The SCDA scan is
//! only useful because it does *not* fire on the 396 coincidental byte matches
//! the recon sweep found across the real corpus.

#[path = "support/esp.rs"]
mod esp;

use mudcrab::merge::{self, MergeRequest, MergeSource};
use mudcrab::plugin::PluginName;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};

/// A plugin with one script: `refs` become SCRO entries, `bytecode` the SCDA.
fn scripted_plugin(form_id: u32, refs: &[u32], bytecode: &[u8]) -> Vec<u8> {
    let mut fields = vec![
        esp::field(b"EDID", &esp::zstring("TestScript")),
        esp::field(b"SCHR", &[0u8; 20]),
        esp::field(b"SCDA", bytecode),
    ];
    for reference in refs {
        fields.push(esp::field(b"SCRO", &reference.to_le_bytes()));
    }

    esp::plugin(
        &["Oblivion.esm"],
        &[esp::group(
            *b"SCPT",
            0,
            &[esp::record(b"SCPT", form_id, 0, &fields)],
        )],
    )
}

/// A plugin with a single STAT, used to force a FormID collision.
fn plain_plugin(form_id: u32) -> Vec<u8> {
    esp::plugin(
        &["Oblivion.esm"],
        &[esp::group(
            *b"STAT",
            0,
            &[esp::record(
                b"STAT",
                form_id,
                0,
                &[esp::field(b"EDID", &esp::zstring("Thing"))],
            )],
        )],
    )
}

struct Fixture {
    _dir: TempDir,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempdir().expect("tempdir");
        let root = dir.path().to_path_buf();
        Fixture { _dir: dir, root }
    }

    /// Write `bytes` as `<mod>/<name>` and return the path.
    fn install(&self, mod_name: &str, name: &str, bytes: &[u8]) -> PathBuf {
        let dir = self.root.join(mod_name);
        std::fs::create_dir_all(&dir).expect("mod dir");
        let path = dir.join(name);
        std::fs::write(&path, bytes).expect("write plugin");
        path
    }

    fn merge(&self, sources: &[(&str, &Path)]) -> Result<(), merge::MergeError> {
        let request = MergeRequest {
            name: "Audit Test".to_string(),
            output: "Audit Test.esp".to_string(),
            sources: sources
                .iter()
                .map(|(name, path)| MergeSource {
                    plugin: PluginName::new(*name),
                    path: path.to_path_buf(),
                })
                .collect(),
            load_order: vec![PluginName::new("Oblivion.esm")],
        };
        merge::run(&request).map(|_| ())
    }
}

#[test]
fn a_script_embedding_a_renumbered_form_id_is_refused() {
    // Alpha and Beta both claim object index 0x801, so Beta's is renumbered to
    // 0x802. Beta's script names its own 0x01000801 in SCRO *and* carries that
    // FormID 4-byte aligned in its bytecode. Renumbering SCRO in place would
    // leave the bytecode pointing at Alpha's record instead.
    let fixture = Fixture::new();
    let alpha = fixture.install("Alpha", "Alpha.esp", &plain_plugin(0x0100_0801));

    let mut bytecode = vec![0u8; 8];
    bytecode[4..8].copy_from_slice(&0x0100_0801u32.to_le_bytes());
    let beta = fixture.install(
        "Beta",
        "Beta.esp",
        &scripted_plugin(0x0100_0801, &[0x0100_0801], &bytecode),
    );

    let err = fixture
        .merge(&[("Alpha.esp", &alpha), ("Beta.esp", &beta)])
        .expect_err("the embedded FormID must be refused")
        .to_string();

    assert!(err.contains("compiled script bytecode"), "{err}");
    assert!(err.contains("byte offset 4"), "the offset locates it: {err}");
    assert!(
        err.contains("merge-recon.md"),
        "the refusal must cite the evidence it overturns: {err}"
    );
}

#[test]
fn an_unchanged_form_id_in_bytecode_is_not_refused() {
    // The false-positive case the recon sweep measured: `0x00000014` is both
    // the Player's FormID and the integer 20, and appears in bytecode all over
    // the corpus. It lives in Oblivion.esm, which is master index 0 before and
    // after the merge, so it never changes and cannot be misdirected.
    let fixture = Fixture::new();
    let mut bytecode = vec![0u8; 8];
    bytecode[0..4].copy_from_slice(&0x0000_0014u32.to_le_bytes());
    let alpha = fixture.install(
        "Alpha",
        "Alpha.esp",
        &scripted_plugin(0x0100_0801, &[0x0000_0014], &bytecode),
    );

    fixture
        .merge(&[("Alpha.esp", &alpha)])
        .expect("an unchanged FormID in bytecode is harmless");
}

#[test]
fn an_unaligned_byte_match_is_not_refused() {
    // Same changed FormID, but straddling a word boundary, so it is a byte
    // coincidence rather than an operand. 312 of the 432 corpus hits were
    // unaligned like this.
    let fixture = Fixture::new();
    let alpha = fixture.install("Alpha", "Alpha.esp", &plain_plugin(0x0100_0801));

    let mut bytecode = vec![0u8; 12];
    bytecode[5..9].copy_from_slice(&0x0100_0801u32.to_le_bytes());
    let beta = fixture.install(
        "Beta",
        "Beta.esp",
        &scripted_plugin(0x0100_0801, &[0x0100_0801], &bytecode),
    );

    fixture
        .merge(&[("Alpha.esp", &alpha), ("Beta.esp", &beta)])
        .expect("an unaligned coincidence is not an operand");
}

#[test]
fn voice_data_belonging_to_a_merged_plugin_is_refused() {
    // Voice files live under Sound/Voice/<plugin>.esp/, keyed by plugin name
    // and INFO FormID. Merging changes both, so the lookup breaks.
    let fixture = Fixture::new();
    let alpha = fixture.install("Alpha", "Alpha.esp", &plain_plugin(0x0100_0801));
    std::fs::create_dir_all(fixture.root.join("Alpha/Sound/Voice/Alpha.esp/GREETING"))
        .expect("voice tree");

    let err = fixture
        .merge(&[("Alpha.esp", &alpha)])
        .expect_err("voice data must be refused")
        .to_string();

    assert!(err.contains("voice data"), "{err}");
    assert!(
        err.contains("exclude this plugin from the merge"),
        "the refusal must say what to do about it: {err}"
    );
}

#[test]
fn a_merge_with_none_of_it_succeeds() {
    // The control: nothing in this fixture trips a detector, so a plain merge
    // must still run. Without this the tests above would pass if `run` simply
    // always failed.
    let fixture = Fixture::new();
    let alpha = fixture.install("Alpha", "Alpha.esp", &plain_plugin(0x0100_0801));
    let beta = fixture.install("Beta", "Beta.esp", &plain_plugin(0x0100_0801));

    fixture
        .merge(&[("Alpha.esp", &alpha), ("Beta.esp", &beta)])
        .expect("an ordinary merge must still succeed");
}
