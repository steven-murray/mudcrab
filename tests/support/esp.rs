//! Minimal TES4 plugin builder for tests.
//!
//! Produces real plugin bytes (a few hundred bytes each) so behaviours can be
//! covered without committing binaries or depending on a game install.

#![allow(dead_code)]

pub const FLAG_COMPRESSED: u32 = 0x0004_0000;
pub const FLAG_PERSISTENT: u32 = 0x0000_0400;

pub fn field(sig: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    if data.len() > u16::MAX as usize {
        out.extend_from_slice(b"XXXX");
        out.extend_from_slice(&4u16.to_le_bytes());
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(sig);
        out.extend_from_slice(&0u16.to_le_bytes());
    } else {
        out.extend_from_slice(sig);
        out.extend_from_slice(&(data.len() as u16).to_le_bytes());
    }
    out.extend_from_slice(data);
    out
}

pub fn zstring(text: &str) -> Vec<u8> {
    let mut out = text.as_bytes().to_vec();
    out.push(0);
    out
}

pub fn record(sig: &[u8; 4], form_id: u32, flags: u32, fields: &[Vec<u8>]) -> Vec<u8> {
    let body: Vec<u8> = fields.concat();
    let body = if flags & FLAG_COMPRESSED != 0 {
        use flate2::write::ZlibEncoder;
        use flate2::Compression;
        use std::io::Write;
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&body).unwrap();
        let compressed = encoder.finish().unwrap();
        let mut out = (fields.concat().len() as u32).to_le_bytes().to_vec();
        out.extend_from_slice(&compressed);
        out
    } else {
        body
    };

    let mut out = Vec::new();
    out.extend_from_slice(sig);
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&form_id.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // version control
    out.extend_from_slice(&body);
    out
}

pub fn group(label: [u8; 4], group_type: i32, children: &[Vec<u8>]) -> Vec<u8> {
    let payload: Vec<u8> = children.concat();
    let mut out = Vec::new();
    out.extend_from_slice(b"GRUP");
    // A GRUP's size includes its own 20-byte header.
    out.extend_from_slice(&((payload.len() + 20) as u32).to_le_bytes());
    out.extend_from_slice(&label);
    out.extend_from_slice(&group_type.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // stamp
    out.extend_from_slice(&payload);
    out
}

/// A TES4 header record with the given masters.
pub fn header(masters: &[&str]) -> Vec<u8> {
    let mut fields = vec![field(b"HEDR", &{
        let mut hedr = 1.0f32.to_le_bytes().to_vec();
        hedr.extend_from_slice(&0u32.to_le_bytes()); // num records
        hedr.extend_from_slice(&0x800u32.to_le_bytes()); // next object id
        hedr
    })];
    fields.push(field(b"CNAM", &zstring("mudcrab-test")));
    for master in masters {
        fields.push(field(b"MAST", &zstring(master)));
        fields.push(field(b"DATA", &0u64.to_le_bytes()));
    }
    record(b"TES4", 0, 0, &fields)
}

/// Assemble a whole plugin.
pub fn plugin(masters: &[&str], top_level: &[Vec<u8>]) -> Vec<u8> {
    let mut out = header(masters);
    for chunk in top_level {
        out.extend_from_slice(chunk);
    }
    out
}
