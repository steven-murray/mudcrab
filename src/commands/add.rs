use crate::cli::AddArgs;
use crate::config;
use crate::config::add::{
    LoadPosition, NewMod, OracleMeta, Placement, find_plugins, parse_meta_ini, place_plugins,
    plan_insertion_at,
};
use crate::config::install::safe_mod_dir_name;
use std::path::{Path, PathBuf};

pub async fn run(args: AddArgs) -> anyhow::Result<()> {
    let (mut new_mod, mod_dir) = resolve(&args)?;
    if new_mod.file_name.is_none() {
        new_mod.file_name = nexus_file_name(new_mod.path.as_deref()).await;
    }

    // The id becomes a directory name at install time, so reject here what
    // install would reject there rather than after 600 entries are written.
    safe_mod_dir_name(&new_mod.id)?;

    let original = std::fs::read_to_string(&args.input)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", args.input.display()))?;
    let source = config::loader::load_modlist(&args.input)?;

    if let Some(existing) = source.mods.iter().find(|entry| entry.id == new_mod.id) {
        anyhow::bail!(
            "{} already contains a mod with id '{}' (section {}); \
             pass --id to add this one under a different name",
            args.input.display(),
            existing.id,
            if existing.section.is_empty() {
                "<none>".to_string()
            } else {
                existing.section.join(" - ")
            }
        );
    }

    let placement = Placement {
        before_mod: args.before_mod.clone(),
        before_section: (!args.before_sections.is_empty()).then(|| args.before_sections.clone()),
    };
    let mut plan = plan_insertion_at(&original, &source.mods, &new_mod, &placement)?;

    // Plugins the load order does not list yet go where the author said. A
    // plugin in the wrong place fails silently in game, so with no place given
    // this stops here, before anything is written.
    let unplaced: Vec<String> = new_mod
        .plugins
        .iter()
        .filter(|plugin| {
            !source
                .plugins
                .iter()
                .any(|listed| listed.eq_ignore_ascii_case(plugin))
        })
        .cloned()
        .collect();
    let position = match (&args.load_after, args.load_last) {
        (Some(after), _) => Some(LoadPosition::After(after.clone())),
        (None, true) => Some(LoadPosition::Last),
        (None, false) => None,
    };
    match (&position, unplaced.is_empty()) {
        (Some(position), false) => {
            let placed = place_plugins(&plan.new_text, &unplaced, position)?;
            // The load order sits above every mod, so the block moves down by
            // however many lines went into it.
            plan.line += placed.lines().count() - plan.new_text.lines().count();
            plan.new_text = placed;
        }
        (None, false) => anyhow::bail!(
            "{} {} not in the load order yet; say where with --load-after <plugin> or \
             --load-last (or add {} to `plugins` yourself first)",
            unplaced.join(", "),
            if unplaced.len() == 1 { "is" } else { "are" },
            if unplaced.len() == 1 { "it" } else { "them" },
        ),
        (_, true) => {}
    }

    if new_mod.section.is_empty() {
        eprintln!(
            "warning: no --section given, so '{}' is being added with no section; \
             it will not sit under any MO2 separator.",
            new_mod.id
        );
    }
    match (new_mod.non_nexus, new_mod.known_mod_id) {
        (true, Some(mod_id)) => eprintln!(
            "warning: '{}' is Nexus mod {mod_id}, but its meta.ini records no file id, so the \
             descriptor cannot be built. Written as a manual archive with a TODO; resolve the \
             file id and replace the `path`.",
            new_mod.id
        ),
        (true, None) => eprintln!(
            "warning: '{}' has modid=0 in its meta.ini, so it did not come from Nexus. \
             The block is written with no `path` and a TODO; fill in the source yourself.",
            new_mod.id
        ),
        (false, _) => {}
    }

    if let Some(dir) = &mod_dir {
        report_plugins(&new_mod.id, dir);
    }

    if args.dry_run {
        println!(
            "would insert into {} at line {}, {}:\n",
            args.input.display(),
            plan.line,
            plan.anchor
        );
        println!("{}", plan.block);
        if let (Some(position), false) = (&position, unplaced.is_empty()) {
            let at = match position {
                LoadPosition::After(after) => format!("after {after}"),
                LoadPosition::Last => "at the end".to_string(),
            };
            println!("\nand {} into the load order {at}", unplaced.join(", "));
        }
        println!("\n(--dry-run: nothing written)");
        return Ok(());
    }

    write_atomic(&args.input, &plan.new_text)?;

    // A modlist that no longer parses is worse than one missing a mod, so the
    // insert is only kept if the whole file still loads and validates.
    if let Err(err) = reload_and_validate(&args.input) {
        write_atomic(&args.input, &original).map_err(|restore_err| {
            anyhow::anyhow!(
                "adding '{}' produced an invalid modlist ({err}), and restoring the original \
                 also failed ({restore_err}). The file may be damaged.",
                new_mod.id
            )
        })?;
        anyhow::bail!(
            "adding '{}' produced a modlist that no longer validates: {err}. \
             The original file has been restored unchanged.",
            new_mod.id
        );
    }

    println!(
        "added '{}' to {} at line {}, {}",
        new_mod.id,
        args.input.display(),
        plan.line,
        plan.anchor
    );

    Ok(())
}

/// The archive's filename, from Nexus's list of the mod's files, for a
/// `nexus:` source given without --file-name. It lets `download` save the file
/// under its real name and later stages find one already on disk. Best effort:
/// with no API key, or no answer, the block is written without it.
async fn nexus_file_name(path: Option<&str>) -> Option<String> {
    let spec = path?.strip_prefix("nexus:")?;
    let mut parts = spec.split('/');
    let (game, mod_id, file_id) = (parts.next()?, parts.next()?, parts.next()?);
    let (mod_id, file_id): (u64, u64) = (mod_id.parse().ok()?, file_id.parse().ok()?);
    let api_key = std::env::var("NEXUS_API_KEY").ok()?;
    let api_base = std::env::var("NEXUS_API_BASE").ok();
    let client = reqwest::Client::builder()
        .user_agent("mudcrab")
        .build()
        .ok()?;
    let files =
        crate::config::identify::nexus_files(&client, api_base.as_deref(), &api_key, game, mod_id)
            .await
            .map_err(|err| eprintln!("note: could not ask Nexus for the file's name ({err})"))
            .ok()?;
    let name = files
        .into_iter()
        .find(|file| file.file_id == file_id)
        .and_then(|file| file.file_name);
    if name.is_none() {
        eprintln!("note: Nexus does not list file {file_id} for mod {mod_id}");
    }
    name
}

/// Build the entry to insert, plus the Oracle mod folder when there is one (it
/// is scanned afterwards for plugins).
fn resolve(args: &AddArgs) -> anyhow::Result<(NewMod, Option<PathBuf>)> {
    if let Some(archive) = &args.archive {
        return Ok((from_archive(archive, args)?, None));
    }
    match (&args.from_oracle, &args.nexus) {
        (Some(oracle_dir), None) => {
            let folder = args
                .mod_folder
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("--from-oracle requires --mod <mod folder name>"))?;
            let mod_dir = oracle_dir.join(folder);
            let new_mod = from_oracle(&mod_dir, folder, args)?;
            Ok((new_mod, Some(mod_dir)))
        }
        (None, Some(nexus)) => Ok((from_nexus(nexus, args)?, None)),
        (None, None) => anyhow::bail!(
            "give a source: --archive <file>, --nexus <modid>/<fileid>, or \
             --from-oracle <ORACLE_MODS_DIR> --mod <folder>"
        ),
        (Some(_), Some(_)) => {
            unreachable!("clap declares --from-oracle and --nexus as conflicting")
        }
    }
}

/// An entry written from the archive itself, as `inspect` reads it. What the
/// archive cannot decide -- which FOMOD options, which of several BAIN
/// packages, where its plugins load -- is reported for the author to settle.
fn from_archive(archive: &Path, args: &AddArgs) -> anyhow::Result<NewMod> {
    use crate::config::inspect::{LayoutGuess, inspect_archive, render_toml_snippet_with};
    use std::io::IsTerminal;

    let id = args
        .id
        .clone()
        .ok_or_else(|| anyhow::anyhow!("--archive requires --id <mod id>"))?;
    let report = inspect_archive(archive, false)?;
    let source = match (&args.nexus, args.manual) {
        (Some(spec), _) => {
            let (mod_id, file_id) = parse_nexus_arg(spec)?;
            format!("nexus:oblivion/{mod_id}/{file_id}")
        }
        (None, true) => format!("manual:{}", report.file_name),
        (None, false) => crate::config::archive_store::meta_source(archive).ok_or_else(|| {
            anyhow::anyhow!(
                "{} has no MO2 `.meta` saying where it came from; give the source with \
                 --nexus <modid>/<fileid>, or --manual for an archive fetched by hand",
                archive.display()
            )
        })?,
    };
    // In a terminal, an installer's questions are put to the author, and the
    // answers are what the entry records.
    let interactive = !args.defaults && std::io::stdin().is_terminal();
    let (chosen, actions) = choose_installer(archive, &id, &args.input, &report, interactive)?;

    // Comments in the snippet talk to someone reading `inspect`'s report; in
    // the list they would only be noise.
    let block = render_toml_snippet_with(
        &report.file_name,
        Some(&source),
        &report.layout,
        report.fomod.as_ref(),
        chosen.as_ref(),
    )
    .lines()
    .filter(|line| !line.trim_start().starts_with('#'))
    .collect::<Vec<_>>()
    .join("\n");
    // Actions the installer's choices call for follow the archive.
    let block = actions
        .iter()
        .fold(block, |block, action| format!("{block}\n\n{action}"));

    match &report.layout {
        _ if chosen.is_some() => {}
        LayoutGuess::Fomod { .. } => eprintln!(
            "note: '{id}' has a FOMOD installer; the entry picks each group's default. \
             `mudcrab inspect` lists the options, to change `fomod_selections`."
        ),
        LayoutGuess::Bain { subpackages, .. } if subpackages.len() > 1 => eprintln!(
            "note: '{id}' is a BAIN package and every subpackage is listed; delete from \
             `bain_subpackages` the ones that are alternatives to each other."
        ),
        _ => {}
    }
    let unlisted: Vec<&String> = report
        .plugins
        .iter()
        .filter(|path| {
            let name = path.rsplit('/').next().unwrap_or(path);
            !args.plugins.iter().any(|p| p.eq_ignore_ascii_case(name))
        })
        .collect();
    if !unlisted.is_empty() {
        eprintln!("note: the archive holds plugins not given with --plugin:");
        for path in unlisted {
            eprintln!("  {path}");
        }
    }

    Ok(NewMod {
        id,
        section: args.sections.clone(),
        condition: args.condition.clone(),
        plugins: args.plugins.clone(),
        archive_block: Some(block),
        ..NewMod::default()
    })
}

/// Walk an archive's installer with the author: each FOMOD group in the order
/// install meets them, or the BAIN package's wizard, or its package list; and
/// any actions those choices call for. `None` for a layout with nothing to
/// choose. Not interactive, only a wizard is run (on its defaults): what it
/// selects beats listing every subpackage.
fn choose_installer(
    archive: &Path,
    id: &str,
    modlist: &Path,
    report: &crate::config::inspect::InspectReport,
    interactive: bool,
) -> anyhow::Result<(Option<crate::config::inspect::Chosen>, Vec<String>)> {
    use super::choose::{Availability, Choice, Question, Rule, ask};
    use crate::config::inspect::{Chosen, LayoutGuess, extract_single_entry};
    use crate::config::install::layout::fomod::{FomodOptionType, choose_fomod};

    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let mut output = std::io::stdout();
    match &report.layout {
        LayoutGuess::Fomod { config_path } if interactive => {
            let xml = extract_single_entry(archive, config_path)?;
            // A step can depend on which plugins are active; install answers
            // that from the list's load order, so the walk here does too.
            let active: std::collections::HashSet<String> =
                crate::config::loader::load_modlist(modlist)?
                    .plugins
                    .iter()
                    .map(|plugin| plugin.to_ascii_lowercase())
                    .collect();
            println!("'{id}' has a FOMOD installer. Enter takes the installer's own choice.");
            let selections = choose_fomod(&xml, config_path, &active, &mut |group| {
                let question = Question {
                    title: format!("{}: {}", group.step, group.group),
                    rule: Rule::from_fomod(group.group_type),
                    choices: group
                        .options
                        .iter()
                        .map(|option| Choice {
                            name: option.name.clone(),
                            description: option.description.clone(),
                            availability: match option.kind {
                                FomodOptionType::Required => Availability::Required,
                                FomodOptionType::Recommended => Availability::Recommended,
                                FomodOptionType::NotUsable => Availability::Unavailable,
                                FomodOptionType::Optional | FomodOptionType::CouldBeUsable => {
                                    Availability::Free
                                }
                            },
                        })
                        .collect(),
                    default: group.default.clone(),
                };
                ask(&mut input, &mut output, &question)
            })?;
            Ok((Some(Chosen::Fomod(selections)), Vec::new()))
        }
        LayoutGuess::Bain { subpackages, .. } if subpackages.len() > 1 => {
            // A wizard asks its own questions, in words, and decides what
            // they mean; run it, rather than ask about folder names.
            let wizard_path = report
                .top_level_files
                .iter()
                .find(|name| name.eq_ignore_ascii_case("wizard.txt"));
            if let Some(path) = wizard_path {
                let script = extract_single_entry(archive, path)?;
                let listing = crate::config::inspect::inspect_archive(archive, true)?;
                let package = crate::config::bain_wizard::run::Package {
                    subpackages: subpackages.clone(),
                    files: listing.files.unwrap_or_default(),
                };
                if interactive {
                    println!(
                        "'{id}' is a BAIN package with a wizard. Enter takes the wizard's own \
                         choice."
                    );
                }
                let result = super::add_wizard::run_wizard(
                    &script,
                    &package,
                    modlist,
                    interactive,
                    &mut input,
                    &mut output,
                )?;
                if let Some(result) = result {
                    println!("  subpackages: {}", result.subpackages.join(", "));
                    if !result.exclude.is_empty() {
                        println!("  plugins left out: {}", result.exclude.join(", "));
                    }
                    return Ok((
                        Some(Chosen::Wizard {
                            subpackages: result.subpackages,
                            exclude: result.exclude,
                        }),
                        result.actions,
                    ));
                }
                if !interactive {
                    return Ok((None, Vec::new()));
                }
                println!("Pick the subpackages directly instead.");
            } else if !interactive {
                return Ok((None, Vec::new()));
            } else {
                println!(
                    "'{id}' is a BAIN package. Pick the subpackages to install; numbered alike \
                     (01 ..., 01 ...) usually means alternatives."
                );
            }
            let question = Question {
                title: "Subpackages".to_string(),
                rule: Rule::AtLeastOne,
                choices: subpackages
                    .iter()
                    .map(|name| Choice {
                        name: name.clone(),
                        description: String::new(),
                        availability: Availability::Free,
                    })
                    .collect(),
                default: (0..subpackages.len()).collect(),
            };
            let picked = ask(&mut input, &mut output, &question)?;
            Ok((
                Some(Chosen::Bain(
                    picked.into_iter().map(|i| subpackages[i].clone()).collect(),
                )),
                Vec::new(),
            ))
        }
        _ => Ok((None, Vec::new())),
    }
}

fn parse_nexus_arg(spec: &str) -> anyhow::Result<(u64, u64)> {
    let (mod_id, file_id) = spec
        .split_once('/')
        .ok_or_else(|| anyhow::anyhow!("--nexus must be <modid>/<fileid>, got '{spec}'"))?;
    let mod_id: u64 = mod_id
        .trim()
        .parse()
        .map_err(|_| anyhow::anyhow!("--nexus mod id must be a number, got '{mod_id}'"))?;
    let file_id: u64 = file_id
        .trim()
        .parse()
        .map_err(|_| anyhow::anyhow!("--nexus file id must be a number, got '{file_id}'"))?;
    Ok((mod_id, file_id))
}

fn from_oracle(mod_dir: &Path, folder: &str, args: &AddArgs) -> anyhow::Result<NewMod> {
    let meta_path = mod_dir.join("meta.ini");
    let text = std::fs::read_to_string(&meta_path)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", meta_path.display()))?;
    let meta = parse_meta_ini(&text);

    let id = args.id.clone().unwrap_or_else(|| folder.to_string());
    let file_name = args
        .file_name
        .clone()
        .or_else(|| meta.installation_file.clone());

    let mut new_mod = NewMod {
        id,
        section: args.sections.clone(),
        condition: args.condition.clone(),
        plugins: args.plugins.clone(),
        file_name,
        version: meta.version.clone(),
        non_nexus: args.manual,
        ..NewMod::default()
    };

    match nexus_ids(&meta) {
        NexusSource::Nexus { mod_id, file_id } => {
            new_mod.path = Some(format!("nexus:oblivion/{mod_id}/{file_id}"));
            new_mod.download_handler = Some("nexus".to_string());
        }
        NexusSource::NotNexus => new_mod.non_nexus = true,
        // A real mod id with no file id. Erroring is right when someone is
        // sitting there to answer it, but the archive is usually already on
        // disk and the build can proceed without the descriptor -- so --manual
        // records what is known, flags what is not, and carries on.
        NexusSource::MissingFileId { mod_id } if args.manual => {
            new_mod.non_nexus = true;
            new_mod.known_mod_id = Some(mod_id);
        }
        NexusSource::MissingFileId { mod_id } => anyhow::bail!(
            "{} has modid={mod_id} but no `1\\fileid` under [installedFiles], so the download \
             URL cannot be built. Add it with --nexus {mod_id}/<fileid>, or with --manual to \
             record it as a hand-fetched archive and move on.",
            meta_path.display()
        ),
    }

    Ok(new_mod)
}

enum NexusSource {
    Nexus {
        mod_id: u64,
        file_id: u64,
    },
    /// `modid=0`: MO2's marker for a mod installed from somewhere else.
    NotNexus,
    MissingFileId {
        mod_id: u64,
    },
}

fn nexus_ids(meta: &OracleMeta) -> NexusSource {
    match (meta.mod_id, meta.file_id) {
        (None | Some(0), _) => NexusSource::NotNexus,
        (Some(mod_id), Some(file_id)) if file_id != 0 => NexusSource::Nexus { mod_id, file_id },
        (Some(mod_id), _) => NexusSource::MissingFileId { mod_id },
    }
}

fn from_nexus(spec: &str, args: &AddArgs) -> anyhow::Result<NewMod> {
    let (mod_id, file_id) = parse_nexus_arg(spec)?;

    let id = args
        .id
        .clone()
        .ok_or_else(|| anyhow::anyhow!("--nexus requires --id <mod id>"))?;
    if args.sections.is_empty() {
        anyhow::bail!("--nexus requires --section <name>");
    }

    Ok(NewMod {
        id,
        section: args.sections.clone(),
        condition: args.condition.clone(),
        plugins: args.plugins.clone(),
        path: Some(format!("nexus:oblivion/{mod_id}/{file_id}")),
        download_handler: Some("nexus".to_string()),
        file_name: args.file_name.clone(),
        version: None,
        non_nexus: false,
        known_mod_id: None,
        archive_block: None,
    })
}

/// Report plugins the mod folder ships, without touching the load order.
///
/// `validate` hard-fails on a mod declaring a plugin missing from the top-level
/// `plugins` array, and a plugin in the wrong place in load order fails
/// silently in game, which is worse. So the position is the author's call: this
/// only tells them what there is to place.
fn report_plugins(mod_id: &str, mod_dir: &Path) {
    let plugins = find_plugins(mod_dir);
    if plugins.is_empty() {
        return;
    }

    eprintln!(
        "'{mod_id}' ships {} plugin{}:",
        plugins.len(),
        if plugins.len() == 1 { "" } else { "s" }
    );
    for plugin in &plugins {
        eprintln!("  {plugin}");
    }
    eprintln!(
        "Place them with --plugin <file> and --load-after <plugin> (or --load-last). \
         mudcrab will not guess the position, and without those the mod block is written \
         with no `plugins` key so the modlist still validates."
    );
}

/// Write via a temp file next to the target, then rename, so an interrupted
/// write cannot leave a half-written modlist behind.
pub(crate) fn write_atomic(path: &Path, content: &str) -> anyhow::Result<()> {
    let mut temp = path.as_os_str().to_os_string();
    temp.push(format!(".mudcrab-add-{}.tmp", std::process::id()));
    let temp = PathBuf::from(temp);

    std::fs::write(&temp, content)
        .map_err(|err| anyhow::anyhow!("failed to write {}: {err}", temp.display()))?;

    std::fs::rename(&temp, path).map_err(|err| {
        let _ = std::fs::remove_file(&temp);
        anyhow::anyhow!("failed to replace {}: {err}", path.display())
    })
}

pub(crate) fn reload_and_validate(path: &Path) -> anyhow::Result<()> {
    let reloaded = config::loader::load_modlist(path)?;
    config::validator::validate(&reloaded).map(|_| ())
}
