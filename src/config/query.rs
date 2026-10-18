use crate::config::condition::Condition;
use crate::config::schema::{
    BainSubpackage, CompiledArchive, CompiledModlist, FomodSelection, InputSpec, InputType,
    InputValue, PersonalizedMod, PersonalizedPlan,
};
use std::collections::{HashMap, HashSet};

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

/// Asks the user one input's question; see [`build_plan`].
pub type Ask<'a> = &'a mut dyn FnMut(&InputSpec) -> anyhow::Result<InputValue>;

/// The plan for these answers: each input from `overrides` if given there,
/// else from `ask`, or with no `ask` (headless) its declared default.
pub fn build_plan(
    compiled: &CompiledModlist,
    overrides: &Overrides,
    ask: Option<Ask<'_>>,
) -> anyhow::Result<PersonalizedPlan> {
    let responses = resolve_responses(&compiled.inputs, overrides, ask)?;

    let mod_order = compiled.mods.iter().map(|entry| entry.id.clone()).collect();
    let included = resolve_included(compiled, &responses)?;

    let mut selected_mods = Vec::new();
    let mut mods = Vec::new();
    for entry in &compiled.mods {
        if !included[entry.id.as_str()] {
            continue;
        }
        if let Some(dep) = entry
            .dependencies
            .iter()
            .find(|dep| !included.get(dep.as_str()).copied().unwrap_or(false))
        {
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
            enabled: entry.enabled,
            merge: entry.merge.clone(),
            plugins: entry.plugins.clone(),
            archives,
            files: entry.files.clone(),
            actions,
        });
    }

    // A left-out mod takes its plugins with it: the load order names only
    // plugins something installs. A merge built from a left-out mod's plugin
    // would fail later, at merge time, so it is refused here instead.
    let mut left_out_plugins: HashSet<String> = HashSet::new();
    for entry in compiled
        .mods
        .iter()
        .filter(|entry| !included[entry.id.as_str()])
    {
        left_out_plugins.extend(entry.plugins.iter().map(|p| p.to_ascii_lowercase()));
        if let Some(merge) = &entry.merge {
            left_out_plugins.insert(merge.output.to_ascii_lowercase());
        }
    }
    // ...unless an included mod ships the same plugin, as alternatives do.
    for entry in compiled
        .mods
        .iter()
        .filter(|entry| included[entry.id.as_str()])
    {
        for plugin in &entry.plugins {
            left_out_plugins.remove(&plugin.to_ascii_lowercase());
        }
        if let Some(merge) = &entry.merge {
            left_out_plugins.remove(&merge.output.to_ascii_lowercase());
        }
    }
    for entry in compiled
        .mods
        .iter()
        .filter(|entry| included[entry.id.as_str()])
    {
        let Some(merge) = &entry.merge else { continue };
        if let Some(source) = merge.sources.iter().find(|source| {
            !included
                .get(source.mod_id.as_str())
                .copied()
                .unwrap_or(true)
        }) {
            anyhow::bail!(
                "merge {} takes {} from mod {}, which is left out; give the merge `if = '{}'`, \
                 or a second merge without it for when it is out",
                entry.id,
                source.plugin,
                source.mod_id,
                crate::config::condition::mod_term(&source.mod_id)
            );
        }
    }
    let keep = |plugin: &String| !left_out_plugins.contains(&plugin.to_ascii_lowercase());
    let plugins = compiled
        .plugins
        .iter()
        .filter(|p| keep(p))
        .cloned()
        .collect();
    let inactive_plugins = compiled
        .inactive_plugins
        .iter()
        .filter(|p| keep(p))
        .cloned()
        .collect();

    Ok(PersonalizedPlan {
        guide: compiled.guide.clone(),
        schema_version: compiled.schema_version,
        name: compiled.name.clone(),
        responses,
        mod_order,
        selected_mods,
        actions: compiled.actions.clone(),
        post_install_actions: compiled.post_install_actions.clone(),
        manual_steps: compiled.manual_steps.clone(),
        mo2: compiled.mo2.clone(),
        mo2_modlist_entries: compiled.mo2_modlist_entries.clone(),
        mods,
        plugins,
        inactive_plugins,
    })
}

/// Every input's answer: from `--set` if given, else by asking, else (headless)
/// the declared default.
fn resolve_responses(
    inputs: &HashMap<String, InputSpec>,
    overrides: &Overrides,
    mut ask: Option<Ask<'_>>,
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
        } else if let Some(ask) = ask.as_mut() {
            ask(spec)?
        } else {
            spec.default.clone().ok_or_else(|| {
                anyhow::anyhow!(
                    "input {id} has no default, so a headless run must be given it with --set {id}=..."
                )
            })?
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
                selections.push(FomodSelection {
                    condition: None,
                    ..selection
                });
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
            anyhow::bail!(
                "mod conditions depend on each other: {}",
                cycle.join(" -> ")
            );
        }
        let answer = match condition {
            None => true,
            Some(condition) => {
                stack.push(key);
                let answer = condition
                    .eval(responses, &mut |other| {
                        resolve(other, conditions, responses, done, stack)
                    })
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
        resolve(
            &entry.id,
            &conditions,
            responses,
            &mut done,
            &mut Vec::new(),
        )?;
    }
    Ok(done)
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
        let got = resolve_responses(&inputs, &none, None).unwrap();
        assert!(matches!(got["flag"], InputValue::Bool(true)));

        let set = parse_overrides(&["flag=false".to_string()]).unwrap();
        let got = resolve_responses(&inputs, &set, None).unwrap();
        assert!(matches!(got["flag"], InputValue::Bool(false)));
    }

    #[test]
    fn headless_without_a_default_or_set_is_an_error() {
        let err = resolve_responses(&bool_input(None), &Overrides::new(), None).unwrap_err();
        assert!(err.to_string().contains("--set flag="), "{err}");
    }

    #[test]
    fn set_must_name_a_declared_input_with_a_valid_value() {
        let inputs = bool_input(Some(true));
        let typo = parse_overrides(&["flg=true".to_string()]).unwrap();
        assert!(resolve_responses(&inputs, &typo, None).is_err());
        let bad = parse_overrides(&["flag=maybe".to_string()]).unwrap();
        assert!(resolve_responses(&inputs, &bad, None).is_err());
    }
}
