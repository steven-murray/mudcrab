//! `mudcrab bsa`: look inside an archive without a Windows tool.
//!
//! Debugging an installed list keeps needing the vanilla copy of a file that a
//! mod overrides -- a menu XML, a texture -- and those live in the game's own
//! BSAs. This exposes the reader the merge and repack code already use.

use crate::bsa::Bsa;
use crate::cli::{BsaCommand, BsaExtractArgs, BsaListArgs};
use std::path::Path;

pub fn run(command: BsaCommand) -> anyhow::Result<()> {
    match command {
        BsaCommand::List(args) => list(args),
        BsaCommand::Extract(args) => extract(args),
    }
}

fn read(path: &Path) -> anyhow::Result<Vec<u8>> {
    std::fs::read(path).map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))
}

/// Case-insensitive substring match, with `/` and `\` treated alike.
fn matcher(filter: Option<&str>) -> impl Fn(&str) -> bool {
    let needle = filter.map(|text| text.replace('/', "\\").to_ascii_lowercase());
    move |path| match &needle {
        Some(needle) => path.to_ascii_lowercase().contains(needle.as_str()),
        None => true,
    }
}

fn list(args: BsaListArgs) -> anyhow::Result<()> {
    let data = read(&args.archive)?;
    let bsa = Bsa::parse(&data)?;
    let keep = matcher(args.filter.as_deref());
    for path in bsa.paths().filter(|path| keep(path)) {
        println!("{path}");
    }
    Ok(())
}

fn extract(args: BsaExtractArgs) -> anyhow::Result<()> {
    let data = read(&args.archive)?;
    let bsa = Bsa::parse(&data)?;
    let keep = matcher(args.filter.as_deref());

    let mut written = 0usize;
    for (folder, file) in bsa.files() {
        let path = file.path_in(folder);
        if !keep(&path) {
            continue;
        }
        let relative = crate::util::fs::normalize_relative_path(&path.replace('\\', "/"))
            .map_err(|_| anyhow::anyhow!("refusing unsafe path in archive: {path}"))?;
        let target = args.destination.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| anyhow::anyhow!("failed to create {}: {err}", parent.display()))?;
        }
        std::fs::write(&target, file.data(&path)?.as_ref())
            .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", target.display()))?;
        written += 1;
    }
    crate::ui::status(
        crate::ui::Tone::Good,
        format!(
            "Extracted {written} file(s) to {}",
            crate::ui::dim(args.destination.display())
        ),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::matcher;

    #[test]
    fn filter_ignores_case_and_slash_direction() {
        let keep = matcher(Some("Menus/Main/HUD"));
        assert!(keep("menus\\main\\hud_main_menu.xml"));
        assert!(!keep("menus\\prefabs\\hud.xml"));
        assert!(matcher(None)("anything"));
    }
}
