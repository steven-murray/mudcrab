//! Just enough DDS to undo DXT1's punch-through alpha.
//!
//! DXT1 (BC1) has a second block mode in which one of the four palette entries
//! is "transparent black". Menu textures use it for the see-through middle of a
//! frame. On the DXVK + NVIDIA stack this list is tested on, Oblivion's menu
//! renderer draws those texels as opaque black instead: DarNified UI's compass
//! frame comes out as a solid black box over the compass face. The same texture
//! decoded to uncompressed 32-bit BGRA, which carries its alpha explicitly,
//! renders correctly.
//!
//! So this decodes DXT1 to uncompressed BGRA. Decoding is exact -- every texel
//! the GPU would have produced is written out as-is -- so there is no quality
//! cost, only size (8x a DXT1 payload). Nothing here encodes.

use std::path::Path;

const HEADER_LEN: usize = 128;
const DDSD_CAPS: u32 = 0x1;
const DDSD_HEIGHT: u32 = 0x2;
const DDSD_WIDTH: u32 = 0x4;
const DDSD_PITCH: u32 = 0x8;
const DDSD_PIXELFORMAT: u32 = 0x1000;
const DDSD_MIPMAPCOUNT: u32 = 0x20000;
const DDPF_ALPHAPIXELS: u32 = 0x1;
const DDPF_FOURCC: u32 = 0x4;
const DDPF_RGB: u32 = 0x40;
const DDSCAPS_COMPLEX: u32 = 0x8;
const DDSCAPS_TEXTURE: u32 = 0x1000;
const DDSCAPS_MIPMAP: u32 = 0x40_0000;

/// A DXT1 texture: its dimensions and every mip level's blocks.
pub struct Dxt1<'a> {
    pub width: u32,
    pub height: u32,
    /// Number of mip levels stored, at least 1.
    pub mip_count: u32,
    blocks: &'a [u8],
}

/// What a file turned out to be, for a caller deciding whether to touch it.
pub enum Kind<'a> {
    Dxt1(Dxt1<'a>),
    /// A valid DDS in some other format, named by FourCC or bit depth.
    Other(String),
}

fn u32_at(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}

/// Blocks across one dimension: DXT works in 4x4 tiles, rounded up.
fn blocks_for(pixels: u32) -> usize {
    pixels.div_ceil(4).max(1) as usize
}

fn level_dims(width: u32, height: u32, level: u32) -> (u32, u32) {
    ((width >> level).max(1), (height >> level).max(1))
}

pub fn parse(data: &[u8]) -> anyhow::Result<Kind<'_>> {
    if data.len() < HEADER_LEN || &data[0..4] != b"DDS " || u32_at(data, 4) != 124 {
        anyhow::bail!("not a DDS file");
    }
    let height = u32_at(data, 12);
    let width = u32_at(data, 16);
    let pf_flags = u32_at(data, 80);
    if pf_flags & DDPF_FOURCC == 0 {
        return Ok(Kind::Other(format!("uncompressed {}-bit", u32_at(data, 88))));
    }
    let fourcc = &data[84..88];
    if fourcc != b"DXT1" {
        return Ok(Kind::Other(String::from_utf8_lossy(fourcc).into_owned()));
    }
    if data[HEADER_LEN..].len() >= 4 && &data[HEADER_LEN..HEADER_LEN + 4] == b"DX10" {
        anyhow::bail!("DX10 extended header is not supported");
    }

    let mip_count = u32_at(data, 28).max(1);
    let needed: usize = (0..mip_count)
        .map(|level| {
            let (w, h) = level_dims(width, height, level);
            blocks_for(w) * blocks_for(h) * 8
        })
        .sum();
    let blocks = &data[HEADER_LEN..];
    if blocks.len() < needed {
        anyhow::bail!(
            "truncated: {width}x{height} with {mip_count} mip level(s) needs {needed} bytes of blocks, file has {}",
            blocks.len()
        );
    }
    Ok(Kind::Dxt1(Dxt1 {
        width,
        height,
        mip_count,
        blocks: &blocks[..needed],
    }))
}

fn rgb565(c: u16) -> [u8; 3] {
    let r = (c >> 11) & 0x1f;
    let g = (c >> 5) & 0x3f;
    let b = c & 0x1f;
    [
        ((r << 3) | (r >> 2)) as u8,
        ((g << 2) | (g >> 4)) as u8,
        ((b << 3) | (b >> 2)) as u8,
    ]
}

/// The four RGBA colours a block's 2-bit indices select from.
fn palette(block: &[u8]) -> [[u8; 4]; 4] {
    let c0 = u16::from_le_bytes([block[0], block[1]]);
    let c1 = u16::from_le_bytes([block[2], block[3]]);
    let (a, b) = (rgb565(c0), rgb565(c1));
    let mix = |wa: u16, wb: u16| -> [u8; 4] {
        let total = wa + wb;
        let ch = |i: usize| ((a[i] as u16 * wa + b[i] as u16 * wb) / total) as u8;
        [ch(0), ch(1), ch(2), 255]
    };
    let opaque = |c: [u8; 3]| [c[0], c[1], c[2], 255];
    if c0 > c1 {
        [opaque(a), opaque(b), mix(2, 1), mix(1, 2)]
    } else {
        // The punch-through mode: index 3 is transparent black.
        [opaque(a), opaque(b), mix(1, 1), [0, 0, 0, 0]]
    }
}

fn each_block(blocks: &[u8]) -> impl Iterator<Item = &[u8]> {
    blocks.chunks_exact(8)
}

impl Dxt1<'_> {
    /// Whether any texel, at any mip level, is transparent -- i.e. whether the
    /// texture depends on the alpha this module exists to make explicit.
    pub fn has_transparency(&self) -> bool {
        // Mip levels below 4x4 have padding texels, but those are never
        // sampled, and a block that is transparent only in its padding is
        // vanishingly rare in practice; counting them errs towards converting.
        each_block(self.blocks).any(|block| {
            let c0 = u16::from_le_bytes([block[0], block[1]]);
            let c1 = u16::from_le_bytes([block[2], block[3]]);
            let indices = u32::from_le_bytes(block[4..8].try_into().unwrap());
            c0 <= c1 && (0..16).any(|i| (indices >> (2 * i)) & 3 == 3)
        })
    }

    /// The whole texture as an uncompressed 32-bit BGRA DDS, mips included.
    pub fn to_bgra_dds(&self) -> Vec<u8> {
        let mut out = header(self.width, self.height, self.mip_count);
        let mut offset = 0usize;
        for level in 0..self.mip_count {
            let (w, h) = level_dims(self.width, self.height, level);
            let (bw, bh) = (blocks_for(w), blocks_for(h));
            let level_blocks = &self.blocks[offset..offset + bw * bh * 8];
            offset += bw * bh * 8;

            let mut pixels = vec![0u8; w as usize * h as usize * 4];
            for (index, block) in each_block(level_blocks).enumerate() {
                let (bx, by) = (index % bw, index / bw);
                let colours = palette(block);
                let indices = u32::from_le_bytes(block[4..8].try_into().unwrap());
                for texel in 0..16 {
                    let x = bx * 4 + texel % 4;
                    let y = by * 4 + texel / 4;
                    if x >= w as usize || y >= h as usize {
                        continue;
                    }
                    let [r, g, b, a] = colours[((indices >> (2 * texel)) & 3) as usize];
                    let at = (y * w as usize + x) * 4;
                    pixels[at..at + 4].copy_from_slice(&[b, g, r, a]);
                }
            }
            out.extend_from_slice(&pixels);
        }
        out
    }
}

fn header(width: u32, height: u32, mip_count: u32) -> Vec<u8> {
    let mut flags = DDSD_CAPS | DDSD_HEIGHT | DDSD_WIDTH | DDSD_PITCH | DDSD_PIXELFORMAT;
    let mut caps = DDSCAPS_TEXTURE;
    if mip_count > 1 {
        flags |= DDSD_MIPMAPCOUNT;
        caps |= DDSCAPS_COMPLEX | DDSCAPS_MIPMAP;
    }
    let fields: [u32; 31] = [
        124,
        flags,
        height,
        width,
        width * 4, // pitch
        0,         // depth
        if mip_count > 1 { mip_count } else { 0 },
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // reserved1[11]
        32,                              // pixel format size
        DDPF_RGB | DDPF_ALPHAPIXELS,
        0,
        32,
        0x00ff_0000, // red
        0x0000_ff00, // green
        0x0000_00ff, // blue
        0xff00_0000, // alpha
        caps,
        0, 0, 0, 0, // caps2..4, reserved2
    ];
    let mut out = Vec::with_capacity(HEADER_LEN);
    out.extend_from_slice(b"DDS ");
    for field in fields {
        out.extend_from_slice(&field.to_le_bytes());
    }
    debug_assert_eq!(out.len(), HEADER_LEN);
    out
}

/// Every `.dds` below `root`, case-insensitively by extension, sorted.
/// MO2-hidden files are skipped: they are not in the virtual file system.
pub fn find_textures(root: &Path) -> anyhow::Result<Vec<std::path::PathBuf>> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let entries = std::fs::read_dir(&dir)
            .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", dir.display()))?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                if !path.to_string_lossy().to_ascii_lowercase().ends_with(".mohidden") {
                    pending.push(path);
                }
            } else if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("dds"))
            {
                found.push(path);
            }
        }
    }
    found.sort();
    Ok(found)
}

/// A 4x4 single-block DXT1 file: fully transparent, or opaque red.
#[cfg(test)]
pub(crate) fn test_dxt1(transparent: bool) -> Vec<u8> {
    let block: [u8; 8] = if transparent {
        [0, 0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]
    } else {
        [0x00, 0xf8, 0x00, 0x00, 0, 0, 0, 0]
    };
    let mut out = header(4, 4, 1);
    out[80..84].copy_from_slice(&DDPF_FOURCC.to_le_bytes());
    out[84..88].copy_from_slice(b"DXT1");
    out.extend_from_slice(&block);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A DXT1 DDS from raw blocks, with `mips` levels.
    fn dxt1(width: u32, height: u32, mips: u32, blocks: &[u8]) -> Vec<u8> {
        let mut out = header(width, height, mips);
        // Turn the BGRA header into a DXT1 one: FOURCC flag and code.
        out[80..84].copy_from_slice(&DDPF_FOURCC.to_le_bytes());
        out[84..88].copy_from_slice(b"DXT1");
        out.extend_from_slice(blocks);
        out
    }

    /// One block, c0 <= c1 (punch-through mode), every index 3.
    const TRANSPARENT: [u8; 8] = [0, 0, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
    /// One block, c0 > c1: pure red c0, every index 0.
    const OPAQUE_RED: [u8; 8] = [0x00, 0xf8, 0x00, 0x00, 0, 0, 0, 0];

    #[test]
    fn decodes_punch_through_texels_to_zero_alpha() {
        let file = dxt1(4, 4, 1, &TRANSPARENT);
        let Kind::Dxt1(texture) = parse(&file).unwrap() else { panic!() };
        assert!(texture.has_transparency());
        let out = texture.to_bgra_dds();
        assert_eq!(out.len(), HEADER_LEN + 4 * 4 * 4);
        assert!(out[HEADER_LEN..].iter().all(|byte| *byte == 0));
        assert!(matches!(parse(&out).unwrap(), Kind::Other(_)));
    }

    #[test]
    fn opaque_texture_reports_no_transparency_and_decodes_bgra() {
        let file = dxt1(4, 4, 1, &OPAQUE_RED);
        let Kind::Dxt1(texture) = parse(&file).unwrap() else { panic!() };
        assert!(!texture.has_transparency());
        let out = texture.to_bgra_dds();
        assert_eq!(&out[HEADER_LEN..HEADER_LEN + 4], &[0, 0, 255, 255]);
    }

    #[test]
    fn decodes_every_mip_level_and_clips_sub_block_levels() {
        // 8x4: two blocks, then 4x2 and 2x1, one (padded) block each.
        let mut blocks = Vec::new();
        for block in [OPAQUE_RED, TRANSPARENT, OPAQUE_RED, OPAQUE_RED] {
            blocks.extend_from_slice(&block);
        }
        let file = dxt1(8, 4, 3, &blocks);
        let Kind::Dxt1(texture) = parse(&file).unwrap() else { panic!() };
        let out = texture.to_bgra_dds();
        assert_eq!(out.len(), HEADER_LEN + (8 * 4 + 4 * 2 + 2) * 4);
        assert_eq!(u32_at(&out, 28), 3);
    }

    #[test]
    fn truncated_file_is_an_error() {
        let file = dxt1(8, 8, 1, &TRANSPARENT);
        assert!(parse(&file).is_err());
    }
}
