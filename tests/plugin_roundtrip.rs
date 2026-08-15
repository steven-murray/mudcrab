//! M2 gate: `write(parse(bytes)) == bytes`.
//!
//! This is the highest-value test in the plugin layer -- it proves the
//! container model is complete before any semantics are built on top. If a
//! field, group type or flag is not modelled, the bytes come back different.

#[path = "support/esp.rs"]
mod esp;

use mudcrab::plugin::{Entry, FormId, GroupType, Plugin};

fn assert_round_trips(label: &str, bytes: &[u8]) -> Plugin {
    let plugin = Plugin::parse(bytes).unwrap_or_else(|err| panic!("{label}: parse failed: {err}"));
    let written = plugin.to_bytes();
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
    plugin
}

#[test]
fn round_trips_a_minimal_plugin() {
    let bytes = esp::plugin(
        &["Oblivion.esm"],
        &[esp::group(
            *b"STAT",
            0,
            &[esp::record(
                b"STAT",
                0x0100_0801,
                0,
                &[
                    esp::field(b"EDID", &esp::zstring("aRock")),
                    esp::field(b"MODL", &esp::zstring("rock.nif")),
                ],
            )],
        )],
    );

    let plugin = assert_round_trips("minimal", &bytes);
    assert_eq!(plugin.masters.len(), 1);
    assert_eq!(plugin.masters.own_mod_index(), 1);
    assert_eq!(plugin.records().count(), 1);
}

#[test]
fn round_trips_nested_cell_groups() {
    // CELL -> children(6) -> persistent(8) / temporary(9), the shape that
    // carries almost every REFR in the corpus.
    let cell_form_id = 0x0100_26B6u32;
    let bytes = esp::plugin(
        &["Oblivion.esm"],
        &[esp::group(
            *b"CELL",
            0,
            &[esp::group(
                0i32.to_le_bytes(),
                2,
                &[esp::group(
                    1i32.to_le_bytes(),
                    3,
                    &[
                        esp::record(
                            b"CELL",
                            cell_form_id,
                            0,
                            &[esp::field(b"EDID", &esp::zstring("TestCell"))],
                        ),
                        esp::group(
                            cell_form_id.to_le_bytes(),
                            6,
                            &[
                                esp::group(
                                    cell_form_id.to_le_bytes(),
                                    8,
                                    &[esp::record(
                                        b"REFR",
                                        0x0100_0802,
                                        esp::FLAG_PERSISTENT,
                                        &[esp::field(b"NAME", &0x0100_0801u32.to_le_bytes())],
                                    )],
                                ),
                                esp::group(
                                    cell_form_id.to_le_bytes(),
                                    9,
                                    &[esp::record(
                                        b"REFR",
                                        0x0100_0803,
                                        0,
                                        &[esp::field(b"NAME", &0x0100_0801u32.to_le_bytes())],
                                    )],
                                ),
                            ],
                        ),
                    ],
                )],
            )],
        )],
    );

    let plugin = assert_round_trips("nested cells", &bytes);
    assert_eq!(plugin.records().count(), 3);
    // 3 records + 6 groups: CELL top-level, block, sub-block, children,
    // persistent and temporary.
    assert_eq!(plugin.record_and_group_count(), 3 + 6);

    // The persistent flag, not the source grouping, is what marks a reference.
    let persistent: Vec<_> = plugin
        .records()
        .filter(|r| &r.signature == b"REFR" && r.is_persistent())
        .map(|r| r.form_id)
        .collect();
    assert_eq!(persistent, vec![FormId(0x0100_0802)]);
}

#[test]
fn round_trips_a_compressed_record() {
    // Re-deflating would produce different bytes, so an untouched compressed
    // record must be written back from its original body.
    let bytes = esp::plugin(
        &["Oblivion.esm"],
        &[esp::group(
            *b"NPC_",
            0,
            &[esp::record(
                b"NPC_",
                0x0100_0801,
                esp::FLAG_COMPRESSED,
                &[
                    esp::field(b"EDID", &esp::zstring("CompressedNpc")),
                    esp::field(b"FGGS", &vec![0x42; 200]),
                ],
            )],
        )],
    );

    let plugin = assert_round_trips("compressed", &bytes);
    let record = plugin.records().next().expect("one record");
    assert!(record.is_compressed());
    // and the decompressed view is still readable
    assert_eq!(record.field(b"FGGS").expect("FGGS").data.len(), 200);
}

#[test]
fn round_trips_an_xxxx_overflow_field() {
    let big = vec![0xCDu8; u16::MAX as usize + 512];
    let bytes = esp::plugin(
        &["Oblivion.esm"],
        &[esp::group(
            *b"LAND",
            0,
            &[esp::record(
                b"LAND",
                0x0100_0801,
                0,
                &[esp::field(b"EDID", &esp::zstring("l")), esp::field(b"VNML", &big)],
            )],
        )],
    );

    let plugin = assert_round_trips("xxxx overflow", &bytes);
    assert_eq!(
        plugin.records().next().unwrap().field(b"VNML").unwrap().data.len(),
        big.len()
    );
}

#[test]
fn round_trips_empty_fields() {
    // FNAM:0, MNAM:0, NAM0:0, ONAM:0 and XMRK:0 all occur in the real corpus.
    let bytes = esp::plugin(
        &["Oblivion.esm"],
        &[esp::group(
            *b"REFR",
            0,
            &[esp::record(
                b"REFR",
                0x0100_0801,
                0,
                &[esp::field(b"XMRK", b""), esp::field(b"FNAM", b"")],
            )],
        )],
    );
    assert_round_trips("empty fields", &bytes);
}

#[test]
fn parses_group_labels_as_form_ids_where_applicable() {
    let cell = 0x0100_26B6u32;
    let bytes = esp::plugin(
        &["Oblivion.esm"],
        &[esp::group(
            *b"CELL",
            0,
            &[esp::group(cell.to_le_bytes(), 6, &[])],
        )],
    );
    let plugin = Plugin::parse(&bytes).expect("parse");

    let Entry::Group(top) = &plugin.entries[0] else {
        panic!("expected a top-level group");
    };
    assert_eq!(top.group_type, GroupType::TopLevel(*b"CELL"));

    let Entry::Group(children) = &top.entries[0] else {
        panic!("expected a nested group");
    };
    assert_eq!(children.group_type.parent_form_id(), Some(FormId(cell)));
}

#[test]
fn own_object_indices_are_ascending_and_deduplicated() {
    // Some real plugins contain duplicate FormIDs; iterating in file order
    // would produce phantom allocations during a merge.
    let bytes = esp::plugin(
        &["Oblivion.esm"],
        &[esp::group(
            *b"STAT",
            0,
            &[
                esp::record(b"STAT", 0x0100_0803, 0, &[]),
                esp::record(b"STAT", 0x0100_0801, 0, &[]),
                esp::record(b"STAT", 0x0100_0803, 0, &[]),
                // a reference into a master, not our own
                esp::record(b"STAT", 0x0000_0014, 0, &[]),
            ],
        )],
    );
    let plugin = Plugin::parse(&bytes).expect("parse");
    let own: Vec<u32> = plugin.own_object_indices().into_iter().collect();
    assert_eq!(own, vec![0x801, 0x803]);
}

#[test]
fn rejects_non_plugin_data() {
    assert!(Plugin::parse(b"not a plugin at all").is_err());
    assert!(Plugin::parse(&[]).is_err());
}

#[test]
fn rejects_a_truncated_plugin() {
    let bytes = esp::plugin(
        &["Oblivion.esm"],
        &[esp::group(
            *b"STAT",
            0,
            &[esp::record(b"STAT", 0x0100_0801, 0, &[esp::field(b"EDID", b"abc\0")])],
        )],
    );
    assert!(Plugin::parse(&bytes[..bytes.len() - 4]).is_err());
}
