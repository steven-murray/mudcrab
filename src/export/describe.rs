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

/// An INI value as a reader should type it; an empty one has to be said.
fn value(text: &str) -> String {
    if text.is_empty() {
        "nothing (leave it empty)".to_string()
    } else {
        code(text)
    }
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
///
/// `listing` is the archive's entries, when the archive is to hand: with it,
/// patterns are spelled as the files they match, and a data folder MO2 finds
/// by itself goes unmentioned.
pub fn archive_instructions(
    archive: &CompiledArchive,
    listing: Option<&[String]>,
    wizard: Option<&crate::config::bain_wizard::Wizard>,
) -> Vec<Instruction> {
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
    let names: Vec<String> = archive
        .bain_subpackages
        .iter()
        .map(|s| s.name().to_string())
        .collect();
    // The archive's own wizard says it the way a person clicks it.
    let answers = wizard.and_then(|w| w.explain(&names));
    if let Some(answers) = &answers {
        for (question, chosen) in answers {
            let chosen = chosen
                .iter()
                .map(|c| format!("“{c}”"))
                .collect::<Vec<_>>()
                .join(", ");
            push(
                "bain",
                format!("In the BAIN wizard, at “{question}”, choose {chosen}."),
            );
        }
    } else if !archive.bain_subpackages.is_empty() {
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
    // What the archive holds, below the folder the mod is made from, when the
    // archive is at hand: patterns become the files they mean.
    let content: Option<Vec<String>> =
        listing.map(|paths| content_paths(paths, archive.data_folder.as_deref()));
    if let Some(folder) = &archive.data_folder
        && !listing.is_some_and(|paths| mo2_unwraps(paths, folder))
    {
        push(
            "data_folder",
            format!(
                "Manual install: in MO2's installer, right-click {} and choose *Set data directory*.",
                code(folder)
            ),
        );
    }
    let included = content
        .as_deref()
        .and_then(|c| matching(c, &archive.include, true));
    if let Some(subdir) = &archive.target_subdir {
        let what = match &included {
            Some(files) if !archive.include.is_empty() => code_list(files),
            _ => "the archive's contents".to_string(),
        };
        let them = if what.starts_with("the") || what.contains(", ") {
            "them"
        } else {
            "it"
        };
        let subdir = subdir.trim_matches('/');
        let text = if let Some(rest) = subdir
            .strip_prefix("root")
            .filter(|r| r.is_empty() || r.starts_with('/'))
        {
            // Root Builder's folder: the files belong beside Oblivion.exe.
            let place = rest.trim_start_matches('/');
            let place = if place.is_empty() {
                String::new()
            } else {
                format!(" in {}", code(place))
            };
            format!(
                "{what} go{} in the game folder{place}, next to `Oblivion.exe`. With Root Builder, \
                 put {them} in a new `root/` folder inside the mod instead.",
                if them == "it" { "es" } else { "" }
            )
        } else {
            let levels: Vec<&str> = subdir.split('/').collect();
            let steps = levels
                .iter()
                .enumerate()
                .map(|(i, level)| {
                    if i == 0 {
                        code(level)
                    } else {
                        format!("then {} inside it", code(level))
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            let last = code(levels.last().expect("split yields one"));
            let drag = if what.starts_with("the archive's") {
                "all of the archive's original files and folders".to_string()
            } else {
                what.clone()
            };
            format!(
                "Manual install: {what} belong{} in a new {} folder. In MO2's installer, right-click \
                 and *Create directory* {steps}, then drag {drag} into {last}.",
                if them == "it" { "s" } else { "" },
                code(&format!("{subdir}/"))
            )
        };
        push("subdir", text);
    } else if !archive.include.is_empty() {
        let what = included
            .map(|files| code_list(&files))
            .unwrap_or_else(|| glob_words(&archive.include));
        push("include", format!("Install only {what}."));
    }
    // An archive that is a packaged MO2 mod folder ships MO2's own
    // `meta.ini`; leaving it out is install mechanics (mudcrab writes that
    // file), not something a person installing through MO2 ever does.
    let exclude: Vec<String> = archive
        .exclude
        .iter()
        .filter(|p| {
            !p.rsplit('/')
                .next()
                .is_some_and(|name| name.eq_ignore_ascii_case("meta.ini"))
        })
        .cloned()
        .collect();
    if !exclude.is_empty() {
        let what = content
            .as_deref()
            .and_then(|c| matching(c, &exclude, false))
            .map(|files| code_list(&files))
            .unwrap_or_else(|| glob_words(&exclude));
        push("exclude", format!("Leave out {what}."));
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

/// Files below `data_folder` (all files, without one), relative to it.
fn content_paths(paths: &[String], data_folder: Option<&str>) -> Vec<String> {
    // Without a data folder, paths are given as MO2's installer shows them:
    // it opens a top-level `Data` (or an archive's only folder) by itself, so
    // a reader never sees that level.
    let folder = data_folder.map(str::to_string).or_else(|| mo2_root(paths));
    let prefix = folder.map(|f| format!("{}/", f.trim_matches('/').to_ascii_lowercase()));
    paths
        .iter()
        .filter_map(|path| match &prefix {
            Some(prefix) if path.to_ascii_lowercase().starts_with(prefix.as_str()) => {
                Some(path[prefix.len()..].to_string())
            }
            Some(_) => None,
            None => Some(path.clone()),
        })
        .filter(|path| !path.is_empty() && !path.ends_with('/'))
        .collect()
}

/// The files the patterns pick out (`keep` = include semantics), if few
/// enough to list; a reader is better off with a pattern than with forty
/// names.
///
/// A pattern naming one file wherever it is (`**/x.dll`) is always better as
/// its path. A wildcard (`*.esp`) says what it means more plainly than a list
/// of the files it matches, unless it matches only one or two.
fn matching(content: &[String], patterns: &[String], keep: bool) -> Option<Vec<String>> {
    let wildcard = patterns.iter().any(|p| {
        p.rsplit('/')
            .next()
            .is_some_and(|name| name.contains(['*', '?', '[']))
    });
    let filters = if keep {
        crate::archive::ArchiveFilters::new(patterns, &[]).ok()?
    } else {
        crate::archive::ArchiveFilters::new(&[], patterns).ok()?
    };
    let hits: Vec<String> = content
        .iter()
        .filter(|path| filters.should_extract(path) == keep)
        .cloned()
        .collect();
    let limit = if wildcard { 2 } else { 8 };
    (!hits.is_empty() && hits.len() <= limit).then_some(hits)
}

/// Globs as words: `**/x.dll` is "`x.dll`, wherever it is in the archive",
/// and `*.esp` "every `.esp` file", which is what they mean and easier to act
/// on than the pattern.
fn glob_words(patterns: &[String]) -> String {
    patterns
        .iter()
        .map(|pattern| {
            let (anywhere, name) = match pattern.strip_prefix("**/") {
                Some(rest) => (true, rest),
                None => (false, pattern.as_str()),
            };
            if name.contains('/') {
                return code(pattern);
            }
            let place = if anywhere {
                ", wherever it is in the archive"
            } else {
                ""
            };
            match name.strip_prefix('*') {
                Some(ext) if ext.starts_with('.') && !ext.contains(['*', '?', '[']) => {
                    format!(
                        "every {} file{}",
                        code(ext),
                        if anywhere { " in the archive" } else { "" }
                    )
                }
                _ if !name.contains(['*', '?', '[']) => format!("{}{place}", code(name)),
                _ => code(pattern),
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// The folder MO2's installer opens by itself, if any: a top-level `Data`, or
/// the archive's only top-level entry when that is a folder.
fn mo2_root(paths: &[String]) -> Option<String> {
    let tops: std::collections::BTreeSet<&str> = paths
        .iter()
        .filter_map(|p| p.split('/').next())
        .filter(|t| !t.is_empty())
        .collect();
    if let Some(data) = tops.iter().find(|t| t.eq_ignore_ascii_case("data")) {
        return Some((*data).to_string());
    }
    let only = (tops.len() == 1).then(|| *tops.iter().next().expect("one"))?;
    paths
        .iter()
        .all(|p| p.contains('/'))
        .then(|| only.to_string())
}

/// Whether MO2's installer would take `folder` as the data folder by itself:
/// it is the archive's only top-level entry, which MO2 steps into, or it is a
/// top-level `Data`.
pub fn mo2_unwraps(paths: &[String], folder: &str) -> bool {
    let folder = folder.trim_matches('/');
    if folder.contains('/') {
        return false;
    }
    // MO2 also takes a top-level `Data` folder as the data folder, whatever
    // sits beside it.
    if folder.eq_ignore_ascii_case("data") {
        return paths.iter().any(|p| {
            p.split('/')
                .next()
                .is_some_and(|t| t.eq_ignore_ascii_case("data"))
                && p.contains('/')
        });
    }
    let mut tops = paths
        .iter()
        .filter_map(|p| p.split('/').next())
        .filter(|t| !t.is_empty());
    let Some(first) = tops.next() else {
        return false;
    };
    first.eq_ignore_ascii_case(folder)
        && tops.all(|t| t.eq_ignore_ascii_case(first))
        && paths.iter().all(|p| p.contains('/'))
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
        ModAction::IniSet(_)
        | ModAction::IniTweak(_)
        | ModAction::IniAppendBlock(_)
        | ModAction::IniComment(_) => "ini",
        ModAction::Qac(_) => "qac",
        ModAction::PackBsa(_) => "bsa",
        ModAction::CreateDummyPlugin(_) => "plugin",
        ModAction::FilePrune(_) => "delete",
        ModAction::FileHide(_) => "delete",
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
                value(&set.value.to_string())
            ))
        }
        ModAction::IniTweak(tweak) => plain(format!(
            "In {}, set {} to {}.",
            code(&tweak.file),
            code(&tweak.key),
            value(&tweak.value.to_string())
        )),
        ModAction::IniComment(comment) => plain(format!(
            "In {}, comment out the {} line (put `;` at its start).",
            code(&comment.file),
            code(&comment.key)
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
        ModAction::FilePrune(prune) if !prune.keep.is_empty() => plain(format!(
            "Delete everything in the mod except {}.",
            code_list(&prune.keep)
        )),
        ModAction::FilePrune(prune) => plain(selection_step(
            "Delete",
            &prune.paths,
            &prune.conflicts_with,
            prune.under.as_deref(),
            &prune.except,
        )),
        // mudcrab hides (MO2's `.mohidden`) so the archive stays intact, but
        // to the game the two are the same, and "delete" is one instruction
        // where "hide or delete" is a choice the reader has to make.
        ModAction::FileHide(hide) => Step {
            kind: "delete",
            text: selection_step(
                "Delete",
                &hide.paths,
                &hide.conflicts_with,
                hide.under.as_deref(),
                &hide.except,
            ),
            code: None,
        },
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
    let how = if conflicts_with.is_empty() {
        String::new()
    } else {
        " In MO2, open this mod's Conflicts tab: they are the files it lists against those mods, \
         whichever side wins."
            .to_string()
    };
    format!("{verb} {}{except}.{how}", what.join(" and "))
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

    fn archive(f: impl FnOnce(&mut CompiledArchive)) -> CompiledArchive {
        let mut archive = CompiledArchive::default();
        f(&mut archive);
        archive
    }

    fn texts(archive: &CompiledArchive, listing: Option<&[String]>) -> Vec<String> {
        archive_instructions(archive, listing, None)
            .into_iter()
            .map(|i| i.text)
            .collect()
    }

    #[test]
    fn patterns_become_the_files_they_mean_when_the_archive_is_at_hand() {
        let excluding = archive(|a| {
            a.data_folder = Some("Oblivion/Data".into());
            a.exclude = vec!["**/AddActorValues_CS.dll".into()];
        });
        let listing: Vec<String> = [
            "Oblivion/Data/OBSE/Plugins/AddActorValues.dll",
            "Oblivion/Data/OBSE/Plugins/AddActorValues_CS.dll",
        ]
        .map(String::from)
        .into();
        let with = texts(&excluding, Some(&listing));
        assert!(
            with.contains(&"Leave out `OBSE/Plugins/AddActorValues_CS.dll`.".to_string()),
            "{with:?}"
        );
        let without = texts(&excluding, None);
        assert!(
            without.contains(
                &"Leave out `AddActorValues_CS.dll`, wherever it is in the archive.".to_string()
            ),
            "{without:?}"
        );
    }

    #[test]
    fn paths_are_given_as_mo2_shows_them() {
        let oco = archive(|a| a.exclude = vec!["**/Oblivion_Character_Overhaul.esp".into()]);
        let listing: Vec<String> = vec![
            "Data/Oblivion_Character_Overhaul.esp".into(),
            "Data/meshes/x.nif".into(),
            "readme.txt".into(),
        ];
        assert_eq!(
            texts(&oco, Some(&listing)),
            ["Leave out `Oblivion_Character_Overhaul.esp`."]
        );
    }

    #[test]
    fn meta_ini_is_not_a_readers_concern() {
        let packaged = archive(|a| a.exclude = vec!["**/meta.ini".into()]);
        assert!(texts(&packaged, None).is_empty());
        let both = archive(|a| a.exclude = vec!["**/meta.ini".into(), "*.txt".into()]);
        assert_eq!(texts(&both, None), ["Leave out every `.txt` file."]);
    }

    #[test]
    fn a_wildcard_stays_a_pattern_in_words() {
        let esps = archive(|a| a.exclude = vec!["**/*.esp".into(), "*.txt".into()]);
        let listing: Vec<String> = vec!["a.esp".into(), "b/c.esp".into(), "readme.txt".into()];
        assert_eq!(
            texts(&esps, Some(&listing)),
            ["Leave out every `.esp` file in the archive, every `.txt` file."]
        );
    }

    #[test]
    fn a_data_folder_mo2_finds_itself_needs_no_instruction() {
        let wrapped = archive(|a| a.data_folder = Some("Non Selectable Player".into()));
        let only: Vec<String> = vec!["Non Selectable Player/OBSE/Plugins/x.dll".into()];
        assert!(texts(&wrapped, Some(&only)).is_empty());
        let beside: Vec<String> = vec![
            "Non Selectable Player/OBSE/Plugins/x.dll".into(),
            "readme.txt".into(),
        ];
        assert_eq!(texts(&wrapped, Some(&beside)).len(), 1);
        // MO2 takes a top-level Data folder whatever is beside it.
        let data = archive(|a| a.data_folder = Some("Data".into()));
        let with_readme: Vec<String> = vec!["Data/x.esp".into(), "readme.txt".into()];
        assert!(texts(&data, Some(&with_readme)).is_empty());
        // Unknown without the archive: say it.
        assert_eq!(texts(&wrapped, None).len(), 1);
    }

    #[test]
    fn a_target_folder_is_spelled_out_level_by_level() {
        let dll = archive(|a| {
            a.target_subdir = Some("OBSE/Plugins".into());
            a.include = vec!["**/*.dll".into()];
        });
        let listing: Vec<String> = vec!["FractionalMagicDamage.dll".into(), "readme.txt".into()];
        assert_eq!(
            texts(&dll, Some(&listing)),
            [
                "Manual install: `FractionalMagicDamage.dll` belongs in a new `OBSE/Plugins/` folder. In MO2's \
              installer, right-click and *Create directory* `OBSE`, then `Plugins` inside it, then drag `FractionalMagicDamage.dll` into `Plugins`."
            ]
        );
    }
}
