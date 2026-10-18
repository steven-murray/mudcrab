//! `mudcrab conflicts`: what two mods both provide, before either is trusted.
//!
//! `conflicts_with` decides which files a mod gives up, and the guide's rows
//! that use it delete hundreds at a time. That is not a thing to run blind.
//! This prints the same set the action would act on, from the same code, so a
//! row can be checked against the guide -- or against a list read off a
//! finished install -- before it is written.

use crate::cli::ConflictsArgs;
use crate::config;
use crate::config::tools::ToolsConfig;

pub async fn run(args: ConflictsArgs) -> anyhow::Result<()> {
    let plan = config::pipeline::read_plan(&args.plan)?;

    // A sweep reads installed folders only; one mod may be read from its
    // archives when it is not installed, which needs the downloads folder.
    let downloads = super::resolve_downloads_dir(args.downloads.clone(), None);
    let downloads = match (&args.mod_id, downloads) {
        (None, downloads) => downloads.unwrap_or_default(),
        (Some(_), downloads) => downloads?,
    };
    let settings = config::install::InstallSettings {
        cache_dir: downloads,
        mods_dir: args.mods_dir.clone(),
        mo2_instance_dir: None,
        profile_name: String::new(),
        game_dir: None,
        execute_actions: false,
        dry_run: true,
        tools: ToolsConfig::default(),
        filter: Default::default(),
        force_merges: false,
        force: false,
        redo_actions: Vec::new(),
    };

    let Some(mod_id) = &args.mod_id else {
        return sweep(&plan, &args, &settings);
    };

    if args.with.is_empty() {
        let mine = config::conflicts::files_of(&plan, &settings, mod_id)?;
        for path in &mine {
            println!("{path}");
        }
        crate::ui::status_err(
            crate::ui::Tone::Good,
            format!("{mod_id}: {} files", mine.len()),
        );
        return Ok(());
    }

    let shared =
        config::conflicts::shared(&plan, &settings, mod_id, &args.with, args.under.as_deref())?;
    for other in &shared.others {
        eprintln!(
            "{}",
            crate::ui::dim(format!("{}: {} files", other.mod_id, other.files))
        );
    }
    for path in &shared.overlap {
        println!("{path}");
    }
    crate::ui::status_err(
        if shared.overlap.is_empty() {
            crate::ui::Tone::Note
        } else {
            crate::ui::Tone::Warning
        },
        format!(
            "{} of {mod_id}'s {} files are also provided by [{}]",
            shared.overlap.len(),
            shared.files,
            args.with.join(", ")
        ),
    );

    // Without this, asking about a row whose `file_hide` has already run answers
    // "0", which reads as "the row was wrong" when it means "the row worked".
    // The hidden files are out of the VFS and so genuinely conflict with
    // nothing -- but that is not what the person asking wants to hear.
    let already = shared.already_hidden;
    if already > 0 {
        crate::ui::status_err(
            crate::ui::Tone::Note,
            format!(
                "{already} further file{} {} already hidden in {mod_id}, so out of the \
             virtual file system and not counted above. Re-install the mod to see \
             the selection as it was first made.",
                if already == 1 { "" } else { "s" },
                if already == 1 { "is" } else { "are" },
            ),
        );
    }
    Ok(())
}

/// Every installed mod's files, compared with every other's, in MO2's order.
///
/// `--mod` answers "what does this mod share", which is only a question once
/// you know to ask it. This is what tells you to.
fn sweep(
    plan: &config::schema::PersonalizedPlan,
    args: &ConflictsArgs,
    settings: &config::install::InstallSettings,
) -> anyhow::Result<()> {
    let found = config::conflicts::sweep(plan, settings, &args.filter.to_mod_filter())?;
    let partners = |list: &[config::conflicts::Partner]| {
        let shown: Vec<String> = list
            .iter()
            .take(5)
            .map(|p| format!("{} ({})", p.mod_id, p.files))
            .collect();
        let more = list.len().saturating_sub(5);
        if more > 0 {
            format!("{}, and {more} more", shown.join(", "))
        } else {
            shown.join(", ")
        }
    };
    let files = |n: usize| if n == 1 { "file" } else { "files" };

    for entry in &found.mods {
        let section = if entry.section.is_empty() {
            String::new()
        } else {
            format!(
                "  {}",
                crate::ui::dim(format!("[{}]", entry.section.join(" - ")))
            )
        };
        println!("\n{}{section}", crate::ui::name(&entry.id));
        if !entry.wins.is_empty() {
            let total: usize = entry.wins.iter().map(|p| p.files).sum();
            println!(
                "  {} {total} {} over: {}",
                console::style("wins").green(),
                files(total),
                partners(&entry.wins)
            );
        }
        if !entry.losses.is_empty() {
            let total: usize = entry.losses.iter().map(|p| p.files).sum();
            println!(
                "  {} {total} {} to: {}",
                console::style("loses").yellow(),
                files(total),
                partners(&entry.losses)
            );
        }
    }

    let reported = found.mods.len();
    let missing = found.missing;
    eprintln!();
    crate::ui::status_err(
        crate::ui::Tone::Good,
        format!(
            "{reported} of {} installed mods share files with another (loose files and BSA \
         contents; a later mod wins){}.",
            found.installed,
            if missing > 0 {
                format!(
                    "; {missing} in the plan {} not installed, so not compared",
                    if missing == 1 { "is" } else { "are" }
                )
            } else {
                String::new()
            }
        ),
    );
    if reported > 0 {
        eprintln!(
            "{}",
            crate::ui::dim(format!(
                "To see the files: mudcrab conflicts --plan {} --mods-dir {} --mod <id> --with \
                 <other>",
                args.plan.display(),
                args.mods_dir.display()
            ))
        );
    }
    Ok(())
}
