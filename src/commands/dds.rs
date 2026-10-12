//! `mudcrab dds`: find the textures `dds_decompress` exists for.

use crate::cli::{DdsAuditArgs, DdsCommand};
use crate::dds::{self, Kind};

pub fn run(command: DdsCommand) -> anyhow::Result<()> {
    match command {
        DdsCommand::Audit(args) => audit(args),
    }
}

fn audit(args: DdsAuditArgs) -> anyhow::Result<()> {
    let needle = args
        .filter
        .as_deref()
        .map(|text| text.replace('\\', "/").to_ascii_lowercase());

    let mut scanned = 0usize;
    let mut flagged = 0usize;
    for path in dds::find_textures(&args.root)? {
        let relative = path.strip_prefix(&args.root).unwrap_or(&path);
        let shown = relative.to_string_lossy().replace('\\', "/");
        if needle
            .as_deref()
            .is_some_and(|needle| !shown.to_ascii_lowercase().contains(needle))
        {
            continue;
        }
        scanned += 1;
        let data = std::fs::read(&path)
            .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?;
        match dds::parse(&data) {
            Ok(Kind::Dxt1(texture)) if texture.has_transparency() => {
                flagged += 1;
                println!("{shown}\t{}x{}", texture.width, texture.height);
            }
            Ok(_) => {}
            Err(err) => crate::ui::status_err(crate::ui::Tone::Warning, format!("{shown}: {err}")),
        }
    }
    crate::ui::status_err(
        crate::ui::Tone::Good,
        format!("{flagged} of {scanned} texture(s) use DXT1 punch-through transparency"),
    );
    Ok(())
}
