//! `dds_decompress`: DXT1 punch-through alpha, made explicit.
//!
//! See [`crate::dds`] for why. Files are replaced by writing a sibling and
//! renaming it over the original, never by writing in place: staged files can
//! be hard links into the archive cache, and truncating one would rewrite the
//! cached copy too.

use super::ActionCx;
use crate::config::schema::DdsDecompressAction;
use crate::dds::{self, Kind};
use crate::util::fs::{normalize_relative_path, resolve_existing_path_case_insensitive};
use std::path::Path;

pub(super) fn apply(action: &DdsDecompressAction, cx: &ActionCx<'_>) -> anyhow::Result<()> {
    let Some(mod_target) = cx.mod_target else {
        anyhow::bail!("{}: dds_decompress is only valid as a per-mod action", cx.owner);
    };
    if action.paths.is_empty() {
        anyhow::bail!("{}: dds_decompress requires at least one path", cx.owner);
    }

    let mut converted = 0usize;
    for path in &action.paths {
        let relative = normalize_relative_path(path)?;
        let Some(resolved) = resolve_existing_path_case_insensitive(&mod_target.join(&relative))
        else {
            anyhow::bail!("{}: dds_decompress path '{path}' does not exist", cx.owner);
        };

        if resolved.is_dir() {
            for texture in dds::find_textures(&resolved)? {
                if convert(&texture, cx.settings.dry_run)? {
                    converted += 1;
                }
            }
        } else if convert(&resolved, cx.settings.dry_run)? {
            converted += 1;
        } else {
            anyhow::bail!(
                "{}: '{path}' is not a DXT1 texture with transparency, so there is nothing to decompress",
                cx.owner
            );
        }
    }

    // A row that converts nothing is a row whose reason has gone away -- an
    // updated mod, or a path that drifted -- and should be looked at.
    if converted == 0 {
        anyhow::bail!(
            "{}: dds_decompress found no DXT1 textures with transparency under {:?}",
            cx.owner,
            action.paths
        );
    }
    tracing::info!(
        owner = cx.owner,
        converted,
        dry_run = cx.settings.dry_run,
        "decompressed DXT1 textures with transparency"
    );
    Ok(())
}

/// Convert one texture if it needs it. Returns whether it did (or would).
fn convert(path: &Path, dry_run: bool) -> anyhow::Result<bool> {
    let data = std::fs::read(path)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?;
    let decoded = match dds::parse(&data) {
        Ok(Kind::Dxt1(texture)) if texture.has_transparency() => texture.to_bgra_dds(),
        Ok(_) => return Ok(false),
        // Mods ship TGAs and PNGs named `.dds`; the game sniffs them, and so
        // they are not this action's business.
        Err(_) => return Ok(false),
    };
    if dry_run {
        return Ok(true);
    }

    let mut sibling = path.as_os_str().to_owned();
    sibling.push(".mudcrab-tmp");
    let sibling = std::path::PathBuf::from(sibling);
    std::fs::write(&sibling, decoded)
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", sibling.display()))?;
    std::fs::rename(&sibling, path)
        .map_err(|err| anyhow::anyhow!("failed to replace {}: {err}", path.display()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::convert;
    use crate::dds::{self, test_dxt1, Kind};

    #[test]
    fn converts_transparent_dxt1_and_leaves_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        let alpha = dir.path().join("frame.dds");
        let opaque = dir.path().join("solid.dds");
        let not_dds = dir.path().join("really_a.tga.dds");
        std::fs::write(&alpha, test_dxt1(true)).unwrap();
        std::fs::write(&opaque, test_dxt1(false)).unwrap();
        std::fs::write(&not_dds, b"TRUEVISION").unwrap();

        assert!(convert(&alpha, false).unwrap());
        assert!(!convert(&opaque, false).unwrap());
        assert!(!convert(&not_dds, false).unwrap());

        let data = std::fs::read(&alpha).unwrap();
        assert!(matches!(dds::parse(&data).unwrap(), Kind::Other(_)));
        assert_eq!(std::fs::read(&opaque).unwrap(), test_dxt1(false));
    }

    /// Staged files can be hard links into the archive cache.
    #[test]
    fn replacing_a_hard_linked_texture_leaves_the_other_link_alone() {
        let dir = tempfile::tempdir().unwrap();
        let cached = dir.path().join("cached.dds");
        let staged = dir.path().join("staged.dds");
        std::fs::write(&cached, test_dxt1(true)).unwrap();
        std::fs::hard_link(&cached, &staged).unwrap();

        assert!(convert(&staged, false).unwrap());

        assert_eq!(std::fs::read(&cached).unwrap(), test_dxt1(true));
        assert_ne!(std::fs::read(&staged).unwrap(), test_dxt1(true));
    }

    #[test]
    fn dry_run_reports_without_writing() {
        let dir = tempfile::tempdir().unwrap();
        let alpha = dir.path().join("frame.dds");
        std::fs::write(&alpha, test_dxt1(true)).unwrap();
        assert!(convert(&alpha, true).unwrap());
        assert_eq!(std::fs::read(&alpha).unwrap(), test_dxt1(true));
    }
}
