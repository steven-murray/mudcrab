//! An installer's choices as questions: a FOMOD group, a BAIN wizard's
//! `SelectOne`/`SelectMany`, a BAIN package list. All are "pick some of
//! these", with different rules about how many. `add` asks them on a
//! terminal; the GUI asks them in a form. Both take what is settled here
//! without asking, and check an answer here.

use serde::{Deserialize, Serialize};

/// How many options a question takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rule {
    ExactlyOne,
    AtMostOne,
    AtLeastOne,
    Any,
    /// Every usable option is installed; nothing to ask.
    All,
}

impl Rule {
    pub fn from_fomod(group_type: &str) -> Rule {
        match group_type {
            "SelectExactlyOne" => Rule::ExactlyOne,
            "SelectAtMostOne" => Rule::AtMostOne,
            "SelectAtLeastOne" => Rule::AtLeastOne,
            "SelectAll" => Rule::All,
            _ => Rule::Any,
        }
    }

    /// The rule, said to someone choosing at a terminal.
    pub fn instruction(self) -> &'static str {
        match self {
            Rule::ExactlyOne => "choose one",
            Rule::AtMostOne => "choose one, or 0 for none",
            Rule::AtLeastOne => "choose one or more, separated by spaces",
            Rule::Any => "choose any, separated by spaces, or 0 for none",
            Rule::All => "all are installed",
        }
    }
}

/// Whether an option can be chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    Free,
    /// Free, and the installer recommends it.
    Recommended,
    /// Always installed, whatever is chosen.
    Required,
    /// The installer rules it out (a missing dependency, say).
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Choice {
    pub name: String,
    pub description: String,
    pub availability: Availability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Question {
    /// What is being chosen, e.g. `HD Septim: "Step1" / "Meshes"`.
    pub title: String,
    pub rule: Rule,
    pub choices: Vec<Choice>,
    /// What install would pick unasked.
    pub default: Vec<usize>,
}

impl Question {
    fn usable(&self) -> Vec<usize> {
        (0..self.choices.len())
            .filter(|&i| self.choices[i].availability != Availability::Unavailable)
            .collect()
    }

    fn required(&self) -> Vec<usize> {
        self.usable()
            .into_iter()
            .filter(|&i| self.choices[i].availability == Availability::Required)
            .collect()
    }

    /// The answer when there is nothing to decide: everything (`All`), only
    /// what is required, or the one free option of a choose-one. `None` when
    /// there is a choice to make.
    pub fn settled(&self) -> Option<Vec<usize>> {
        let usable = self.usable();
        let required = self.required();
        let free: Vec<usize> = usable
            .iter()
            .copied()
            .filter(|i| !required.contains(i))
            .collect();
        match self.rule {
            Rule::All => Some(usable),
            _ if free.is_empty() => Some(required),
            Rule::ExactlyOne if free.len() == 1 && required.is_empty() => Some(free),
            _ => None,
        }
    }

    /// `picked`, checked against the rule, with the required options added:
    /// the answer as install will take it. Unavailable options and indices
    /// past the end are refused; an empty pick means none.
    pub fn accept(&self, picked: &[usize]) -> Result<Vec<usize>, String> {
        let required = self.required();
        let mut chosen: Vec<usize> = Vec::new();
        for &index in picked {
            let choice = self
                .choices
                .get(index)
                .ok_or_else(|| format!("there is no option {}", index + 1))?;
            if choice.availability == Availability::Unavailable {
                return Err(format!("{} is not available", choice.name));
            }
            if !required.contains(&index) && !chosen.contains(&index) {
                chosen.push(index);
            }
        }
        let count = chosen.len();
        match self.rule {
            Rule::ExactlyOne if count != 1 => return Err("choose exactly one".to_string()),
            Rule::AtMostOne if count > 1 => return Err("choose one at most".to_string()),
            Rule::AtLeastOne if count == 0 && required.is_empty() => {
                return Err("choose at least one".to_string());
            }
            _ => {}
        }
        chosen.extend(required);
        chosen.sort_unstable();
        Ok(chosen)
    }

    /// The chosen options' names, or "nothing".
    pub fn names(&self, chosen: &[usize]) -> String {
        if chosen.is_empty() {
            return "nothing".to_string();
        }
        chosen
            .iter()
            .map(|&i| self.choices[i].name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Availability::{Free, Required, Unavailable};

    fn question(rule: Rule, kinds: &[Availability]) -> Question {
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
            default: Vec::new(),
        }
    }

    #[test]
    fn what_is_settled_is_not_a_question() {
        assert_eq!(
            question(Rule::ExactlyOne, &[Free, Unavailable]).settled(),
            Some(vec![0])
        );
        assert_eq!(
            question(Rule::All, &[Free, Unavailable, Free]).settled(),
            Some(vec![0, 2])
        );
        assert_eq!(
            question(Rule::Any, &[Required, Unavailable]).settled(),
            Some(vec![0])
        );
        assert_eq!(question(Rule::ExactlyOne, &[Free, Free]).settled(), None);
    }

    #[test]
    fn an_answer_follows_the_rule_and_keeps_what_is_required() {
        let q = question(Rule::Any, &[Required, Free, Unavailable]);
        assert_eq!(q.accept(&[1]), Ok(vec![0, 1]));
        assert_eq!(q.accept(&[]), Ok(vec![0]));
        assert!(q.accept(&[2]).is_err());
        assert!(q.accept(&[7]).is_err());
        let q = question(Rule::ExactlyOne, &[Free, Free]);
        assert!(q.accept(&[0, 1]).is_err());
        assert!(q.accept(&[]).is_err());
        let q = question(Rule::AtLeastOne, &[Free, Free]);
        assert!(q.accept(&[]).is_err());
    }
}
