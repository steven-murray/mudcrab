//! Running a BAIN wizard, as Wrye Bash would, to find out what it installs.
//!
//! Everything a wizard needs from outside the archive goes through [`Host`]:
//! the questions it asks, whether a file is already in the game's data, a
//! plugin's status. `add` answers those from the modlist and the person at the
//! keyboard; tests answer them from a script.
//!
//! What it records is what a modlist can say: which subpackages, which
//! plugins are left out or renamed, and the INI edits (`EditINI` writes an INI
//! Tweak fragment, which is mudcrab's `ini_tweak`).

use super::script::{Case, Expr, ForKind, Stmt, Value, parse};
use std::collections::HashMap;

/// The archive the wizard is choosing from.
pub struct Package {
    /// Subpackage names, in archive order.
    pub subpackages: Vec<String>,
    /// Every file path in the archive, `/`-separated.
    pub files: Vec<String>,
}

/// A question the wizard asks.
pub struct Prompt {
    pub question: String,
    pub many: bool,
    /// (name, description) per option.
    pub options: Vec<(String, String)>,
    /// What the wizard pre-selects.
    pub defaults: Vec<usize>,
}

pub trait Host {
    /// Ask a question; the chosen option indices.
    fn choose(&mut self, prompt: &Prompt) -> anyhow::Result<Vec<usize>>;
    /// Whether a file (relative to `Data`) is already installed.
    fn data_file_exists(&mut self, path: &str) -> anyhow::Result<bool>;
    /// Wrye Bash's plugin status: -1 missing, 0 inactive, 1 active.
    fn plugin_status(&mut self, name: &str) -> i64;
    /// A message the wizard shows.
    fn note(&mut self, text: &str);
}

#[derive(Debug, Clone, PartialEq)]
pub struct IniEdit {
    pub file: String,
    pub section: String,
    pub key: String,
    pub value: String,
}

#[derive(Debug, Default, PartialEq)]
pub struct Outcome {
    /// The subpackages to install, in archive order.
    pub subpackages: Vec<String>,
    /// Plugins in those subpackages that the wizard left unselected.
    pub left_out_plugins: Vec<String>,
    /// (installed name, new name) for plugins the wizard renamed.
    pub renamed_plugins: Vec<(String, String)>,
    pub ini_edits: Vec<IniEdit>,
    /// `DisableINILine(file, section, key)` calls: recorded, not acted on.
    pub disabled_ini_lines: Vec<(String, String, String)>,
}

pub enum Ending {
    Finished(Outcome),
    /// The wizard called `Cancel`, with its message.
    Cancelled(String),
}

/// Run `script` against `package`, asking `host` whatever it needs.
pub fn run(script: &str, package: &Package, host: &mut dyn Host) -> anyhow::Result<Ending> {
    let program = parse(script)?;
    let mut machine = Machine::new(package, host);
    match machine.block(&program)? {
        Flow::Cancel(message) => Ok(Ending::Cancelled(message)),
        _ => Ok(Ending::Finished(machine.outcome())),
    }
}

enum Flow {
    Normal,
    Break,
    Continue,
    Return,
    Cancel(String),
}

/// A plugin at the top of a subpackage, which BAIN lets a wizard select.
struct Espm {
    name: String,
    selected: bool,
    renamed: Option<String>,
}

/// Guards against a wizard loop that never ends.
const MAX_ITERATIONS: usize = 100_000;

struct Machine<'a> {
    package: &'a Package,
    host: &'a mut dyn Host,
    vars: HashMap<String, Value>,
    selected: Vec<bool>,
    espms: Vec<Espm>,
    ini_edits: Vec<IniEdit>,
    disabled: Vec<(String, String, String)>,
    iterations: usize,
}

fn is_plugin(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".esp") || lower.ends_with(".esm")
}

impl<'a> Machine<'a> {
    fn new(package: &'a Package, host: &'a mut dyn Host) -> Self {
        let mut espms: Vec<Espm> = Vec::new();
        for path in &package.files {
            let Some((sub, name)) = path.split_once('/') else {
                continue;
            };
            let in_package = package
                .subpackages
                .iter()
                .any(|s| s.eq_ignore_ascii_case(sub));
            if in_package
                && !name.contains('/')
                && is_plugin(name)
                && !espms.iter().any(|e| e.name.eq_ignore_ascii_case(name))
            {
                espms.push(Espm {
                    name: name.to_string(),
                    selected: true,
                    renamed: None,
                });
            }
        }
        Machine {
            package,
            host,
            vars: HashMap::new(),
            selected: vec![false; package.subpackages.len()],
            espms,
            ini_edits: Vec::new(),
            disabled: Vec::new(),
            iterations: 0,
        }
    }

    fn outcome(&self) -> Outcome {
        let chosen: Vec<&String> = self
            .package
            .subpackages
            .iter()
            .zip(&self.selected)
            .filter(|(_, on)| **on)
            .map(|(name, _)| name)
            .collect();
        // Only plugins that the chosen subpackages actually install matter.
        let installed = |espm: &Espm| {
            chosen.iter().any(|sub| {
                self.package.files.iter().any(|path| {
                    path.split_once('/').is_some_and(|(s, rest)| {
                        s.eq_ignore_ascii_case(sub) && rest.eq_ignore_ascii_case(&espm.name)
                    })
                })
            })
        };
        Outcome {
            subpackages: chosen.iter().map(|s| s.to_string()).collect(),
            left_out_plugins: self
                .espms
                .iter()
                .filter(|e| !e.selected && installed(e))
                .map(|e| e.name.clone())
                .collect(),
            renamed_plugins: self
                .espms
                .iter()
                .filter(|e| e.selected && installed(e))
                .filter_map(|e| e.renamed.clone().map(|to| (e.name.clone(), to)))
                .collect(),
            ini_edits: self.ini_edits.clone(),
            disabled_ini_lines: self.disabled.clone(),
        }
    }

    fn tick(&mut self) -> anyhow::Result<()> {
        self.iterations += 1;
        if self.iterations > MAX_ITERATIONS {
            anyhow::bail!("the wizard loops without end");
        }
        Ok(())
    }

    fn block(&mut self, stmts: &[Stmt]) -> anyhow::Result<Flow> {
        for stmt in stmts {
            match self.statement(stmt)? {
                Flow::Normal => {}
                other => return Ok(other),
            }
        }
        Ok(Flow::Normal)
    }

    fn statement(&mut self, stmt: &Stmt) -> anyhow::Result<Flow> {
        match stmt {
            Stmt::Command(name, args) => self.command(name, args),
            Stmt::Assign(name, op, expr) => {
                let value = self.eval(expr)?;
                let key = name.to_ascii_lowercase();
                let value = if op == "=" {
                    value
                } else {
                    let current = self
                        .vars
                        .get(&key)
                        .cloned()
                        .ok_or_else(|| anyhow::anyhow!("{name} is used before it is set"))?;
                    binary(&op[..1], current, value)?
                };
                self.vars.insert(key, value);
                Ok(Flow::Normal)
            }
            Stmt::Select {
                many,
                args,
                cases,
                default,
            } => self.select(*many, args, cases, default.as_deref()),
            Stmt::If {
                branches,
                otherwise,
            } => {
                for (cond, body) in branches {
                    if truthy(&self.eval(cond)?) {
                        return self.block(body);
                    }
                }
                match otherwise {
                    Some(body) => self.block(body),
                    None => Ok(Flow::Normal),
                }
            }
            Stmt::While(cond, body) => {
                while truthy(&self.eval(cond)?) {
                    self.tick()?;
                    match self.block(body)? {
                        Flow::Break => break,
                        Flow::Normal | Flow::Continue => {}
                        other => return Ok(other),
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::For(var, kind, body) => {
                let items = self.for_items(kind)?;
                for item in items {
                    self.tick()?;
                    self.vars.insert(var.to_ascii_lowercase(), item);
                    match self.block(body)? {
                        Flow::Break => break,
                        Flow::Normal | Flow::Continue => {}
                        other => return Ok(other),
                    }
                }
                Ok(Flow::Normal)
            }
            Stmt::Break => Ok(Flow::Break),
            Stmt::Continue => Ok(Flow::Continue),
            Stmt::Return => Ok(Flow::Return),
        }
    }

    fn for_items(&mut self, kind: &ForKind) -> anyhow::Result<Vec<Value>> {
        match kind {
            ForKind::Range { from, to, by } => {
                let from = as_int(&self.eval(from)?)?;
                let to = as_int(&self.eval(to)?)?;
                let by = match by {
                    Some(by) => as_int(&self.eval(by)?)?,
                    None if to >= from => 1,
                    None => -1,
                };
                if by == 0 {
                    anyhow::bail!("For ... by 0 never ends");
                }
                let mut out = Vec::new();
                let mut i = from;
                while (by > 0 && i <= to) || (by < 0 && i >= to) {
                    out.push(Value::Int(i));
                    i += by;
                    if out.len() > MAX_ITERATIONS {
                        anyhow::bail!("the wizard loops without end");
                    }
                }
                Ok(out)
            }
            ForKind::In(Expr::Var(name)) if name.eq_ignore_ascii_case("subpackages") => Ok(self
                .package
                .subpackages
                .iter()
                .map(|s| Value::Str(s.clone()))
                .collect()),
            ForKind::In(expr) => match self.eval(expr)? {
                // A subpackage's name iterates over its files.
                Value::Str(name) if self.subpackage(&name).is_some() => Ok(self
                    .package
                    .files
                    .iter()
                    .filter_map(|path| {
                        let (sub, rest) = path.split_once('/')?;
                        sub.eq_ignore_ascii_case(&name)
                            .then(|| Value::Str(rest.to_string()))
                    })
                    .collect()),
                Value::Str(text) => Ok(text.chars().map(|c| Value::Str(c.to_string())).collect()),
                other => anyhow::bail!("cannot loop over {other:?}"),
            },
        }
    }

    fn select(
        &mut self,
        many: bool,
        args: &[Expr],
        cases: &[Case],
        default: Option<&[Stmt]>,
    ) -> anyhow::Result<Flow> {
        let mut strings = Vec::new();
        for arg in args {
            strings.push(as_str(&self.eval(arg)?));
        }
        let Some((question, triples)) = strings.split_first() else {
            anyhow::bail!("a Select has no question");
        };
        let mut options = Vec::new();
        let mut defaults = Vec::new();
        for (index, triple) in triples.chunks(3).enumerate() {
            let name = &triple[0];
            if let Some(stripped) = name.strip_prefix('|') {
                defaults.push(index);
                options.push((
                    stripped.to_string(),
                    triple.get(1).cloned().unwrap_or_default(),
                ));
            } else {
                options.push((name.clone(), triple.get(1).cloned().unwrap_or_default()));
            }
        }
        // A SelectOne with nothing marked starts on its first option.
        if !many && defaults.is_empty() && !options.is_empty() {
            defaults.push(0);
        }
        let prompt = Prompt {
            question: question.clone(),
            many,
            options,
            defaults,
        };
        let chosen = self.host.choose(&prompt)?;
        let names: Vec<&str> = chosen
            .iter()
            .filter_map(|&i| prompt.options.get(i).map(|(n, _)| n.as_str()))
            .collect();

        let mut matched = false;
        for case in cases {
            if !names.iter().any(|n| n.eq_ignore_ascii_case(&case.label)) {
                continue;
            }
            matched = true;
            match self.block(&case.body)? {
                // `Break` ends the case.
                Flow::Normal | Flow::Break => {}
                other => return Ok(other),
            }
        }
        if !matched && let Some(body) = default {
            match self.block(body)? {
                Flow::Normal | Flow::Break => {}
                other => return Ok(other),
            }
        }
        Ok(Flow::Normal)
    }

    fn subpackage(&self, name: &str) -> Option<usize> {
        self.package
            .subpackages
            .iter()
            .position(|s| s.eq_ignore_ascii_case(name))
    }

    fn espm(&mut self, name: &str) -> Option<&mut Espm> {
        self.espms
            .iter_mut()
            .find(|e| e.name.eq_ignore_ascii_case(name))
    }

    fn command(&mut self, name: &str, args: &[Expr]) -> anyhow::Result<Flow> {
        let mut values = Vec::new();
        for arg in args {
            values.push(self.eval(arg)?);
        }
        let text = |i: usize| values.get(i).map(as_str).unwrap_or_default();
        match name.to_ascii_lowercase().as_str() {
            "selectsubpackage" | "deselectsubpackage" => {
                let on = name.eq_ignore_ascii_case("selectsubpackage");
                match self.subpackage(&text(0)) {
                    Some(index) => {
                        self.selected[index] = on;
                        // As in Wrye Bash: a subpackage brings its plugins
                        // with it, and takes them away again.
                        let sub = self.package.subpackages[index].clone();
                        let names: Vec<String> = self
                            .package
                            .files
                            .iter()
                            .filter_map(|path| {
                                let (s, rest) = path.split_once('/')?;
                                (s.eq_ignore_ascii_case(&sub)
                                    && !rest.contains('/')
                                    && is_plugin(rest))
                                .then(|| rest.to_string())
                            })
                            .collect();
                        for name in names {
                            if let Some(espm) = self.espm(&name) {
                                espm.selected = on;
                            }
                        }
                    }
                    None => self.host.note(&format!(
                        "the wizard names a subpackage the archive lacks: {}",
                        text(0)
                    )),
                }
            }
            "selectall" | "deselectall" => {
                let on = name.eq_ignore_ascii_case("selectall");
                self.selected.iter_mut().for_each(|s| *s = on);
                self.espms.iter_mut().for_each(|e| e.selected = on);
            }
            "selectespm" | "deselectespm" => {
                let on = name.eq_ignore_ascii_case("selectespm");
                if let Some(espm) = self.espm(&text(0)) {
                    espm.selected = on;
                }
            }
            "selectallespms" | "deselectallespms" => {
                let on = name.eq_ignore_ascii_case("selectallespms");
                self.espms.iter_mut().for_each(|e| e.selected = on);
            }
            "renameespm" => {
                let to = text(1);
                if let Some(espm) = self.espm(&text(0)) {
                    espm.renamed = Some(to);
                }
            }
            "resetespmname" => {
                if let Some(espm) = self.espm(&text(0)) {
                    espm.renamed = None;
                }
            }
            "resetallespmnames" => self.espms.iter_mut().for_each(|e| e.renamed = None),
            "editini" => {
                if values.len() < 4 {
                    anyhow::bail!("EditINI needs a file, section, key and value");
                }
                self.ini_edits.push(IniEdit {
                    file: text(0),
                    section: text(1),
                    key: text(2),
                    value: text(3),
                });
            }
            "disableiniline" => self.disabled.push((text(0), text(1), text(2))),
            "note" => self.host.note(&text(0)),
            "cancel" => return Ok(Flow::Cancel(text(0))),
            // Versions the wizard requires: the person installing has them,
            // or will be told by the game.
            "requireversions" => {}
            "exec" => anyhow::bail!("Exec runs code built at run time, which is not followed"),
            other => anyhow::bail!("unknown command {other}"),
        }
        Ok(Flow::Normal)
    }

    fn eval(&mut self, expr: &Expr) -> anyhow::Result<Value> {
        match expr {
            Expr::Value(value) => Ok(value.clone()),
            Expr::Var(name) => self
                .vars
                .get(&name.to_ascii_lowercase())
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("{name} is used before it is set")),
            Expr::Unary(op, inner) => {
                let value = self.eval(inner)?;
                match op.as_str() {
                    "!" => Ok(Value::Bool(!truthy(&value))),
                    _ => match value {
                        Value::Int(i) => Ok(Value::Int(-i)),
                        Value::Float(f) => Ok(Value::Float(-f)),
                        other => anyhow::bail!("cannot negate {other:?}"),
                    },
                }
            }
            Expr::Binary(op, left, right) => match op.as_str() {
                // Short-circuiting, as a later `DataFileExists` may ask the
                // person something the first operand already settled.
                "&" | "and" => {
                    let left = self.eval(left)?;
                    if !truthy(&left) {
                        return Ok(Value::Bool(false));
                    }
                    Ok(Value::Bool(truthy(&self.eval(right)?)))
                }
                "|" | "or" => {
                    let left = self.eval(left)?;
                    if truthy(&left) {
                        return Ok(Value::Bool(true));
                    }
                    Ok(Value::Bool(truthy(&self.eval(right)?)))
                }
                _ => {
                    let left = self.eval(left)?;
                    let right = self.eval(right)?;
                    binary(op, left, right)
                }
            },
            Expr::Call(name, args) => {
                let mut values = Vec::new();
                for arg in args {
                    values.push(self.eval(arg)?);
                }
                self.call(name, values)
            }
            Expr::Method(receiver, name, args) => {
                let mut values = vec![self.eval(receiver)?];
                for arg in args {
                    values.push(self.eval(arg)?);
                }
                self.call(name, values)
            }
            Expr::Index(target, index) => {
                let text = as_str(&self.eval(target)?);
                let chars: Vec<char> = text.chars().collect();
                let i = as_int(&self.eval(index)?)?;
                let i = if i < 0 { chars.len() as i64 + i } else { i };
                chars
                    .get(usize::try_from(i).unwrap_or(usize::MAX))
                    .map(|c| Value::Str(c.to_string()))
                    .ok_or_else(|| anyhow::anyhow!("index {i} is past the end of '{text}'"))
            }
        }
    }

    fn call(&mut self, name: &str, args: Vec<Value>) -> anyhow::Result<Value> {
        let text = |i: usize| args.get(i).map(as_str).unwrap_or_default();
        Ok(match name {
            // The person installing has a working game and the versions the
            // list needs; a wizard checking for them gets "the same".
            "compareobversion" | "compareobseversion" | "compareobgeversion"
            | "comparewbversion" | "comparegameversion" | "compareseversion"
            | "comparegeversion" => Value::Int(0),
            "datafileexists" => {
                for arg in &args {
                    if !self.host.data_file_exists(&as_str(arg))? {
                        return Ok(Value::Bool(false));
                    }
                }
                Value::Bool(true)
            }
            "getespmstatus" | "getpluginstatus" => Value::Int(self.host.plugin_status(&text(0))),
            "str" => Value::Str(text(0)),
            "int" => Value::Int(match args.first() {
                Some(Value::Str(s)) => s.trim().parse().unwrap_or(0),
                Some(other) => as_int(other)?,
                None => 0,
            }),
            "float" => Value::Float(match args.first() {
                Some(Value::Int(i)) => *i as f64,
                Some(Value::Float(f)) => *f,
                Some(other) => as_str(other).trim().parse().unwrap_or(0.0),
                None => 0.0,
            }),
            "len" => Value::Int(text(0).chars().count() as i64),
            "lower" => Value::Str(text(0).to_lowercase()),
            "upper" => Value::Str(text(0).to_uppercase()),
            "endswith" => Value::Bool(args[1..].iter().any(|s| text(0).ends_with(&as_str(s)))),
            "startswith" => Value::Bool(args[1..].iter().any(|s| text(0).starts_with(&as_str(s)))),
            "find" => Value::Int(text(0).find(&text(1)).map_or(-1, |i| i as i64)),
            "rfind" => Value::Int(text(0).rfind(&text(1)).map_or(-1, |i| i as i64)),
            "getfilename" => Value::Str(
                text(0)
                    .rsplit(['\\', '/'])
                    .next()
                    .unwrap_or_default()
                    .to_string(),
            ),
            "getfolder" => Value::Str(
                text(0)
                    .rsplit_once(['\\', '/'])
                    .map(|(folder, _)| folder.to_string())
                    .unwrap_or_default(),
            ),
            other => anyhow::bail!("unknown function {other}"),
        })
    }
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Bool(b) => *b,
        Value::Int(i) => *i != 0,
        Value::Float(f) => *f != 0.0,
        Value::Str(s) => !s.is_empty(),
    }
}

fn as_str(value: &Value) -> String {
    match value {
        Value::Str(s) => s.clone(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => if *b { "True" } else { "False" }.to_string(),
    }
}

fn as_int(value: &Value) -> anyhow::Result<i64> {
    match value {
        Value::Int(i) => Ok(*i),
        Value::Bool(b) => Ok(i64::from(*b)),
        Value::Float(f) => Ok(*f as i64),
        Value::Str(s) => s
            .trim()
            .parse()
            .map_err(|_| anyhow::anyhow!("'{s}' is not a number")),
    }
}

fn as_float(value: &Value) -> Option<f64> {
    match value {
        Value::Int(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        Value::Bool(b) => Some(f64::from(u8::from(*b))),
        Value::Str(_) => None,
    }
}

fn binary(op: &str, left: Value, right: Value) -> anyhow::Result<Value> {
    // `:` makes a comparison ignore case.
    let (op, fold) = match op.strip_suffix(':') {
        Some(base) => (base, true),
        None => (op, false),
    };
    if let (Value::Str(a), Value::Str(b)) = (&left, &right) {
        let (a, b) = if fold {
            (a.to_lowercase(), b.to_lowercase())
        } else {
            (a.clone(), b.clone())
        };
        return Ok(match op {
            "+" => Value::Str(a + &b),
            "==" => Value::Bool(a == b),
            "!=" => Value::Bool(a != b),
            ">" => Value::Bool(a > b),
            "<" => Value::Bool(a < b),
            ">=" => Value::Bool(a >= b),
            "<=" => Value::Bool(a <= b),
            "in" => Value::Bool(b.contains(&a)),
            other => anyhow::bail!("'{other}' does not apply to text"),
        });
    }
    if op == "in" {
        let (a, b) = (as_str(&left), as_str(&right));
        return Ok(Value::Bool(if fold {
            b.to_lowercase().contains(&a.to_lowercase())
        } else {
            b.contains(&a)
        }));
    }
    let (Some(a), Some(b)) = (as_float(&left), as_float(&right)) else {
        anyhow::bail!("cannot apply '{op}' to {left:?} and {right:?}");
    };
    let integers = !matches!(left, Value::Float(_)) && !matches!(right, Value::Float(_));
    let number = |x: f64| {
        if integers && x.fract() == 0.0 {
            Value::Int(x as i64)
        } else {
            Value::Float(x)
        }
    };
    Ok(match op {
        "+" => number(a + b),
        "-" => number(a - b),
        "*" => number(a * b),
        "/" if b == 0.0 => anyhow::bail!("division by zero"),
        "/" if integers => Value::Int((a / b).floor() as i64),
        "/" => Value::Float(a / b),
        "%" if b == 0.0 => anyhow::bail!("division by zero"),
        "%" => number(a % b),
        "^" => number(a.powf(b)),
        "==" => Value::Bool(a == b),
        "!=" => Value::Bool(a != b),
        ">" => Value::Bool(a > b),
        "<" => Value::Bool(a < b),
        ">=" => Value::Bool(a >= b),
        "<=" => Value::Bool(a <= b),
        other => anyhow::bail!("unknown operator '{other}'"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Answers from a script: per question, the option names to choose
    /// (`None` takes the defaults), and the data files that exist.
    struct Scripted {
        answers: Vec<(&'static str, Option<Vec<&'static str>>)>,
        files: Vec<&'static str>,
        plugins: Vec<&'static str>,
        asked: Vec<String>,
        notes: Vec<String>,
    }

    impl Scripted {
        fn new(answers: Vec<(&'static str, Option<Vec<&'static str>>)>) -> Self {
            Scripted {
                answers,
                files: Vec::new(),
                plugins: Vec::new(),
                asked: Vec::new(),
                notes: Vec::new(),
            }
        }
    }

    impl Host for Scripted {
        fn choose(&mut self, prompt: &Prompt) -> anyhow::Result<Vec<usize>> {
            self.asked.push(prompt.question.clone());
            let answer = self
                .answers
                .iter()
                .find(|(q, _)| *q == prompt.question)
                .and_then(|(_, a)| a.clone());
            Ok(match answer {
                None => prompt.defaults.clone(),
                Some(names) => names
                    .iter()
                    .map(|n| prompt.options.iter().position(|(o, _)| o == n).unwrap())
                    .collect(),
            })
        }
        fn data_file_exists(&mut self, path: &str) -> anyhow::Result<bool> {
            Ok(self.files.iter().any(|f| f.eq_ignore_ascii_case(path)))
        }
        fn plugin_status(&mut self, name: &str) -> i64 {
            if self.plugins.iter().any(|p| p.eq_ignore_ascii_case(name)) {
                1
            } else {
                -1
            }
        }
        fn note(&mut self, text: &str) {
            self.notes.push(text.to_string());
        }
    }

    fn package() -> Package {
        Package {
            subpackages: vec![
                "00 Core".into(),
                "01 Plain".into(),
                "01 Coloured".into(),
                "02 Tweaks".into(),
            ],
            files: vec![
                "00 Core/Core.esm".into(),
                "00 Core/Core - SI.esp".into(),
                "01 Plain/textures/a.dds".into(),
                "01 Coloured/textures/a.dds".into(),
                "02 Tweaks/Tweak - A.esp".into(),
                "02 Tweaks/Tweak - B.esp".into(),
            ],
        }
    }

    const SCRIPT: &str = r#"
DeSelectAll
SelectSubPackage "00 Core"
If !DataFileExists("DLCShiveringIsles - Meshes.bsa")
    DeSelectEspm "Core - SI.esp"
EndIf
coloured = False
SelectOne "Style", "|Plain", "No colour.", "", "Coloured", "Colour.", ""
    Case "Plain"
        SelectSubPackage "01 Plain"
        Break
    Case "Coloured"
        SelectSubPackage "01 Coloured"
        coloured = True
        Break
EndSelect
If coloured
    SelectMany "Tweaks", "A", "", "", "B", "", ""
        Case "A"
            SelectSubPackage "02 Tweaks"
            For file in "02 Tweaks"
                If file !=: "tweak - a.esp"
                    DeSelectEspm file
                EndIf
            EndFor
            EditINI('ini\\Core.ini', 'set', 'Core.bColour', 1)
            Break
    EndSelect
EndIf
Note "done " + str(len("abc"))
"#;

    fn finished(ending: Ending) -> Outcome {
        match ending {
            Ending::Finished(outcome) => outcome,
            Ending::Cancelled(message) => panic!("cancelled: {message}"),
        }
    }

    #[test]
    fn the_defaults_and_a_missing_dlc() {
        let mut host = Scripted::new(vec![]);
        let outcome = finished(run(SCRIPT, &package(), &mut host).unwrap());
        assert_eq!(outcome.subpackages, ["00 Core", "01 Plain"]);
        assert_eq!(outcome.left_out_plugins, ["Core - SI.esp"]);
        assert!(outcome.ini_edits.is_empty());
        assert_eq!(
            host.asked,
            ["Style"],
            "the tweaks question depends on the style"
        );
        assert_eq!(host.notes, ["done 3"]);
    }

    #[test]
    fn answers_open_a_later_question_a_loop_and_an_ini_edit() {
        let mut host = Scripted::new(vec![
            ("Style", Some(vec!["Coloured"])),
            ("Tweaks", Some(vec!["A"])),
        ]);
        host.files.push("DLCShiveringIsles - Meshes.bsa");
        let outcome = finished(run(SCRIPT, &package(), &mut host).unwrap());
        assert_eq!(outcome.subpackages, ["00 Core", "01 Coloured", "02 Tweaks"]);
        assert_eq!(outcome.left_out_plugins, ["Tweak - B.esp"]);
        assert_eq!(
            outcome.ini_edits,
            [IniEdit {
                file: "ini\\Core.ini".into(),
                section: "set".into(),
                key: "Core.bColour".into(),
                value: "1".into(),
            }]
        );
    }

    #[test]
    fn cancel_while_loops_and_arithmetic() {
        let script = "i = 0\nWhile i < 5\n  i += 1\n  If i == 3\n    Continue\n  EndIf\n  If i >= 4\n    Break\n  EndIf\nEndWhile\nIf i == 4 & 7 / 2 == 3 & 2 ^ 3 == 8\n  Cancel \"stop\"\nEndIf\n";
        let mut host = Scripted::new(vec![]);
        match run(script, &package(), &mut host).unwrap() {
            Ending::Cancelled(message) => assert_eq!(message, "stop"),
            Ending::Finished(_) => panic!("expected a cancel"),
        }
    }

    #[test]
    fn an_endless_loop_is_an_error() {
        let mut host = Scripted::new(vec![]);
        assert!(run("While True\nEndWhile\n", &package(), &mut host).is_err());
    }
}
