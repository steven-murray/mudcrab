//! Which fields contain FormIDs, and where inside them.
//!
//! This is the safety-critical part of the merge. A naive "rewrite every
//! 4-byte field" heuristic silently corrupts plugins: `NPC_/HCLR` is a hair
//! *colour*, `NPC_/LNAM` a hair *length*, `REFR/XSCL` a scale, `REFR/XCNT` a
//! count and `SCPT/SCRV` a local-variable index -- all four bytes, none of them
//! FormIDs.
//!
//! The table is therefore a closed world. An unknown record signature, an
//! unknown field inside a known record, or a struct whose declared size
//! disagrees with the payload is an **error**, never a pass-through. There is
//! no fallback branch: the only way to obtain a [`FieldKind`] is from a
//! `Result`, so "unknown" cannot silently become "copy verbatim".
//!
//! Extending it is meant to be routine -- the error names the exact
//! `(record, field)` pair and where to look it up.

pub mod ctda;
pub mod tes4;

use super::formid::FormId;
use super::record::Record;

/// How a field's bytes should be treated when rewriting FormIDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    /// Contains no FormIDs. Copied verbatim.
    Opaque,
    /// Text. No FormIDs.
    ZString,
    /// The whole payload is exactly one FormID.
    FormId,
    /// A packed array of FormIDs; length must be a multiple of 4.
    FormIdArray,
    /// Fixed-layout struct. `sizes` lists every payload length seen in the
    /// wild; anything else is an error rather than a guess.
    Struct {
        sizes: &'static [usize],
        form_id_offsets: &'static [usize],
    },
    /// A repeating struct: the payload is N of them back to back, with the
    /// FormIDs at the same offsets within each. A payload that is not a whole
    /// multiple of `stride` is an error, not a truncated last element.
    StructArray {
        stride: usize,
        form_id_offsets: &'static [usize],
    },
    /// One FormID followed by an opaque, variable-length payload.
    FormIdPrefix,
    /// Needs bespoke logic; see [`CustomKind`].
    Custom(CustomKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomKind {
    /// CTDA: whether the parameters are FormIDs depends on the function index.
    Condition,
    /// PACK/PLDT: `location` is a FormID only for location types 0 and 1.
    PackageLocation,
    /// PACK/PTDT: `target` is a FormID only for target types 0 and 1.
    PackageTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaError {
    UnknownRecord {
        record: [u8; 4],
        form_id: FormId,
    },
    UnknownField {
        record: [u8; 4],
        field: [u8; 4],
        form_id: FormId,
        size: usize,
    },
    FieldSizeMismatch {
        record: [u8; 4],
        field: [u8; 4],
        form_id: FormId,
        expected: &'static [usize],
        actual: usize,
    },
    UnknownConditionFunction {
        record: [u8; 4],
        form_id: FormId,
        function: u32,
    },
}

impl SchemaError {
    /// A short key for deduplicating gaps in a whole-corpus audit.
    pub fn gap_key(&self) -> String {
        match self {
            SchemaError::UnknownRecord { record, .. } => sig(record),
            SchemaError::UnknownField { record, field, .. }
            | SchemaError::FieldSizeMismatch { record, field, .. } => {
                format!("{}/{}", sig(record), sig(field))
            }
            SchemaError::UnknownConditionFunction { function, .. } => {
                format!("CTDA function {function}")
            }
        }
    }
}

impl std::fmt::Display for SchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SchemaError::UnknownRecord { record, form_id } => write!(
                f,
                "record type {} (first seen at {form_id}) is not in the TES4 schema.\n\
                 Add it to src/plugin/schema/tes4.rs. Determine each field from:\n  \
                 https://en.uesp.net/wiki/Oblivion_Mod:Mod_File_Format/{}\n  \
                 xEdit wbDefinitionsTES4.pas, search for wbRecord({}",
                sig(record),
                sig(record),
                sig(record)
            ),
            SchemaError::UnknownField {
                record,
                field,
                form_id,
                size,
            } => write!(
                f,
                "field {}/{} ({size} bytes, first seen at {form_id}) is not in the TES4 schema.\n\
                 Add it to src/plugin/schema/tes4.rs. In xEdit's wbDefinitionsTES4.pas a\n\
                 wbFormIDCk/wbFormID entry means it holds a FormID; wbInteger/wbFloat means\n\
                 it does not.",
                sig(record),
                sig(field)
            ),
            SchemaError::FieldSizeMismatch {
                record,
                field,
                form_id,
                expected,
                actual,
            } => write!(
                f,
                "field {}/{} at {form_id} is {actual} bytes, but the schema only knows {expected:?}.\n\
                 Refusing to rewrite it: a wrong layout would corrupt neighbouring data.",
                sig(record),
                sig(field)
            ),
            SchemaError::UnknownConditionFunction {
                record,
                form_id,
                function,
            } => write!(
                f,
                "CTDA in {} at {form_id} uses condition function {function}, which is not in\n\
                 src/plugin/schema/ctda.rs. Whether its parameters are FormIDs decides whether\n\
                 they must be rewritten, so guessing is unsafe.",
                sig(record)
            ),
        }
    }
}

impl std::error::Error for SchemaError {}

fn sig(value: &[u8; 4]) -> String {
    String::from_utf8_lossy(value).into_owned()
}

/// Byte offsets of every FormID inside a field's payload.
///
/// Returns an error rather than an empty list when the field is not described,
/// so an unmodelled field can never be silently treated as FormID-free.
pub fn form_id_offsets(
    record_sig: [u8; 4],
    field_sig: [u8; 4],
    data: &[u8],
    form_id: FormId,
) -> Result<Vec<usize>, SchemaError> {
    let kind = tes4::lookup(record_sig, field_sig).ok_or_else(|| {
        if tes4::knows_record(record_sig) {
            SchemaError::UnknownField {
                record: record_sig,
                field: field_sig,
                form_id,
                size: data.len(),
            }
        } else {
            SchemaError::UnknownRecord {
                record: record_sig,
                form_id,
            }
        }
    })?;

    Ok(match kind {
        FieldKind::Opaque | FieldKind::ZString => Vec::new(),

        FieldKind::FormId => {
            if data.len() != 4 {
                return Err(SchemaError::FieldSizeMismatch {
                    record: record_sig,
                    field: field_sig,
                    form_id,
                    expected: &[4],
                    actual: data.len(),
                });
            }
            vec![0]
        }

        FieldKind::FormIdArray => {
            if !data.len().is_multiple_of(4) {
                return Err(SchemaError::FieldSizeMismatch {
                    record: record_sig,
                    field: field_sig,
                    form_id,
                    expected: &[4],
                    actual: data.len(),
                });
            }
            (0..data.len()).step_by(4).collect()
        }

        FieldKind::Struct {
            sizes,
            form_id_offsets,
        } => {
            if !sizes.contains(&data.len()) {
                return Err(SchemaError::FieldSizeMismatch {
                    record: record_sig,
                    field: field_sig,
                    form_id,
                    expected: sizes,
                    actual: data.len(),
                });
            }
            form_id_offsets
                .iter()
                .copied()
                .filter(|offset| offset + 4 <= data.len())
                .collect()
        }

        FieldKind::StructArray {
            stride,
            form_id_offsets,
        } => {
            if stride == 0 || !data.len().is_multiple_of(stride) {
                return Err(SchemaError::FieldSizeMismatch {
                    record: record_sig,
                    field: field_sig,
                    form_id,
                    expected: &[],
                    actual: data.len(),
                });
            }
            (0..data.len())
                .step_by(stride)
                .flat_map(|base| form_id_offsets.iter().map(move |offset| base + offset))
                .filter(|offset| offset + 4 <= data.len())
                .collect()
        }

        FieldKind::FormIdPrefix => {
            if data.len() < 4 {
                return Err(SchemaError::FieldSizeMismatch {
                    record: record_sig,
                    field: field_sig,
                    form_id,
                    expected: &[4],
                    actual: data.len(),
                });
            }
            vec![0]
        }

        FieldKind::Custom(CustomKind::Condition) => {
            ctda::form_id_offsets(record_sig, data, form_id)?
        }
        FieldKind::Custom(CustomKind::PackageLocation) => {
            package_offsets(data, &[0, 1], 4)
        }
        FieldKind::Custom(CustomKind::PackageTarget) => package_offsets(data, &[0, 1], 4),
    })
}

/// PLDT/PTDT: a leading i32 discriminant decides whether the next u32 is a FormID.
fn package_offsets(data: &[u8], form_id_types: &[i32], offset: usize) -> Vec<usize> {
    if data.len() < offset + 4 {
        return Vec::new();
    }
    let discriminant = i32::from_le_bytes(data[0..4].try_into().unwrap());
    if form_id_types.contains(&discriminant) {
        vec![offset]
    } else {
        Vec::new()
    }
}

/// Rewrite every FormID in `record` through `f`.
pub fn map_form_ids(
    record: &mut Record,
    mut f: impl FnMut(FormId) -> FormId,
) -> Result<(), SchemaError> {
    let record_sig = record.signature;
    let form_id = record.form_id;

    for field in record.fields_mut() {
        let offsets = form_id_offsets(record_sig, field.signature, &field.data, form_id)?;
        for offset in offsets {
            let old = FormId(u32::from_le_bytes(
                field.data[offset..offset + 4].try_into().unwrap(),
            ));
            let new = f(old);
            if new != old {
                field.data[offset..offset + 4].copy_from_slice(&new.0.to_le_bytes());
            }
        }
    }

    Ok(())
}

/// Read every FormID in `record` without modifying it.
pub fn visit_form_ids(
    record: &Record,
    mut f: impl FnMut(FormId),
) -> Result<(), SchemaError> {
    for field in record.fields() {
        for offset in form_id_offsets(
            record.signature,
            field.signature,
            &field.data,
            record.form_id,
        )? {
            f(FormId(u32::from_le_bytes(
                field.data[offset..offset + 4].try_into().unwrap(),
            )));
        }
    }
    Ok(())
}

/// Report every schema gap in `record` instead of stopping at the first.
///
/// Used by `plugin-audit` to turn "is the table finished?" into a worklist.
pub fn audit(record: &Record) -> Vec<SchemaError> {
    if !tes4::knows_record(record.signature) {
        return vec![SchemaError::UnknownRecord {
            record: record.signature,
            form_id: record.form_id,
        }];
    }

    record
        .fields()
        .iter()
        .filter_map(|field| {
            form_id_offsets(
                record.signature,
                field.signature,
                &field.data,
                record.form_id,
            )
            .err()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::record::Subrecord;

    fn record_with(sig: &[u8; 4], fields: Vec<Subrecord>) -> Record {
        Record::new(sig, FormId(0x0100_0801), fields)
    }

    #[test]
    fn unknown_record_type_is_an_error_naming_where_to_look() {
        let record = record_with(b"ZZZZ", vec![Subrecord::new(b"EDID", b"x\0".to_vec())]);
        let errors = audit(&record);
        assert_eq!(errors.len(), 1);
        let text = errors[0].to_string();
        assert!(text.contains("ZZZZ"), "{text}");
        assert!(text.contains("uesp.net"), "{text}");
    }

    #[test]
    fn unknown_field_in_a_known_record_is_an_error() {
        let record = record_with(b"STAT", vec![Subrecord::new(b"ZZZZ", vec![0; 4])]);
        let errors = audit(&record);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].gap_key(), "STAT/ZZZZ");
    }

    #[test]
    fn wrong_struct_size_is_an_error_rather_than_a_guess() {
        // XTEL is 28 bytes: a 32-byte payload means the table is wrong, and
        // rewriting at the assumed offset would scribble on the destination.
        let record = record_with(b"REFR", vec![Subrecord::new(b"XTEL", vec![0; 32])]);
        let errors = audit(&record);
        assert_eq!(errors.len(), 1);
        assert!(matches!(
            errors[0],
            SchemaError::FieldSizeMismatch { actual: 32, .. }
        ));
    }

    #[test]
    fn four_byte_fields_that_are_not_form_ids_stay_untouched() {
        // The exact reason the table exists.
        let traps: &[(&[u8; 4], &[u8; 4])] = &[
            (b"NPC_", b"HCLR"),
            (b"NPC_", b"LNAM"),
            (b"REFR", b"XSCL"),
            (b"REFR", b"XLCM"),
            (b"SCPT", b"SCRV"),
        ];
        for (rec, field) in traps {
            let offsets =
                form_id_offsets(**rec, **field, &[0xAA; 4], FormId(0x0100_0801)).unwrap();
            assert!(
                offsets.is_empty(),
                "{}/{} must not be treated as a FormID",
                String::from_utf8_lossy(*rec),
                String::from_utf8_lossy(*field)
            );
        }
    }

    #[test]
    fn known_form_id_fields_are_found() {
        let cases: &[(&[u8; 4], &[u8; 4], usize)] = &[
            (b"REFR", b"NAME", 0),
            (b"REFR", b"XOWN", 0),
            (b"NPC_", b"HNAM", 0),
            (b"NPC_", b"ENAM", 0),
            (b"SCPT", b"SCRO", 0),
        ];
        for (rec, field, at) in cases {
            let offsets =
                form_id_offsets(**rec, **field, &[0xAA; 4], FormId(0x0100_0801)).unwrap();
            assert_eq!(offsets, vec![*at], "{}", String::from_utf8_lossy(*field));
        }
    }

    #[test]
    fn struct_fields_report_their_internal_offsets() {
        // XTEL: door FormID then 24 bytes of destination data.
        assert_eq!(
            form_id_offsets(*b"REFR", *b"XTEL", &[0; 28], FormId(0)).unwrap(),
            vec![0]
        );
        // XLOC: lock level, then the key FormID at offset 4.
        assert_eq!(
            form_id_offsets(*b"REFR", *b"XLOC", &[0; 12], FormId(0)).unwrap(),
            vec![4]
        );
        // CNTO: item FormID then a u32 count.
        assert_eq!(
            form_id_offsets(*b"NPC_", *b"CNTO", &[0; 8], FormId(0)).unwrap(),
            vec![0]
        );
    }

    #[test]
    fn form_id_arrays_yield_every_element() {
        // CELL/XCLR is a packed list of region FormIDs.
        assert_eq!(
            form_id_offsets(*b"CELL", *b"XCLR", &[0; 12], FormId(0)).unwrap(),
            vec![0, 4, 8]
        );
        assert!(form_id_offsets(*b"CELL", *b"XCLR", &[0; 10], FormId(0)).is_err());
    }

    #[test]
    fn form_id_prefix_fields_only_expose_the_leading_form_id() {
        // PGRD/PGRL: a REFR FormID followed by point indices.
        for len in [8usize, 24, 48] {
            assert_eq!(
                form_id_offsets(*b"PGRD", *b"PGRL", &vec![0; len], FormId(0)).unwrap(),
                vec![0]
            );
        }
    }

    #[test]
    fn map_form_ids_rewrites_only_the_described_positions() {
        let mut record = record_with(
            b"REFR",
            vec![
                Subrecord::new(b"NAME", 0x0100_0801u32.to_le_bytes().to_vec()),
                Subrecord::new(b"XSCL", 0x3F80_0000u32.to_le_bytes().to_vec()), // 1.0f
            ],
        );
        map_form_ids(&mut record, |id| FormId(id.0 + 0x0100_0000)).unwrap();

        assert_eq!(
            record.field(b"NAME").unwrap().data,
            0x0200_0801u32.to_le_bytes()
        );
        assert_eq!(
            record.field(b"XSCL").unwrap().data,
            0x3F80_0000u32.to_le_bytes(),
            "the scale float must be untouched"
        );
    }
}
