use crate::config::schema::{
    BainSubpackage, CompiledArchive, CompiledModlist, FomodSelection, InputSpec, InputType, InputValue, PersonalizedMod, PersonalizedPlan,
};
use crate::config::condition::Condition;
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
    let included = resolve_included(compiled, &responses)?;

    let mut selected_mods = Vec::new();
    let mut mods = Vec::new();
    for entry in &compiled.mods {
        if !included[entry.id.as_str()] {
            continue;
        }
        if let Some(dep) = entry.dependencies.iter().find(|dep| !included.get(dep.as_str()).copied().unwrap_or(false)) {
            anyhow::bail!(
                "mod {} is included but depends on {dep}, which is not; give {} `if = '{}'` \
                 (or change the answers) so they come and go together",
                entry.id,
                entry.id,
                crate::config::condition::mod_term(dep)
            );
        }

        let applies = |expr: Option<&str>| -> anyhow::Result<bool> {
            match expr {
                Some(expr) => Condition::parse(expr)?
                    .eval(&responses, &mut |id| known(&included, id))
                    .map_err(|err| anyhow::anyhow!("mod {}: {err:#}", entry.id)),
                None => Ok(true),
            }
        };

        let mut actions = Vec::new();
        for action in &entry.actions {
            if applies(action.condition.as_deref())? {
                actions.push(action.action.clone());
            }
        }
        let archives = resolve_archives(&entry.id, &entry.archives, &applies)?;
        if archives.is_empty() && !entry.archives.is_empty() {
            anyhow::bail!(
                "mod {}: every archive's `if` left it out, so it would install nothing; \
                 put the condition on the mod instead",
                entry.id
            );
        }

        selected_mods.push(entry.id.clone());
        mods.push(PersonalizedMod {
            id: entry.id.clone(),
            oracle_name: entry.oracle_name.clone(),
            section: entry.section.clone(),
            mod_type: entry.mod_type,
            merge: entry.merge.clone(),
            archives,
            files: entry.files.clone(),
            actions,
        });
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

/// The archives, FOMOD selections and BAIN subpackages that apply, with their
/// conditions resolved away: nothing after `query` knows they existed.
fn resolve_archives(
    mod_id: &str,
    archives: &[CompiledArchive],
    applies: &dyn Fn(Option<&str>) -> anyhow::Result<bool>,
) -> anyhow::Result<Vec<CompiledArchive>> {
    let mut out = Vec::new();
    for archive in archives {
        if !applies(archive.condition.as_deref())? {
            continue;
        }
        let mut archive = archive.clone();
        archive.condition = None;

        let mut selections = Vec::new();
        for selection in archive.fomod_selections {
            if applies(selection.condition.as_deref())? {
                selections.push(FomodSelection { condition: None, ..selection });
            }
        }
        archive.fomod_selections = selections;

        if !archive.bain_subpackages.is_empty() {
            let mut subpackages = Vec::new();
            for subpackage in &archive.bain_subpackages {
                if applies(subpackage.condition())? {
                    subpackages.push(BainSubpackage::from(subpackage.name()));
                }
            }
            if subpackages.is_empty() {
                anyhow::bail!(
                    "mod {mod_id}: every BAIN subpackage's `if` left it out of {}, so it would \
                     install nothing; put the condition on the archive instead",
                    archive.path.as_deref().unwrap_or("its archive")
                );
            }
            archive.bain_subpackages = subpackages;
        }
        out.push(archive);
    }
    Ok(out)
}

fn known(included: &HashMap<&str, bool>, id: &str) -> anyhow::Result<bool> {
    included
        .get(id)
        .copied()
        .ok_or_else(|| anyhow::anyhow!("asks about mod {id}, which the modlist does not have"))
}

/// Whether each mod is in the list, by its `if`.
///
/// A condition may ask about another mod, so mods are resolved on demand:
/// asking about one resolves its own condition first. A mod met again while
/// its own condition is still being answered is a cycle, which `validate`
/// rejects but a hand-edited compiled file could still contain.
fn resolve_included<'a>(
    compiled: &'a CompiledModlist,
    responses: &HashMap<String, InputValue>,
) -> anyhow::Result<HashMap<&'a str, bool>> {
    let mut conditions: HashMap<&str, Option<Condition>> = HashMap::new();
    for entry in &compiled.mods {
        let condition = entry
            .condition
            .as_deref()
            .map(Condition::parse)
            .transpose()
            .map_err(|err| anyhow::anyhow!("mod {}: {err:#}", entry.id))?;
        conditions.insert(entry.id.as_str(), condition);
    }

    fn resolve<'a>(
        id: &str,
        conditions: &HashMap<&'a str, Option<Condition>>,
        responses: &HashMap<String, InputValue>,
        done: &mut HashMap<&'a str, bool>,
        stack: &mut Vec<&'a str>,
    ) -> anyhow::Result<bool> {
        let Some((&key, condition)) = conditions.get_key_value(id) else {
            anyhow::bail!("asks about mod {id}, which the modlist does not have");
        };
        if let Some(&answer) = done.get(key) {
            return Ok(answer);
        }
        if let Some(at) = stack.iter().position(|seen| *seen == key) {
            let mut cycle = stack[at..].to_vec();
            cycle.push(key);
            anyhow::bail!("mod conditions depend on each other: {}", cycle.join(" -> "));
        }
        let answer = match condition {
            None => true,
            Some(condition) => {
                stack.push(key);
                let answer = condition
                    .eval(responses, &mut |other| resolve(other, conditions, responses, done, stack))
                    .map_err(|err| anyhow::anyhow!("mod {key}: {err:#}"));
                stack.pop();
                answer?
            }
        };
        done.insert(key, answer);
        Ok(answer)
    }

    let mut done = HashMap::new();
    for entry in &compiled.mods {
        resolve(&entry.id, &conditions, responses, &mut done, &mut Vec::new())?;
    }
    Ok(done)
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

#[cfg(test)]
mod tests {
    use super::*;

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
