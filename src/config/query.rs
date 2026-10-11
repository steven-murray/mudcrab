use crate::config::schema::{
    CompiledModlist, InputSpec, InputType, InputValue, PersonalizedMod, PersonalizedPlan,
};
use std::collections::HashMap;
use std::io::{self, Write};

/// Answers given on the command line, as `key=value`, before any prompting.
pub type Overrides = HashMap<String, String>;

/// Parse `--set key=value` arguments, refusing duplicates.
pub fn parse_overrides(pairs: &[String]) -> anyhow::Result<Overrides> {
    let mut out = Overrides::new();
    for pair in pairs {
        let Some((key, value)) = pair.split_once('=') else {
            anyhow::bail!("--set {pair}: expected key=value");
        };
        let key = key.trim().to_string();
        if out.insert(key.clone(), value.trim().to_string()).is_some() {
            anyhow::bail!("--set {key} given twice");
        }
    }
    Ok(out)
}

pub fn build_plan(
    compiled: &CompiledModlist,
    headless: bool,
    overrides: &Overrides,
) -> anyhow::Result<PersonalizedPlan> {
    let responses = resolve_responses(&compiled.inputs, headless, overrides)?;

    let mod_order = compiled.mods.iter().map(|entry| entry.id.clone()).collect();
    let mut selected_mods = Vec::new();
    let mut mods = Vec::new();
    for entry in &compiled.mods {
        let include = match &entry.condition {
            Some(expr) => evaluate_condition(expr, &responses),
            None => true,
        };

        if include {
            selected_mods.push(entry.id.clone());
            mods.push(PersonalizedMod {
                id: entry.id.clone(),
                oracle_name: entry.oracle_name.clone(),
                section: entry.section.clone(),
                mod_type: entry.mod_type,
                merge: entry.merge.clone(),
                archives: entry.archives.clone(),
                files: entry.files.clone(),
                actions: entry
                    .actions
                    .iter()
                    .filter(|action| {
                        action
                            .condition
                            .as_deref()
                            .is_none_or(|expr| evaluate_condition(expr, &responses))
                    })
                    .map(|action| action.action.clone())
                    .collect(),
            });
        }
    }

    Ok(PersonalizedPlan {
        guide: compiled.guide.clone(),
        schema_version: compiled.schema_version,
        name: compiled.name.clone(),
        responses,
        mod_order,
        selected_mods,
        actions: compiled.actions.clone(),
        post_install_actions: compiled.post_install_actions.clone(),
        mo2_modlist_entries: compiled.mo2_modlist_entries.clone(),
        mods,
        plugins: compiled.plugins.clone(),
        inactive_plugins: compiled.inactive_plugins.clone(),
    })
}

/// Every input's answer: from `--set` if given, else the declared default
/// when headless, else by asking.
fn resolve_responses(
    inputs: &HashMap<String, InputSpec>,
    headless: bool,
    overrides: &Overrides,
) -> anyhow::Result<HashMap<String, InputValue>> {
    if let Some(unknown) = overrides.keys().find(|key| !inputs.contains_key(*key)) {
        let mut known: Vec<&String> = inputs.keys().collect();
        known.sort();
        anyhow::bail!("--set {unknown}: no such input (the modlist declares {known:?})");
    }

    let mut sorted_ids: Vec<&String> = inputs.keys().collect();
    sorted_ids.sort();

    let mut out = HashMap::new();
    for id in sorted_ids {
        let spec = &inputs[id];
        let value = if let Some(raw) = overrides.get(id) {
            parse_answer(id, spec, raw)?
        } else if headless {
            spec.default.clone().ok_or_else(|| {
                anyhow::anyhow!(
                    "input {id} has no default, so a headless run must be given it with --set {id}=..."
                )
            })?
        } else {
            prompt_input(spec)?
        };
        out.insert(id.clone(), value);
    }
    Ok(out)
}

/// A `--set` value, checked against the input's type.
fn parse_answer(id: &str, spec: &InputSpec, raw: &str) -> anyhow::Result<InputValue> {
    match spec.input_type {
        InputType::Bool => match raw.to_ascii_lowercase().as_str() {
            "true" | "yes" | "y" | "1" => Ok(InputValue::Bool(true)),
            "false" | "no" | "n" | "0" => Ok(InputValue::Bool(false)),
            _ => anyhow::bail!("--set {id}={raw}: {id} is a bool; use true or false"),
        },
        InputType::Choice => {
            if spec.choices.iter().any(|choice| choice == raw) {
                Ok(InputValue::Text(raw.to_string()))
            } else {
                anyhow::bail!("--set {id}={raw}: not one of {:?}", spec.choices)
            }
        }
        InputType::Text => Ok(InputValue::Text(raw.to_string())),
    }
}

/// The input a condition asks about. Conditions are `flag`, `!flag`,
/// `key == value` and `key != value`, each naming exactly one input.
pub fn condition_input(expr: &str) -> &str {
    let trimmed = expr.trim();
    if let Some(rest) = trimmed.strip_prefix('!') {
        return rest.trim();
    }
    for op in ["==", "!="] {
        if let Some((key, _)) = trimmed.split_once(op) {
            return key.trim();
        }
    }
    trimmed
}

fn prompt_input(spec: &InputSpec) -> anyhow::Result<InputValue> {
    match spec.input_type {
        InputType::Bool => {
            let default = matches!(spec.default, Some(InputValue::Bool(true)));
            prompt_bool(&spec.query, default).map(InputValue::Bool)
        }
        InputType::Choice => prompt_choice(&spec.query, &spec.choices).map(InputValue::Text),
        InputType::Text => prompt_text(&spec.query).map(InputValue::Text),
    }
}

fn prompt_bool(query: &str, default: bool) -> anyhow::Result<bool> {
    let hint = if default { "[Y/n]" } else { "[y/N]" };
    loop {
        print!("{query} {hint}: ");
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

        eprintln!("Please answer with y/yes or n/no.");
    }
}

fn prompt_choice(query: &str, choices: &[String]) -> anyhow::Result<String> {
    if choices.is_empty() {
        anyhow::bail!("choice input is missing available choices");
    }

    println!("{query}");
    for (idx, choice) in choices.iter().enumerate() {
        println!("  {}. {}", idx + 1, choice);
    }

    loop {
        print!("Select [1-{}]: ", choices.len());
        io::stdout().flush()?;

        let mut buf = String::new();
        io::stdin().read_line(&mut buf)?;
        let answer = buf.trim();

        if let Ok(idx) = answer.parse::<usize>()
            && (1..=choices.len()).contains(&idx)
        {
            return Ok(choices[idx - 1].clone());
        }

        eprintln!("Please enter a valid selection index.");
    }
}

fn prompt_text(query: &str) -> anyhow::Result<String> {
    print!("{}: ", query);
    io::stdout().flush()?;

    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    Ok(buf.trim().to_string())
}

pub fn evaluate_condition(expr: &str, responses: &HashMap<String, InputValue>) -> bool {
    let trimmed = expr.trim();

    if let Some(rest) = trimmed.strip_prefix('!') {
        return !resolve_truthy(rest.trim(), responses);
    }

    if let Some((key, expected)) = split_binary(trimmed, "==") {
        return resolve_string(key, responses)
            .map(|value| value == expected)
            .unwrap_or(false);
    }

    if let Some((key, expected)) = split_binary(trimmed, "!=") {
        return resolve_string(key, responses)
            .map(|value| value != expected)
            .unwrap_or(false);
    }

    resolve_truthy(trimmed, responses)
}

fn split_binary<'a>(expr: &'a str, op: &str) -> Option<(&'a str, String)> {
    let (left, right) = expr.split_once(op)?;
    let expected = right.trim().trim_matches('"').to_string();
    Some((left.trim(), expected))
}

fn resolve_truthy(key: &str, responses: &HashMap<String, InputValue>) -> bool {
    match responses.get(key) {
        Some(InputValue::Bool(v)) => *v,
        Some(InputValue::Text(v)) => !v.trim().is_empty(),
        None => false,
    }
}

fn resolve_string(key: &str, responses: &HashMap<String, InputValue>) -> Option<String> {
    match responses.get(key) {
        Some(InputValue::Bool(v)) => Some(v.to_string()),
        Some(InputValue::Text(v)) => Some(v.clone()),
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_basic_expressions() {
        let mut responses = HashMap::new();
        responses.insert("a".to_string(), InputValue::Bool(true));
        responses.insert("b".to_string(), InputValue::Text("orange".to_string()));

        assert!(evaluate_condition("a", &responses));
        assert!(!evaluate_condition("!a", &responses));
        assert!(evaluate_condition("b == orange", &responses));
        assert!(evaluate_condition("b == \"orange\"", &responses));
        assert!(evaluate_condition("b != blue", &responses));
        assert!(!evaluate_condition("missing", &responses));
    }

    #[test]
    fn condition_input_names_the_one_key_asked_about() {
        assert_eq!(condition_input("a"), "a");
        assert_eq!(condition_input(" ! a "), "a");
        assert_eq!(condition_input("b == orange"), "b");
        assert_eq!(condition_input("b!=\"blue\""), "b");
    }

    fn bool_input(default: Option<bool>) -> HashMap<String, InputSpec> {
        HashMap::from([(
            "flag".to_string(),
            InputSpec {
                input_type: InputType::Bool,
                query: "?".to_string(),
                choices: Vec::new(),
                default: default.map(InputValue::Bool),
            },
        )])
    }

    #[test]
    fn headless_uses_the_declared_default_and_set_overrides_it() {
        let inputs = bool_input(Some(true));
        let none = Overrides::new();
        let got = resolve_responses(&inputs, true, &none).unwrap();
        assert!(matches!(got["flag"], InputValue::Bool(true)));

        let set = parse_overrides(&["flag=false".to_string()]).unwrap();
        let got = resolve_responses(&inputs, true, &set).unwrap();
        assert!(matches!(got["flag"], InputValue::Bool(false)));
    }

    #[test]
    fn headless_without_a_default_or_set_is_an_error() {
        let err = resolve_responses(&bool_input(None), true, &Overrides::new()).unwrap_err();
        assert!(err.to_string().contains("--set flag="), "{err}");
    }

    #[test]
    fn set_must_name_a_declared_input_with_a_valid_value() {
        let inputs = bool_input(Some(true));
        let typo = parse_overrides(&["flg=true".to_string()]).unwrap();
        assert!(resolve_responses(&inputs, true, &typo).is_err());
        let bad = parse_overrides(&["flag=maybe".to_string()]).unwrap();
        assert!(resolve_responses(&inputs, true, &bad).is_err());
    }
}
