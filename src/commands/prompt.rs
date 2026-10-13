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
    }
}

fn prompt_bool(query: &str, default: bool) -> anyhow::Result<bool> {
    let hint = if default { "[Y/n]" } else { "[y/N]" };
    loop {
        print!("{} {}: ", crate::ui::question(query), crate::ui::dim(hint));
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

    println!("{}", crate::ui::question(query));
    for (idx, choice) in choices.iter().enumerate() {
        println!(
            "{}",
            crate::ui::option(idx + 1, choice, Some(choice.as_str()) == default)
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
            crate::ui::question(query),
            crate::ui::dim(format!("[{default}]"))
        ),
        None => print!("{}: ", crate::ui::question(query)),
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
}
