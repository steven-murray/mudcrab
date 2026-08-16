//! The TES4 BSA name hash.
//!
//! Every folder and file record is keyed by a 64-bit hash of its name, and the
//! records are sorted by it. Oblivion looks assets up by hash alone, so a
//! wrong hash produces an archive the game silently ignores rather than one it
//! rejects -- which is why this is verified directly against the real corpus in
//! `tests/bsa_roundtrip_real.rs` rather than only against hand-written vectors.
//!
//! Two things about it are easy to get wrong, and both were established by
//! checking all 86,209 file records in the reference install:
//!
//! * Folder names are hashed *whole*, with no extension split. A folder such as
//!   `sound\voice\molapi.esp\breton\m` contains a `.`, and splitting on it
//!   yields the wrong hash.
//! * File names carry four special-cased extension bits (`.kf`, `.nif`,
//!   `.dds`, `.wav`). Without them 43,196 of the corpus's file hashes are wrong.

/// The mixing constant used by both accumulator folds.
const MULTIPLIER: u32 = 0x0001_003F;

/// Lowercase and switch to the backslash separator the format stores.
fn normalize(name: &str) -> String {
    name.chars()
        .map(|c| if c == '/' { '\\' } else { c.to_ascii_lowercase() })
        .collect()
}

/// Extension bits folded into the low word.
///
/// Oblivion's own hasher special-cases exactly these four extensions.
fn extension_bits(ext: &[u8]) -> u32 {
    match ext {
        b".kf" => 0x0000_0080,
        b".nif" => 0x0000_8000,
        b".dds" => 0x0000_8080,
        b".wav" => 0x8000_0000,
        _ => 0,
    }
}

/// Hash a name already split into its root and extension parts.
///
/// `ext` includes the leading `.`, or is empty.
fn hash_parts(root: &[u8], ext: &[u8]) -> u64 {
    if root.is_empty() {
        return 0;
    }

    let last = u32::from(root[root.len() - 1]);
    let second_to_last = if root.len() > 2 {
        u32::from(root[root.len() - 2]) << 8
    } else {
        0
    };

    let low = last
        | second_to_last
        | (root.len() as u32) << 16
        | u32::from(root[0]) << 24
        | extension_bits(ext);

    // The middle of the root, excluding the first character and the last two.
    let mut middle = 0u32;
    if root.len() > 3 {
        for &byte in &root[1..root.len() - 2] {
            middle = middle
                .wrapping_mul(MULTIPLIER)
                .wrapping_add(u32::from(byte));
        }
    }

    let mut extension = 0u32;
    for &byte in ext {
        extension = extension
            .wrapping_mul(MULTIPLIER)
            .wrapping_add(u32::from(byte));
    }

    u64::from(middle.wrapping_add(extension)) << 32 | u64::from(low)
}

/// Hash a folder name, e.g. `textures\menus\icons`.
///
/// Hashed whole: folder names are never split on `.`, even when they contain
/// one (`sound\voice\molapi.esp\breton\m` does).
pub fn hash_folder_name(name: &str) -> u64 {
    let normalized = normalize(name);
    hash_parts(normalized.as_bytes(), b"")
}

/// Hash a file name, e.g. `iconarrow1.dds`.
///
/// Split at the last `.` of the final path component, so that a `.` appearing
/// in a directory part never becomes the extension.
pub fn hash_file_name(name: &str) -> u64 {
    let normalized = normalize(name);
    let bytes = normalized.as_bytes();

    let base_start = match bytes.iter().rposition(|&b| b == b'\\') {
        Some(index) => index + 1,
        None => 0,
    };

    match bytes[base_start..].iter().rposition(|&b| b == b'.') {
        Some(dot) => {
            let split = base_start + dot;
            hash_parts(&bytes[..split], &bytes[split..])
        }
        None => hash_parts(bytes, b""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_names_are_hashed_whole() {
        // A '.' in a folder name is part of the name, not an extension. Real
        // archives store `sound\voice\<plugin>.esp\...` folders.
        let with_dot = "sound\\voice\\molapi.esp\\breton\\m";
        assert_eq!(
            hash_folder_name(with_dot),
            hash_parts(with_dot.as_bytes(), b""),
            "a folder name must never be split on '.'"
        );
    }

    #[test]
    fn separators_and_case_are_normalized() {
        assert_eq!(
            hash_folder_name("Textures/Menus"),
            hash_folder_name("textures\\menus")
        );
        assert_eq!(hash_file_name("ICON.DDS"), hash_file_name("icon.dds"));
    }

    #[test]
    fn extension_bits_are_applied() {
        // Without the special-cased bits these would collide with the plain
        // fold, which is exactly the bug that makes an archive load empty.
        for (name, bits) in [
            ("a.kf", 0x0000_0080u32),
            ("a.nif", 0x0000_8000),
            ("a.dds", 0x0000_8080),
            ("a.wav", 0x8000_0000),
        ] {
            let hashed = hash_file_name(name) as u32;
            assert_eq!(hashed & bits, bits, "{name} is missing its extension bits");
        }
        // An extension with no special case contributes nothing to the low word.
        assert_eq!(hash_file_name("a.txt") as u32 & 0x8000_8080, 0);
    }

    #[test]
    fn a_dot_in_a_directory_part_is_not_an_extension() {
        // Split at the last '.' of the *basename*, not of the whole string.
        assert_eq!(
            hash_file_name("voice\\molapi.esp\\greeting"),
            hash_folder_name("voice\\molapi.esp\\greeting"),
            "a name whose only '.' is in a directory part has no extension"
        );
    }

    #[test]
    fn short_names_do_not_panic_or_read_out_of_bounds() {
        for name in ["", "a", "ab", "abc", ".", "a.", ".dds"] {
            let _ = hash_file_name(name);
            let _ = hash_folder_name(name);
        }
        assert_eq!(hash_file_name(""), 0);
    }

    #[test]
    fn the_length_and_first_and_last_characters_land_in_the_low_word() {
        let hashed = hash_file_name("abcd") as u32;
        assert_eq!(hashed & 0xFF, u32::from(b'd'));
        assert_eq!((hashed >> 8) & 0xFF, u32::from(b'c'));
        assert_eq!((hashed >> 16) & 0xFF, 4);
        assert_eq!((hashed >> 24) & 0xFF, u32::from(b'a'));
    }
}
