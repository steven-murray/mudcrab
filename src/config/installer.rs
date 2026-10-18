//! Walking an archive's installer to write its modlist entry: each FOMOD
//! group in the order install meets them, or a BAIN package's wizard, or its
//! package list -- and what the entry then says.
//!
//! The walk is resumable, so a front end that cannot block on a question (the
//! GUI asks in a form, between calls) drives it the same way a terminal does:
//! give it the answers so far, and it replays them and returns either the next
//! question or the finished choice. Replaying is cheap -- an installer script
//! is small -- and keeps the walk the installer's own: a FOMOD step hidden by
//! an earlier choice is never asked, and a wizard's later questions follow its
//! earlier answers. Questions with nothing to decide are answered here and
//! never put to anyone.

use crate::config::bain_wizard::run::{Ending, Host, Outcome, Package, Prompt, run};
use crate::config::choose::{Availability, Choice, Question, Rule};
use crate::config::inspect::{
    Chosen, InspectReport, LayoutGuess, extract_single_entry, inspect_archive,
    render_toml_snippet_with,
};
use crate::config::install::layout::fomod::{FomodOptionType, choose_fomod};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// An answer to a question the walk asked, in the order asked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Answer {
    /// The chosen options' indices.
    Pick(Vec<usize>),
    /// Yes or no, to a wizard asking whether a file is installed.
    Yes(bool),
}

/// What the walk needs to know next.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Asked {
    Choose {
        question: Question,
    },
    /// A wizard asks whether a file is already installed, by a mod earlier
    /// in the list: something only the list's author knows.
    YesNo {
        path: String,
    },
}

/// The installer walked: what the entry records, the actions those choices
/// call for, and anything worth telling the author.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Installed {
    pub chosen: Option<Chosen>,
    /// `[[mods.actions]]` blocks to follow the archive.
    pub actions: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Walk {
    Ask { asked: Asked },
    Done { installed: Installed },
}

/// The list the archive is being added to, as far as an installer asks about
/// it: its load order, and the plugins in it left inactive.
#[derive(Debug, Clone, Default)]
pub struct ListPlugins {
    pub active: Vec<String>,
    pub inactive: Vec<String>,
}

impl ListPlugins {
    pub fn of(list: &crate::config::schema::SourceModlist) -> Self {
        let lower = |names: &[String]| names.iter().map(|p| p.to_ascii_lowercase()).collect();
        Self {
            active: lower(&list.plugins),
            inactive: lower(&list.inactive_plugins),
        }
    }
}

/// What an archive's entry already chooses, to offer as each question's
/// default when its installer is walked again: FOMOD selections by step and
/// group, and BAIN packages by name. Empty for a new entry, where the
/// installer's own choice is the default.
#[derive(Debug, Clone, Default)]
pub struct Prior {
    pub fomod: Vec<crate::config::schema::FomodSelection>,
    pub packages: Vec<String>,
    /// Walk the installer even on defaults, so each choice is written out
    /// (`choose`). Adding leaves an installer nobody walked to its defaults.
    pub always: bool,
}

/// The walk stopped at a question with no answer yet.
#[derive(Debug)]
struct NeedAnswer(Asked);

impl std::fmt::Display for NeedAnswer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the installer asks a question not yet answered")
    }
}

impl std::error::Error for NeedAnswer {}

/// The answers given so far, handed out in order.
struct Answers<'a> {
    given: std::slice::Iter<'a, Answer>,
    /// Take each question's default instead of asking (`add --defaults`, or
    /// no terminal).
    defaults: bool,
}

impl Answers<'_> {
    fn pick(&mut self, question: Question) -> anyhow::Result<Vec<usize>> {
        if let Some(settled) = question.settled() {
            return Ok(settled);
        }
        if self.defaults {
            return Ok(question.default.clone());
        }
        match self.given.next() {
            Some(Answer::Pick(picked)) => question
                .accept(picked)
                .map_err(|problem| anyhow::anyhow!("{}: {problem}", question.title)),
            Some(Answer::Yes(_)) => anyhow::bail!(
                "the answers given do not match the installer's questions (a yes or no for \
                 {:?}); start again",
                question.title
            ),
            None => Err(NeedAnswer(Asked::Choose { question }).into()),
        }
    }

    fn yes_no(&mut self, path: &str) -> anyhow::Result<Option<bool>> {
        if self.defaults {
            return Ok(None);
        }
        match self.given.next() {
            Some(Answer::Yes(yes)) => Ok(Some(*yes)),
            Some(Answer::Pick(_)) => anyhow::bail!(
                "the answers given do not match the installer's questions (a choice for \
                 whether {path} is installed); start again"
            ),
            None => Err(NeedAnswer(Asked::YesNo {
                path: path.to_string(),
            })
            .into()),
        }
    }
}

/// Walk `archive`'s installer (as `report` read it) with `answers` so far:
/// the next question, or what was chosen. With `defaults`, nothing is asked
/// and each question takes what install would pick on its own (a wizard runs
/// on its defaults; a package list with no wizard is left for the author).
pub fn walk(
    archive: &Path,
    report: &InspectReport,
    list: &ListPlugins,
    prior: &Prior,
    answers: &[Answer],
    defaults: bool,
) -> anyhow::Result<Walk> {
    let mut given = Answers {
        given: answers.iter(),
        defaults,
    };
    match walk_with(archive, report, list, prior, &mut given) {
        Ok(installed) => Ok(Walk::Done { installed }),
        Err(err) => match err.downcast::<NeedAnswer>() {
            Ok(NeedAnswer(asked)) => Ok(Walk::Ask { asked }),
            Err(err) => Err(err),
        },
    }
}

fn walk_with(
    archive: &Path,
    report: &InspectReport,
    list: &ListPlugins,
    prior: &Prior,
    answers: &mut Answers<'_>,
) -> anyhow::Result<Installed> {
    let mut installed = Installed::default();
    match &report.layout {
        LayoutGuess::Fomod { config_path } => {
            if answers.defaults && !prior.always {
                installed.notes.push(
                    "it has a FOMOD installer; the entry picks each group's default. `mudcrab \
                     inspect` lists the options, to change `fomod_selections`."
                        .to_string(),
                );
                return Ok(installed);
            }
            let xml = extract_single_entry(archive, config_path)?;
            // A step can depend on which plugins are active; install answers
            // that from the list's load order, so the walk here does too.
            let active: HashSet<String> = list.active.iter().cloned().collect();
            let selections = choose_fomod(&xml, config_path, &active, &mut |group| {
                // What the entry chooses already, where it names this group.
                let default = prior
                    .fomod
                    .iter()
                    .find(|s| s.step == group.step && s.group == group.group)
                    .map(|s| {
                        (0..group.options.len())
                            .filter(|&i| s.options.contains(&group.options[i].name))
                            .collect()
                    })
                    .unwrap_or_else(|| group.default.clone());
                answers.pick(Question {
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
                    default,
                })
            })?;
            installed.chosen = Some(Chosen::Fomod(selections));
        }
        LayoutGuess::Bain { subpackages, .. } if subpackages.len() > 1 => {
            // A wizard asks its own questions, in words, and decides what
            // they mean; run it, rather than ask about folder names.
            let wizard = report
                .top_level_files
                .iter()
                .find(|name| name.eq_ignore_ascii_case("wizard.txt"));
            if let Some(path) = wizard {
                let script = extract_single_entry(archive, path)?;
                let listing = inspect_archive(archive, true)?;
                let package = Package {
                    subpackages: subpackages.clone(),
                    files: listing.files.unwrap_or_default(),
                };
                if let Some(result) = run_wizard(&script, &package, list, answers, &mut installed)?
                {
                    installed.chosen = Some(Chosen::Wizard {
                        subpackages: result.0,
                        exclude: result.1,
                    });
                    return Ok(installed);
                }
                if answers.defaults {
                    return Ok(installed);
                }
                installed.notes.push(
                    "the wizard did not choose; the packages are chosen directly".to_string(),
                );
            } else if answers.defaults && !prior.always {
                installed.notes.push(
                    "it is a BAIN package and every subpackage is listed; delete from \
                     `bain_subpackages` the ones that are alternatives to each other."
                        .to_string(),
                );
                return Ok(installed);
            }
            let picked = answers.pick(Question {
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
                default: if prior.packages.is_empty() {
                    (0..subpackages.len()).collect()
                } else {
                    (0..subpackages.len())
                        .filter(|&i| {
                            prior
                                .packages
                                .iter()
                                .any(|p| p.eq_ignore_ascii_case(&subpackages[i]))
                        })
                        .collect()
                },
            })?;
            installed.chosen = Some(Chosen::Bain(
                picked.into_iter().map(|i| subpackages[i].clone()).collect(),
            ));
        }
        _ => {}
    }
    Ok(installed)
}

/// What a wizard is asked about the rest of the install, answered from the
/// list where it can say -- a plugin is in the load order or not, the game's
/// own files are there -- and otherwise asked.
struct WizardHost<'a, 'b> {
    answers: &'a mut Answers<'b>,
    list: &'a ListPlugins,
    /// Asked once per path in a run, however often the wizard asks.
    answered: HashMap<String, bool>,
    notes: Vec<String>,
}

/// The game's own files: Oblivion GOTY ships them, so any list has them.
fn is_vanilla(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.starts_with("oblivion") && (lower.ends_with(".esm") || lower.ends_with(".bsa"))
        || lower.starts_with("dlc")
        || lower.starts_with("knights")
}

fn is_plugin(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".esp") || lower.ends_with(".esm")
}

impl Host for WizardHost<'_, '_> {
    fn choose(&mut self, prompt: &Prompt) -> anyhow::Result<Vec<usize>> {
        let question = Question {
            title: prompt.question.clone(),
            rule: if prompt.many {
                Rule::Any
            } else {
                Rule::ExactlyOne
            },
            choices: prompt
                .options
                .iter()
                .map(|(name, description)| Choice {
                    name: name.clone(),
                    description: description.clone(),
                    availability: Availability::Free,
                })
                .collect(),
            default: prompt.defaults.clone(),
        };
        self.answers.pick(question)
    }

    fn data_file_exists(&mut self, path: &str) -> anyhow::Result<bool> {
        let normal = path.replace('\\', "/");
        let name = normal.rsplit('/').next().unwrap_or(&normal).to_string();
        if is_vanilla(&name) && !normal.contains('/') {
            return Ok(true);
        }
        if is_plugin(&name) && !normal.contains('/') {
            return Ok(self.list.active.contains(&name.to_ascii_lowercase()));
        }
        let key = normal.to_ascii_lowercase();
        if let Some(answer) = self.answered.get(&key) {
            return Ok(*answer);
        }
        let answer = match self.answers.yes_no(&normal)? {
            Some(answer) => answer,
            None => {
                self.notes.push(format!(
                    "the wizard asks whether `{normal}` is installed; taken as no"
                ));
                false
            }
        };
        self.answered.insert(key, answer);
        Ok(answer)
    }

    fn plugin_status(&mut self, name: &str) -> i64 {
        let lower = name.to_ascii_lowercase();
        if self.list.inactive.contains(&lower) {
            0
        } else if self.list.active.contains(&lower) || is_vanilla(name) {
            1
        } else {
            -1
        }
    }

    fn note(&mut self, text: &str) {
        if !text.trim().is_empty() {
            self.notes.push(format!("wizard: {}", text.trim()));
        }
    }
}

/// Run a wizard: the subpackages it chose and the plugins in them it left
/// out, with the actions its other choices call for added to `installed`.
/// `None` when it cancelled, could not be run, or chose nothing, having said
/// why in the notes.
fn run_wizard(
    script: &str,
    package: &Package,
    list: &ListPlugins,
    answers: &mut Answers<'_>,
    installed: &mut Installed,
) -> anyhow::Result<Option<(Vec<String>, Vec<String>)>> {
    let mut host = WizardHost {
        answers,
        list,
        answered: HashMap::new(),
        notes: Vec::new(),
    };
    let ending = run(script, package, &mut host);
    installed.notes.append(&mut host.notes);
    let outcome = match ending {
        Ok(Ending::Finished(outcome)) => outcome,
        Ok(Ending::Cancelled(message)) => {
            installed
                .notes
                .push(format!("the wizard cancelled: {message}"));
            return Ok(None);
        }
        // A question not yet answered is not a failure: the walk resumes.
        Err(err) if err.is::<NeedAnswer>() => return Err(err),
        Err(err) => {
            installed
                .notes
                .push(format!("the wizard could not be run ({err})"));
            return Ok(None);
        }
    };
    // A package installing nothing is not what anyone adding it meant.
    if outcome.subpackages.is_empty() {
        installed
            .notes
            .push("the wizard's answers select no subpackage".to_string());
        return Ok(None);
    }
    Ok(Some(wizard_entry(outcome, installed)))
}

fn toml_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The entry's parts from what the wizard did. What the modlist has no way to
/// say is noted, not dropped silently.
fn wizard_entry(outcome: Outcome, installed: &mut Installed) -> (Vec<String>, Vec<String>) {
    for (from, to) in &outcome.renamed_plugins {
        installed.actions.push(format!(
            "[[mods.actions]]\naction = \"file_move\"\nfrom = {}\nto = {}",
            toml_string(from),
            toml_string(to)
        ));
    }
    for edit in &outcome.ini_edits {
        // `set` is how an OBSE script's INI is written; Wrye Bash files the
        // fragment under the INI's own name, wherever the INI lives.
        let file = edit.file.rsplit(['\\', '/']).next().unwrap_or(&edit.file);
        if edit.section.eq_ignore_ascii_case("set") || edit.section.eq_ignore_ascii_case("setgs") {
            installed.actions.push(format!(
                "[[mods.actions]]\naction = \"ini_tweak\"\nfile = {}\nkey = {}\nvalue = {}\nformat = \"set-to\"",
                toml_string(file),
                toml_string(&edit.key),
                toml_string(&edit.value)
            ));
        } else {
            installed.notes.push(format!(
                "the wizard sets [{}] {} = {} in {}; ini_tweak cannot say a section yet, so it \
                 is not recorded",
                edit.section, edit.key, edit.value, edit.file
            ));
        }
    }
    for (file, section, key) in &outcome.disabled_ini_lines {
        installed.notes.push(format!(
            "the wizard disables [{section}] {key} in {file}; not recorded"
        ));
    }
    (outcome.subpackages, outcome.left_out_plugins)
}

/// The `[[mods.archives]]` block (and any actions after it) for `archive`,
/// from `source` (`nexus:...`, `manual:...`) and the installer as walked. A
/// hand-fetched archive also records its size and MD5, which find it under
/// any name its host gives it.
pub fn archive_block(
    archive: &Path,
    report: &InspectReport,
    source: &str,
    installed: &Installed,
) -> anyhow::Result<String> {
    // Comments in the snippet talk to someone reading `inspect`'s report; in
    // the list they would only be noise.
    let block = render_toml_snippet_with(
        &report.file_name,
        Some(source),
        &report.layout,
        report.fomod.as_ref(),
        installed.chosen.as_ref(),
    )
    .lines()
    .filter(|line| !line.trim_start().starts_with('#'))
    .collect::<Vec<_>>()
    .join("\n");
    let block = if source.starts_with("manual:") {
        let size = std::fs::metadata(archive)
            .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", archive.display()))?
            .len();
        let md5 = crate::config::identify::md5_of_file(archive)?;
        block
            .lines()
            .flat_map(|line| {
                let mut out = vec![line.to_string()];
                if line.starts_with("file_name = ") {
                    out.push(format!("size = {size}"));
                    out.push(format!("md5 = \"{md5}\""));
                    // Printed by `download` and `check`, so whoever installs
                    // the list knows where to go without opening it.
                    // A Nexus download Nexus could not name (or that was
                    // not asked) still names its mod: its page, at least.
                    out.push(
                        match crate::config::identify::nexus_mod_in_name(&report.file_name) {
                            Some(id) => format!(
                                "download_page = \"https://www.nexusmods.com/oblivion/mods/{id}?tab=files\"  # from the file's name"
                            ),
                            None => "# download_page = \"https://...\"".to_string(),
                        },
                    );
                    out.push("# download_note = \"Which file on the page, and how.\"".to_string());
                }
                out
            })
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        block
    };
    // Actions the installer's choices call for follow the archive.
    Ok(installed
        .actions
        .iter()
        .fold(block, |block, action| format!("{block}\n\n{action}")))
}

/// Where `archive` came from, as a modlist `path`: what the MO2 `.meta` beside
/// it says, else `manual:<its name>`.
pub fn source_of(archive: &Path, report: &InspectReport) -> String {
    crate::config::archive_store::meta_source(archive)
        .unwrap_or_else(|| format!("manual:{}", report.file_name))
}

/// An archive of the list, found for walking its installer again.
pub struct Existing {
    /// Where the archive is.
    pub archive: std::path::PathBuf,
    pub report: InspectReport,
    /// What its entry chooses now.
    pub prior: Prior,
    pub list: ListPlugins,
}

/// Mod `id`'s `number`th archive (from 1) in the list at `path`, as planned
/// with `overrides` answering its questions, found in `downloads` or
/// `search`; with what its entry chooses now.
pub fn existing(
    path: &Path,
    id: &str,
    number: usize,
    overrides: &crate::config::query::Overrides,
    downloads: &Path,
    search: &[std::path::PathBuf],
) -> anyhow::Result<Existing> {
    let text = std::fs::read_to_string(path)
        .map_err(|err| anyhow::anyhow!("failed to read {}: {err}", path.display()))?;
    let source = crate::config::loader::parse_modlist(&text, path)?;
    let spec = source
        .mods
        .iter()
        .find(|entry| entry.id == id)
        .and_then(|entry| entry.archives.get(number.wrapping_sub(1)))
        .ok_or_else(|| anyhow::anyhow!("'{id}' has no archive {number}"))?;
    let planned = crate::config::pipeline::planned_archive(&text, path, id, number, overrides)?;
    let reference = crate::config::archive_store::ArchiveRef::for_archive(&planned.archive);
    let archive =
        crate::config::archive_store::find(downloads, search, &reference).ok_or_else(|| {
            anyhow::anyhow!(
                "'{id}'s archive {number} is not in {}; download it first",
                downloads.display()
            )
        })?;
    let report = inspect_archive(&archive, false)?;
    Ok(Existing {
        prior: Prior {
            fomod: spec.fomod_selections.clone(),
            packages: spec
                .bain_subpackages
                .iter()
                .map(|p| p.name().to_string())
                .collect(),
            always: true,
        },
        list: ListPlugins::of(&source),
        report,
        archive,
    })
}

/// Plan writing what the walk chose into mod `id`'s `number`th archive, in
/// the list at `path`.
pub fn plan_choice(
    path: &Path,
    id: &str,
    number: usize,
    installed: &Installed,
) -> anyhow::Result<crate::config::edit::Edit> {
    let chosen = installed
        .chosen
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("this archive's installer has nothing to choose"))?;
    crate::config::edit::plan_mod(path, id, |text, _, id| {
        crate::config::edit::doc::set_installer(text, id, number, chosen)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const CONFIG: &str = r#"<config>
  <moduleName>Fort</moduleName>
  <installSteps order="Explicit">
    <installStep name="Main">
      <optionalFileGroups order="Explicit">
        <group name="Core" type="SelectExactlyOne">
          <plugins order="Explicit">
            <plugin name="Full"><files><folder source="full" destination="" /></files>
              <typeDescriptor><type name="Recommended" /></typeDescriptor></plugin>
            <plugin name="Lite"><files><folder source="lite" destination="" /></files>
              <typeDescriptor><type name="Optional" /></typeDescriptor></plugin>
          </plugins>
        </group>
      </optionalFileGroups>
    </installStep>
    <installStep name="Extras">
      <optionalFileGroups order="Explicit">
        <group name="Always" type="SelectAll">
          <plugins order="Explicit">
            <plugin name="Base"><files><file source="base.txt" destination="base.txt" /></files>
              <typeDescriptor><type name="Required" /></typeDescriptor></plugin>
          </plugins>
        </group>
        <group name="Patches" type="SelectAny">
          <plugins order="Explicit">
            <plugin name="UOP"><files><file source="uop.esp" destination="uop.esp" /></files>
              <typeDescriptor><type name="Optional" /></typeDescriptor></plugin>
          </plugins>
        </group>
      </optionalFileGroups>
    </installStep>
  </installSteps>
</config>"#;

    fn archive(dir: &Path) -> std::path::PathBuf {
        let path = dir.join("Fort-1-0.zip");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        let options: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
        for (name, bytes) in [
            ("fomod/ModuleConfig.xml", CONFIG.as_bytes()),
            ("full/Meshes/a.nif", b"x"),
            ("lite/Meshes/a.nif", b"x"),
            ("base.txt", b"x"),
            ("uop.esp", b"TES4"),
        ] {
            zip.start_file(name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
        path
    }

    fn asked(walked: Walk) -> Question {
        match walked {
            Walk::Ask {
                asked: Asked::Choose { question },
            } => question,
            other => panic!("expected a question, got {other:?}"),
        }
    }

    #[test]
    fn a_fomod_is_walked_a_question_at_a_time() {
        let dir = tempfile::tempdir().unwrap();
        let archive = archive(dir.path());
        let report = inspect_archive(&archive, false).unwrap();
        let list = ListPlugins::default();

        let first = asked(walk(&archive, &report, &list, &Prior::default(), &[], false).unwrap());
        assert_eq!(first.title, "Main: Core");
        assert_eq!(first.default, [0]);
        // "Always" has nothing to decide, so the next question is "Patches".
        let answers = [Answer::Pick(vec![1])];
        let second =
            asked(walk(&archive, &report, &list, &Prior::default(), &answers, false).unwrap());
        assert_eq!(second.title, "Extras: Patches");

        let answers = [Answer::Pick(vec![1]), Answer::Pick(vec![0])];
        let Walk::Done { installed } =
            walk(&archive, &report, &list, &Prior::default(), &answers, false).unwrap()
        else {
            panic!("the walk should be done");
        };
        let Some(Chosen::Fomod(selections)) = &installed.chosen else {
            panic!("expected FOMOD selections");
        };
        let picked: Vec<(&str, &[String])> = selections
            .iter()
            .map(|s| (s.group.as_str(), s.options.as_slice()))
            .collect();
        assert_eq!(
            picked,
            [
                ("Core", &["Lite".to_string()][..]),
                ("Always", &["Base".to_string()][..]),
                ("Patches", &["UOP".to_string()][..])
            ]
        );
        let block = archive_block(&archive, &report, "manual:Fort-1-0.zip", &installed).unwrap();
        assert!(block.contains("\"Lite\""), "{block}");
        assert!(block.contains("md5 = "), "{block}");

        // An answer the rule refuses, or of the wrong kind, is an error.
        assert!(
            walk(
                &archive,
                &report,
                &list,
                &Prior::default(),
                &[Answer::Pick(vec![0, 1])],
                false
            )
            .is_err()
        );
        assert!(
            walk(
                &archive,
                &report,
                &list,
                &Prior::default(),
                &[Answer::Yes(true)],
                false
            )
            .is_err()
        );
        // On defaults nothing is asked, and the entry is left to its defaults.
        let Walk::Done { installed } =
            walk(&archive, &report, &list, &Prior::default(), &[], true).unwrap()
        else {
            panic!("defaults ask nothing");
        };
        assert!(installed.chosen.is_none());
        assert_eq!(installed.notes.len(), 1);
    }
}
