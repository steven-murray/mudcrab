//! The TES4 field table.
//!
//! One entry per `(record, field)` pair, sorted, binary-searched. Deliberately
//! a plain table rather than a macro DSL: it has to be greppable and reviewable
//! against xEdit's `wbDefinitionsTES4.pas`, where `wbFormIDCk`/`wbFormID` marks
//! a FormID field and `wbInteger`/`wbFloat` marks one that only looks like one.
//!
//! Scope is demand-driven. It currently covers the 21 record types the
//! "Unique Forts Merged" merge needs; anything else hard-errors with a message
//! saying how to extend it. `tests/fixtures/plugin/subrecord_matrix.txt` is the
//! completeness target for full MOFAM coverage.
//!
//! Note how the same field signature means different things in different
//! records -- `DOOR/ANAM` is a sound FormID, `WEAP/ANAM` is a u16 enchantment
//! point count. That is why the key is the pair, never the field alone.

use super::{CustomKind, FieldKind};

use FieldKind::{FormId, FormIdArray, FormIdPrefix, Opaque, Struct, ZString};

const CONDITION: FieldKind = FieldKind::Custom(CustomKind::Condition);
const PACKAGE_LOCATION: FieldKind = FieldKind::Custom(CustomKind::PackageLocation);
const PACKAGE_TARGET: FieldKind = FieldKind::Custom(CustomKind::PackageTarget);

/// A leading FormID followed by a fixed number of trailing bytes.
const FORM_ID_THEN_8: FieldKind = Struct {
    sizes: &[8],
    form_id_offsets: &[0],
};
const FORM_ID_THEN_28: FieldKind = Struct {
    sizes: &[28],
    form_id_offsets: &[0],
};
/// Fixed-size structs containing no FormIDs.
const PLAIN_8: FieldKind = Struct {
    sizes: &[8],
    form_id_offsets: &[],
};
const PLAIN_24: FieldKind = Struct {
    sizes: &[24],
    form_id_offsets: &[],
};

/// MUST stay sorted by `(record, field)`; a test enforces it.
static FIELDS: &[([u8; 4], [u8; 4], FieldKind)] = &[
    // ---- ACHR: placed NPC ----
    (*b"ACHR", *b"DATA", PLAIN_24), // position + rotation floats
    (*b"ACHR", *b"EDID", ZString),
    (*b"ACHR", *b"NAME", FormId), // base NPC_
    (*b"ACHR", *b"XESP", FORM_ID_THEN_8), // enable parent + flags
    (*b"ACHR", *b"XHRS", FormId), // horse reference
    (*b"ACHR", *b"XMRC", FormId), // merchant container
    (*b"ACHR", *b"XRGD", Opaque), // ragdoll data
    // ---- ACRE: placed creature ----
    (*b"ACHR", *b"XSCL", Opaque), // f32 scale -- NOT a FormID
    (*b"ACRE", *b"DATA", PLAIN_24),
    (*b"ACRE", *b"EDID", ZString),
    (*b"ACRE", *b"NAME", FormId), // base CREA
    (*b"ACRE", *b"XESP", FORM_ID_THEN_8),
    (*b"ACRE", *b"XOWN", FormId),
    (*b"ACRE", *b"XRGD", Opaque),
    // ---- ACTI ----
    (*b"ACRE", *b"XSCL", Opaque), // f32 scale -- NOT a FormID
    (*b"ACTI", *b"EDID", ZString),
    (*b"ACTI", *b"FULL", ZString),
    (*b"ACTI", *b"MODB", Opaque), // f32 bound radius
    (*b"ACTI", *b"MODL", ZString),
    (*b"ACTI", *b"MODT", Opaque), // model texture hashes
    (*b"ACTI", *b"SCRI", FormId),
    // ---- BOOK ----
    (*b"ACTI", *b"SNAM", FormId), // looping sound
    (*b"BOOK", *b"ANAM", Opaque), // u16 enchantment points -- NOT DOOR/ANAM
    (*b"BOOK", *b"DATA", Opaque), // flags, teaches, value, weight
    (*b"BOOK", *b"DESC", ZString),
    (*b"BOOK", *b"EDID", ZString),
    (*b"BOOK", *b"ENAM", FormId), // enchantment
    (*b"BOOK", *b"FULL", ZString),
    (*b"BOOK", *b"ICON", ZString),
    (*b"BOOK", *b"MODB", Opaque),
    (*b"BOOK", *b"MODL", ZString),
    (*b"BOOK", *b"MODT", Opaque),
    (*b"BOOK", *b"SCRI", FormId),
    // ---- CELL ----
    (*b"CELL", *b"DATA", Opaque), // u8 flags
    (*b"CELL", *b"EDID", ZString),
    (*b"CELL", *b"FULL", ZString),
    (*b"CELL", *b"XCCM", FormId), // climate -- distinct from XCMT, the u8 music type
    (*b"CELL", *b"XCLC", PLAIN_8), // grid x,y
    (*b"CELL", *b"XCLL", Opaque),      // lighting
    (*b"CELL", *b"XCLR", FormIdArray), // regions
    (*b"CELL", *b"XCLW", Opaque),      // f32 water height
    (*b"CELL", *b"XCMT", Opaque),      // u8 music type
    (*b"CELL", *b"XCWT", FormId),      // water
    (*b"CELL", *b"XGLB", FormId), // global
    (*b"CELL", *b"XOWN", FormId),
    (*b"CELL", *b"XRNK", Opaque), // i32 faction rank
    // ---- CONT ----
    (*b"CONT", *b"CNTO", FORM_ID_THEN_8), // item + u32 count
    (*b"CONT", *b"DATA", Opaque),
    (*b"CONT", *b"EDID", ZString),
    (*b"CONT", *b"FULL", ZString),
    (*b"CONT", *b"MODB", Opaque),
    (*b"CONT", *b"MODL", ZString),
    (*b"CONT", *b"MODT", Opaque),
    (*b"CONT", *b"QNAM", FormId), // close sound
    (*b"CONT", *b"SCRI", FormId),
    (*b"CONT", *b"SNAM", FormId), // open sound
    // ---- DOOR ----
    (*b"DOOR", *b"ANAM", FormId), // loop sound -- NOT the same as WEAP/ANAM
    (*b"DOOR", *b"BNAM", FormId), // close sound
    (*b"DOOR", *b"EDID", ZString),
    (*b"DOOR", *b"FNAM", Opaque), // u8 flags
    (*b"DOOR", *b"FULL", ZString),
    (*b"DOOR", *b"MODB", Opaque),
    (*b"DOOR", *b"MODL", ZString),
    (*b"DOOR", *b"MODT", Opaque),
    (*b"DOOR", *b"SCRI", FormId),
    (*b"DOOR", *b"SNAM", FormId), // open sound
    // ---- ENCH ----
    (*b"DOOR", *b"TNAM", FormId), // random teleport destination
    (*b"ENCH", *b"EDID", ZString),
    // EFID/EFIT begin with a 4-byte MGEF *code* (a FourCC such as "REHE"),
    // not a FormID. Rewriting it would corrupt the effect.
    (*b"ENCH", *b"EFID", Opaque),
    (*b"ENCH", *b"EFIT", Opaque),
    (*b"ENCH", *b"ENIT", Opaque),
    (*b"ENCH", *b"FULL", ZString),
    // ---- FACT ----
    (*b"FACT", *b"CNAM", Opaque), // f32 crime gold multiplier
    (*b"FACT", *b"DATA", Opaque),
    (*b"FACT", *b"EDID", ZString),
    (*b"FACT", *b"FNAM", ZString), // female rank name
    (*b"FACT", *b"FULL", ZString),
    (*b"FACT", *b"MNAM", ZString), // male rank name
    (*b"FACT", *b"RNAM", Opaque),  // i32 rank number
    (*b"FACT", *b"XNAM", FORM_ID_THEN_8), // faction + i32 modifier
    // ---- KEYM ----
    (*b"KEYM", *b"DATA", Opaque),
    (*b"KEYM", *b"EDID", ZString),
    (*b"KEYM", *b"FULL", ZString),
    (*b"KEYM", *b"ICON", ZString),
    (*b"KEYM", *b"MODB", Opaque),
    (*b"KEYM", *b"MODL", ZString),
    (*b"KEYM", *b"MODT", Opaque),
    (*b"KEYM", *b"SCRI", FormId),
    // ---- LAND ----
    (*b"LAND", *b"ATXT", FORM_ID_THEN_8), // LTEX + quadrant/layer
    (*b"LAND", *b"BTXT", FORM_ID_THEN_8),
    (*b"LAND", *b"DATA", Opaque),
    (*b"LAND", *b"VCLR", Opaque),
    (*b"LAND", *b"VHGT", Opaque),
    (*b"LAND", *b"VNML", Opaque),
    (*b"LAND", *b"VTEX", FormIdArray), // LTEX references
    (*b"LAND", *b"VTXT", Opaque),
    // ---- NPC_ ----
    (*b"NPC_", *b"ACBS", Opaque), // configuration
    (*b"NPC_", *b"AIDT", Opaque), // AI data
    (*b"NPC_", *b"CNAM", FormId), // class
    (*b"NPC_", *b"CNTO", FORM_ID_THEN_8), // inventory item + count
    (*b"NPC_", *b"DATA", Opaque), // skills/attributes
    (*b"NPC_", *b"EDID", ZString),
    (*b"NPC_", *b"ENAM", FormId), // eyes
    (*b"NPC_", *b"FGGA", Opaque), // facegen geometry, asymmetric
    (*b"NPC_", *b"FGGS", Opaque), // facegen geometry, symmetric
    (*b"NPC_", *b"FGTS", Opaque), // facegen texture
    (*b"NPC_", *b"FNAM", Opaque),
    (*b"NPC_", *b"FULL", ZString),
    (*b"NPC_", *b"HCLR", Opaque), // hair COLOUR (rgba) -- NOT a FormID
    (*b"NPC_", *b"HNAM", FormId), // hair
    (*b"NPC_", *b"INAM", FormId), // death item
    (*b"NPC_", *b"KFFZ", Opaque), // animation filenames
    (*b"NPC_", *b"LNAM", Opaque), // hair LENGTH (f32) -- NOT a FormID
    (*b"NPC_", *b"MODB", Opaque),
    (*b"NPC_", *b"MODL", ZString),
    (*b"NPC_", *b"PKID", FormId), // AI package
    (*b"NPC_", *b"RNAM", FormId), // race
    (*b"NPC_", *b"SCRI", FormId),
    (*b"NPC_", *b"SNAM", FORM_ID_THEN_8), // faction + u8 rank
    (*b"NPC_", *b"SPLO", FormId),          // spell
    (*b"NPC_", *b"ZNAM", FormId),          // combat style
    // ---- PACK ----
    (*b"PACK", *b"CTDA", CONDITION),
    (*b"PACK", *b"EDID", ZString),
    (*b"PACK", *b"PKDT", Opaque),
    (*b"PACK", *b"PLDT", PACKAGE_LOCATION),
    (*b"PACK", *b"PSDT", Opaque),
    (*b"PACK", *b"PTDT", PACKAGE_TARGET),
    // ---- PGRD: path grid ----
    (*b"PGRD", *b"DATA", Opaque),
    (*b"PGRD", *b"PGAG", Opaque),
    (*b"PGRD", *b"PGRI", Opaque), // inter-cell point links: indices + floats
    (*b"PGRD", *b"PGRL", FormIdPrefix), // REFR FormID + point indices
    (*b"PGRD", *b"PGRP", Opaque), // point array
    (*b"PGRD", *b"PGRR", Opaque), // point-to-point connections
    // ---- QUST ----
    (*b"QUST", *b"CNAM", ZString), // log entry text
    (*b"QUST", *b"CTDA", CONDITION),
    (*b"QUST", *b"DATA", Opaque),
    (*b"QUST", *b"EDID", ZString),
    (*b"QUST", *b"FULL", ZString),
    (*b"QUST", *b"ICON", ZString),
    (*b"QUST", *b"INDX", Opaque), // i16 stage index
    (*b"QUST", *b"QSDT", Opaque),
    (*b"QUST", *b"QSTA", FORM_ID_THEN_8), // target + flags
    // Compiled script bytecode. Verified across the whole install that it
    // references forms by index into SCRO, never inline, so it needs no
    // rewriting -- see MOFAM-test/notes/merge-recon.md.
    (*b"QUST", *b"SCDA", Opaque),
    (*b"QUST", *b"SCHR", Opaque),
    (*b"QUST", *b"SCRI", FormId),
    (*b"QUST", *b"SCRO", FormId), // script-referenced object
    (*b"QUST", *b"SCTX", ZString),
    // ---- REFR ----
    (*b"REFR", *b"DATA", PLAIN_24),
    (*b"REFR", *b"EDID", ZString),
    (*b"REFR", *b"FNAM", Opaque),
    (*b"REFR", *b"FULL", ZString),
    (*b"REFR", *b"NAME", FormId), // base object
    (*b"REFR", *b"ONAM", Opaque), // open by default
    (*b"REFR", *b"TNAM", Opaque), // map marker type
    (*b"REFR", *b"XACT", Opaque), // action flags
    (*b"REFR", *b"XCHG", Opaque), // f32 charge -- NOT a FormID
    (*b"REFR", *b"XCNT", Opaque), // i32 count -- NOT a FormID
    (*b"REFR", *b"XESP", FORM_ID_THEN_8),
    (*b"REFR", *b"XGLB", FormId), // global
    (*b"REFR", *b"XHLT", Opaque), // i32 health -- NOT a FormID
    (*b"REFR", *b"XLCM", Opaque), // i32 level modifier -- NOT a FormID
    (*b"REFR", *b"XLOC", Struct {
        sizes: &[12],
        form_id_offsets: &[4], // lock level, then the key FormID
    }),
    (*b"REFR", *b"XLOD", Opaque),
    (*b"REFR", *b"XMRK", Opaque), // zero-length map marker flag
    (*b"REFR", *b"XOWN", FormId),
    (*b"REFR", *b"XRNK", Opaque), // i32 faction rank
    (*b"REFR", *b"XSCL", Opaque), // f32 scale -- NOT a FormID
    (*b"REFR", *b"XSED", Opaque), // SpeedTree seed
    (*b"REFR", *b"XTEL", FORM_ID_THEN_28), // destination door + 24B transform
    // ---- SCPT ----
    (*b"REFR", *b"XTRG", FormId), // target reference
    (*b"SCPT", *b"EDID", ZString),
    (*b"SCPT", *b"SCDA", Opaque), // see QUST/SCDA
    (*b"SCPT", *b"SCHR", Opaque),
    (*b"SCPT", *b"SCRO", FormId), // referenced object -- rewritten in place
    (*b"SCPT", *b"SCRV", Opaque), // local VARIABLE INDEX -- NOT a FormID
    (*b"SCPT", *b"SCTX", ZString),
    (*b"SCPT", *b"SCVR", ZString),
    (*b"SCPT", *b"SLSD", Opaque),
    // ---- SOUN ----
    (*b"SOUN", *b"EDID", ZString),
    (*b"SOUN", *b"FNAM", ZString), // sound filename
    (*b"SOUN", *b"SNDX", Opaque),
    // ---- STAT ----
    (*b"STAT", *b"EDID", ZString),
    (*b"STAT", *b"MODB", Opaque),
    (*b"STAT", *b"MODL", ZString),
    // ---- TES4: the file header ----
    (*b"STAT", *b"MODT", Opaque),
    (*b"TES4", *b"CNAM", ZString), // author
    (*b"TES4", *b"DATA", Opaque),  // master file size
    (*b"TES4", *b"DELE", Opaque),
    (*b"TES4", *b"HEDR", Opaque),
    (*b"TES4", *b"MAST", ZString),
    (*b"TES4", *b"OFST", Opaque),
    (*b"TES4", *b"SNAM", ZString), // description
    // ---- WEAP ----
    (*b"WEAP", *b"ANAM", Opaque), // u16 enchantment points -- NOT DOOR/ANAM
    (*b"WEAP", *b"DATA", Opaque),
    (*b"WEAP", *b"EDID", ZString),
    (*b"WEAP", *b"ENAM", FormId), // enchantment
    (*b"WEAP", *b"FULL", ZString),
    (*b"WEAP", *b"ICON", ZString),
    (*b"WEAP", *b"MODB", Opaque),
    (*b"WEAP", *b"MODL", ZString),
    (*b"WEAP", *b"MODT", Opaque),
    (*b"WEAP", *b"SCRI", FormId),
    // ---- WRLD ----
    (*b"WRLD", *b"CNAM", FormId), // climate
    (*b"WRLD", *b"DATA", Opaque),
    (*b"WRLD", *b"EDID", ZString),
    (*b"WRLD", *b"FULL", ZString),
    (*b"WRLD", *b"ICON", ZString),
    (*b"WRLD", *b"MNAM", Opaque), // map data
    (*b"WRLD", *b"NAM0", Opaque), // min object bounds
    (*b"WRLD", *b"NAM2", FormId), // water
    (*b"WRLD", *b"NAM9", Opaque), // max object bounds
    (*b"WRLD", *b"SNAM", FormId), // water (alternate)
    (*b"WRLD", *b"WNAM", FormId), // parent worldspace
];

pub fn lookup(record: [u8; 4], field: [u8; 4]) -> Option<FieldKind> {
    FIELDS
        .binary_search_by(|(r, f, _)| (r, f).cmp(&(&record, &field)))
        .ok()
        .map(|position| FIELDS[position].2)
}

/// Whether the table describes this record type at all.
///
/// Distinguishes "unknown record" from "known record, unknown field", which
/// changes what the user has to go and look up.
pub fn knows_record(record: [u8; 4]) -> bool {
    FIELDS.iter().any(|(r, _, _)| *r == record)
}

/// Record signatures currently covered.
pub fn known_records() -> Vec<[u8; 4]> {
    let mut out: Vec<[u8; 4]> = FIELDS.iter().map(|(r, _, _)| *r).collect();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted_and_has_no_duplicates() {
        // lookup() binary-searches, so this is a correctness requirement.
        for pair in FIELDS.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            assert!(
                (a.0, a.1) < (b.0, b.1),
                "table out of order at {}/{} before {}/{}",
                String::from_utf8_lossy(&a.0),
                String::from_utf8_lossy(&a.1),
                String::from_utf8_lossy(&b.0),
                String::from_utf8_lossy(&b.1),
            );
        }
    }

    #[test]
    fn struct_offsets_fit_inside_every_declared_size() {
        for (record, field, kind) in FIELDS {
            if let Struct {
                sizes,
                form_id_offsets,
            } = kind
            {
                for size in *sizes {
                    for offset in *form_id_offsets {
                        assert!(
                            offset + 4 <= *size,
                            "{}/{}: FormID at offset {offset} does not fit in {size} bytes",
                            String::from_utf8_lossy(record),
                            String::from_utf8_lossy(field),
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn same_field_signature_can_differ_between_records() {
        // The reason the key is the pair: a sound FormID vs an integer.
        assert_eq!(lookup(*b"DOOR", *b"ANAM"), Some(FormId));
        assert_eq!(lookup(*b"WEAP", *b"ANAM"), Some(Opaque));
    }

    #[test]
    fn covers_every_record_type_unique_forts_uses() {
        let required: &[&[u8; 4]] = &[
            b"REFR", b"CELL", b"PGRD", b"PACK", b"ACHR", b"NPC_", b"SCPT", b"WRLD", b"LAND",
            b"QUST", b"ACRE", b"ACTI", b"FACT", b"STAT", b"BOOK", b"KEYM", b"CONT", b"DOOR",
            b"ENCH", b"WEAP", b"SOUN",
        ];
        for record in required {
            assert!(
                knows_record(**record),
                "{} is used by Unique Forts but missing from the table",
                String::from_utf8_lossy(*record)
            );
        }
    }

    #[test]
    fn unknown_pairs_return_none() {
        assert!(lookup(*b"ZZZZ", *b"EDID").is_none());
        assert!(lookup(*b"STAT", *b"ZZZZ").is_none());
    }
}
