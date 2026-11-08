use crate::config::condition::Condition;
use crate::config::schema::{
    BainSubpackage, CompiledArchive, CompiledModlist, FomodSelection, InputSpec, InputType,
    InputValue, PersonalizedMod, PersonalizedPlan,
};
use indexmap::IndexMap;
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
    plan_with(compiled, overrides, ask, false).map(|planned| planned.plan)
}

/// The list planned for writing it rather than installing it: every mod,
/// the ones the answers leave out too, as each would install if it were in
/// -- its own `if` set aside, while what it installs (its archives', FOMOD
/// selections' and actions' conditions) still follows the answers. What
/// would stop an install (a dependency left out, a merge's source left out)
/// is noted instead.
#[derive(Debug, Clone)]
pub struct AuthoringPlan {
    /// Its `mods` hold every mod that can be planned; `selected_mods` only
    /// those the answers install, and `plugins` their load order.
    pub plan: PersonalizedPlan,
    /// The mods the answers leave out, by id.
    pub left_out: Vec<String>,
    /// What an install with these answers would refuse, or could not plan.
    pub notes: Vec<String>,
}

/// Plan `compiled` for writing it: see [`AuthoringPlan`].
pub fn build_authoring_plan(
    compiled: &CompiledModlist,
    overrides: &Overrides,
) -> anyhow::Result<AuthoringPlan> {
    plan_with(compiled, overrides, None, true)
}

fn plan_with(
    compiled: &CompiledModlist,
    overrides: &Overrides,
    ask: Option<Ask<'_>>,
    authoring: bool,
) -> anyhow::Result<AuthoringPlan> {
    let mut notes: Vec<String> = Vec::new();
    // Refused for an install; for writing the list, noted.
    let mut refuse = |message: String| -> anyhow::Result<()> {
        if authoring {
            notes.push(message);
            Ok(())
        } else {
            Err(anyhow::anyhow!(message))
        }
    };
    anyhow::ensure!(
        !compiled.component,
        "{} is a component, written to be included in other lists, not installed on its own",
        compiled.name_or("this list")
    );
    let responses = resolve_responses(&compiled.inputs, overrides, ask)?;

    let mod_order = compiled.mods.iter().map(|entry| entry.id.clone()).collect();
    let included = resolve_included(compiled, &responses)?;

    let mut selected_mods = Vec::new();
    let mut mods = Vec::new();
    let mut left_out = Vec::new();
    for entry in &compiled.mods {
        let installed = included[entry.id.as_str()];
        if !installed {
            left_out.push(entry.id.clone());
            if !authoring {
                continue;
            }
        }
        if let Some(dep) = entry
            .dependencies
            .iter()
            .filter(|_| installed)
            .find(|dep| !included.get(dep.as_str()).copied().unwrap_or(false))
        {
            refuse(format!(
                "mod {} is included but depends on {dep}, which is not; give {} `if = '{}'` \
                 (or change the answers) so they come and go together",
                entry.id,
                entry.id,
                crate::config::condition::mod_term(dep)
            ))?;
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
        let archives = match resolve_archives(&entry.id, &entry.archives, &applies) {
            Ok(archives) => archives,
            Err(err) if authoring => {
                refuse(format!("{err:#}"))?;
                continue;
            }
            Err(err) => return Err(err),
        };
        if archives.is_empty() && !entry.archives.is_empty() {
            refuse(format!(
                "mod {}: every archive's `if` left it out, so it would install nothing; \
                 put the condition on the mod instead",
                entry.id
            ))?;
            continue;
        }

        if installed {
            selected_mods.push(entry.id.clone());
        }
        mods.push(PersonalizedMod {
            id: entry.id.clone(),
            origin: entry.origin.clone(),
            oracle_name: entry.oracle_name.clone(),
            section: entry.section.clone(),
            mod_type: entry.mod_type,
            enabled: entry.enabled,
            merge: entry.merge.clone(),
            plugins: entry.plugins.clone(),
            dependencies: entry.dependencies.clone(),
            found: entry.found.clone(),
            optional_in: entry.optional_in.clone(),
            follows: match &entry.condition {
                Some(expr) => Condition::parse(expr)?
                    .mods()
                    .into_iter()
                    .map(str::to_string)
                    .collect(),
                None => Vec::new(),
            },
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
            refuse(format!(
                "merge {} takes {} from mod {}, which is left out; give the merge `if = '{}'`, \
                 or a second merge without it for when it is out",
                entry.id,
                source.plugin,
                source.mod_id,
                crate::config::condition::mod_term(&source.mod_id)
            ))?;
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

    let plan = PersonalizedPlan {
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
    };
    Ok(AuthoringPlan {
        plan,
        left_out,
        notes,
    })
}

/// Every input's answer, in the order the list declares them: from `--set`
/// if given, else by asking, else (headless) its default. A question whose
/// `if` does not hold on the answers before it is not asked: it takes its
/// default, whatever `--set` says (a front end sends every answer, even a
/// hidden question's).
fn resolve_responses(
    inputs: &IndexMap<String, InputSpec>,
    overrides: &Overrides,
    mut ask: Option<Ask<'_>>,
) -> anyhow::Result<HashMap<String, InputValue>> {
    if let Some(unknown) = overrides.keys().find(|key| !inputs.contains_key(*key)) {
        let mut known: Vec<&String> = inputs.keys().collect();
        known.sort();
        anyhow::bail!("--set {unknown}: no such input (the modlist declares {known:?})");
    }

    let mut out = HashMap::new();
    for (id, spec) in inputs {
        // Answered by the list that includes the file asking it.
        if let Some(fixed) = &spec.fixed {
            anyhow::ensure!(
                !overrides.contains_key(id),
                "--set {id}: the list answers {id} itself, so it cannot be set"
            );
            out.insert(id.clone(), fixed.clone());
            continue;
        }
        let default = default_for(id, spec, &out)?;
        let value = if !asked(id, spec, &out)? {
            match default {
                Some(value) => value,
                None => unasked(id, spec)?,
            }
        } else if let Some(raw) = overrides.get(id) {
            parse_answer(id, spec, raw)?
        } else if let Some(ask) = ask.as_mut() {
            ask(&InputSpec {
                default,
                ..spec.clone()
            })?
        } else {
            default.ok_or_else(|| {
                anyhow::anyhow!(
                    "input {id} has no default, so a headless run must be given it with --set {id}=..."
                )
            })?
        };
        out.insert(id.clone(), value);
    }
    Ok(out)
}

/// Whether `spec` is asked, on the answers before it.
fn asked(id: &str, spec: &InputSpec, before: &HashMap<String, InputValue>) -> anyhow::Result<bool> {
    match &spec.condition {
        Some(expr) => Condition::parse(expr)?
            .eval_inputs(before)
            .map_err(|err| anyhow::anyhow!("input {id}: its `if`: {err:#}")),
        None => Ok(true),
    }
}

/// `spec`'s default, on the answers before it: the first `default_when`
/// that holds, else `default`; a multi's once each, in its choices' order.
fn default_for(
    id: &str,
    spec: &InputSpec,
    before: &HashMap<String, InputValue>,
) -> anyhow::Result<Option<InputValue>> {
    let mut default = spec.default.clone();
    for case in &spec.default_when {
        if Condition::parse(&case.condition)?
            .eval_inputs(before)
            .map_err(|err| anyhow::anyhow!("input {id}: a default_when: {err:#}"))?
        {
            default = Some(case.value.clone());
            break;
        }
    }
    Ok(match default {
        Some(InputValue::List(picked)) if spec.input_type == InputType::Multi => {
            Some(multi_answer(spec, picked.iter().map(String::as_str))?)
        }
        other => other,
    })
}

/// The answer to a question not asked and with no default: no for a bool,
/// none picked, nothing written. A choice must say which.
fn unasked(id: &str, spec: &InputSpec) -> anyhow::Result<InputValue> {
    Ok(match spec.input_type {
        InputType::Bool => InputValue::Bool(false),
        InputType::Multi => InputValue::List(Vec::new()),
        InputType::Text => InputValue::Text(String::new()),
        InputType::Choice => anyhow::bail!(
            "input {id} is not asked (its `if` does not hold) and has no default to take"
        ),
    })
}

/// A question as the answers so far leave it: whether it is asked, and its
/// default -- for a front end showing the questions as they are answered.
#[derive(Debug, Clone, serde::Serialize)]
pub struct QuestionState {
    pub id: String,
    pub asked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<InputValue>,
}

/// Each question, in order, as `answers` (as `--set` gives them) leave it.
/// An answer that does not fit its question is taken as not given.
pub fn question_states(
    inputs: &IndexMap<String, InputSpec>,
    answers: &Overrides,
) -> anyhow::Result<Vec<QuestionState>> {
    let mut before = HashMap::new();
    let mut out = Vec::new();
    for (id, spec) in inputs {
        if let Some(fixed) = &spec.fixed {
            before.insert(id.clone(), fixed.clone());
            continue;
        }
        let asked = asked(id, spec, &before)?;
        let default = default_for(id, spec, &before)?;
        let value = if asked {
            answers
                .get(id)
                .and_then(|raw| parse_answer(id, spec, raw).ok())
                .or_else(|| default.clone())
        } else {
            default.clone().or_else(|| unasked(id, spec).ok())
        };
        if let Some(value) = value {
            before.insert(id.clone(), value);
        }
        out.push(QuestionState {
            id: id.clone(),
            asked,
            default,
        });
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
        InputType::Multi => multi_answer(spec, raw.split(',').map(str::trim))
            .map_err(|err| anyhow::anyhow!("--set {id}={raw}: {err}")),
    }
}

/// A `multi` answer from the choices picked, each one of the input's own:
/// kept in the order of its `choices`, each once.
pub fn multi_answer<'a>(
    spec: &InputSpec,
    picked: impl IntoIterator<Item = &'a str>,
) -> anyhow::Result<InputValue> {
    let picked: Vec<&str> = picked.into_iter().filter(|p| !p.is_empty()).collect();
    if let Some(stray) = picked
        .iter()
        .find(|p| !spec.choices.iter().any(|c| c == *p))
    {
        anyhow::bail!("{stray} is not one of {:?}", spec.choices);
    }
    Ok(InputValue::List(
        spec.choices
            .iter()
            .filter(|choice| picked.contains(&choice.as_str()))
            .cloned()
            .collect(),
    ))
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

    fn bool_input(default: Option<bool>) -> IndexMap<String, InputSpec> {
        IndexMap::from([(
            "flag".to_string(),
            InputSpec {
                input_type: InputType::Bool,
                query: "?".to_string(),
                choices: Vec::new(),
                default: default.map(InputValue::Bool),
                fixed: None,
                condition: None,
                default_when: Vec::new(),
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
