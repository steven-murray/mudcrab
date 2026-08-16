use crate::config::schema::{
    CompiledModlist, InputSpec, InputType, InputValue, PersonalizedMod, PersonalizedPlan,
};
use std::collections::HashMap;
use std::io::{self, Write};

pub fn build_plan(compiled: &CompiledModlist, headless: bool) -> anyhow::Result<PersonalizedPlan> {
    let responses = if headless {
        default_responses(&compiled.inputs)
    } else {
        prompt_responses(&compiled.inputs)?
    };

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
                actions: entry.actions.clone(),
            });
        }
    }

    Ok(PersonalizedPlan {
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
    })
}

fn default_responses(inputs: &HashMap<String, InputSpec>) -> HashMap<String, InputValue> {
    let mut out = HashMap::new();

    for (id, spec) in inputs {
        let value = match spec.input_type {
            InputType::Bool => InputValue::Bool(false),
            InputType::Choice => InputValue::Text(spec.choices.first().cloned().unwrap_or_default()),
            InputType::Text => InputValue::Text(String::new()),
        };

        out.insert(id.clone(), value);
    }

    out
}

fn prompt_responses(inputs: &HashMap<String, InputSpec>) -> anyhow::Result<HashMap<String, InputValue>> {
    let mut sorted_ids: Vec<&String> = inputs.keys().collect();
    sorted_ids.sort();

    let mut out = HashMap::new();
    for id in sorted_ids {
        let spec = &inputs[id];
        let value = prompt_input(spec)?;
        out.insert(id.clone(), value);
    }

    Ok(out)
}

fn prompt_input(spec: &InputSpec) -> anyhow::Result<InputValue> {
    match spec.input_type {
        InputType::Bool => prompt_bool(&spec.query).map(InputValue::Bool),
        InputType::Choice => prompt_choice(&spec.query, &spec.choices).map(InputValue::Text),
        InputType::Text => prompt_text(&spec.query).map(InputValue::Text),
    }
}

fn prompt_bool(query: &str) -> anyhow::Result<bool> {
    loop {
        print!("{} [y/N]: ", query);
        io::stdout().flush()?;

        let mut buf = String::new();
        io::stdin().read_line(&mut buf)?;
        let answer = buf.trim().to_lowercase();

        if answer.is_empty() || answer == "n" || answer == "no" {
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
}
