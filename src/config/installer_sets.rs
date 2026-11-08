//! An archive's installer choices as sets, one per condition: a base set,
//! chosen when no condition holds, and a set for each condition it names.
//!
//! FOMOD selections are a group's options, the last row naming a group the
//! one install takes: the base set is the rows without an `if`, and a
//! condition's set overrides it group by group with rows of that `if`,
//! written after them, only where they differ.
//!
//! BAIN packages add up, so a package is written by the sets it is in: plain
//! when it is in every one, `{ name, if = C }` for each condition's set it
//! is in, and `{ name, if = <none of the conditions> }` when it is in the
//! base set but not every one.

use crate::config::condition;
use crate::config::schema::{BainSubpackage, FomodSelection};

/// The conditions an archive's choices name: its sets beside the base, in
/// the order first written.
pub fn conditions(fomod: &[FomodSelection], bain: &[BainSubpackage]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for row in fomod {
        if let Some(c) = &row.condition {
            push_new(&mut out, c);
        }
    }
    for c in bain_sets(bain) {
        push_new(&mut out, &c);
    }
    out
}

fn push_new(out: &mut Vec<String>, c: &str) {
    let c = condition::normalized(c);
    if !out.contains(&c) {
        out.push(c);
    }
}

/// The FOMOD selections set `set` (`None`: the base) chooses: the base rows,
/// each overridden by that condition's row for its group.
pub fn fomod_set(rows: &[FomodSelection], set: Option<&str>) -> Vec<FomodSelection> {
    let want = set.map(condition::normalized);
    let mut out: Vec<FomodSelection> = Vec::new();
    for row in rows {
        let of = row.condition.as_deref().map(condition::normalized);
        if of.is_some() && of != want {
            continue;
        }
        let plain = FomodSelection {
            condition: None,
            ..row.clone()
        };
        match out.iter_mut().find(|r| same_group(r, &plain)) {
            Some(was) => *was = plain,
            None => out.push(plain),
        }
    }
    out
}

fn same_group(a: &FomodSelection, b: &FomodSelection) -> bool {
    a.step.eq_ignore_ascii_case(&b.step) && a.group.eq_ignore_ascii_case(&b.group)
}

/// `rows` with set `set` (`None`: the base) choosing `chosen`: the base rows
/// first, then each condition's, a condition's only where they differ from
/// the base.
pub fn write_fomod(
    rows: &[FomodSelection],
    set: Option<&str>,
    chosen: &[FomodSelection],
) -> Vec<FomodSelection> {
    let want = set.map(condition::normalized);
    let of = |row: &FomodSelection| row.condition.as_deref().map(condition::normalized);
    let plain = |row: &FomodSelection| FomodSelection {
        condition: None,
        ..row.clone()
    };
    let base: Vec<FomodSelection> = match &want {
        None => chosen.iter().map(plain).collect(),
        Some(_) => rows.iter().filter(|r| of(r).is_none()).cloned().collect(),
    };
    let mut out = base.clone();
    // The other conditions' rows, as they were.
    let mut placed = false;
    for row in rows.iter().filter(|r| of(r).is_some()) {
        if of(row) == want {
            if !placed {
                out.extend(overrides(&base, set, chosen));
                placed = true;
            }
            continue;
        }
        out.push(row.clone());
    }
    if !placed && want.is_some() {
        out.extend(overrides(&base, set, chosen));
    }
    out
}

/// Set `set`'s rows: those of `chosen` its base does not already choose.
fn overrides(
    base: &[FomodSelection],
    set: Option<&str>,
    chosen: &[FomodSelection],
) -> Vec<FomodSelection> {
    let Some(set) = set else { return Vec::new() };
    let sorted = |options: &[String]| {
        let mut o: Vec<String> = options.iter().map(|s| s.to_lowercase()).collect();
        o.sort();
        o
    };
    chosen
        .iter()
        .filter(|row| {
            !base
                .iter()
                .any(|b| same_group(b, row) && sorted(&b.options) == sorted(&row.options))
        })
        .map(|row| FomodSelection {
            condition: Some(set.trim().to_string()),
            ..row.clone()
        })
        .collect()
}

/// What an archive's BAIN packages say of each set: the conditions, the
/// base set's packages and each condition's, and entries that fit no set,
/// kept as they are.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BainSets {
    pub conditions: Vec<String>,
    pub base: Vec<String>,
    pub by_set: Vec<Vec<String>>,
    pub kept: Vec<BainSubpackage>,
}

/// The conditions BAIN packages name as sets: each `if` that is not the
/// base's "none of the others".
fn bain_sets(entries: &[BainSubpackage]) -> Vec<String> {
    let mut candidates: Vec<String> = Vec::new();
    for e in entries {
        if let Some(c) = e.condition() {
            let c = condition::normalized(c);
            if !candidates.contains(&c) {
                candidates.push(c);
            }
        }
    }
    // Taken as sets in order of fewest negations, each one that is "none of
    // the sets so far" being the base's: a condition and its opposite read
    // either way, and `potato` is the set, `!potato` the base.
    let negations = |c: &String| c.matches('!').count();
    let mut order: Vec<&String> = candidates.iter().collect();
    order.sort_by_key(|c| negations(c));
    let mut sets: Vec<String> = Vec::new();
    for c in order {
        let so_far: Vec<&str> = sets.iter().map(String::as_str).collect();
        let base = !so_far.is_empty()
            && condition::none_of(&so_far)
                .map(|n| condition::normalized(&n) == *c)
                .unwrap_or(false);
        if !base {
            sets.push(c.clone());
        }
    }
    // In the order first written.
    candidates.retain(|c| sets.contains(c));
    candidates
}

/// `entries` read as sets.
pub fn bain_read(entries: &[BainSubpackage]) -> BainSets {
    let conditions = bain_sets(entries);
    let none = conditions_none(&conditions);
    let mut sets = BainSets {
        by_set: vec![Vec::new(); conditions.len()],
        conditions,
        ..BainSets::default()
    };
    let add = |list: &mut Vec<String>, name: &str| {
        if !list.iter().any(|n| n.eq_ignore_ascii_case(name)) {
            list.push(name.to_string());
        }
    };
    for e in entries {
        match e.condition().map(condition::normalized) {
            None => {
                add(&mut sets.base, e.name());
                for list in &mut sets.by_set {
                    add(list, e.name());
                }
            }
            Some(c) if Some(&c) == none.as_ref() => add(&mut sets.base, e.name()),
            Some(c) => match sets.conditions.iter().position(|s| *s == c) {
                Some(at) => add(&mut sets.by_set[at], e.name()),
                None => sets.kept.push(e.clone()),
            },
        }
    }
    sets
}

/// The base's condition: none of the sets'.
fn conditions_none(conditions: &[String]) -> Option<String> {
    let all: Vec<&str> = conditions.iter().map(String::as_str).collect();
    (!all.is_empty())
        .then(|| {
            condition::none_of(&all)
                .ok()
                .map(|n| condition::normalized(&n))
        })
        .flatten()
}

/// The packages set `set` (`None`: the base) installs.
pub fn bain_set(entries: &[BainSubpackage], set: Option<&str>) -> Vec<String> {
    let sets = bain_read(entries);
    match set.map(condition::normalized) {
        None => sets.base,
        Some(c) => match sets.conditions.iter().position(|s| *s == c) {
            Some(at) => sets.by_set[at].clone(),
            // A set not yet made starts from the base.
            None => sets.base,
        },
    }
}

/// `entries` with set `set` (`None`: the base) installing `chosen`, every
/// package written by the sets it is in; `order` is the archive's packages,
/// to write them in.
pub fn write_bain(
    entries: &[BainSubpackage],
    set: Option<&str>,
    chosen: &[String],
    order: &[String],
) -> anyhow::Result<Vec<BainSubpackage>> {
    let mut sets = bain_read(entries);
    match set {
        None => sets.base = chosen.to_vec(),
        Some(c) => {
            let c = condition::normalized(c);
            match sets.conditions.iter().position(|s| *s == c) {
                Some(at) => sets.by_set[at] = chosen.to_vec(),
                None => {
                    sets.conditions.push(c);
                    sets.by_set.push(chosen.to_vec());
                }
            }
        }
    }
    let has = |list: &[String], name: &str| list.iter().any(|n| n.eq_ignore_ascii_case(name));
    // Every package any set has, in the archive's order.
    let mut names: Vec<String> = Vec::new();
    let candidates = order
        .iter()
        .filter(|n| has(&sets.base, n) || sets.by_set.iter().any(|l| has(l, n)))
        .chain(std::iter::once(&sets.base).chain(&sets.by_set).flatten());
    for n in candidates {
        if !has(&names, n) {
            names.push(n.clone());
        }
    }
    let none = conditions_none(&sets.conditions);
    let mut out = Vec::new();
    for name in &names {
        let in_base = has(&sets.base, name);
        let in_sets: Vec<usize> = (0..sets.conditions.len())
            .filter(|&i| has(&sets.by_set[i], name))
            .collect();
        if in_base && in_sets.len() == sets.conditions.len() {
            out.push(BainSubpackage::Name(name.clone()));
            continue;
        }
        for &i in &in_sets {
            out.push(BainSubpackage::Conditional {
                name: name.clone(),
                condition: sets.conditions[i].clone(),
            });
        }
        if in_base {
            let none = none
                .clone()
                .ok_or_else(|| anyhow::anyhow!("no condition to set the base apart by"))?;
            out.push(BainSubpackage::Conditional {
                name: name.clone(),
                condition: none,
            });
        }
    }
    out.extend(sets.kept);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(cond: Option<&str>, group: &str, options: &[&str]) -> FomodSelection {
        FomodSelection {
            condition: cond.map(str::to_string),
            step: "S".to_string(),
            group: group.to_string(),
            options: options.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn a_fomod_set_overrides_the_base_only_where_it_differs() {
        let base = vec![row(None, "Size", &["1K"]), row(None, "Extras", &["Rocks"])];
        let chosen = vec![row(None, "Size", &["2K"]), row(None, "Extras", &["Rocks"])];
        let written = write_fomod(&base, Some("res == 2k"), &chosen);
        assert_eq!(
            written,
            [
                row(None, "Size", &["1K"]),
                row(None, "Extras", &["Rocks"]),
                row(Some("res == 2k"), "Size", &["2K"]),
            ]
        );
        assert_eq!(fomod_set(&written, Some("res == 2k")), chosen);
        assert_eq!(fomod_set(&written, None), base);
        assert_eq!(conditions(&written, &[]), ["res == 2k"]);

        // The base changed keeps the condition's override.
        let rebased = write_fomod(
            &written,
            None,
            &[row(None, "Size", &["1K"]), row(None, "Extras", &[])],
        );
        assert_eq!(
            fomod_set(&rebased, Some("res == 2k")),
            [row(None, "Size", &["2K"]), row(None, "Extras", &[])]
        );
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn bain_sets_are_written_by_the_sets_each_package_is_in() {
        let order = names(&["00 Core", "01 1K", "02 2K", "03 Extra"]);
        let plain: Vec<BainSubpackage> = order[..2]
            .iter()
            .map(|n| BainSubpackage::Name(n.clone()))
            .collect();
        // 2k: Core and 2K, not 1K.
        let written = write_bain(
            &plain,
            Some("res == 2k"),
            &names(&["00 Core", "02 2K"]),
            &order,
        )
        .unwrap();
        assert_eq!(
            written,
            [
                BainSubpackage::Name("00 Core".to_string()),
                BainSubpackage::Conditional {
                    name: "01 1K".to_string(),
                    condition: "res != 2k".to_string()
                },
                BainSubpackage::Conditional {
                    name: "02 2K".to_string(),
                    condition: "res == 2k".to_string()
                },
            ]
        );
        assert_eq!(bain_set(&written, None), names(&["00 Core", "01 1K"]));
        assert_eq!(
            bain_set(&written, Some("res == 2k")),
            names(&["00 Core", "02 2K"])
        );
        assert_eq!(conditions(&[], &written), ["res == 2k"]);

        // A second, compound condition: the base is none of them, written out.
        let more = write_bain(
            &written,
            Some("enb && potato"),
            &names(&["00 Core", "03 Extra"]),
            &order,
        )
        .unwrap();
        assert_eq!(bain_set(&more, None), names(&["00 Core", "01 1K"]));
        assert_eq!(
            bain_set(&more, Some("res == 2k")),
            names(&["00 Core", "02 2K"])
        );
        assert_eq!(
            bain_set(&more, Some("enb && potato")),
            names(&["00 Core", "03 Extra"])
        );
        let none = more
            .iter()
            .find(|e| e.name() == "01 1K")
            .and_then(|e| e.condition())
            .unwrap()
            .to_string();
        assert_eq!(none, "res != 2k && !enb || res != 2k && !potato");
    }
}
