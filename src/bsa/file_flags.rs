//! Derive a BSA's asset-kind flags from what it contains.
//!
//! The header's `file_flags` word says which kinds of asset the archive holds.
//! It is not decoration: Oblivion consults it when deciding whether an archive
//! can serve a given kind of request, so an archive full of meshes that
//! declares none reaches the game as an archive full of nothing -- and does so
//! silently, since the files are all present and the archive parses fine.
//!
//! mudcrab wrote zero here until every real archive in the corpus was checked
//! and not one of them did.

/// `.nif`, or anything under `meshes\`.
pub const KIND_MESHES: u32 = 0x0000_0001;
/// `.dds`, or anything under `textures\`.
pub const KIND_TEXTURES: u32 = 0x0000_0002;
/// `.xml`.
pub const KIND_MENUS: u32 = 0x0000_0004;
/// `.wav`.
pub const KIND_SOUNDS: u32 = 0x0000_0008;
/// `.mp3` and `.lip`.
pub const KIND_VOICES: u32 = 0x0000_0010;
/// Anything under `shaders\`.
pub const KIND_SHADERS: u32 = 0x0000_0020;
/// `.spt`, the SpeedTree format.
pub const KIND_TREES: u32 = 0x0000_0040;
/// `.fnt` and `.tex`.
pub const KIND_FONTS: u32 = 0x0000_0080;
/// Everything else.
pub const KIND_MISC: u32 = 0x0000_0100;

/// OR together the kind of every file in the archive.
///
/// `paths` are archive-internal paths, backslash-separated and lowercase, as
/// the format stores them.
pub fn derive<'a>(paths: impl IntoIterator<Item = &'a str>) -> u32 {
    let mut flags = 0;
    for path in paths {
        flags |= kind_of(path);
    }
    flags
}

fn kind_of(path: &str) -> u32 {
    let normalized = path.replace('/', "\\").to_ascii_lowercase();

    // Folder first: `meshes\landscape\lod\*.lod` is a mesh archive as far as
    // the engine is concerned, though no extension there says so. This is what
    // separates a real archive's flags from a purely extension-based guess.
    let by_folder = if normalized.starts_with("meshes\\") || normalized.starts_with("distantlod\\") {
        KIND_MESHES
    } else if normalized.starts_with("textures\\") {
        KIND_TEXTURES
    } else if normalized.starts_with("shaders\\") {
        KIND_SHADERS
    } else {
        0
    };

    let extension = normalized
        .rsplit_once('.')
        .map(|(_, ext)| ext)
        .unwrap_or_default();

    let by_extension = match extension {
        "nif" => KIND_MESHES,
        "dds" => KIND_TEXTURES,
        "xml" => KIND_MENUS,
        "wav" => KIND_SOUNDS,
        "mp3" => KIND_VOICES,
        // Voices alone. `Oblivion - Voices1.bsa` holds 16603 `.mp3` and 16595
        // `.lip` files and declares `0x010`; `Oblivion - Sounds.bsa` is 1533
        // `.wav` files and declares `0x008`. Bethesda separates the two
        // cleanly, so a lip file is not a sound. Declaring it as both used to
        // look like the safe direction -- it made every voiced mod's archive
        // claim a kind it does not hold.
        "lip" => KIND_VOICES,
        "spt" => KIND_TREES,
        "fnt" | "tex" => KIND_FONTS,
        // Anything the engine has no dedicated bucket for. `.kf`, `.egm`,
        // `.tri`, `.lod`, `.cmp`, `.ini`, `.txt` all land here.
        _ => KIND_MISC,
    };

    // A file counts once, under whichever bucket its folder implies, falling
    // back to its extension. `meshes\x.lod` is a mesh, not miscellaneous --
    // which is why this is not a plain OR of the two.
    if by_folder != 0 && by_extension == KIND_MISC {
        by_folder
    } else {
        by_folder | by_extension
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meshes_and_textures_by_extension() {
        assert_eq!(
            derive(["meshes\\rocks\\rock01.nif", "textures\\rocks\\rock01.dds"]),
            KIND_MESHES | KIND_TEXTURES
        );
    }

    #[test]
    fn a_lod_file_under_meshes_is_a_mesh_not_miscellaneous() {
        assert_eq!(
            derive(["meshes\\landscape\\lod\\x.lod", "docs\\y.cmp"]),
            KIND_MESHES | KIND_MISC
        );
    }

    /// The shape of `Oblivion - Voices1.bsa`, which declares `0x010` over
    /// 16603 mp3s and 16595 lips. An archive of voice lines is not an archive
    /// of sounds.
    #[test]
    fn a_voice_line_is_voices_only() {
        assert_eq!(
            derive(["sound\\voice\\a.mp3", "sound\\voice\\a.lip"]),
            KIND_VOICES
        );
    }

    /// And a mod that ships both -- Feldscar has `sound\fx` as well as voice
    /// lines -- declares both, because it holds both.
    #[test]
    fn fx_alongside_voices_declares_both() {
        assert_eq!(
            derive(["sound\\fx\\thud.wav", "sound\\voice\\a.mp3"]),
            KIND_SOUNDS | KIND_VOICES
        );
    }

    #[test]
    fn distantlod_counts_as_meshes() {
        // MergedLOD - LODs.bsa is 11998 files, every one under `distantlod\`,
        // and it declares meshes plus miscellaneous.
        assert_eq!(derive(["distantlod\\x.lod"]), KIND_MESHES);
    }

    #[test]
    fn an_unknown_extension_outside_a_known_folder_is_miscellaneous() {
        assert_eq!(derive(["docs\\readme.rtf"]), KIND_MISC);
    }

    #[test]
    fn separators_and_case_do_not_matter() {
        assert_eq!(derive(["Meshes/Rocks/Rock01.NIF"]), KIND_MESHES);
    }
}
