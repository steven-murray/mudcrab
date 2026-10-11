//! What a BAIN wizard (`wizard.txt`) offers, enough to name the choice that
//! gives a known set of subpackages.
//!
//! A wizard is a script: `SelectSubPackage` lines that always run, then
//! `SelectOne`/`SelectMany` prompts whose `Case` blocks select more. A modlist
//! records the resulting subpackages, which is exact; a guide says "choose
//! *Performance (no rocks)*", which is what a person clicks. This recovers the
//! second from the first.
//!
//! Deliberately not an interpreter: `If`, variables and nested prompts are
//! read past, not evaluated. That can only cost a match -- an explanation is
//! offered only when the choices it names reproduce the selection exactly.

use std::collections::BTreeSet;

#[derive(Debug, Default)]
pub struct Wizard {
    /// Subpackages selected outside any prompt.
    pub always: BTreeSet<String>,
    pub prompts: Vec<Prompt>,
    /// The script also chooses plugins or edits INIs. Its prompts then decide
    /// more than which subpackages install, so naming only the answers that
    /// explain the subpackages would leave a reader's other choices to chance.
    pub chooses_more: bool,
    /// The first thing in the script that its prompts, read as above, do not
    /// account for: a condition, a variable, a prompt inside a case. With one,
    /// asking the prompts in order would not reproduce what Wrye Bash does.
    pub beyond_prompts: Option<String>,
}

#[derive(Debug)]
pub struct Prompt {
    pub question: String,
    pub many: bool,
    /// Each option's name and the subpackages its `Case` selects.
    pub options: Vec<(String, BTreeSet<String>)>,
    /// Each option's description, in the same order.
    pub descriptions: Vec<String>,
    /// Options the wizard pre-selects (written with a leading `|`).
    pub defaults: Vec<usize>,
}

/// The quoted strings on a line, in order.
fn quoted(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find('"') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('"') else { break };
        out.push(after[..end].to_string());
        rest = &after[end + 1..];
    }
    out
}

fn keyword(line: &str) -> String {
    line.split(|c: char| c.is_whitespace() || c == '"')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

pub fn parse(script: &str) -> Wizard {
    // Join continuation lines (`\` at the end) and drop comments.
    let mut statements = Vec::new();
    let mut pending = String::new();
    for raw in script.lines() {
        let line = raw.trim();
        if line.starts_with(';') {
            continue;
        }
        if let Some(head) = line.strip_suffix('\\') {
            pending.push_str(head);
            pending.push(' ');
            continue;
        }
        pending.push_str(line);
        if !pending.trim().is_empty() {
            statements.push(std::mem::take(&mut pending));
        }
        pending.clear();
    }

    let mut wizard = Wizard::default();
    // The top-level prompt being read, and the option whose case it is in.
    let mut current: Option<(usize, Option<usize>)> = None;
    let mut nested = 0usize;
    for statement in statements {
        let word = keyword(&statement);
        if matches!(
            word.as_str(),
            "selectespm"
                | "deselectespm"
                | "selectallespms"
                | "deselectallespms"
                | "renameespm"
                | "editini"
        ) {
            wizard.chooses_more = true;
        }
        if wizard.beyond_prompts.is_none() {
            let unaccounted = match word.as_str() {
                "selectone" | "selectmany" if current.is_some() => {
                    Some("a question inside an answer")
                }
                "if" | "elif" | "else" | "endif" => Some("a condition (If)"),
                "while" | "endwhile" | "for" | "endfor" | "continue" => Some("a loop"),
                "default" => Some("a Default answer"),
                "deselectsubpackage" | "selectall" => {
                    Some("a change to the selection outside the questions")
                }
                // Harmless before anything is selected, which is where wizards put it.
                "deselectall" if !wizard.always.is_empty() || !wizard.prompts.is_empty() => {
                    Some("a change to the selection outside the questions")
                }
                "return" | "cancel" | "exec" => Some("a way to end or redirect the script"),
                "selectsubpackage" | "selectone" | "selectmany" | "case" | "break"
                | "endselect" | "note" | "requireversions" | "deselectall" => None,
                _ if statement.contains('=') => Some("a variable"),
                _ if wizard.chooses_more => Some("a choice of plugins or an INI edit"),
                _ => None,
            };
            wizard.beyond_prompts = unaccounted.map(str::to_string);
        }
        match word.as_str() {
            // A prompt inside a case (an "individual selection" branch) is
            // read past: only top-level prompts are offered as answers.
            "selectone" | "selectmany" if current.is_some() => nested += 1,
            "selectone" | "selectmany" => {
                let strings = quoted(&statement);
                let question = strings.first().cloned().unwrap_or_default();
                // After the question come (option, description, image) triples.
                let triples: Vec<&[String]> = strings[1.min(strings.len())..].chunks(3).collect();
                let options = triples
                    .iter()
                    .map(|triple| {
                        (
                            triple[0].trim_start_matches('|').to_string(),
                            BTreeSet::new(),
                        )
                    })
                    .collect();
                wizard.prompts.push(Prompt {
                    question,
                    many: word == "selectmany",
                    options,
                    descriptions: triples
                        .iter()
                        .map(|triple| triple.get(1).cloned().unwrap_or_default())
                        .collect(),
                    defaults: triples
                        .iter()
                        .enumerate()
                        .filter(|(_, triple)| triple[0].starts_with('|'))
                        .map(|(index, _)| index)
                        .collect(),
                });
                current = Some((wizard.prompts.len() - 1, None));
            }
            "endselect" => {
                if nested > 0 {
                    nested -= 1;
                } else {
                    current = None;
                }
            }
            "case" if nested == 0 => {
                if let Some((prompt, _)) = current {
                    let name = quoted(&statement).into_iter().next().unwrap_or_default();
                    let index = wizard.prompts[prompt]
                        .options
                        .iter()
                        .position(|(o, _)| *o == name);
                    current = Some((prompt, index));
                }
            }
            "break" if nested == 0 => {
                if let Some((prompt, _)) = current {
                    current = Some((prompt, None));
                }
            }
            "selectsubpackage" if nested == 0 => {
                let Some(name) = quoted(&statement).into_iter().next() else {
                    continue;
                };
                match current {
                    None => {
                        wizard.always.insert(name.to_ascii_lowercase());
                    }
                    Some((prompt, Some(option))) => {
                        wizard.prompts[prompt].options[option]
                            .1
                            .insert(name.to_ascii_lowercase());
                    }
                    Some((_, None)) => {}
                }
            }
            _ => {}
        }
    }
    wizard
}

impl Wizard {
    /// Whether asking the prompts in order, and selecting what each chosen
    /// option's `Case` selects, does exactly what the script does: `Err` with
    /// what stands in the way.
    pub fn askable(&self) -> Result<(), String> {
        if self.chooses_more {
            return Err("it also chooses plugins or edits INIs".to_string());
        }
        match &self.beyond_prompts {
            Some(what) => Err(format!("it contains {what}")),
            None if self.prompts.is_empty() => Err("it asks nothing".to_string()),
            None => Ok(()),
        }
    }

    /// The subpackages the given answers select (lowercased, as recorded):
    /// one list of chosen option indices per prompt.
    pub fn selection(&self, answers: &[Vec<usize>]) -> BTreeSet<String> {
        let mut out = self.always.clone();
        for (prompt, chosen) in self.prompts.iter().zip(answers) {
            for &index in chosen {
                if let Some((_, picks)) = prompt.options.get(index) {
                    out.extend(picks.iter().cloned());
                }
            }
        }
        out
    }

    /// The answers -- (question, chosen options) per prompt -- that select
    /// exactly `selected`, if the wizard offers such a path.
    pub fn explain(&self, selected: &[String]) -> Option<Vec<(String, Vec<String>)>> {
        if self.chooses_more {
            return None;
        }
        let target: BTreeSet<String> = selected.iter().map(|s| s.to_ascii_lowercase()).collect();
        let mut got = self.always.clone();
        if !got.is_subset(&target) {
            return None;
        }
        let mut answers = Vec::new();
        for prompt in &self.prompts {
            let fits: Vec<&(String, BTreeSet<String>)> = prompt
                .options
                .iter()
                .filter(|(_, picks)| !picks.is_empty() && picks.is_subset(&target))
                .collect();
            let chosen: Vec<&(String, BTreeSet<String>)> = if prompt.many {
                fits
            } else {
                fits.into_iter()
                    .max_by_key(|(_, picks)| picks.len())
                    .into_iter()
                    .collect()
            };
            if chosen.is_empty() {
                continue;
            }
            for (_, picks) in &chosen {
                got.extend(picks.iter().cloned());
            }
            answers.push((
                prompt.question.clone(),
                chosen.iter().map(|(name, _)| name.clone()).collect(),
            ));
        }
        (got == target && !answers.is_empty()).then_some(answers)
    }
}

#[cfg(test)]
mod tests {
    const J3: &str = r#"DeSelectAll
SelectSubPackage "00 core"
SelectOne "Select preset", \
    "All","Everything.","", \
    "Performance (no rocks)","Crucial parts.","", \
    "Select parts invidually","Each part.",""
    Case "All"
	SelectSubPackage "02 a"
	SelectSubPackage "03 rocks"
        Break
    Case "Performance (no rocks)"
	SelectSubPackage "02 a"
        Break
    Case "Select parts invidually"
        SelectMany "Parts", "02 a","","", "03 rocks","",""
            Case "02 a"
                SelectSubPackage "02 a"
                Break
        EndSelect
        Break
EndSelect
"#;

    #[test]
    fn names_the_preset_that_gives_the_selection() {
        let wizard = super::parse(J3);
        assert_eq!(
            wizard.explain(&["00 Core".into(), "02 a".into()]),
            Some(vec![(
                "Select preset".to_string(),
                vec!["Performance (no rocks)".to_string()]
            )])
        );
        assert_eq!(
            wizard
                .explain(&["00 core".into(), "02 a".into(), "03 rocks".into()])
                .unwrap()[0]
                .1,
            ["All"]
        );
        // No preset gives this one; say nothing rather than something wrong.
        assert_eq!(wizard.explain(&["00 core".into(), "03 rocks".into()]), None);

        // A wizard that also picks plugins decides more than subpackages.
        let picky = super::parse(&J3.replace(
            "Break\n    Case \"Select",
            "SelectEspm \"x.esp\"\n        Break\n    Case \"Select",
        ));
        assert!(picky.chooses_more);
        assert_eq!(picky.explain(&["00 core".into(), "02 a".into()]), None);
        assert!(picky.askable().is_err());
    }

    #[test]
    fn a_wizard_of_questions_alone_can_be_asked() {
        // J3's third answer opens a question of its own, which asking the
        // top-level questions in order would skip.
        let j3 = super::parse(J3);
        assert_eq!(
            j3.askable(),
            Err("it contains a question inside an answer".to_string())
        );

        let simple = super::parse(
            "DeSelectAll\nSelectSubPackage \"00 Core\"\n\
             SelectOne \"Style\", \"|Plain\",\"No colour.\",\"\", \"Coloured\",\"Colour.\",\"\"\n\
             Case \"Plain\"\nSelectSubPackage \"01 Plain\"\nBreak\n\
             Case \"Coloured\"\nSelectSubPackage \"01 Coloured\"\nBreak\nEndSelect\n",
        );
        assert_eq!(simple.askable(), Ok(()));
        let prompt = &simple.prompts[0];
        assert_eq!(prompt.defaults, [0]);
        assert_eq!(prompt.descriptions, ["No colour.", "Colour."]);
        assert_eq!(
            simple.selection(&[vec![1]]).into_iter().collect::<Vec<_>>(),
            ["00 core", "01 coloured"]
        );

        let conditional =
            super::parse("If CompareObVersion(\"1.2\") > 0\nSelectSubPackage \"a\"\nEndIf\n");
        assert_eq!(
            conditional.askable(),
            Err("it contains a condition (If)".to_string())
        );
    }
}
