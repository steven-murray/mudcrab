//! Reading a BAIN wizard script into statements and expressions.
//!
//! The language is Wrye Bash's: one statement per line (a trailing `\`
//! continues it), `;` starts a comment, and blocks run from a keyword to its
//! `End...`. Keywords are case-insensitive, as Wrye Bash reads them.

/// A value in the script.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i64),
    Float(f64),
    Str(String),
    Bool(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Num(Value),
    Str(String),
    Ident(String),
    /// Operators and punctuation, as written: `==`, `==:`, `(`, `,` ...
    Op(String),
}

#[derive(Debug, Clone)]
pub enum Expr {
    Value(Value),
    Var(String),
    Unary(String, Box<Expr>),
    Binary(String, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    /// `value.method(args)`
    Method(Box<Expr>, String, Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone)]
pub struct Case {
    pub label: String,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub enum ForKind {
    Range {
        from: Expr,
        to: Expr,
        by: Option<Expr>,
    },
    In(Expr),
}

#[derive(Debug, Clone)]
pub enum Stmt {
    /// A command: `SelectSubPackage "x"`, `EditINI(...)`, `Note "..."`.
    Command(String, Vec<Expr>),
    Assign(String, String, Expr),
    Select {
        many: bool,
        args: Vec<Expr>,
        cases: Vec<Case>,
        default: Option<Vec<Stmt>>,
    },
    If {
        branches: Vec<(Expr, Vec<Stmt>)>,
        otherwise: Option<Vec<Stmt>>,
    },
    While(Expr, Vec<Stmt>),
    For(String, ForKind, Vec<Stmt>),
    Break,
    Continue,
    Return,
}

/// One line of the script, with its number for messages.
struct Line {
    number: usize,
    tokens: Vec<Token>,
}

pub fn parse(script: &str) -> anyhow::Result<Vec<Stmt>> {
    let lines = lines(script)?;
    let mut at = 0;
    let body = block(&lines, &mut at, &[])?;
    if let Some(line) = lines.get(at) {
        anyhow::bail!(
            "line {}: unexpected {}",
            line.number,
            describe(&line.tokens)
        );
    }
    Ok(body)
}

/// Statements joined across `\` continuations, comments dropped, tokenised.
fn lines(script: &str) -> anyhow::Result<Vec<Line>> {
    let mut out = Vec::new();
    let mut pending = String::new();
    let mut start = 0;
    for (index, raw) in script.lines().enumerate() {
        if pending.is_empty() {
            start = index + 1;
        }
        let line = strip_comment(raw).trim_end();
        if let Some(head) = line.strip_suffix('\\') {
            pending.push_str(head);
            pending.push(' ');
            continue;
        }
        pending.push_str(line);
        let text = std::mem::take(&mut pending);
        if text.trim().is_empty() {
            continue;
        }
        let tokens = tokenize(&text).map_err(|err| anyhow::anyhow!("line {start}: {err}"))?;
        out.push(Line {
            number: start,
            tokens,
        });
    }
    Ok(out)
}

/// The line up to a `;` that is not inside a string.
fn strip_comment(line: &str) -> &str {
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (index, c) in line.char_indices() {
        match quote {
            Some(q) => {
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == q {
                    quote = None;
                }
            }
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == ';' => return &line[..index],
            None => {}
        }
    }
    line
}

const OPERATORS: [&str; 33] = [
    "==:", "!=:", ">=:", "<=:", "in:", "+=", "-=", "*=", "/=", "^=", "%=", "==", "!=", ">=", "<=",
    ">", "<", "=", "+", "-", "*", "/", "%", "^", "&", "|", "!", "(", ")", "[", "]", ",", ":",
];

fn tokenize(text: &str) -> anyhow::Result<Vec<Token>> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '"' || c == '\'' {
            let mut value = String::new();
            i += 1;
            loop {
                let Some(&d) = chars.get(i) else {
                    anyhow::bail!("a string is not closed");
                };
                i += 1;
                if d == c {
                    break;
                }
                if d == '\\' {
                    let Some(&e) = chars.get(i) else {
                        anyhow::bail!("a string ends in a backslash");
                    };
                    i += 1;
                    value.push(match e {
                        'n' => '\n',
                        't' => '\t',
                        other => other,
                    });
                } else {
                    value.push(d);
                }
            }
            tokens.push(Token::Str(value));
        } else if c.is_ascii_digit()
            || (c == '.' && chars.get(i + 1).is_some_and(|d| d.is_ascii_digit()))
        {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            let value = if text.contains('.') {
                Value::Float(text.parse()?)
            } else {
                Value::Int(text.parse()?)
            };
            tokens.push(Token::Num(value));
        } else if c.is_alphabetic() || c == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            // `in:` is an operator, not `in` and a colon.
            if word.eq_ignore_ascii_case("in") && chars.get(i) == Some(&':') {
                i += 1;
                tokens.push(Token::Op("in:".to_string()));
            } else {
                tokens.push(Token::Ident(word));
            }
        } else if c == '.' {
            i += 1;
            tokens.push(Token::Op(".".to_string()));
        } else {
            let rest: String = chars[i..chars.len().min(i + 3)].iter().collect();
            let Some(op) = OPERATORS.iter().find(|op| rest.starts_with(**op)) else {
                anyhow::bail!("unexpected '{c}'");
            };
            i += op.chars().count();
            tokens.push(Token::Op(op.to_string()));
        }
    }
    Ok(tokens)
}

fn keyword(tokens: &[Token]) -> Option<String> {
    match tokens.first() {
        Some(Token::Ident(word)) => Some(word.to_ascii_lowercase()),
        _ => None,
    }
}

fn describe(tokens: &[Token]) -> String {
    match tokens.first() {
        Some(Token::Ident(word)) => format!("'{word}'"),
        Some(Token::Op(op)) => format!("'{op}'"),
        Some(Token::Str(s)) => format!("\"{s}\""),
        Some(Token::Num(_)) => "a number".to_string(),
        None => "an empty line".to_string(),
    }
}

/// Statements until a line whose keyword is in `until` (left unconsumed).
fn block(lines: &[Line], at: &mut usize, until: &[&str]) -> anyhow::Result<Vec<Stmt>> {
    let mut out = Vec::new();
    while let Some(line) = lines.get(*at) {
        let word = keyword(&line.tokens);
        if word.as_deref().is_some_and(|w| until.contains(&w)) {
            return Ok(out);
        }
        *at += 1;
        let stmt = statement(lines, at, line).map_err(|err| {
            if err.to_string().starts_with("line ") {
                err
            } else {
                anyhow::anyhow!("line {}: {err}", line.number)
            }
        })?;
        out.push(stmt);
    }
    if until.is_empty() {
        Ok(out)
    } else {
        anyhow::bail!("the script ends before {}", until[0])
    }
}

fn expect_end(lines: &[Line], at: &mut usize, end: &str) -> anyhow::Result<()> {
    match lines.get(*at).and_then(|l| keyword(&l.tokens)) {
        Some(word) if word == end => {
            *at += 1;
            Ok(())
        }
        _ => anyhow::bail!("expected {end}"),
    }
}

fn statement(lines: &[Line], at: &mut usize, line: &Line) -> anyhow::Result<Stmt> {
    let tokens = &line.tokens;
    let word = keyword(tokens).unwrap_or_default();
    let rest = &tokens[1.min(tokens.len())..];
    match word.as_str() {
        "if" => {
            let mut branches = vec![(
                expression(rest)?,
                block(lines, at, &["elif", "else", "endif"])?,
            )];
            let mut otherwise = None;
            loop {
                let next = &lines[*at];
                *at += 1;
                match keyword(&next.tokens).as_deref() {
                    Some("elif") => {
                        let cond = expression(&next.tokens[1..])?;
                        branches.push((cond, block(lines, at, &["elif", "else", "endif"])?));
                    }
                    Some("else") => otherwise = Some(block(lines, at, &["endif"])?),
                    _ => break,
                }
            }
            Ok(Stmt::If {
                branches,
                otherwise,
            })
        }
        "while" => {
            let cond = expression(rest)?;
            let body = block(lines, at, &["endwhile"])?;
            expect_end(lines, at, "endwhile")?;
            Ok(Stmt::While(cond, body))
        }
        "for" => {
            let Some(Token::Ident(var)) = rest.first() else {
                anyhow::bail!("For needs a variable");
            };
            let tail = &rest[1..];
            let kind = match tail.first() {
                Some(Token::Ident(w)) if w.eq_ignore_ascii_case("in") => {
                    ForKind::In(expression(&tail[1..])?)
                }
                Some(Token::Ident(w)) if w.eq_ignore_ascii_case("from") => {
                    let words: Vec<usize> = tail
                        .iter()
                        .enumerate()
                        .filter(|(_, t)| {
                            matches!(t, Token::Ident(w) if w.eq_ignore_ascii_case("to") || w.eq_ignore_ascii_case("by"))
                        })
                        .map(|(i, _)| i)
                        .collect();
                    let to = *words
                        .first()
                        .ok_or_else(|| anyhow::anyhow!("For ... from needs 'to'"))?;
                    let by = words.get(1).copied();
                    ForKind::Range {
                        from: expression(&tail[1..to])?,
                        to: expression(&tail[to + 1..by.unwrap_or(tail.len())])?,
                        by: by.map(|b| expression(&tail[b + 1..])).transpose()?,
                    }
                }
                _ => anyhow::bail!("For needs 'in' or 'from'"),
            };
            let body = block(lines, at, &["endfor"])?;
            expect_end(lines, at, "endfor")?;
            Ok(Stmt::For(var.clone(), kind, body))
        }
        "selectone" | "selectmany" => {
            let args = arguments(rest)?;
            let mut cases = Vec::new();
            let mut default = None;
            loop {
                let Some(next) = lines.get(*at) else {
                    anyhow::bail!("the script ends before EndSelect");
                };
                *at += 1;
                match keyword(&next.tokens).as_deref() {
                    Some("case") => {
                        let label = match expression(&next.tokens[1..])? {
                            Expr::Value(Value::Str(s)) => s,
                            _ => anyhow::bail!("line {}: Case needs a string", next.number),
                        };
                        let body = block(lines, at, &["case", "default", "endselect"])?;
                        cases.push(Case { label, body });
                    }
                    Some("default") => {
                        default = Some(block(lines, at, &["case", "default", "endselect"])?);
                    }
                    Some("endselect") => break,
                    _ => anyhow::bail!(
                        "line {}: expected Case, Default or EndSelect, found {}",
                        next.number,
                        describe(&next.tokens)
                    ),
                }
            }
            Ok(Stmt::Select {
                many: word == "selectmany",
                args,
                cases,
                default,
            })
        }
        "break" => Ok(Stmt::Break),
        "continue" => Ok(Stmt::Continue),
        "return" => Ok(Stmt::Return),
        _ => {
            // `name = value`, `name += value` ...
            if let (Some(Token::Ident(name)), Some(Token::Op(op))) = (tokens.first(), tokens.get(1))
                && matches!(op.as_str(), "=" | "+=" | "-=" | "*=" | "/=" | "^=" | "%=")
            {
                return Ok(Stmt::Assign(
                    name.clone(),
                    op.clone(),
                    expression(&tokens[2..])?,
                ));
            }
            match tokens.first() {
                Some(Token::Ident(name)) => Ok(Stmt::Command(name.clone(), arguments(rest)?)),
                _ => anyhow::bail!("unexpected {}", describe(tokens)),
            }
        }
    }
}

/// A command's arguments: comma-separated, optionally in one pair of
/// parentheses (`EditINI(a, b)` and `Note "x"` alike).
fn arguments(tokens: &[Token]) -> anyhow::Result<Vec<Expr>> {
    let inner = match (tokens.first(), tokens.last()) {
        (Some(Token::Op(open)), Some(Token::Op(close)))
            if open == "(" && close == ")" && closing(tokens, 0) == Some(tokens.len() - 1) =>
        {
            &tokens[1..tokens.len() - 1]
        }
        _ => tokens,
    };
    split_commas(inner).into_iter().map(expression).collect()
}

/// The index of the bracket closing the one at `open`.
fn closing(tokens: &[Token], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    for (i, token) in tokens.iter().enumerate().skip(open) {
        match token {
            Token::Op(op) if op == "(" || op == "[" => depth += 1,
            Token::Op(op) if op == ")" || op == "]" => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn split_commas(tokens: &[Token]) -> Vec<&[Token]> {
    if tokens.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, token) in tokens.iter().enumerate() {
        match token {
            Token::Op(op) if op == "(" || op == "[" => depth += 1,
            Token::Op(op) if op == ")" || op == "]" => depth -= 1,
            Token::Op(op) if op == "," && depth == 0 => {
                out.push(&tokens[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(&tokens[start..]);
    out
}

pub fn expression(tokens: &[Token]) -> anyhow::Result<Expr> {
    let mut parser = Parser { tokens, at: 0 };
    let expr = parser.binary(0)?;
    if parser.at != tokens.len() {
        anyhow::bail!(
            "unexpected {} in an expression",
            describe(&tokens[parser.at..])
        );
    }
    Ok(expr)
}

struct Parser<'a> {
    tokens: &'a [Token],
    at: usize,
}

/// Binding strength of a binary operator; `None` for anything else.
fn precedence(token: &Token) -> Option<(u8, String)> {
    let op = match token {
        Token::Op(op) => op.to_ascii_lowercase(),
        Token::Ident(word) => word.to_ascii_lowercase(),
        _ => return None,
    };
    let level = match op.as_str() {
        "|" | "or" => 1,
        "&" | "and" => 2,
        "in" | "in:" => 3,
        "==" | "!=" | ">=" | "<=" | ">" | "<" | "==:" | "!=:" | ">=:" | "<=:" => 4,
        "+" | "-" => 5,
        "*" | "/" | "%" => 6,
        "^" => 7,
        _ => return None,
    };
    Some((level, op))
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn eat(&mut self, op: &str) -> bool {
        if matches!(self.peek(), Some(Token::Op(o)) if o == op) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn binary(&mut self, min: u8) -> anyhow::Result<Expr> {
        let mut left = self.unary()?;
        while let Some((level, op)) = self.peek().and_then(precedence) {
            if level <= min {
                break;
            }
            self.at += 1;
            // `^` is right-associative; the rest bind left.
            let right = self.binary(if op == "^" { level - 1 } else { level })?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn unary(&mut self) -> anyhow::Result<Expr> {
        if self.eat("!") || self.eat("-") {
            let op = match &self.tokens[self.at - 1] {
                Token::Op(op) => op.clone(),
                _ => unreachable!(),
            };
            return Ok(Expr::Unary(op, Box::new(self.unary()?)));
        }
        if matches!(self.peek(), Some(Token::Ident(w)) if w.eq_ignore_ascii_case("not")) {
            self.at += 1;
            return Ok(Expr::Unary("!".to_string(), Box::new(self.unary()?)));
        }
        let mut expr = self.primary()?;
        loop {
            if self.eat("[") {
                let index = self.binary(0)?;
                if !self.eat("]") {
                    anyhow::bail!("expected ']'");
                }
                expr = Expr::Index(Box::new(expr), Box::new(index));
            } else if self.eat(".") {
                let Some(Token::Ident(name)) = self.peek().cloned() else {
                    anyhow::bail!("expected a method name after '.'");
                };
                self.at += 1;
                let args = if self.eat("(") {
                    self.call_args()?
                } else {
                    Vec::new()
                };
                expr = Expr::Method(Box::new(expr), name.to_ascii_lowercase(), args);
            } else {
                return Ok(expr);
            }
        }
    }

    fn call_args(&mut self) -> anyhow::Result<Vec<Expr>> {
        let mut args = Vec::new();
        if self.eat(")") {
            return Ok(args);
        }
        loop {
            args.push(self.binary(0)?);
            if self.eat(")") {
                return Ok(args);
            }
            if !self.eat(",") {
                anyhow::bail!("expected ',' or ')'");
            }
        }
    }

    fn primary(&mut self) -> anyhow::Result<Expr> {
        let Some(token) = self.peek().cloned() else {
            anyhow::bail!("an expression is missing");
        };
        self.at += 1;
        match token {
            Token::Num(value) => Ok(Expr::Value(value)),
            Token::Str(s) => Ok(Expr::Value(Value::Str(s))),
            Token::Ident(word) => match word.to_ascii_lowercase().as_str() {
                "true" => Ok(Expr::Value(Value::Bool(true))),
                "false" => Ok(Expr::Value(Value::Bool(false))),
                _ if self.eat("(") => Ok(Expr::Call(word.to_ascii_lowercase(), self.call_args()?)),
                _ => Ok(Expr::Var(word)),
            },
            Token::Op(op) if op == "(" => {
                let inner = self.binary(0)?;
                if !self.eat(")") {
                    anyhow::bail!("expected ')'");
                }
                Ok(inner)
            }
            Token::Op(op) => anyhow::bail!("unexpected '{op}'"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_blocks_commands_and_expressions() {
        let script = "; header\n\
            x = 1 ; trailing\n\
            If x == 1 & !DataFileExists(\"a.esp\")\n\
              SelectSubPackage \"00 Core\"\n\
            Elif x > 2\n\
              Note 'it\\'s'\n\
            Else\n\
              EditINI('ini\\\\a.ini', 'set', 'k', '1')\n\
            EndIf\n\
            SelectOne \"Q\", \\\n\
              \"|A\", \"first\", \"\", \\\n\
              \"B\", \"second\", \"\"\n\
              Case \"A\"\n\
                Break\n\
              Default\n\
            EndSelect\n\
            For f in \"01 Tweaks\"\n\
              If f.lower().endswith('.esp')\n\
                DeSelectEspm f\n\
              EndIf\n\
            EndFor\n";
        let stmts = parse(script).unwrap();
        assert_eq!(stmts.len(), 4);
        let Stmt::If {
            branches,
            otherwise,
        } = &stmts[1]
        else {
            panic!("{stmts:?}")
        };
        assert_eq!(branches.len(), 2);
        let Some(otherwise) = otherwise else { panic!() };
        let Stmt::Command(name, args) = &otherwise[0] else {
            panic!()
        };
        assert_eq!(name, "EditINI");
        assert_eq!(args.len(), 4);
        let Stmt::Select {
            cases,
            default,
            args,
            ..
        } = &stmts[2]
        else {
            panic!()
        };
        assert_eq!(args.len(), 7);
        assert_eq!(cases[0].label, "A");
        assert!(default.is_some());
        assert!(
            matches!(&stmts[3], Stmt::For(var, ForKind::In(_), body) if var == "f" && body.len() == 1)
        );
    }

    #[test]
    fn says_where_a_script_goes_wrong() {
        let err = parse("If x\nSelectSubPackage 'a'\n")
            .unwrap_err()
            .to_string();
        assert!(err.contains("EndIf") || err.contains("elif"), "{err}");
        let err = parse("x = (1\n").unwrap_err().to_string();
        assert!(err.starts_with("line 1"), "{err}");
    }
}
