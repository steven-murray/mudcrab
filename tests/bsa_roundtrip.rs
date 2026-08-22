//! BSA gate: `write(parse(bytes)) == bytes`.
//!
//! The same gate the plugin layer uses, for the same reason: it proves the
//! container model is complete before anything is built on top. If an offset,
//! a hash, a flag or a payload is not modelled, the bytes come back different.

#[path = "support/bsa.rs"]
mod bsa_support;

use bsa_support::{archive, file, file_with_compression, DataOrder, FLAG_COMPRESSED};
use mudcrab::bsa::Bsa;

fn assert_round_trips<'a>(label: &str, bytes: &'a [u8]) -> Bsa<'a> {
    let parsed = Bsa::parse(bytes).unwrap_or_else(|err| panic!("{label}: parse failed: {err}"));
    let written = parsed
        .to_bytes()
        .unwrap_or_else(|err| panic!("{label}: write failed: {err}"));
    assert_eq!(
        written.len(),
        bytes.len(),
        "{label}: length differs ({} written vs {} original)",
        written.len(),
        bytes.len()
    );
    if written != bytes {
        let at = written
            .iter()
            .zip(bytes)
            .position(|(a, b)| a != b)
            .unwrap_or(0);
        panic!("{label}: bytes differ at offset {at}");
    }
    parsed
}

#[test]
fn round_trips_a_minimal_archive() {
    let bytes = archive(
        0,
        &[(
            "meshes\\rocks",
            vec![file("rock01.nif", b"NIF DATA"), file("rock02.nif", b"MORE")],
        )],
        DataOrder::RecordOrder,
    );

    let parsed = assert_round_trips("minimal", &bytes);
    assert_eq!(parsed.folders.len(), 1);
    assert_eq!(parsed.file_count(), 2);
    assert!(!parsed.compressed_by_default());
}

#[test]
fn round_trips_several_folders() {
    let bytes = archive(
        0,
        &[
            ("textures\\menus\\icons", vec![file("icon.dds", b"DDS ....")]),
            (
                "meshes\\architecture",
                vec![file("wall.nif", b"WALL"), file("door.nif", b"DOOR")],
            ),
            ("sound\\fx", vec![file("thud.wav", b"RIFF....")]),
        ],
        DataOrder::RecordOrder,
    );

    let parsed = assert_round_trips("several folders", &bytes);
    assert_eq!(parsed.folders.len(), 3);
    assert_eq!(parsed.file_count(), 4);

    // Folder records are keyed by hash, so they come back in hash order rather
    // than the order they were declared.
    let mut hashes: Vec<u64> = parsed
        .folders
        .iter()
        .map(|folder| mudcrab::bsa::hash_folder_name(&folder.name))
        .collect();
    let sorted = {
        let mut copy = hashes.clone();
        copy.sort_unstable();
        copy
    };
    hashes.dedup();
    assert_eq!(parsed.folders.len(), hashes.len(), "duplicate folder hashes");
    assert_eq!(
        parsed
            .folders
            .iter()
            .map(|f| mudcrab::bsa::hash_folder_name(&f.name))
            .collect::<Vec<_>>(),
        sorted,
        "folder records must be sorted by hash"
    );
}

#[test]
fn round_trips_a_compressed_archive() {
    // Re-deflating produces a valid but different stream, so an untouched
    // payload has to be written back from the original bytes.
    let payload = b"the quick brown fox ".repeat(64);
    let bytes = archive(
        FLAG_COMPRESSED,
        &[(
            "meshes\\rocks",
            vec![file("rock01.nif", &payload), file("rock02.nif", b"tiny")],
        )],
        DataOrder::RecordOrder,
    );

    let parsed = assert_round_trips("compressed", &bytes);
    assert!(parsed.compressed_by_default());

    // and the decompressed view is still readable
    let (folder, entry) = parsed
        .files()
        .find(|(_, f)| f.name == "rock01.nif")
        .expect("rock01.nif");
    assert!(entry.compressed);
    assert_eq!(
        entry.data(&entry.path_in(folder)).unwrap().as_ref(),
        payload.as_slice()
    );
}

#[test]
fn round_trips_per_file_compression_overrides() {
    // Bounty Quests.bsa is an uncompressed archive in which 427 of 428 files
    // set the compression-differs bit.
    let payload = b"compressible ".repeat(40);
    let bytes = archive(
        0,
        &[(
            "meshes\\creatures",
            vec![
                file_with_compression("goblin.nif", &payload, true),
                file("plain.nif", b"stored as-is"),
            ],
        )],
        DataOrder::RecordOrder,
    );

    let parsed = assert_round_trips("mixed compression", &bytes);
    assert!(!parsed.compressed_by_default());

    let (folder, goblin) = parsed
        .files()
        .find(|(_, f)| f.name == "goblin.nif")
        .expect("goblin.nif");
    assert!(goblin.compressed, "the differs bit must flip compression on");
    assert_eq!(
        goblin.data(&goblin.path_in(folder)).unwrap().as_ref(),
        payload.as_slice()
    );

    let (_, plain) = parsed
        .files()
        .find(|(_, f)| f.name == "plain.nif")
        .expect("plain.nif");
    assert!(!plain.compressed);
}

#[test]
fn round_trips_a_data_region_that_is_not_in_record_order() {
    // Most real archives permute their payloads relative to the file records.
    let bytes = archive(
        0,
        &[(
            "textures\\stone",
            vec![
                file("a.dds", b"AAAAAAAA"),
                file("b.dds", b"BBBB"),
                file("c.dds", b"CCCCCCCCCCCC"),
            ],
        )],
        DataOrder::Reversed,
    );

    let parsed = assert_round_trips("reversed payloads", &bytes);
    for (folder, entry) in parsed.files() {
        let expected = match entry.name.as_str() {
            "a.dds" => b"AAAAAAAA".to_vec(),
            "b.dds" => b"BBBB".to_vec(),
            "c.dds" => b"CCCCCCCCCCCC".to_vec(),
            other => panic!("unexpected file {other}"),
        };
        assert_eq!(
            entry.data(&entry.path_in(folder)).unwrap().as_ref(),
            expected.as_slice(),
            "{} read the wrong payload",
            entry.name
        );
    }
}

#[test]
fn round_trips_deduplicated_payloads() {
    // Many real archives point two file records at the same bytes.
    let bytes = archive(
        0,
        &[(
            "textures\\potions",
            vec![
                file("yellow_n.dds", b"SHARED NORMAL MAP"),
                file("purple_n.dds", b"SHARED NORMAL MAP"),
                file("green_n.dds", b"different"),
            ],
        )],
        DataOrder::Deduplicated,
    );

    let parsed = assert_round_trips("deduplicated payloads", &bytes);
    assert_eq!(parsed.file_count(), 3);
    for (folder, entry) in parsed.files() {
        let expected: &[u8] = if entry.name == "green_n.dds" {
            b"different"
        } else {
            b"SHARED NORMAL MAP"
        };
        assert_eq!(entry.data(&entry.path_in(folder)).unwrap().as_ref(), expected);
    }
}

#[test]
fn round_trips_interstitial_bytes_no_record_points_at() {
    // WACIntegration.bsa prefixes every payload with a redundant u32 length
    // that no file record addresses. Rebuilding the region from the payloads
    // alone would drop those bytes.
    let bytes = archive(
        0,
        &[(
            "textures\\menus",
            vec![file("iconarrow1.dds", b"DDS 1234"), file("iconarrow2.dds", b"DDS 5678")],
        )],
        DataOrder::LengthPrefixed,
    );

    let parsed = assert_round_trips("length-prefixed payloads", &bytes);
    let (folder, first) = parsed
        .files()
        .find(|(_, f)| f.name == "iconarrow1.dds")
        .expect("iconarrow1.dds");
    assert_eq!(
        first.data(&first.path_in(folder)).unwrap().as_ref(),
        b"DDS 1234".as_slice()
    );
}

#[test]
fn round_trips_an_empty_file() {
    let bytes = archive(
        0,
        &[("meshes\\empty", vec![file("nothing.nif", b"")])],
        DataOrder::RecordOrder,
    );
    let parsed = assert_round_trips("empty payload", &bytes);
    assert_eq!(parsed.file_count(), 1);
}

#[test]
fn round_trips_a_deeply_nested_folder() {
    let bytes = archive(
        0,
        &[(
            "sound\\voice\\molapi.esp\\breton\\m",
            vec![file("greeting_00007757_1.mp3", b"ID3 audio")],
        )],
        DataOrder::RecordOrder,
    );

    // The folder name contains a '.', which must not be treated as an
    // extension when hashing.
    let parsed = assert_round_trips("nested voice folder", &bytes);
    assert_eq!(parsed.folders[0].name, "sound\\voice\\molapi.esp\\breton\\m");
}

#[test]
fn paths_join_folder_and_file_with_a_backslash() {
    let bytes = archive(
        0,
        &[("meshes\\rocks", vec![file("rock01.nif", b"x")])],
        DataOrder::RecordOrder,
    );
    let parsed = Bsa::parse(&bytes).expect("parse");
    assert_eq!(
        parsed.paths().collect::<Vec<_>>(),
        vec!["meshes\\rocks\\rock01.nif".to_string()]
    );
}

#[test]
fn extracts_files_to_a_directory() {
    let bytes = archive(
        FLAG_COMPRESSED,
        &[
            ("meshes\\rocks", vec![file("rock01.nif", b"NIF DATA")]),
            (
                "textures\\rocks",
                vec![file("rock01.dds", &b"DDS ".repeat(50))],
            ),
        ],
        DataOrder::RecordOrder,
    );
    let parsed = Bsa::parse(&bytes).expect("parse");

    let dir = tempfile::tempdir().unwrap();
    let written = parsed.extract_to(dir.path()).expect("extract");
    assert_eq!(written, 2);

    assert_eq!(
        std::fs::read(dir.path().join("meshes/rocks/rock01.nif")).unwrap(),
        b"NIF DATA"
    );
    assert_eq!(
        std::fs::read(dir.path().join("textures/rocks/rock01.dds")).unwrap(),
        b"DDS ".repeat(50)
    );
}

#[test]
fn packs_a_directory_into_a_readable_archive() {
    let source = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(source.path().join("meshes/rocks")).unwrap();
    std::fs::create_dir_all(source.path().join("textures/rocks")).unwrap();
    std::fs::write(source.path().join("meshes/rocks/rock01.nif"), b"NIF DATA").unwrap();
    std::fs::write(source.path().join("textures/rocks/rock01.dds"), b"DDS DATA").unwrap();

    let filters = mudcrab::archive::ArchiveFilters::new(&[], &[]).unwrap();
    let built = Bsa::from_directory(source.path(), &filters).expect("pack");
    let bytes = built.to_bytes().expect("write");

    // A freshly built archive must parse, and re-parsing it must round-trip.
    let parsed = assert_round_trips("packed", &bytes);
    let mut paths: Vec<String> = parsed.paths().collect();
    paths.sort();
    assert_eq!(
        paths,
        vec![
            "meshes\\rocks\\rock01.nif".to_string(),
            "textures\\rocks\\rock01.dds".to_string(),
        ]
    );

    let extracted = tempfile::tempdir().unwrap();
    parsed.extract_to(extracted.path()).expect("extract");
    assert_eq!(
        std::fs::read(extracted.path().join("meshes/rocks/rock01.nif")).unwrap(),
        b"NIF DATA"
    );
}

#[test]
fn packing_applies_include_and_exclude_globs() {
    let source = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(source.path().join("meshes")).unwrap();
    std::fs::create_dir_all(source.path().join("textures")).unwrap();
    std::fs::write(source.path().join("meshes/keep.nif"), b"keep").unwrap();
    std::fs::write(source.path().join("textures/skip.dds"), b"skip").unwrap();

    let filters =
        mudcrab::archive::ArchiveFilters::new(&["meshes/**".to_string()], &[]).unwrap();
    let built = Bsa::from_directory(source.path(), &filters).expect("pack");
    assert_eq!(
        built.paths().collect::<Vec<_>>(),
        vec!["meshes\\keep.nif".to_string()]
    );
}

#[test]
fn packing_lowercases_names() {
    let source = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(source.path().join("Meshes/Rocks")).unwrap();
    std::fs::write(source.path().join("Meshes/Rocks/Rock01.NIF"), b"x").unwrap();

    let filters = mudcrab::archive::ArchiveFilters::new(&[], &[]).unwrap();
    let built = Bsa::from_directory(source.path(), &filters).expect("pack");
    assert_eq!(
        built.paths().collect::<Vec<_>>(),
        vec!["meshes\\rocks\\rock01.nif".to_string()]
    );
}

#[test]
fn packing_skips_files_at_the_archive_root() {
    // The format has no way to address a file outside a folder, and a staged
    // mod routinely has a readme or a plugin at its top level.
    let source = tempfile::tempdir().unwrap();
    std::fs::write(source.path().join("loose.txt"), b"x").unwrap();
    std::fs::create_dir_all(source.path().join("meshes")).unwrap();
    std::fs::write(source.path().join("meshes/kept.nif"), b"y").unwrap();

    let filters = mudcrab::archive::ArchiveFilters::new(&[], &[]).unwrap();
    let built = Bsa::from_directory(source.path(), &filters).expect("pack");
    assert_eq!(
        built.paths().collect::<Vec<_>>(),
        vec!["meshes\\kept.nif".to_string()]
    );

    // and the caller can report what stayed loose
    assert_eq!(
        mudcrab::bsa::root_level_files(source.path()).unwrap(),
        vec!["loose.txt".to_string()]
    );
}

#[test]
fn rejects_a_non_bsa() {
    assert!(Bsa::parse(b"not an archive at all").is_err());
    assert!(Bsa::parse(&[]).is_err());
}

#[test]
fn rejects_an_unsupported_version() {
    let mut bytes = archive(
        0,
        &[("meshes", vec![file("a.nif", b"x")])],
        DataOrder::RecordOrder,
    );
    bytes[4..8].copy_from_slice(&104u32.to_le_bytes());

    let err = Bsa::parse(&bytes).unwrap_err();
    let message = err.to_string();
    assert!(message.contains("104") && message.contains("103"), "{message}");
}

#[test]
fn rejects_an_archive_without_names() {
    let mut bytes = archive(
        0,
        &[("meshes", vec![file("a.nif", b"x")])],
        DataOrder::RecordOrder,
    );
    // Clear the file-names bit.
    let flags = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
    bytes[12..16].copy_from_slice(&(flags & !0x2).to_le_bytes());

    let err = Bsa::parse(&bytes).unwrap_err();
    assert!(err.to_string().contains("file"), "{err}");
}

#[test]
fn rejects_a_truncated_archive() {
    let bytes = archive(
        0,
        &[("meshes\\rocks", vec![file("rock01.nif", b"NIF DATA")])],
        DataOrder::RecordOrder,
    );
    assert!(Bsa::parse(&bytes[..bytes.len() - 4]).is_err());
    assert!(Bsa::parse(&bytes[..20]).is_err());
}

/// BSArch stores one copy of a repeated payload and points every record at it.
/// mudcrab used to store one copy per record, which made a packed mod visibly
/// larger than the same mod packed by BSArch while holding exactly the same
/// files -- a difference that reads as wrong content until somebody checks.
#[test]
fn packing_stores_a_repeated_payload_once() {
    let source = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(source.path().join("sound/voice/argonian")).unwrap();
    std::fs::create_dir_all(source.path().join("sound/voice/breton")).unwrap();
    std::fs::create_dir_all(source.path().join("meshes")).unwrap();

    // The village-mod shape: one recording reused across race folders.
    let shared = vec![b'V'; 4096];
    std::fs::write(source.path().join("sound/voice/argonian/line.mp3"), &shared).unwrap();
    std::fs::write(source.path().join("sound/voice/breton/line.mp3"), &shared).unwrap();
    std::fs::write(source.path().join("meshes/unique.nif"), b"NIF").unwrap();

    let filters = mudcrab::archive::ArchiveFilters::new(&[], &[]).unwrap();
    let built = Bsa::from_directory(source.path(), &filters).expect("pack");
    let bytes = built.to_bytes().expect("write");

    let parsed = assert_round_trips("deduplicated", &bytes);
    assert_eq!(parsed.file_count(), 3, "every record is still there");

    // One copy of the shared payload, not two. Two copies alone would exceed
    // 8192 bytes before any metadata.
    assert!(
        bytes.len() < 2 * shared.len(),
        "the repeated payload was stored twice: archive is {} bytes",
        bytes.len()
    );

    // And both records still read back correctly, which is the point: sharing
    // an offset must not cost either file its content.
    let extracted = tempfile::tempdir().unwrap();
    parsed.extract_to(extracted.path()).expect("extract");
    for race in ["argonian", "breton"] {
        assert_eq!(
            std::fs::read(extracted.path().join(format!("sound/voice/{race}/line.mp3"))).unwrap(),
            shared,
            "{race} lost its line"
        );
    }
    assert_eq!(
        std::fs::read(extracted.path().join("meshes/unique.nif")).unwrap(),
        b"NIF"
    );
}

/// All 18 archives Bethesda shipped set bits 8-10, and `0x703` is the plain
/// uncompressed shape among them. mudcrab wrote `0x003`, which not one of the
/// 74 archives in `docs/design/bsa-header-flags.csv` does.
#[test]
fn a_packed_archive_declares_the_flags_every_real_one_does() {
    let source = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(source.path().join("meshes")).unwrap();
    std::fs::write(source.path().join("meshes/a.nif"), b"NIF").unwrap();

    let filters = mudcrab::archive::ArchiveFilters::new(&[], &[]).unwrap();
    let built = Bsa::from_directory(source.path(), &filters).expect("pack");
    assert_eq!(
        built.archive_flags, 0x0000_0703,
        "expected the corpus-wide baseline, got {:#010x}",
        built.archive_flags
    );

    // And it survives the write, since the header ORs its own name bits in.
    let bytes = built.to_bytes().expect("write");
    let parsed = assert_round_trips("conventional flags", &bytes);
    assert_eq!(parsed.archive_flags, 0x0000_0703);
}
