//! Asking a list's questions on the terminal, for `query`.

use crate::config::schema::{InputSpec, InputType, InputValue};
use std::io::{self, Write};

/// Ask `spec`'s question and read the answer, with its default on Enter.
pub fn input(spec: &InputSpec) -> anyhow::Result<InputValue> {
    match spec.input_type {
        InputType::Bool => {
            let default = matches!(spec.default, Some(InputValue::Bool(true)));
            prompt_bool(&spec.query, default).map(InputValue::Bool)
        }
        InputType::Choice => {
            prompt_choice(&spec.query, &spec.choices, text_default(spec)).map(InputValue::Text)
        }
        InputType::Text => prompt_text(&spec.query, text_default(spec)).map(InputValue::Text),
        InputType::Multi => {
            let default: Vec<String> = match &spec.default {
                Some(InputValue::List(picked)) => picked.clone(),
                _ => Vec::new(),
            };
            let picked = prompt_multi(&spec.query, &spec.choices, &default)?;
            crate::config::query::multi_answer(spec, picked.iter().map(String::as_str))
        }
    }
}

fn prompt_multi(
    query: &str,
    choices: &[String],
    default: &[String],
) -> anyhow::Result<Vec<String>> {
    println!("{}", crate::ui::question(printable(query)));
    for (idx, choice) in choices.iter().enumerate() {
        println!(
            "{}",
            crate::ui::option(idx + 1, printable(choice), default.contains(choice))
        );
    }
    loop {
        print!(
            "  {} ",
            crate::ui::dim(
                "Numbers to install, with spaces or commas (Enter for the defaults, 0 for none):"
            )
        );
        io::stdout().flush()?;
        let mut buf = String::new();
        io::stdin().read_line(&mut buf)?;
        if let Some(picked) = pick_many(buf.trim(), choices, default) {
            return Ok(picked);
        }
        crate::ui::status(
            crate::ui::Tone::Warning,
            format!(
                "Please enter numbers from 1 to {}, or 0 for none.",
                choices.len()
            ),
        );
    }
}

/// An answer to a multi prompt: numbers, `0` for none, nothing for the
/// defaults.
fn pick_many(answer: &str, choices: &[String], default: &[String]) -> Option<Vec<String>> {
    if answer.is_empty() {
        return Some(default.to_vec());
    }
    if answer == "0" {
        return Some(Vec::new());
    }
    answer
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let idx = part.parse::<usize>().ok()?;
            choices.get(idx.checked_sub(1)?).cloned()
        })
        .collect()
}

/// A list's text as the terminal shows it: control characters (an escape
/// sequence a list could use to rewrite the prompt) shown as `?`.
fn printable(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { '?' } else { c })
        .collect()
}

fn prompt_bool(query: &str, default: bool) -> anyhow::Result<bool> {
    let hint = if default { "[Y/n]" } else { "[y/N]" };
    loop {
        print!(
            "{} {}: ",
            crate::ui::question(printable(query)),
            crate::ui::dim(hint)
        );
        io::stdout().flush()?;

        let mut buf = String::new();
        io::stdin().read_line(&mut buf)?;
        let answer = buf.trim().to_lowercase();

        if answer.is_empty() {
            return Ok(default);
        }
        if answer == "n" || answer == "no" {
            return Ok(false);
        }
        if answer == "y" || answer == "yes" {
            return Ok(true);
        }

        crate::ui::status(crate::ui::Tone::Warning, "Please answer y or n.");
    }
}

/// The declared default of a choice or text input, as text.
fn text_default(spec: &InputSpec) -> Option<&str> {
    match &spec.default {
        Some(InputValue::Text(text)) => Some(text),
        _ => None,
    }
}

fn prompt_choice(query: &str, choices: &[String], default: Option<&str>) -> anyhow::Result<String> {
    if choices.is_empty() {
        anyhow::bail!("choice input is missing available choices");
    }

    println!("{}", crate::ui::question(printable(query)));
    for (idx, choice) in choices.iter().enumerate() {
        println!(
            "{}",
            crate::ui::option(idx + 1, printable(choice), Some(choice.as_str()) == default)
        );
    }

    loop {
        print!(
            "  {} ",
            crate::ui::dim(format!("Select [1-{}]:", choices.len()))
        );
        io::stdout().flush()?;

        let mut buf = String::new();
        io::stdin().read_line(&mut buf)?;
        if let Some(choice) = pick_choice(buf.trim(), choices, default) {
            return Ok(choice);
        }

        crate::ui::status(
            crate::ui::Tone::Warning,
            format!("Please enter a number from 1 to {}.", choices.len()),
        );
    }
}

/// An answer to a choice prompt: its number, or nothing for the default.
fn pick_choice(answer: &str, choices: &[String], default: Option<&str>) -> Option<String> {
    if answer.is_empty() {
        return default
            .filter(|d| choices.iter().any(|c| c == d))
            .map(str::to_string);
    }
    let idx = answer.parse::<usize>().ok()?;
    choices.get(idx.checked_sub(1)?).cloned()
}

fn prompt_text(query: &str, default: Option<&str>) -> anyhow::Result<String> {
    match default {
        Some(default) => print!(
            "{} {}: ",
            crate::ui::question(printable(query)),
            crate::ui::dim(format!("[{}]", printable(default)))
        ),
        None => print!("{}: ", crate::ui::question(printable(query))),
    }
    io::stdout().flush()?;

    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    let answer = buf.trim();
    Ok(match default {
        Some(default) if answer.is_empty() => default.to_string(),
        _ => answer.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_takes_a_choices_default() {
        let choices = vec!["none".to_string(), "v0500".to_string()];
        assert_eq!(
            pick_choice("", &choices, Some("none")),
            Some("none".to_string())
        );
        assert_eq!(
            pick_choice("2", &choices, Some("none")),
            Some("v0500".to_string())
        );
        assert_eq!(pick_choice("", &choices, None), None);
        assert_eq!(pick_choice("0", &choices, None), None);
        assert_eq!(pick_choice("3", &choices, None), None);
    }

    #[test]
    fn a_multi_answer_is_numbers_none_or_the_defaults() {
        let choices = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let default = vec!["b".to_string()];
        assert_eq!(pick_many("", &choices, &default), Some(default.clone()));
        assert_eq!(pick_many("0", &choices, &default), Some(vec![]));
        assert_eq!(
            pick_many("3, 1", &choices, &default),
            Some(vec!["c".to_string(), "a".to_string()])
        );
        assert_eq!(pick_many("4", &choices, &default), None);
    }
}
