//! `mudcrab conflicts`: what two mods both provide, before either is trusted.
//!
//! `conflicts_with` decides which files a mod gives up, and the guide's rows
//! that use it delete hundreds at a time. That is not a thing to run blind.
//! This prints the same set the action would act on, from the same code, so a
//! row can be checked against the guide -- or against a list read off a
//! finished install -- before it is written.

use crate::cli::ConflictsArgs;
use crate::config;
use crate::config::install::index::staged_paths;
use crate::config::install::safe_mod_dir_name;
use crate::config::tools::ToolsConfig;
use std::collections::BTreeSet;
use std::path::PathBuf;

pub async fn run(args: ConflictsArgs) -> anyhow::Result<()> {
    let raw = std::fs::read_to_string(&args.plan)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", args.plan.display()))?;
    let plan = serde_json::from_str::<config::schema::PersonalizedPlan>(&raw)
        .map_err(|err| anyhow::anyhow!("failed to parse plan {}: {err}", args.plan.display()))?;

    let settings = config::install::InstallSettings {
        cache_dir: args
            .cache
            .clone()
            .unwrap_or_else(|| PathBuf::from(".mudcrab-cache")),
        mods_dir: args.mods_dir.clone(),
        mo2_instance_dir: None,
        profile_name: String::new(),
        game_dir: None,
        game_root_dir: None,
        execute_actions: false,
        dry_run: true,
        tools: ToolsConfig::default(),
        filter: Default::default(),
        archive_search_paths: args.archive_sources.archive_search_paths.clone(),
        force_merges: false,
        force: false,
    };

    let active_plugins: std::collections::HashSet<String> = plan
        .plugins
        .iter()
        .map(|plugin| plugin.to_ascii_lowercase())
        .collect();

    let find = |id: &str| {
        plan.mods
            .iter()
            .find(|entry| entry.id == id)
            .ok_or_else(|| anyhow::anyhow!("no mod '{id}' in {}", args.plan.display()))
    };

    let subject = find(&args.mod_id)?;
    let mine = staged_paths(
        subject,
        Some(&args.mods_dir.join(safe_mod_dir_name(&subject.id)?)),
        &settings,
        &active_plugins,
    )?;

    if args.with.is_empty() {
        for path in &mine {
            println!("{path}");
        }
        eprintln!("{}: {} files", subject.id, mine.len());
        return Ok(());
    }

    let mut theirs = BTreeSet::new();
    for id in &args.with {
        let other = find(id)?;
        let paths = staged_paths(
            other,
            Some(&args.mods_dir.join(safe_mod_dir_name(&other.id)?)),
            &settings,
            &active_plugins,
        )?;
        eprintln!("{id}: {} files", paths.len());
        theirs.extend(paths);
    }

    let under = args.under.as_ref().map(|value| {
        format!("{}/", value.replace('\\', "/").trim_matches('/').to_lowercase())
    });

    let overlap: Vec<&String> = mine
        .iter()
        .filter(|path| under.as_deref().is_none_or(|prefix| path.starts_with(prefix)))
        .filter(|path| theirs.contains(*path))
        .collect();

    for path in &overlap {
        println!("{path}");
    }
    eprintln!(
        "{} of {}'s {} files are also provided by [{}]",
        overlap.len(),
        subject.id,
        mine.len(),
        args.with.join(", ")
    );
    Ok(())
}
