//! The 4GB patch: marking `Oblivion.exe` large-address-aware, so the 32-bit
//! game can use 4 GB of memory rather than 2 under a 64-bit Windows (or
//! Wine). A big list runs out of memory without it.
//!
//! This is what the [4GB Patch](https://ntcore.com/4gb-patch/) tool does, and
//! all it does: set `IMAGE_FILE_LARGE_ADDRESS_AWARE` (0x0020) in the PE
//! header's Characteristics, and recompute the header checksum to match.
//! Nothing else in the file changes. The original is kept beside it the
//! first time, and an exe already marked (GOG's is) is left alone.

use std::path::{Path, PathBuf};

const LARGE_ADDRESS_AWARE: u16 = 0x0020;

/// What patching did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Patched {
    /// The flag was set; the original is at this path.
    Now(PathBuf),
    /// It already was.
    Already,
}

/// Where the PE header's fields are.
struct Header {
    /// Characteristics, in the COFF header.
    characteristics: usize,
    /// CheckSum, in the optional header.
    checksum: usize,
}

fn header(bytes: &[u8]) -> anyhow::Result<Header> {
    anyhow::ensure!(
        bytes.starts_with(b"MZ"),
        "not a Windows program (no MZ header)"
    );
    let at = |offset: usize| -> anyhow::Result<u32> {
        bytes
            .get(offset..offset + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or_else(|| anyhow::anyhow!("the file ends inside its header"))
    };
    let pe = at(0x3C)? as usize;
    anyhow::ensure!(
        bytes.get(pe..pe + 4) == Some(b"PE\0\0"),
        "not a Windows program (no PE header)"
    );
    // The COFF header follows the signature: Machine, NumberOfSections,
    // TimeDateStamp, PointerToSymbolTable, NumberOfSymbols,
    // SizeOfOptionalHeader, then Characteristics. The optional header follows
    // it; its CheckSum is 64 bytes in, for PE32 and PE32+ alike.
    let characteristics = pe + 4 + 18;
    let checksum = pe + 4 + 20 + 64;
    anyhow::ensure!(
        checksum + 4 <= bytes.len(),
        "the file ends inside its header"
    );
    Ok(Header {
        characteristics,
        checksum,
    })
}

/// The PE checksum, as `CheckSumMappedFile` computes it: 16-bit words summed
/// with their carries folded back in, skipping the checksum field, plus the
/// file's length.
fn pe_checksum(bytes: &[u8], checksum_at: usize) -> u32 {
    let mut sum: u64 = 0;
    for (index, chunk) in bytes.chunks(2).enumerate() {
        let offset = index * 2;
        if offset == checksum_at || offset == checksum_at + 2 {
            continue;
        }
        let word = u16::from_le_bytes([chunk[0], *chunk.get(1).unwrap_or(&0)]);
        sum += u64::from(word);
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    sum = (sum & 0xFFFF) + (sum >> 16);
    (sum as u32).wrapping_add(bytes.len() as u32)
}

/// Mark `exe` large-address-aware, unless it is. With `dry_run`, only say.
pub fn patch(exe: &Path, dry_run: bool) -> anyhow::Result<Patched> {
    let mut bytes = std::fs::read(exe)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", exe.display()))?;
    let header = header(&bytes).map_err(|err| anyhow::anyhow!("{}: {err}", exe.display()))?;
    let at = header.characteristics;
    let flags = u16::from_le_bytes([bytes[at], bytes[at + 1]]);
    if flags & LARGE_ADDRESS_AWARE != 0 {
        return Ok(Patched::Already);
    }
    let mut backup = exe.as_os_str().to_os_string();
    backup.push(".mudcrab-backup");
    let backup = PathBuf::from(backup);
    if dry_run {
        return Ok(Patched::Now(backup));
    }
    // The first original is the one worth keeping.
    if !backup.exists() {
        std::fs::copy(exe, &backup).map_err(|err| {
            anyhow::anyhow!(
                "failed to back up {} to {}: {err}",
                exe.display(),
                backup.display()
            )
        })?;
    }
    bytes[at..at + 2].copy_from_slice(&(flags | LARGE_ADDRESS_AWARE).to_le_bytes());
    let sum = pe_checksum(&bytes, header.checksum);
    bytes[header.checksum..header.checksum + 4].copy_from_slice(&sum.to_le_bytes());

    // Written beside it and renamed over it, so a failure halfway leaves the
    // game as it was.
    let mut temp = exe.as_os_str().to_os_string();
    temp.push(".mudcrab-tmp");
    let temp = PathBuf::from(temp);
    std::fs::write(&temp, &bytes)
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", temp.display()))?;
    std::fs::rename(&temp, exe).map_err(|err| {
        let _ = std::fs::remove_file(&temp);
        anyhow::anyhow!("failed to replace {}: {err}", exe.display())
    })?;
    Ok(Patched::Now(backup))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The smallest PE32 header there is: MZ, the PE signature at 0x40, a
    /// COFF header, and a 96-byte optional header.
    fn tiny_exe(characteristics: u16) -> Vec<u8> {
        let mut bytes = vec![0u8; 0x40 + 4 + 20 + 96 + 8];
        bytes[0..2].copy_from_slice(b"MZ");
        bytes[0x3C..0x40].copy_from_slice(&0x40u32.to_le_bytes());
        bytes[0x40..0x44].copy_from_slice(b"PE\0\0");
        bytes[0x44..0x46].copy_from_slice(&0x014Cu16.to_le_bytes()); // i386
        bytes[0x40 + 4 + 16..0x40 + 4 + 18].copy_from_slice(&96u16.to_le_bytes());
        bytes[0x40 + 4 + 18..0x40 + 4 + 20].copy_from_slice(&characteristics.to_le_bytes());
        bytes[0x40 + 24..0x40 + 26].copy_from_slice(&0x010Bu16.to_le_bytes()); // PE32
        bytes[100..104].copy_from_slice(b"code");
        bytes
    }

    #[test]
    fn the_flag_is_set_the_checksum_follows_and_the_original_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("Oblivion.exe");
        let original = tiny_exe(0x0102);
        std::fs::write(&exe, &original).unwrap();

        assert!(matches!(patch(&exe, true).unwrap(), Patched::Now(_)));
        assert_eq!(
            std::fs::read(&exe).unwrap(),
            original,
            "a dry run changes nothing"
        );

        let Patched::Now(backup) = patch(&exe, false).unwrap() else {
            panic!("it was not patched");
        };
        assert_eq!(std::fs::read(&backup).unwrap(), original);
        let patched = std::fs::read(&exe).unwrap();
        let header = header(&patched).unwrap();
        let flags = u16::from_le_bytes([
            patched[header.characteristics],
            patched[header.characteristics + 1],
        ]);
        assert_eq!(flags, 0x0102 | LARGE_ADDRESS_AWARE);
        let stored = u32::from_le_bytes(
            patched[header.checksum..header.checksum + 4]
                .try_into()
                .unwrap(),
        );
        assert_eq!(stored, pe_checksum(&patched, header.checksum));
        assert_ne!(stored, 0);
        // Only those two fields differ.
        let differing: Vec<usize> = (0..original.len())
            .filter(|&i| original[i] != patched[i])
            .collect();
        assert!(
            differing
                .iter()
                .all(
                    |&i| (header.characteristics..header.characteristics + 2).contains(&i)
                        || (header.checksum..header.checksum + 4).contains(&i)
                ),
            "{differing:?}"
        );

        assert_eq!(patch(&exe, false).unwrap(), Patched::Already);
    }

    #[test]
    fn a_file_that_is_not_a_program_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("Oblivion.exe");
        std::fs::write(&exe, b"not a program").unwrap();
        assert!(patch(&exe, false).is_err());
        assert_eq!(std::fs::read(&exe).unwrap(), b"not a program");
    }
}
