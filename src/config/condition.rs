//! `if` expressions: what a condition can ask, and how it is answered.
//!
//! A condition is one or more terms joined by `&&` and `||` (`&&` binds
//! tighter; there are no parentheses, which are refused). A term is optionally negated with `!`
//! and is one of:
//!
//! - `flag` -- a bool input is true, or a text input is non-empty;
//! - `key == value`, `key != value` -- an input's answer, value optionally quoted;
//! - `mod:id` -- the mod `id` is in the resolved list (`mod:"Some Name"` when
//!   the id has spaces or operators in it).
//!
//! Asking about mods is what makes a mod's inclusion depend on another's, so
//! `query` resolves mods in whatever order their conditions require and
//! `validate` refuses a set of conditions that would need each other.

use crate::config::schema::InputValue;
use std::collections::HashMap;

/// A parsed condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Condition {
    /// Any of these holds.
    Any(Vec<Condition>),
    /// All of these hold.
    All(Vec<Condition>),
    Not(Box<Condition>),
    Input {
        key: String,
        test: InputTest,
    },
    Mod(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputTest {
    Truthy,
    Equals(String),
    NotEquals(String),
}

impl Condition {
    pub fn parse(expr: &str) -> anyhow::Result<Condition> {
        parse_any(expr).map_err(|err| anyhow::anyhow!("condition `{expr}`: {err}"))
    }

    /// Every input this condition reads.
    pub fn inputs(&self) -> Vec<&str> {
        let mut out = Vec::new();
        self.walk(&mut |c| {
            if let Condition::Input { key, .. } = c {
                out.push(key.as_str());
            }
        });
        out
    }

    /// Every mod this condition asks about.
    pub fn mods(&self) -> Vec<&str> {
        let mut out = Vec::new();
        self.walk(&mut |c| {
            if let Condition::Mod(id) = c {
                out.push(id.as_str());
            }
        });
        out
    }

    fn walk<'a>(&'a self, visit: &mut dyn FnMut(&'a Condition)) {
        visit(self);
        match self {
            Condition::Any(terms) | Condition::All(terms) => {
                terms.iter().for_each(|t| t.walk(visit));
            }
            Condition::Not(inner) => inner.walk(visit),
            Condition::Input { .. } | Condition::Mod(_) => {}
        }
    }

    /// Answer the condition. `mod_included` is asked about each mod term; it
    /// is fallible because answering may mean resolving that mod's own
    /// condition first.
    pub fn eval(
        &self,
        responses: &HashMap<String, InputValue>,
        mod_included: &mut dyn FnMut(&str) -> anyhow::Result<bool>,
    ) -> anyhow::Result<bool> {
        Ok(match self {
            Condition::Any(terms) => {
                for term in terms {
                    if term.eval(responses, mod_included)? {
                        return Ok(true);
                    }
                }
                false
            }
            Condition::All(terms) => {
                for term in terms {
                    if !term.eval(responses, mod_included)? {
                        return Ok(false);
                    }
                }
                true
            }
            Condition::Not(inner) => !inner.eval(responses, mod_included)?,
            Condition::Mod(id) => mod_included(id)?,
            Condition::Input { key, test } => {
                let answer = responses.get(key);
                match test {
                    InputTest::Truthy => match answer {
                        Some(InputValue::Bool(v)) => *v,
                        Some(InputValue::Text(v)) => !v.trim().is_empty(),
                        None => false,
                    },
                    InputTest::Equals(want) => answer.is_some_and(|a| answer_text(a) == *want),
                    InputTest::NotEquals(want) => answer.is_some_and(|a| answer_text(a) != *want),
                }
            }
        })
    }

    /// Answer a condition that asks about no mods.
    pub fn eval_inputs(&self, responses: &HashMap<String, InputValue>) -> anyhow::Result<bool> {
        self.eval(responses, &mut |id| {
            anyhow::bail!("asks about mod {id}, which is not resolved here")
        })
    }
}

impl Condition {
    /// The mods this condition asks about, renamed by `rename` where it
    /// gives a new name.
    pub fn rename_mods(&mut self, rename: &dyn Fn(&str) -> Option<String>) {
        match self {
            Condition::Any(terms) | Condition::All(terms) => {
                terms.iter_mut().for_each(|t| t.rename_mods(rename));
            }
            Condition::Not(inner) => inner.rename_mods(rename),
            Condition::Mod(id) => {
                if let Some(new) = rename(id) {
                    *id = new;
                }
            }
            Condition::Input { .. } => {}
        }
    }

    /// The inputs this condition reads, renamed by `rename` where it gives a
    /// new name.
    pub fn rename_inputs(&mut self, rename: &dyn Fn(&str) -> Option<String>) {
        match self {
            Condition::Any(terms) | Condition::All(terms) => {
                terms.iter_mut().for_each(|t| t.rename_inputs(rename));
            }
            Condition::Not(inner) => inner.rename_inputs(rename),
            Condition::Input { key, .. } => {
                if let Some(new) = rename(key) {
                    *key = new;
                }
            }
            Condition::Mod(_) => {}
        }
    }

    /// The condition as text `parse` reads back to the same answers. With no
    /// parentheses, that is terms joined by `&&`, joined by `||`: `!` is
    /// pushed down onto the terms, and an `&&` of `||`s multiplied out.
    pub fn to_text(&self) -> String {
        dnf(self, false)
            .iter()
            .map(|all| {
                all.iter()
                    .map(|(negated, term)| {
                        format!("{}{}", if *negated { "!" } else { "" }, term_text(term))
                    })
                    .collect::<Vec<_>>()
                    .join(" && ")
            })
            .collect::<Vec<_>>()
            .join(" || ")
    }
}

/// `condition` (negated, if `negated`) as an `||` of `&&`s of terms, each
/// term perhaps negated.
fn dnf(condition: &Condition, negated: bool) -> Vec<Vec<(bool, &Condition)>> {
    match condition {
        Condition::Any(terms) if !negated => sum(terms, false),
        Condition::Any(terms) => product(terms, true),
        Condition::All(terms) if !negated => product(terms, false),
        Condition::All(terms) => sum(terms, true),
        Condition::Not(inner) => dnf(inner, !negated),
        term => vec![vec![(negated, term)]],
    }
}

/// Any of `terms`.
fn sum(terms: &[Condition], negated: bool) -> Vec<Vec<(bool, &Condition)>> {
    terms.iter().flat_map(|term| dnf(term, negated)).collect()
}

/// All of `terms`, multiplied out.
fn product(terms: &[Condition], negated: bool) -> Vec<Vec<(bool, &Condition)>> {
    terms.iter().fold(vec![Vec::new()], |sum, term| {
        let next = dnf(term, negated);
        sum.iter()
            .flat_map(|left| {
                next.iter().map(move |right| {
                    let mut both = left.clone();
                    both.extend(right.iter().cloned());
                    both
                })
            })
            .collect()
    })
}

fn term_text(term: &Condition) -> String {
    let value = |value: &str| {
        if value.is_empty() || value.contains(|c: char| c.is_whitespace() || "&|!=".contains(c)) {
            format!("\"{value}\"")
        } else {
            value.to_string()
        }
    };
    match term {
        Condition::Mod(id) => mod_term(id),
        Condition::Input { key, test } => match test {
            InputTest::Truthy => key.clone(),
            InputTest::Equals(want) => format!("{key} == {}", value(want)),
            InputTest::NotEquals(want) => format!("{key} != {}", value(want)),
        },
        _ => unreachable!("only terms are left once multiplied out"),
    }
}

/// Both `a` and `b`, as one condition's text.
pub fn and(a: &str, b: &str) -> anyhow::Result<String> {
    Ok(Condition::All(vec![Condition::parse(a)?, Condition::parse(b)?]).to_text())
}

/// The term that asks about mod `id`, quoted when it has to be.
pub fn mod_term(id: &str) -> String {
    if id.contains(|c: char| c.is_whitespace() || "&|!=\"".contains(c)) {
        format!("mod:\"{id}\"")
    } else {
        format!("mod:{id}")
    }
}

fn answer_text(answer: &InputValue) -> String {
    match answer {
        InputValue::Bool(v) => v.to_string(),
        InputValue::Text(v) => v.clone(),
    }
}

/// Split `expr` on `op`, ignoring any inside double quotes.
fn split_outside_quotes<'a>(expr: &'a str, op: &str) -> Vec<&'a str> {
    let mut parts = Vec::new();
    let mut in_quotes = false;
    let mut start = 0;
    let mut i = 0;
    let bytes = expr.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'"' {
            in_quotes = !in_quotes;
        } else if !in_quotes && expr[i..].starts_with(op) {
            parts.push(&expr[start..i]);
            i += op.len();
            start = i;
            continue;
        }
        i += 1;
    }
    parts.push(&expr[start..]);
    parts
}

fn parse_any(expr: &str) -> anyhow::Result<Condition> {
    let mut terms = split_outside_quotes(expr, "||")
        .into_iter()
        .map(parse_all)
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(if terms.len() == 1 {
        terms.remove(0)
    } else {
        Condition::Any(terms)
    })
}

fn parse_all(expr: &str) -> anyhow::Result<Condition> {
    let mut terms = split_outside_quotes(expr, "&&")
        .into_iter()
        .map(parse_term)
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(if terms.len() == 1 {
        terms.remove(0)
    } else {
        Condition::All(terms)
    })
}

fn parse_term(term: &str) -> anyhow::Result<Condition> {
    let term = term.trim();
    if let Some(rest) = term.strip_prefix('!')
        && !rest.starts_with('=')
    {
        return Ok(Condition::Not(Box::new(parse_term(rest)?)));
    }
    if let Some(id) = term.strip_prefix("mod:") {
        let id = unquote(id.trim())?;
        anyhow::ensure!(!id.is_empty(), "`mod:` needs a mod id");
        return Ok(Condition::Mod(id));
    }
    for (op, make) in [
        ("==", InputTest::Equals as fn(String) -> InputTest),
        ("!=", InputTest::NotEquals),
    ] {
        let parts = split_outside_quotes(term, op);
        if parts.len() == 2 {
            let key = input_key(parts[0])?;
            return Ok(Condition::Input {
                key,
                test: make(unquote(parts[1].trim())?),
            });
        }
        anyhow::ensure!(parts.len() < 3, "`{term}` uses {op} twice");
    }
    Ok(Condition::Input {
        key: input_key(term)?,
        test: InputTest::Truthy,
    })
}

fn input_key(raw: &str) -> anyhow::Result<String> {
    let key = raw.trim();
    anyhow::ensure!(!key.is_empty(), "a term is empty");
    anyhow::ensure!(
        !key.contains(|c: char| c.is_whitespace()
            || c == '"'
            || c == '!'
            || c == '='
            || c == '('
            || c == ')'),
        "`{key}` is not an input name (to ask about a mod, write mod:\"{key}\")"
    );
    Ok(key.to_string())
}

fn unquote(raw: &str) -> anyhow::Result<String> {
    match raw.strip_prefix('"') {
        Some(rest) => rest
            .strip_suffix('"')
            .filter(|inner| !inner.contains('"'))
            .map(str::to_string)
            .ok_or_else(|| anyhow::anyhow!("unbalanced quotes in `{raw}`")),
        None => Ok(raw.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn responses() -> HashMap<String, InputValue> {
        HashMap::from([
            ("a".to_string(), InputValue::Bool(true)),
            ("b".to_string(), InputValue::Text("orange".to_string())),
        ])
    }

    #[test]
    fn a_condition_written_back_answers_the_same() {
        let answers = [
            responses(),
            HashMap::from([("b".to_string(), InputValue::Text("x y".to_string()))]),
            HashMap::new(),
        ];
        for expr in [
            "a",
            "!a && b == orange",
            "a || b != \"x y\" && !mod:\"Y Z\"",
            "mod:A || mod:B && !a",
        ] {
            let parsed = Condition::parse(expr).unwrap();
            let again = Condition::parse(&parsed.to_text()).unwrap();
            for mods in [&[][..], &["A"][..], &["Y Z", "B"][..]] {
                for responses in &answers {
                    let ask = |c: &Condition| {
                        c.eval(responses, &mut |id| Ok(mods.contains(&id))).unwrap()
                    };
                    assert_eq!(ask(&parsed), ask(&again), "{expr} as {}", parsed.to_text());
                }
            }
        }
    }

    #[test]
    fn two_conditions_are_joined_without_parentheses() {
        assert_eq!(and("a || b", "c").unwrap(), "a && c || b && c");
        assert!(and("a", "!(x)").is_err());
        assert_eq!(
            and("!mod:A", "b == \"x y\"").unwrap(),
            "!mod:A && b == \"x y\""
        );
    }

    #[test]
    fn mods_are_renamed() {
        let mut c = Condition::parse("mod:A && !mod:B || a").unwrap();
        c.rename_mods(&|id| (id == "A").then(|| "Group A".to_string()));
        assert_eq!(c.to_text(), "mod:\"Group A\" && !mod:B || a");
    }

    fn eval(expr: &str, mods: &[&str]) -> bool {
        Condition::parse(expr)
            .unwrap()
            .eval(&responses(), &mut |id| Ok(mods.contains(&id)))
            .unwrap()
    }

    #[test]
    fn input_terms() {
        assert!(eval("a", &[]));
        assert!(!eval("!a", &[]));
        assert!(eval("b == orange", &[]));
        assert!(eval("b == \"orange\"", &[]));
        assert!(eval("b != blue", &[]));
        assert!(eval("b!=\"blue\"", &[]));
        assert!(!eval("missing", &[]));
        assert!(!eval("!b == orange", &[]));
    }

    #[test]
    fn mod_terms_and_combinations() {
        assert!(eval("mod:A", &["A"]));
        assert!(!eval("mod:A", &[]));
        assert!(eval("!mod:A", &[]));
        assert!(eval(
            "mod:\"Cleaned DLC Masters\"",
            &["Cleaned DLC Masters"]
        ));
        assert!(eval("mod:A && a", &["A"]));
        assert!(!eval("mod:A && !a", &["A"]));
        assert!(eval("mod:A || b == orange", &[]));
        // && binds tighter than ||.
        assert!(eval("!a && mod:A || mod:B", &["B"]));
        // Operators inside quotes are part of the id.
        assert!(eval("mod:\"A && B\"", &["A && B"]));
    }

    #[test]
    fn reports_what_it_reads() {
        let c = Condition::parse("a && mod:X || b != y && !mod:\"Y Z\"").unwrap();
        assert_eq!(c.inputs(), ["a", "b"]);
        assert_eq!(c.mods(), ["X", "Y Z"]);
    }

    #[test]
    fn rejects_malformed() {
        for bad in [
            "",
            "a &&",
            "mod:",
            "mod:\"x",
            "Some Mod",
            "a == b == c",
            "(a || b) && c",
        ] {
            assert!(Condition::parse(bad).is_err(), "{bad}");
        }
        let err = Condition::parse("Cleaned DLC Masters").unwrap_err();
        assert!(
            format!("{err:#}").contains("mod:\"Cleaned DLC Masters\""),
            "{err:#}"
        );
    }
}
