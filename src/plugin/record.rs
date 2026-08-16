//! Records and subrecords.

use super::formid::FormId;
use super::PluginError;

/// Record flag: the record body is zlib-compressed.
pub const FLAG_COMPRESSED: u32 = 0x0004_0000;
/// Record flag: the record is a deleted override.
pub const FLAG_DELETED: u32 = 0x0000_0020;
/// REFR/ACHR/ACRE flag: the reference is persistent.
///
/// This, not the source file's grouping, decides whether a reference belongs in
/// a cell's persistent (GRUP 8) or temporary (GRUP 9) children.
pub const FLAG_PERSISTENT: u32 = 0x0000_0400;

/// One field inside a record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subrecord {
    pub signature: [u8; 4],
    pub data: Vec<u8>,
}

impl Subrecord {
    pub fn new(signature: &[u8; 4], data: Vec<u8>) -> Self {
        Subrecord {
            signature: *signature,
            data,
        }
    }

    pub fn sig_str(&self) -> String {
        String::from_utf8_lossy(&self.signature).into_owned()
    }
}

/// A single record.
///
/// `original_body` holds the exact on-disk bytes (still compressed if the
/// record was). While the fields are untouched it is written back verbatim,
/// which is what makes round-trips byte-identical: re-deflating would produce
/// a different byte stream than the tool that wrote the file, even though the
/// decompressed content is identical.
#[derive(Debug, Clone)]
pub struct Record {
    pub signature: [u8; 4],
    pub flags: u32,
    pub form_id: FormId,
    /// Version-control info from the header. Preserved, never interpreted.
    pub version_control: u32,
    fields: Vec<Subrecord>,
    original_body: Option<Vec<u8>>,
}

impl Record {
    pub fn new(signature: &[u8; 4], form_id: FormId, fields: Vec<Subrecord>) -> Self {
        Record {
            signature: *signature,
            flags: 0,
            form_id,
            version_control: 0,
            fields,
            original_body: None,
        }
    }

    pub(super) fn from_parts(
        signature: [u8; 4],
        flags: u32,
        form_id: FormId,
        version_control: u32,
        fields: Vec<Subrecord>,
        original_body: Vec<u8>,
    ) -> Self {
        Record {
            signature,
            flags,
            form_id,
            version_control,
            fields,
            original_body: Some(original_body),
        }
    }

    pub fn sig_str(&self) -> String {
        String::from_utf8_lossy(&self.signature).into_owned()
    }

    pub fn is_compressed(&self) -> bool {
        self.flags & FLAG_COMPRESSED != 0
    }

    pub fn is_deleted(&self) -> bool {
        self.flags & FLAG_DELETED != 0
    }

    pub fn is_persistent(&self) -> bool {
        self.flags & FLAG_PERSISTENT != 0
    }

    pub fn fields(&self) -> &[Subrecord] {
        &self.fields
    }

    /// Mutable access to the fields.
    ///
    /// Drops the cached on-disk bytes: once the caller can change a field, the
    /// original is no longer a faithful representation and the record must be
    /// re-serialised on write.
    pub fn fields_mut(&mut self) -> &mut Vec<Subrecord> {
        self.original_body = None;
        &mut self.fields
    }

    pub fn field(&self, signature: &[u8; 4]) -> Option<&Subrecord> {
        self.fields.iter().find(|f| &f.signature == signature)
    }

    pub fn fields_with(&self, signature: &[u8; 4]) -> impl Iterator<Item = &Subrecord> {
        self.fields.iter().filter(move |f| &f.signature == signature)
    }

    pub(super) fn original_body(&self) -> Option<&[u8]> {
        self.original_body.as_deref()
    }
}

/// Split a decompressed record body into subrecords.
///
/// Handles the `XXXX` overflow convention: a field larger than `u16::MAX` is
/// preceded by an `XXXX` field carrying the real `u32` length, and its own size
/// field is set to zero.
pub(super) fn parse_fields(body: &[u8], record_sig: [u8; 4]) -> Result<Vec<Subrecord>, PluginError> {
    let mut fields = Vec::new();
    let mut offset = 0usize;
    let mut pending_size: Option<usize> = None;

    while offset < body.len() {
        if offset + 6 > body.len() {
            return Err(PluginError::TruncatedField {
                record: record_sig,
                offset,
            });
        }

        let signature: [u8; 4] = body[offset..offset + 4].try_into().unwrap();
        let declared = u16::from_le_bytes([body[offset + 4], body[offset + 5]]) as usize;
        offset += 6;

        if &signature == b"XXXX" {
            if declared != 4 || offset + 4 > body.len() {
                return Err(PluginError::MalformedOverflowField {
                    record: record_sig,
                    offset,
                });
            }
            pending_size = Some(u32::from_le_bytes(
                body[offset..offset + 4].try_into().unwrap(),
            ) as usize);
            offset += declared;
            continue;
        }

        let size = pending_size.take().unwrap_or(declared);
        if offset + size > body.len() {
            return Err(PluginError::TruncatedField {
                record: record_sig,
                offset,
            });
        }

        fields.push(Subrecord {
            signature,
            data: body[offset..offset + size].to_vec(),
        });
        offset += size;
    }

    Ok(fields)
}

/// Serialise subrecords back into a record body, re-emitting `XXXX` overflow
/// headers for any field too large for a `u16` length.
pub(super) fn write_fields(fields: &[Subrecord]) -> Vec<u8> {
    let mut out = Vec::new();
    for field in fields {
        if field.data.len() > u16::MAX as usize {
            out.extend_from_slice(b"XXXX");
            out.extend_from_slice(&4u16.to_le_bytes());
            out.extend_from_slice(&(field.data.len() as u32).to_le_bytes());
            out.extend_from_slice(&field.signature);
            out.extend_from_slice(&0u16.to_le_bytes());
        } else {
            out.extend_from_slice(&field.signature);
            out.extend_from_slice(&(field.data.len() as u16).to_le_bytes());
        }
        out.extend_from_slice(&field.data);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(sig: &[u8; 4], data: &[u8]) -> Subrecord {
        Subrecord::new(sig, data.to_vec())
    }

    #[test]
    fn parses_and_writes_ordinary_fields() {
        let fields = vec![field(b"EDID", b"aRock\0"), field(b"NAME", &[1, 2, 3, 4])];
        let body = write_fields(&fields);
        assert_eq!(parse_fields(&body, *b"STAT").unwrap(), fields);
    }

    #[test]
    fn round_trips_a_field_too_large_for_u16() {
        let big = vec![0xABu8; u16::MAX as usize + 100];
        let fields = vec![field(b"EDID", b"x\0"), field(b"VNML", &big)];
        let body = write_fields(&fields);
        // XXXX header must precede the oversized field
        assert_eq!(&body[8..12], b"XXXX");
        assert_eq!(parse_fields(&body, *b"LAND").unwrap(), fields);
    }

    #[test]
    fn accepts_zero_length_fields() {
        // FNAM:0, MNAM:0, NAM0:0, ONAM:0 and XMRK:0 all occur in the corpus.
        let fields = vec![field(b"XMRK", b""), field(b"EDID", b"a\0")];
        let body = write_fields(&fields);
        assert_eq!(parse_fields(&body, *b"REFR").unwrap(), fields);
    }

    #[test]
    fn rejects_a_truncated_field() {
        let mut body = write_fields(&[field(b"EDID", b"hello\0")]);
        body.truncate(body.len() - 2);
        assert!(parse_fields(&body, *b"STAT").is_err());
    }

    #[test]
    fn mutating_fields_invalidates_the_cached_bytes() {
        let mut record = Record::from_parts(
            *b"STAT",
            0,
            FormId(0x0100_0801),
            0,
            vec![field(b"EDID", b"a\0")],
            vec![1, 2, 3],
        );
        assert!(record.original_body().is_some());
        record.fields_mut().push(field(b"MODL", b"x.nif\0"));
        assert!(
            record.original_body().is_none(),
            "cached bytes must be dropped once fields can change"
        );
    }
}
