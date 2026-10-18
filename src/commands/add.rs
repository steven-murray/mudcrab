use crate::cli::AddArgs;
use crate::config;
use crate::config::add::{
    Added, LoadPosition, NewMod, OracleMeta, Placement, find_plugins, parse_meta_ini, plan_add,
};
use crate::config::install::safe_mod_dir_name;
use std::path::{Path, PathBuf};

pub async fn run(mut args: AddArgs) -> anyhow::Result<()> {
    // An archive whose `.meta` does not say where it came from: ask Nexus
    // which file it is, by its MD5, as `identify` does.
    if let Some(archive) = args.archive.clone()
        && args.nexus.is_none()
        && !args.manual
        && config::archive_store::meta_source(&archive).is_none()
        && let Some((mod_id, file_id)) = identify_on_nexus(&archive).await
    {
        crate::ui::status_err(
            crate::ui::Tone::Note,
            format!(
                "{} has no `.meta` saying where it came from; Nexus knows it by its checksum \
                 as mod {mod_id}, file {file_id}",
                archive.display()
            ),
        );
        args.nexus = Some(format!("{mod_id}/{file_id}"));
    }
    let (mut new_mod, mod_dir) = resolve(&args)?;
    if new_mod.file_name.is_none() {
        new_mod.file_name = nexus_file_name(new_mod.path.as_deref()).await;
    }

    // The id becomes a directory name at install time, so reject here what
    // install would reject there rather than after 600 entries are written.
    safe_mod_dir_name(&new_mod.id)?;

    let placement = Placement {
        before_mod: args.before_mod.clone(),
        before_section: (!args.before_sections.is_empty()).then(|| args.before_sections.clone()),
    };
    let position = match (&args.load_after, args.load_last) {
        (Some(after), _) => Some(LoadPosition::After(after.clone())),
        (None, true) => Some(LoadPosition::Last),
        (None, false) => None,
    };
    let mut added = None;
    let edit = config::edit::plan(&args.input, |text, source| {
        let plan = plan_add(text, source, &new_mod, &placement, position.as_ref())?;
        let new_text = plan.insertion.new_text.clone();
        added = Some(plan);
        Ok(new_text)
    })
    .map_err(|err| anyhow::anyhow!("adding '{}': {err:#}", new_mod.id))?;
    let Some(Added {
        insertion: plan,
        placed: unplaced,
    }) = added
    else {
        unreachable!("a planned edit runs its change");
    };

    if new_mod.section.is_empty() {
        crate::ui::status_err(
            crate::ui::Tone::Warning,
            format!(
                "no --section given, so '{}' is being added with no section; \
             it will not sit under any MO2 separator.",
                new_mod.id
            ),
        );
    }
    match (new_mod.non_nexus, new_mod.known_mod_id) {
        (true, Some(mod_id)) => crate::ui::status_err(
            crate::ui::Tone::Warning,
            format!(
                "'{}' is Nexus mod {mod_id}, but its meta.ini records no file id, so the \
             descriptor cannot be built. Written as a manual archive with a TODO; resolve the \
             file id and replace the `path`.",
                new_mod.id
            ),
        ),
        (true, None) => crate::ui::status_err(
            crate::ui::Tone::Warning,
            format!(
                "'{}' has modid=0 in its meta.ini, so it did not come from Nexus. \
             The block is written with no `path` and a TODO; fill in the source yourself.",
                new_mod.id
            ),
        ),
        (false, _) => {}
    }

    if let Some(dir) = &mod_dir {
        report_plugins(&new_mod.id, dir);
    }

    if args.dry_run {
        crate::ui::status(
            crate::ui::Tone::Note,
            format!(
                "Would insert into {} at line {}, {}:\n",
                crate::ui::dim(args.input.display()),
                plan.line,
                plan.anchor
            ),
        );
        println!("{}", plan.block);
        if let (Some(position), false) = (&position, unplaced.is_empty()) {
            let at = match position {
                LoadPosition::After(after) => format!("after {after}"),
                LoadPosition::Last => "at the end".to_string(),
            };
            println!("\nand {} into the load order {at}", unplaced.join(", "));
        }
        crate::ui::say(crate::ui::dim("\n(--dry-run: nothing written)"));
        return Ok(());
    }

    edit.write()?;

    crate::ui::status(
        crate::ui::Tone::Good,
        format!(
            "Added {} to {} at line {}, {}",
            crate::ui::name(&new_mod.id),
            crate::ui::dim(args.input.display()),
            plan.line,
            plan.anchor
        ),
    );

    Ok(())
}

/// Which Nexus mod and file an archive is, by its MD5. Best effort: `None`
/// with no API key, or no answer.
async fn identify_on_nexus(archive: &Path) -> Option<(u64, u64)> {
    let key = crate::config::tools::ToolsConfig::nexus_api_key()?;
    let client = reqwest::Client::builder()
        .user_agent("mudcrab")
        .build()
        .ok()?;
    let base = std::env::var("NEXUS_API_BASE").ok();
    let file_name = archive.file_name()?.to_string_lossy();
    let hint = config::identify::mod_id_from_file_name(&file_name);
    let found =
        config::identify::identify(&client, archive, "oblivion", &key, base.as_deref(), hint)
            .await
            .ok()?;
    Some((found.mod_id, found.file_id))
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
    let api_key = crate::config::tools::ToolsConfig::nexus_api_key()?;
    let api_base = std::env::var("NEXUS_API_BASE").ok();
    let client = reqwest::Client::builder()
        .user_agent("mudcrab")
        .build()
        .ok()?;
    let files =
        crate::config::identify::nexus_files(&client, api_base.as_deref(), &api_key, game, mod_id)
            .await
            .map_err(|err| {
                crate::ui::status_err(
                    crate::ui::Tone::Note,
                    format!("could not ask Nexus for the file's name ({err})"),
                )
            })
            .ok()?;
    let name = files
        .into_iter()
        .find(|file| file.file_id == file_id)
        .and_then(|file| file.file_name);
    if name.is_none() {
        crate::ui::status_err(
            crate::ui::Tone::Note,
            format!("Nexus does not list file {file_id} for mod {mod_id}"),
        );
    }
    name
}

/// Build the entry to insert, plus the Oracle mod folder when there is one (it
/// is scanned afterwards for plugins).
fn resolve(args: &AddArgs) -> anyhow::Result<(NewMod, Option<PathBuf>)> {
    if let Some(archive) = &args.archive {
        return Ok((from_archive(archive, args)?, None));
    }
    match (&args.from_mo2, &args.nexus) {
        (Some(oracle_dir), None) => {
            let folder = args
                .mod_folder
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("--from-mo2 requires --mod <mod folder name>"))?;
            let mod_dir = oracle_dir.join(folder);
            let new_mod = from_oracle(&mod_dir, folder, args)?;
            Ok((new_mod, Some(mod_dir)))
        }
        (None, Some(nexus)) => Ok((from_nexus(nexus, args)?, None)),
        (None, None) => anyhow::bail!(
            "give a source: --archive <file>, --nexus <modid>/<fileid>, or \
             --from-mo2 <MODS_DIR> --mod <folder>"
        ),
        (Some(_), Some(_)) => {
            unreachable!("clap declares --from-mo2 and --nexus as conflicting")
        }
    }
}

/// An entry written from the archive itself, as `inspect` reads it. What the
/// archive cannot decide -- which FOMOD options, which of several BAIN
/// packages, where its plugins load -- is asked in a terminal, and otherwise
/// reported for the author to settle.
fn from_archive(archive: &Path, args: &AddArgs) -> anyhow::Result<NewMod> {
    use crate::config::inspect::{Chosen, inspect_archive};
    use crate::config::installer::{self, ListPlugins};
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
                "{} has no MO2 `.meta` saying where it came from, and Nexus could not say by \
                 its checksum (or there is no Nexus API key to ask with); give the source with \
                 --nexus <modid>/<fileid>, or --manual for an archive fetched by hand",
                archive.display()
            )
        })?,
    };
    // In a terminal, an installer's questions are put to the author, and the
    // answers are what the entry records.
    let interactive = !args.defaults && std::io::stdin().is_terminal();
    let list = ListPlugins::of(&config::loader::load_modlist(&args.input)?);
    let installed = walk_installer(
        archive,
        &id,
        &report,
        &list,
        &crate::config::installer::Prior::default(),
        interactive,
    )?;
    for note in &installed.notes {
        crate::ui::status_err(crate::ui::Tone::Note, format!("'{id}': {note}"));
    }
    if let Some(Chosen::Wizard {
        subpackages,
        exclude,
    }) = &installed.chosen
    {
        println!("  subpackages: {}", subpackages.join(", "));
        if !exclude.is_empty() {
            println!("  plugins left out: {}", exclude.join(", "));
        }
    }
    let block = installer::archive_block(archive, &report, &source, &installed)?;

    let unlisted: Vec<&String> = report
        .plugins
        .iter()
        .filter(|path| {
            let name = path.rsplit('/').next().unwrap_or(path);
            !args.plugins.iter().any(|p| p.eq_ignore_ascii_case(name))
        })
        .collect();
    if !unlisted.is_empty() {
        crate::ui::status_err(
            crate::ui::Tone::Note,
            "the archive holds plugins not given with --plugin:".to_string(),
        );
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

/// Walk the archive's installer with the author at a terminal (or, not
/// interactive, on its defaults): the library's walk, each question it
/// stops at asked here and the walk resumed with the answer. `prior` is what
/// the entry chooses already, offered as each question's default.
pub(super) fn walk_installer(
    archive: &Path,
    id: &str,
    report: &crate::config::inspect::InspectReport,
    list: &crate::config::installer::ListPlugins,
    prior: &crate::config::installer::Prior,
    interactive: bool,
) -> anyhow::Result<crate::config::installer::Installed> {
    use crate::config::inspect::LayoutGuess;
    use crate::config::installer::{Answer, Asked, Walk, walk};

    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let mut output = std::io::stdout();
    let mut answers = Vec::new();
    let mut introduced = false;
    loop {
        let asked = match walk(archive, report, list, prior, &answers, !interactive)? {
            Walk::Done { installed } => return Ok(installed),
            Walk::Ask { asked } => asked,
        };
        if !introduced {
            introduced = true;
            let wizard = report
                .top_level_files
                .iter()
                .any(|name| name.eq_ignore_ascii_case("wizard.txt"));
            println!(
                "{}",
                match (&report.layout, wizard) {
                    (LayoutGuess::Fomod { .. }, _) => format!(
                        "'{id}' has a FOMOD installer. Enter takes {}.",
                        if prior.fomod.is_empty() {
                            "the installer's own choice"
                        } else {
                            "what the list chooses now"
                        }
                    ),
                    (_, true) => format!(
                        "'{id}' is a BAIN package with a wizard. Enter takes the wizard's own \
                         choice."
                    ),
                    _ => format!(
                        "'{id}' is a BAIN package. Pick the subpackages to install; numbered \
                         alike (01 ..., 01 ...) usually means alternatives."
                    ),
                }
            );
        }
        answers.push(match asked {
            Asked::Choose { question } => {
                Answer::Pick(super::choose::ask(&mut input, &mut output, &question)?)
            }
            Asked::YesNo { path } => Answer::Yes(yes_no(
                &mut input,
                &mut output,
                &format!(
                    "The wizard asks whether `{path}` is already installed (by a mod earlier in \
                     your list). Is it?"
                ),
            )?),
        });
    }
}

/// A yes-or-no question; Enter is no.
fn yes_no(
    input: &mut dyn std::io::BufRead,
    output: &mut dyn std::io::Write,
    question: &str,
) -> anyhow::Result<bool> {
    loop {
        write!(output, "\n{question} [y/N]: ")?;
        output.flush()?;
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            anyhow::bail!("input ended before the wizard's question was answered");
        }
        match line.trim().to_ascii_lowercase().as_str() {
            "" | "n" | "no" => return Ok(false),
            "y" | "yes" => return Ok(true),
            _ => writeln!(output, "  Please answer y or n.")?,
        }
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
