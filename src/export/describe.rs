//! Install data in words: what a guide tells a person to do by hand.
//!
//! Every exporter needs the same sentences -- "in the BAIN installer select
//! ...", "set `x` to 1 in `y.ini`" -- so they are written once, here, and
//! templates receive them as text next to the raw data they came from.
//! Text is Markdown, like the docs an author writes.

use crate::config::condition::{Condition, InputTest};
use crate::config::schema::{CompiledArchive, InputSpec, ModAction};
use indexmap::IndexMap;

/// A condition as a reader would say it: "you answer yes to “Install
/// Swearing Rats?”", "ENBSeries is installed".
pub fn condition(cond: &Condition, inputs: &IndexMap<String, InputSpec>) -> String {
    match cond {
        Condition::Any(terms) => join(terms, inputs, " or "),
        Condition::All(terms) => join(terms, inputs, " and "),
        Condition::Not(inner) => match inner.as_ref() {
            Condition::Input {
                key,
                test: InputTest::Truthy,
            } => {
                format!("you answer no to “{}”", query(key, inputs))
            }
            Condition::Input {
                key,
                test: InputTest::Equals(value),
            } => {
                format!("you do not choose “{value}” for “{}”", query(key, inputs))
            }
            Condition::Input {
                key,
                test: InputTest::NotEquals(value),
            } => {
                format!("you choose “{value}” for “{}”", query(key, inputs))
            }
            Condition::Mod(id) => format!("{id} is not installed"),
            other => format!("not ({})", condition(other, inputs)),
        },
        Condition::Input { key, test } => match test {
            InputTest::Truthy => format!("you answer yes to “{}”", query(key, inputs)),
            InputTest::Equals(value) => {
                format!("you choose “{value}” for “{}”", query(key, inputs))
            }
            InputTest::NotEquals(value) => {
                format!("you do not choose “{value}” for “{}”", query(key, inputs))
            }
        },
        Condition::Mod(id) => format!("{id} is installed"),
    }
}

fn join(terms: &[Condition], inputs: &IndexMap<String, InputSpec>, sep: &str) -> String {
    terms
        .iter()
        .map(|term| condition(term, inputs))
        .collect::<Vec<_>>()
        .join(sep)
}

fn query<'a>(key: &'a str, inputs: &'a IndexMap<String, InputSpec>) -> &'a str {
    inputs
        .get(key)
        .map(|spec| spec.query.as_str())
        .unwrap_or(key)
}

fn code(text: &str) -> String {
    format!("`{text}`")
}

fn code_list(items: &[String]) -> String {
    items
        .iter()
        .map(|item| code(item))
        .collect::<Vec<_>>()
        .join(", ")
}

/// One thing to do, in words, with the kind of thing it is.
///
/// The kind is what an exporter keys its symbols on (`qac` gets a broom, say),
/// so it names the activity rather than the action that implements it: an
/// `ini_set` and an `ini_append_block` are both `ini`. Kinds:
///
/// - installing: `inner_archive`, `bain`, `fomod`, `data_folder`, `subdir`,
///   `include`, `exclude`, `game_root`;
/// - afterwards: `ini`, `qac`, `bsa`, `plugin`, `delete`, `hide`, `move`,
///   `extract`, `records`, `texture`, `write`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Instruction {
    pub kind: &'static str,
    pub text: String,
}

/// What to do with an archive beyond "install it": installer choices, the
/// folder to take, the files to leave out. Empty for an archive installed as
/// it comes.
pub fn archive_instructions(archive: &CompiledArchive) -> Vec<Instruction> {
    let mut out = Vec::new();
    let mut push = |kind: &'static str, text: String| out.push(Instruction { kind, text });
    if let Some(inner) = &archive.inner_archive {
        push(
            "inner_archive",
            format!(
                "The mod is the archive inside it, {}; install that.",
                code(inner)
            ),
        );
    }
    if !archive.bain_subpackages.is_empty() {
        let names: Vec<String> = archive
            .bain_subpackages
            .iter()
            .map(|s| s.name().to_string())
            .collect();
        push(
            "bain",
            format!("In the BAIN installer, select {}.", code_list(&names)),
        );
    }
    for selection in &archive.fomod_selections {
        let chosen = if selection.options.is_empty() {
            "nothing".to_string()
        } else {
            selection
                .options
                .iter()
                .map(|o| format!("“{o}”"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        push(
            "fomod",
            format!(
                "In the installer's “{}” step, under “{}”, choose {chosen}.",
                selection.step, selection.group
            ),
        );
    }
    if let Some(folder) = &archive.data_folder {
        push(
            "data_folder",
            format!(
                "Install the contents of {} as the mod's data folder.",
                code(folder)
            ),
        );
    }
    if let Some(subdir) = &archive.target_subdir {
        push(
            "subdir",
            format!("Install it into {} inside the mod.", code(subdir)),
        );
    }
    if !archive.include.is_empty() {
        push(
            "include",
            format!("Install only {}.", code_list(&archive.include)),
        );
    }
    if !archive.exclude.is_empty() {
        push(
            "exclude",
            format!("Leave out {}.", code_list(&archive.exclude)),
        );
    }
    if !archive.game_root_files.is_empty() {
        push(
            "game_root",
            format!(
                "{} go{} in the game folder, next to `Oblivion.exe` (Root Builder deploys them from the mod's `root/`).",
                code_list(&archive.game_root_files),
                if archive.game_root_files.len() == 1 {
                    "es"
                } else {
                    ""
                }
            ),
        );
    }
    out
}

/// One step, and a block of literal text that goes with it (an INI block to
/// append, a file's contents).
pub struct Step {
    pub kind: &'static str,
    pub text: String,
    pub code: Option<String>,
}

/// The kind of an action, as [`Instruction::kind`].
pub fn action_kind(action: &ModAction) -> &'static str {
    match action {
        ModAction::IniSet(_) | ModAction::IniTweak(_) | ModAction::IniAppendBlock(_) => "ini",
        ModAction::Qac(_) => "qac",
        ModAction::PackBsa(_) => "bsa",
        ModAction::CreateDummyPlugin(_) => "plugin",
        ModAction::FilePrune(_) => "delete",
        ModAction::FileHide(_) => "hide",
        ModAction::FileMove(_) => "move",
        ModAction::ExtractBsa(_) => "extract",
        ModAction::DeleteRecords(_) => "records",
        ModAction::DdsDecompress(_) => "texture",
        ModAction::WriteFile(_) => "write",
    }
}

pub fn action(action: &ModAction) -> Step {
    let kind = action_kind(action);
    let plain = |text: String| Step {
        kind,
        text,
        code: None,
    };
    match action {
        ModAction::IniSet(set) => {
            let section = set
                .section
                .as_deref()
                .map(|s| format!(" under `[{s}]`"))
                .unwrap_or_default();
            let file = match set.scope {
                crate::config::schema::IniScope::Game => format!("the game's {}", code(&set.file)),
                crate::config::schema::IniScope::Mod => code(&set.file),
            };
            plain(format!(
                "In {file}, set {}{section} to {}.",
                code(&set.key),
                code(&set.value.to_string())
            ))
        }
        ModAction::IniTweak(tweak) => plain(format!(
            "In {}, set {} to {}.",
            code(&tweak.file),
            code(&tweak.key),
            code(&tweak.value.to_string())
        )),
        ModAction::IniAppendBlock(block) => Step {
            kind,
            text: format!("Add this to the end of {}:", code(&block.file)),
            code: Some(block.block.clone()),
        },
        ModAction::Qac(qac) => plain(format!(
            "Clean {} with xEdit's Quick Auto Clean.",
            code_list(&qac.plugins)
        )),
        ModAction::PackBsa(pack) => plain(format!(
            "Pack the mod's loose files into {}.",
            code(&pack.output)
        )),
        ModAction::CreateDummyPlugin(dummy) => plain(format!(
            "Create an empty plugin, {}, so the game loads the archive of the same name.",
            code(&dummy.output)
        )),
        ModAction::FilePrune(prune) => plain(selection_step(
            "Delete",
            &prune.paths,
            &prune.conflicts_with,
            prune.under.as_deref(),
            &prune.except,
        )),
        ModAction::FileHide(hide) => plain(format!(
            "{} (MO2: rename to `.mohidden`).",
            selection_step(
                "Hide",
                &hide.paths,
                &hide.conflicts_with,
                hide.under.as_deref(),
                &hide.except
            )
            .trim_end_matches('.')
        )),
        ModAction::FileMove(file_move) => plain(format!(
            "Move {} to {}.",
            code(&file_move.from),
            code(&file_move.to)
        )),
        ModAction::ExtractBsa(extract) => plain(if extract.keep_archive {
            format!("Extract {}, keeping the archive.", code(&extract.archive))
        } else {
            format!("Extract {} and delete it.", code(&extract.archive))
        }),
        ModAction::DeleteRecords(delete) => {
            let mut parts = Vec::new();
            if !delete.groups.is_empty() {
                parts.push(format!("the {} group(s)", code_list(&delete.groups)));
            }
            if !delete.form_ids.is_empty() {
                parts.push(format!("record(s) {}", code_list(&delete.form_ids)));
            }
            plain(format!(
                "In xEdit, remove {} from {}.",
                parts.join(" and "),
                code(&delete.plugin)
            ))
        }
        ModAction::DdsDecompress(dds) => plain(format!(
            "Decompress the DXT1 textures with transparency under {} (they draw black under DXVK).",
            code_list(&dds.paths)
        )),
        ModAction::WriteFile(write) => Step {
            kind,
            text: format!("Create {} containing:", code(&write.path)),
            code: Some(write.content.clone()),
        },
    }
}

fn selection_step(
    verb: &str,
    paths: &[String],
    conflicts_with: &[String],
    under: Option<&str>,
    except: &[String],
) -> String {
    let mut what = Vec::new();
    if !paths.is_empty() {
        what.push(code_list(paths));
    }
    if !conflicts_with.is_empty() {
        let names = conflicts_with
            .iter()
            .map(|m| format!("“{m}”"))
            .collect::<Vec<_>>()
            .join(", ");
        let scope = under
            .map(|u| format!(" under {}", code(u)))
            .unwrap_or_default();
        what.push(format!("the files it shares with {names}{scope}"));
    }
    let except = if except.is_empty() {
        String::new()
    } else {
        format!(", except {}", code_list(except))
    };
    format!("{verb} {}{except}.", what.join(" and "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::{InputType, InputValue};

    #[test]
    fn conditions_read_as_sentences() {
        let inputs = IndexMap::from([(
            "rats".to_string(),
            InputSpec {
                input_type: InputType::Bool,
                query: "Install Swearing Rats?".to_string(),
                choices: Vec::new(),
                default: Some(InputValue::Bool(true)),
            },
        )]);
        let say = |expr: &str| condition(&Condition::parse(expr).unwrap(), &inputs);
        assert_eq!(say("rats"), "you answer yes to “Install Swearing Rats?”");
        assert_eq!(say("!rats"), "you answer no to “Install Swearing Rats?”");
        assert_eq!(
            say("mod:A && !mod:\"B C\""),
            "A is installed and B C is not installed"
        );
        assert_eq!(say("enb == v0500"), "you choose “v0500” for “enb”");
    }
}
