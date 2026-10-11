//! `mudcrab downloads`: housekeeping for the downloads folder.

use crate::cli::{DownloadsCommand, DownloadsTidyArgs};
use crate::config::downloads_tidy::{self, Step};

pub fn run(command: DownloadsCommand) -> anyhow::Result<()> {
    match command {
        DownloadsCommand::Tidy(args) => tidy(args),
    }
}

fn tidy(args: DownloadsTidyArgs) -> anyhow::Result<()> {
    let plan = args
        .plan
        .as_ref()
        .map(|path| -> anyhow::Result<_> {
            let raw = std::fs::read_to_string(path)
                .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?;
            serde_json::from_str(&raw)
                .map_err(|err| anyhow::anyhow!("failed to parse plan {}: {err}", path.display()))
        })
        .transpose()?;
    let steps = downloads_tidy::plan_tidy(&args.dir, plan.as_ref())?;

    let name = |path: &std::path::Path| {
        path.file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let (mut deletes, mut renames, mut metas, mut sidecars) = (0, 0, 0, 0);
    for step in &steps {
        match step {
            Step::Delete { derived, real } => {
                deletes += 1;
                println!("delete   {}  (same file as {})", name(derived), name(real));
            }
            Step::Rename { derived, real } => {
                renames += 1;
                println!("rename   {}  ->  {}", name(derived), name(real));
            }
            Step::Conflict { derived, real } => {
                println!("CONFLICT {}  ({} is a different file; left alone)", name(derived), name(real));
            }
            Step::Unknown { derived } => {
                println!("unknown  {}  (no real name recorded; left alone)", name(derived));
            }
            Step::WriteMeta { archive, source } => {
                metas += 1;
                println!("meta     {}  ({source})", name(archive));
            }
            Step::DeleteSidecar { sidecar } => {
                sidecars += 1;
                println!("delete   {}  (no archive beside it)", name(sidecar));
            }
        }
    }
    let left = steps
        .iter()
        .filter(|s| matches!(s, Step::Conflict { .. } | Step::Unknown { .. }))
        .count();
    eprintln!(
        "{deletes} duplicate name(s) to delete, {renames} to rename, {metas} .meta to write, \
         {sidecars} stray sidecar(s); {left} left for you"
    );

    if args.apply {
        downloads_tidy::apply(&steps)?;
        eprintln!("applied");
    } else if !steps.is_empty() {
        eprintln!("nothing changed; re-run with --apply to do this");
    }
    Ok(())
}
