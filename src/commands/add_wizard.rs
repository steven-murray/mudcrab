//! Running a BAIN wizard for `add`, and turning what it does into the entry.
//!
//! The wizard's questions go to the person (or take its defaults when nobody
//! can be asked). What it asks about the rest of the install is answered from
//! the modlist where the list can say -- a plugin is in the load order or not,
//! the game's own files are there -- and otherwise put to the person too:
//! whether some other mod's file is installed is something only the list's
//! author knows.

use super::choose::{Availability, Choice, Question, Rule, ask};
use crate::config::bain_wizard::run::{Ending, Host, Outcome, Package, Prompt, run};
use std::collections::HashMap;
use std::io::{BufRead, Write};

/// What the wizard decided, ready for the entry.
pub(crate) struct WizardResult {
    pub subpackages: Vec<String>,
    pub exclude: Vec<String>,
    /// `[[mods.actions]]` blocks to follow the archive.
    pub actions: Vec<String>,
}

struct AddHost<'a> {
    input: &'a mut dyn BufRead,
    output: &'a mut dyn Write,
    interactive: bool,
    /// The list's load order, lowercased.
    plugins: Vec<String>,
    inactive: Vec<String>,
    answered: HashMap<String, bool>,
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

impl Host for AddHost<'_> {
    fn choose(&mut self, prompt: &Prompt) -> anyhow::Result<Vec<usize>> {
        if !self.interactive {
            return Ok(prompt.defaults.clone());
        }
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
        ask(self.input, self.output, &question)
    }

    fn data_file_exists(&mut self, path: &str) -> anyhow::Result<bool> {
        let normal = path.replace('\\', "/");
        let name = normal.rsplit('/').next().unwrap_or(&normal).to_string();
        if is_vanilla(&name) && !normal.contains('/') {
            return Ok(true);
        }
        if is_plugin(&name) && !normal.contains('/') {
            return Ok(self.plugins.contains(&name.to_ascii_lowercase()));
        }
        let key = normal.to_ascii_lowercase();
        if let Some(answer) = self.answered.get(&key) {
            return Ok(*answer);
        }
        let answer = if self.interactive {
            loop {
                write!(
                    self.output,
                    "\nThe wizard asks whether `{normal}` is already installed (by a mod \
                     earlier in your list). Is it? [y/N]: "
                )?;
                self.output.flush()?;
                let mut line = String::new();
                if self.input.read_line(&mut line)? == 0 {
                    anyhow::bail!("input ended before the wizard's question was answered");
                }
                match line.trim().to_ascii_lowercase().as_str() {
                    "" | "n" | "no" => break false,
                    "y" | "yes" => break true,
                    _ => writeln!(self.output, "  Please answer y or n.")?,
                }
            }
        } else {
            writeln!(
                self.output,
                "note: the wizard asks whether `{normal}` is installed; taken as no"
            )?;
            false
        };
        self.answered.insert(key, answer);
        Ok(answer)
    }

    fn plugin_status(&mut self, name: &str) -> i64 {
        let lower = name.to_ascii_lowercase();
        if self.inactive.contains(&lower) {
            0
        } else if self.plugins.contains(&lower) || is_vanilla(name) {
            1
        } else {
            -1
        }
    }

    fn note(&mut self, text: &str) {
        if !text.trim().is_empty() {
            let _ = writeln!(self.output, "  wizard: {}", text.trim());
        }
    }
}

/// Run `script`. `None` when the wizard cancelled or could not be run, having
/// said why; the caller then falls back to choosing subpackages directly.
pub(crate) fn run_wizard(
    script: &str,
    package: &Package,
    modlist: &std::path::Path,
    interactive: bool,
    input: &mut dyn BufRead,
    output: &mut dyn Write,
) -> anyhow::Result<Option<WizardResult>> {
    let list = crate::config::loader::load_modlist(modlist)?;
    let lower = |names: &[String]| names.iter().map(|p| p.to_ascii_lowercase()).collect();
    let mut host = AddHost {
        input,
        output,
        interactive,
        plugins: lower(&list.plugins),
        inactive: lower(&list.inactive_plugins),
        answered: HashMap::new(),
    };
    let ending = match run(script, package, &mut host) {
        Ok(ending) => ending,
        Err(err) => {
            writeln!(host.output, "note: the wizard could not be run ({err})")?;
            return Ok(None);
        }
    };
    let outcome = match ending {
        Ending::Finished(outcome) => outcome,
        Ending::Cancelled(message) => {
            writeln!(host.output, "note: the wizard cancelled: {message}")?;
            return Ok(None);
        }
    };
    // A package installing nothing is not what anyone adding it meant.
    if outcome.subpackages.is_empty() {
        writeln!(
            host.output,
            "note: the wizard's answers select no subpackage"
        )?;
        return Ok(None);
    }
    Ok(Some(entry(outcome, host.output)?))
}

fn toml_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

/// The entry's parts from what the wizard did. What the modlist has no way to
/// say is reported, not dropped silently.
fn entry(outcome: Outcome, output: &mut dyn Write) -> anyhow::Result<WizardResult> {
    let mut actions = Vec::new();
    for (from, to) in &outcome.renamed_plugins {
        actions.push(format!(
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
            actions.push(format!(
                "[[mods.actions]]\naction = \"ini_tweak\"\nfile = {}\nkey = {}\nvalue = {}\nformat = \"set-to\"",
                toml_string(file),
                toml_string(&edit.key),
                toml_string(&edit.value)
            ));
        } else {
            writeln!(
                output,
                "note: the wizard sets [{}] {} = {} in {}; ini_tweak cannot say a section yet, \
                 so it is not recorded",
                edit.section, edit.key, edit.value, edit.file
            )?;
        }
    }
    for (file, section, key) in &outcome.disabled_ini_lines {
        writeln!(
            output,
            "note: the wizard disables [{section}] {key} in {file}; not recorded"
        )?;
    }
    Ok(WizardResult {
        subpackages: outcome.subpackages,
        exclude: outcome.left_out_plugins,
        actions,
    })
}
