//! `mudcrab conflicts`: what two mods both provide, before either is trusted.
//!
//! `conflicts_with` decides which files a mod gives up, and the guide's rows
//! that use it delete hundreds at a time. That is not a thing to run blind.
//! This prints the same set the action would act on, from the same code, so a
//! row can be checked against the guide -- or against a list read off a
//! finished install -- before it is written.

use crate::cli::ConflictsArgs;
use crate::config;
use crate::config::install::index::staged_paths_detailed;
use crate::config::install::safe_mod_dir_name;
use crate::config::tools::ToolsConfig;
use std::collections::BTreeSet;

pub async fn run(args: ConflictsArgs) -> anyhow::Result<()> {
    let raw = std::fs::read_to_string(&args.plan)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", args.plan.display()))?;
    let plan = serde_json::from_str::<config::schema::PersonalizedPlan>(&raw)
        .map_err(|err| anyhow::anyhow!("failed to parse plan {}: {err}", args.plan.display()))?;

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

    let Some(mod_id) = &args.mod_id else {
        return sweep(&plan, &args, &settings, &active_plugins);
    };
    let subject = find(mod_id)?;
    let subject_paths = staged_paths_detailed(
        subject,
        Some(&args.mods_dir.join(safe_mod_dir_name(&subject.id)?)),
        &settings,
        &active_plugins,
    )?;
    let mine = subject_paths.visible;

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
        let paths = staged_paths_detailed(
            other,
            Some(&args.mods_dir.join(safe_mod_dir_name(&other.id)?)),
            &settings,
            &active_plugins,
        )?;
        eprintln!("{id}: {} files", paths.visible.len());
        theirs.extend(paths.visible);
    }

    let under = args.under.as_ref().map(|value| {
        format!(
            "{}/",
            value.replace('\\', "/").trim_matches('/').to_lowercase()
        )
    });

    let overlap: Vec<&String> = mine
        .iter()
        .filter(|path| {
            under
                .as_deref()
                .is_none_or(|prefix| path.starts_with(prefix))
        })
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

    // Without this, asking about a row whose `file_hide` has already run answers
    // "0", which reads as "the row was wrong" when it means "the row worked".
    // The hidden files are out of the VFS and so genuinely conflict with
    // nothing -- but that is not what the person asking wants to hear.
    let already = subject_paths
        .hidden
        .iter()
        .filter(|path| under.as_deref().is_none_or(|p| path.starts_with(p)))
        .filter(|path| theirs.contains(*path))
        .count();
    if already > 0 {
        eprintln!(
            "note: {already} further file{} {} already hidden in {}, so out of the \
             virtual file system and not counted above. Re-install the mod to see \
             the selection as it was first made.",
            if already == 1 { "" } else { "s" },
            if already == 1 { "is" } else { "are" },
            subject.id,
        );
    }
    Ok(())
}

/// Every installed mod's files, compared with every other's, in MO2's order:
/// a later mod wins a file an earlier one also provides.
///
/// `--mod` answers "what does this mod share", which is only a question once
/// you know to ask it. This is what tells you to.
fn sweep(
    plan: &config::schema::PersonalizedPlan,
    args: &ConflictsArgs,
    settings: &config::install::InstallSettings,
    active_plugins: &std::collections::HashSet<String>,
) -> anyhow::Result<()> {
    use std::collections::{BTreeMap, HashMap};

    // Each path, and the mods providing it in priority order.
    let mut providers: HashMap<String, Vec<usize>> = HashMap::new();
    let mut installed = Vec::new();
    let mut missing = 0usize;
    for (index, entry) in plan.mods.iter().enumerate() {
        let dir = args.mods_dir.join(safe_mod_dir_name(&entry.id)?);
        if !dir.is_dir() {
            missing += 1;
            continue;
        }
        installed.push(index);
        let paths = staged_paths_detailed(entry, Some(&dir), settings, active_plugins)?;
        for path in paths.visible {
            // MO2 writes a meta.ini into every mod; it is not in the game's
            // view, and would make every mod "conflict" with every other.
            if path == "meta.ini" {
                continue;
            }
            providers.entry(path).or_default().push(index);
        }
    }

    let filter = args.filter.to_mod_filter();
    // Per mod: how many files it wins from each earlier mod, and loses to
    // each later one. A file three mods provide counts once per pair that
    // matters: the winner over each loser.
    let mut wins: BTreeMap<usize, BTreeMap<usize, usize>> = BTreeMap::new();
    let mut losses: BTreeMap<usize, BTreeMap<usize, usize>> = BTreeMap::new();
    for mods in providers.values().filter(|mods| mods.len() > 1) {
        let winner = *mods.last().expect("more than one");
        for &loser in &mods[..mods.len() - 1] {
            *wins.entry(winner).or_default().entry(loser).or_default() += 1;
            *losses.entry(loser).or_default().entry(winner).or_default() += 1;
        }
    }

    let name = |index: usize| plan.mods[index].id.as_str();
    let partners = |counts: &BTreeMap<usize, usize>| {
        let mut list: Vec<(usize, usize)> = counts.iter().map(|(m, n)| (*m, *n)).collect();
        list.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let shown: Vec<String> = list
            .iter()
            .take(5)
            .map(|(m, n)| format!("{} ({n})", name(*m)))
            .collect();
        let more = list.len().saturating_sub(5);
        if more > 0 {
            format!("{}, and {more} more", shown.join(", "))
        } else {
            shown.join(", ")
        }
    };
    let files = |n: usize| if n == 1 { "file" } else { "files" };

    let mut reported = 0usize;
    for &index in &installed {
        let entry = &plan.mods[index];
        if !filter.is_empty() && !filter.matches(&entry.section, &entry.id) {
            continue;
        }
        let won = wins.get(&index);
        let lost = losses.get(&index);
        if won.is_none() && lost.is_none() {
            continue;
        }
        reported += 1;
        let section = if entry.section.is_empty() {
            String::new()
        } else {
            format!("  [{}]", entry.section.join(" - "))
        };
        println!("{}{section}", entry.id);
        if let Some(won) = won {
            let total: usize = won.values().sum();
            println!("  wins {total} {} over: {}", files(total), partners(won));
        }
        if let Some(lost) = lost {
            let total: usize = lost.values().sum();
            println!("  loses {total} {} to: {}", files(total), partners(lost));
        }
    }

    eprintln!(
        "\n{reported} of {} installed mods share files with another (loose files and BSA \
         contents; a later mod wins){}.",
        installed.len(),
        if missing > 0 {
            format!(
                "; {missing} in the plan {} not installed, so not compared",
                if missing == 1 { "is" } else { "are" }
            )
        } else {
            String::new()
        }
    );
    if reported > 0 {
        eprintln!(
            "To see the files: mudcrab conflicts --plan {} --mods-dir {} --mod <id> --with <other>",
            args.plan.display(),
            args.mods_dir.display()
        );
    }
    Ok(())
}
