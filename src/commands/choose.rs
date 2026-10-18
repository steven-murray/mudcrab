//! Putting an installer's choices to the person running `add`.
//!
//! A FOMOD group and a BAIN package list are the same question -- pick some of
//! these -- with different rules about how many. Each is asked as a numbered
//! list; Enter takes what install would pick on its own, so walking through an
//! installer to accept its defaults costs one keypress per question.

use std::io::{BufRead, Write};

pub(crate) use crate::config::choose::{Availability, Choice, Question, Rule};

/// Ask `question` on `output`, reading answers from `input` until one is
/// valid. Returns the chosen indices, required options included.
pub(crate) fn ask(
    input: &mut dyn BufRead,
    output: &mut dyn Write,
    question: &Question,
) -> anyhow::Result<Vec<usize>> {
    let usable: Vec<usize> = (0..question.choices.len())
        .filter(|&i| question.choices[i].availability != Availability::Unavailable)
        .collect();
    let required: Vec<usize> = usable
        .iter()
        .copied()
        .filter(|&i| question.choices[i].availability == Availability::Required)
        .collect();
    let free: Vec<usize> = usable
        .iter()
        .copied()
        .filter(|i| !required.contains(i))
        .collect();

    writeln!(
        output,
        "\n{} {}",
        crate::ui::question(&question.title),
        crate::ui::dim(format!("({})", question.rule.instruction()))
    )?;
    for (index, choice) in question.choices.iter().enumerate() {
        let mut marks = Vec::new();
        if question.default.contains(&index) {
            marks.push(crate::ui::dim("default"));
        }
        match choice.availability {
            Availability::Recommended => {
                marks.push(console::style("recommended").green().to_string())
            }
            Availability::Required => marks.push(crate::ui::dim("always installed")),
            Availability::Unavailable => {
                marks.push(console::style("not available").red().to_string())
            }
            Availability::Free => {}
        }
        let marks = if marks.is_empty() {
            String::new()
        } else {
            format!("  ({})", marks.join(", "))
        };
        let name = if choice.availability == Availability::Unavailable {
            crate::ui::dim(&choice.name)
        } else {
            choice.name.clone()
        };
        writeln!(
            output,
            "  {} {name}{marks}",
            console::style(format!("{:>2}.", index + 1)).cyan()
        )?;
        if let Some(line) = summary(&choice.description) {
            writeln!(output, "      {}", crate::ui::dim(line))?;
        }
    }

    // Nothing to decide: say what happens and move on.
    let settled = question.settled();
    if let Some(chosen) = settled {
        writeln!(
            output,
            "  {} {}",
            crate::ui::arrow(),
            question.names(&chosen)
        )?;
        return Ok(chosen);
    }

    let default_label = if question.default.is_empty() {
        "none".to_string()
    } else {
        question
            .default
            .iter()
            .map(|i| (i + 1).to_string())
            .collect::<Vec<_>>()
            .join(" ")
    };
    loop {
        write!(
            output,
            "  {} ",
            crate::ui::dim(format!("Your choice [{default_label}]:"))
        )?;
        output.flush()?;
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            anyhow::bail!("input ended before {} was answered", question.title);
        }
        match parse(line.trim(), question, &free, &required) {
            Ok(chosen) => return Ok(chosen),
            Err(problem) => writeln!(output, "  {}", console::style(problem).yellow())?,
        }
    }
}

/// An answer, checked against the question's rule.
fn parse(
    answer: &str,
    question: &Question,
    free: &[usize],
    required: &[usize],
) -> Result<Vec<usize>, String> {
    if answer.is_empty() {
        return Ok(question.default.clone());
    }
    let mut picked: Vec<usize> = Vec::new();
    for token in answer.split(|c: char| c.is_whitespace() || c == ',') {
        if token.is_empty() || token == "0" {
            continue;
        }
        let number: usize = token
            .parse()
            .map_err(|_| format!("'{token}' is not a number in the list"))?;
        let index = number
            .checked_sub(1)
            .filter(|i| *i < question.choices.len())
            .ok_or_else(|| format!("there is no option {number}"))?;
        if question.choices[index].availability == Availability::Unavailable {
            return Err(format!("{} is not available", question.choices[index].name));
        }
        if free.contains(&index) && !picked.contains(&index) {
            picked.push(index);
        }
    }
    let count = picked.len();
    let wants_none = answer.split_whitespace().any(|t| t == "0");
    match question.rule {
        Rule::ExactlyOne if count != 1 => return Err("choose exactly one".to_string()),
        Rule::AtMostOne if count > 1 => return Err("choose one at most".to_string()),
        Rule::AtLeastOne if count == 0 && required.is_empty() => {
            return Err("choose at least one".to_string());
        }
        Rule::AtMostOne | Rule::Any if count == 0 && !wants_none && required.is_empty() => {
            return Err("give a number, or 0 for none".to_string());
        }
        _ => {}
    }
    let mut chosen = required.to_vec();
    chosen.extend(picked);
    chosen.sort_unstable();
    Ok(chosen)
}

/// The first line of a description, short enough to sit under its option.
fn summary(description: &str) -> Option<String> {
    let line = description.lines().map(str::trim).find(|l| !l.is_empty())?;
    const MAX: usize = 100;
    Some(if line.chars().count() > MAX {
        format!("{}...", line.chars().take(MAX).collect::<String>())
    } else {
        line.to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn question(rule: Rule, kinds: &[Availability], default: &[usize]) -> Question {
        Question {
            title: "T".to_string(),
            rule,
            choices: kinds
                .iter()
                .enumerate()
                .map(|(i, kind)| Choice {
                    name: format!("Option {}", i + 1),
                    description: String::new(),
                    availability: *kind,
                })
                .collect(),
            default: default.to_vec(),
        }
    }

    fn answer(question: &Question, typed: &str) -> (anyhow::Result<Vec<usize>>, String) {
        let mut input = std::io::Cursor::new(typed.as_bytes().to_vec());
        let mut output = Vec::new();
        let result = ask(&mut input, &mut output, question);
        (result, String::from_utf8(output).unwrap())
    }

    use Availability::{Free, Recommended, Required, Unavailable};

    #[test]
    fn enter_takes_the_default_and_numbers_pick_options() {
        let q = question(Rule::ExactlyOne, &[Free, Recommended, Free], &[1]);
        assert_eq!(answer(&q, "\n").0.unwrap(), [1]);
        assert_eq!(answer(&q, "3\n").0.unwrap(), [2]);
        let (result, shown) = answer(&q, "1 2\n9\n1\n");
        assert_eq!(result.unwrap(), [0]);
        assert!(shown.contains("choose exactly one"), "{shown}");
        assert!(shown.contains("there is no option 9"), "{shown}");
        assert!(
            shown.contains("2. Option 2  (default, recommended)"),
            "{shown}"
        );
    }

    #[test]
    fn several_none_and_required_options() {
        let q = question(Rule::Any, &[Required, Free, Free, Unavailable], &[0]);
        assert_eq!(answer(&q, "3, 2\n").0.unwrap(), [0, 1, 2]);
        assert_eq!(answer(&q, "0\n").0.unwrap(), [0]);
        let (result, shown) = answer(&q, "4\n2\n");
        assert_eq!(result.unwrap(), [0, 1]);
        assert!(shown.contains("Option 4 is not available"), "{shown}");

        let q = question(Rule::AtMostOne, &[Free, Free], &[]);
        assert_eq!(answer(&q, "0\n").0.unwrap(), Vec::<usize>::new());
        assert!(
            answer(&q, "\n").0.unwrap().is_empty(),
            "Enter takes the empty default"
        );
    }

    #[test]
    fn nothing_to_decide_is_not_asked() {
        let q = question(Rule::ExactlyOne, &[Free, Unavailable], &[0]);
        let (result, shown) = answer(&q, "");
        assert_eq!(result.unwrap(), [0]);
        assert!(
            shown.contains(&format!("{} Option 1", crate::ui::arrow())),
            "{shown}"
        );
        let q = question(Rule::All, &[Free, Free], &[0, 1]);
        assert_eq!(answer(&q, "").0.unwrap(), [0, 1]);
    }

    #[test]
    fn running_out_of_input_is_an_error_not_a_loop() {
        let q = question(Rule::ExactlyOne, &[Free, Free], &[0]);
        assert!(answer(&q, "7\n").0.is_err());
    }
}
