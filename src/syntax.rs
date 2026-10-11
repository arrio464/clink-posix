//! Interactive grammar. Parsing and static validation have no external side effects.
//! Expansion results never go back through this parser.
pub type Result<T> = std::result::Result<T, String>;
pub const MAX_INPUT: usize = 65536;
const MAX_DEPTH: usize = 16;

#[derive(Clone, Debug)]
pub enum Part {
    Lit(String, bool),
    Var(String, bool),
    Parameter(String, String, Vec<Word>, bool),
    Length(String, bool),
    Arithmetic(crate::arithmetic::Expr, bool),
    Capture(Box<Node>, bool),
    Process(Box<Node>),
}
#[derive(Clone, Debug)]
pub struct Word(pub Vec<Part>, pub String);
impl Word {
    pub fn literal(&self) -> Option<String> {
        self.0
            .iter()
            .map(|p| match p {
                Part::Lit(s, _) => Some(s.as_str()),
                _ => None,
            })
            .collect()
    }
    pub fn assignment(&self) -> bool {
        matches!(self.0.first(), Some(Part::Lit(s, false)) if s.split_once('=').is_some_and(|(k,_)| identifier(k)))
    }
}
pub fn identifier(s: &str) -> bool {
    let mut c = s.chars();
    c.next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && c.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
#[derive(Clone, Debug)]
pub struct Redirect(pub String, pub Option<Word>);
#[derive(Clone, Debug)]
pub enum Node {
    List(Vec<Node>),
    Conditional(Vec<(String, Node)>),
    Pipeline(Vec<Node>, Vec<bool>),
    Command(Vec<Word>, Vec<Redirect>),
    Group(Box<Node>, Vec<Redirect>),
}
#[derive(Clone, Debug)]
enum Token {
    Word(Word),
    Op(String),
}
const OPS: &[&str] = &[
    "2>&1", "1>&2", "2>>", "1>>", "&>>", "<<<", "&&", "||", "|&", ">>", "2>", "1>", "&>", "<<",
    ";", "|", "&", "<", ">", "(", ")",
];

struct Lexer {
    chars: Vec<char>,
    i: usize,
    depth: usize,
}
impl Lexer {
    fn starts(&self, s: &str) -> bool {
        self.chars[self.i..].starts_with(&s.chars().collect::<Vec<_>>())
    }
    fn fail<T>(&self, message: &str) -> Result<T> {
        Err(format!("{message} (character {})", self.i + 1))
    }
    fn substitution_body(&mut self) -> Result<String> {
        // Cursor follows the opening '('; recursively skip nested substitutions
        // even inside double quotes, preserving the outer quote context.
        let start = self.i;
        let mut quote = None;
        while self.i < self.chars.len() {
            let c = self.chars[self.i];
            if quote == Some('\'') {
                self.i += 1;
                if c == '\'' {
                    quote = None;
                }
            } else if c == '\\' && self.i + 1 < self.chars.len() {
                self.i += 2;
            } else if self.starts("$(") || (quote.is_none() && self.starts("<(")) {
                self.i += 2;
                // Scan nested source; its AST is built by parse_depth below.
                let old_depth = self.depth;
                self.depth += 1;
                if self.depth > MAX_DEPTH {
                    return self.fail("Substitution nesting limit");
                }
                self.substitution_body()?;
                self.depth = old_depth;
            } else if c == '"' {
                quote = if quote.is_some() { None } else { Some('"') };
                self.i += 1;
            } else if c == '\'' && quote.is_none() {
                quote = Some('\'');
                self.i += 1;
            } else if c == ')' && quote.is_none() {
                let body: String = self.chars[start..self.i].iter().collect();
                self.i += 1;
                return Ok(body);
            } else if c == '(' && quote.is_none() {
                self.i += 1;
                self.depth += 1;
                if self.depth > MAX_DEPTH {
                    return self.fail("Group nesting limit");
                }
                self.substitution_body()?;
                self.depth -= 1;
            } else {
                self.i += 1;
            }
        }
        self.fail("Unclosed substitution")
    }
    fn lit(parts: &mut Vec<Part>, c: &str, quoted: bool) {
        if let Some(Part::Lit(s, q)) = parts.last_mut()
            && *q == quoted
        {
            s.push_str(c);
            return;
        }
        parts.push(Part::Lit(c.to_owned(), quoted));
    }
    fn parameter_body(&mut self) -> Result<String> {
        let start = self.i;
        let mut quote = None;
        let mut level = 1;
        while self.i < self.chars.len() {
            let c = self.chars[self.i];
            if c == '\\' && self.i + 1 < self.chars.len() && quote != Some('\'') {
                self.i += 2;
                continue;
            }
            if quote == Some('\'') {
                if c == '\'' {
                    quote = None;
                }
            } else if c == '"' {
                quote = if quote.is_some() { None } else { Some('"') };
            } else if c == '\'' && quote.is_none() {
                quote = Some('\'');
            } else if self.starts("$(") {
                self.i += 2;
                self.depth += 1;
                if self.depth > MAX_DEPTH {
                    return self.fail("Parameter nesting limit");
                }
                self.substitution_body()?;
                self.depth -= 1;
                continue;
            } else if c == '{' && quote.is_none() {
                level += 1;
            } else if c == '}' && quote.is_none() {
                level -= 1;
                if level == 0 {
                    let body = self.chars[start..self.i].iter().collect();
                    self.i += 1;
                    return Ok(body);
                }
            }
            self.i += 1;
        }
        self.fail("Unclosed parameter")
    }
    fn word(&mut self) -> Result<Word> {
        let start = self.i;
        let mut parts = Vec::new();
        let mut quote = None;
        while self.i < self.chars.len() {
            let c = self.chars[self.i];
            if quote == Some('\'') {
                if c == '\'' {
                    quote = None;
                } else {
                    Self::lit(&mut parts, &c.to_string(), true);
                }
                self.i += 1;
                continue;
            }
            if quote.is_none() && (c == '\'' || c == '"') {
                quote = Some(c);
                Self::lit(&mut parts, "", true);
                self.i += 1;
                continue;
            }
            if quote == Some('"') && c == '"' {
                quote = None;
                self.i += 1;
                continue;
            }
            if c == '\\' && self.i + 1 < self.chars.len() {
                let next = self.chars[self.i + 1];
                // Deliberate Windows path rule: \n and \t stay literal.
                let escapes = if quote.is_some() {
                    "$`\"\n"
                } else {
                    " \t\n\"';&|<>()$`*?[]{}#~"
                };
                if escapes.contains(next) {
                    if next != '\n' {
                        Self::lit(&mut parts, &next.to_string(), true);
                    }
                    self.i += 2;
                    continue;
                }
            }
            if self.starts("$((") {
                self.i += 2;
                let body = self.substitution_body()?;
                let body = body
                    .strip_prefix('(')
                    .and_then(|s| s.strip_suffix(')'))
                    .ok_or("Invalid arithmetic expansion")?;
                parts.push(Part::Arithmetic(
                    crate::arithmetic::parse(body)?,
                    quote.is_some(),
                ));
                continue;
            }
            if self.starts("$(") || (quote.is_none() && self.starts("<(")) {
                self.i += 2;
                let body = self.substitution_body()?;
                let node = Box::new(parse_depth(&body, self.depth + 1)?);
                parts.push(if c == '$' {
                    Part::Capture(node, quote.is_some())
                } else {
                    Part::Process(node)
                });
                continue;
            }
            if c == '$' {
                self.i += 1;
                let mut name = String::new();
                if self.i < self.chars.len() && self.chars[self.i] == '{' {
                    self.i += 1;
                    let body = self.parameter_body()?;
                    parts.push(parameter(&body, quote.is_some(), self.depth + 1)?);
                    continue;
                } else if self.i < self.chars.len() && self.chars[self.i] == '?' {
                    name.push('?');
                    self.i += 1;
                } else {
                    if self.i == self.chars.len()
                        || !(self.chars[self.i].is_ascii_alphabetic() || self.chars[self.i] == '_')
                    {
                        if self.i < self.chars.len()
                            && "'@*#!0123456789".contains(self.chars[self.i])
                        {
                            return self.fail("Unsupported dollar expansion");
                        }
                        Self::lit(&mut parts, "$", quote.is_some());
                        continue;
                    }
                    while self.i < self.chars.len()
                        && (self.chars[self.i].is_ascii_alphanumeric() || self.chars[self.i] == '_')
                    {
                        name.push(self.chars[self.i]);
                        self.i += 1;
                    }
                }
                if name != "?" && !identifier(&name) {
                    return self.fail(
                        "Only $NAME, ${NAME} and $? parameters are supported; quote literal '$'",
                    );
                }
                parts.push(Part::Var(name, quote.is_some()));
                continue;
            }
            if c == '`' {
                return self.fail("Use $(...) instead of backticks");
            }
            if quote.is_none() && (c.is_whitespace() || ";&|<>()".contains(c)) {
                break;
            }
            Self::lit(&mut parts, &c.to_string(), quote.is_some());
            self.i += 1;
        }
        if quote.is_some() {
            return self.fail("Unclosed quote");
        }
        if parts.is_empty() {
            return self.fail("Expected a word");
        }
        Ok(Word(parts, self.chars[start..self.i].iter().collect()))
    }
    fn tokens(mut self) -> Result<Vec<Token>> {
        let mut out = Vec::new();
        while self.i < self.chars.len() {
            let c = self.chars[self.i];
            if c == '\n' {
                if !out.is_empty()
                    && !matches!(out.last(), Some(Token::Op(op)) if [";", "&&", "||", "|", "|&", "("].contains(&op.as_str()))
                {
                    out.push(Token::Op(";".into()));
                }
                self.i += 1;
                continue;
            }
            if c == ' ' || c == '\t' || c == '\r' {
                self.i += 1;
                continue;
            }
            if c == '#' {
                while self.i < self.chars.len() && self.chars[self.i] != '\n' {
                    self.i += 1;
                }
                continue;
            }
            if !self.starts("<(")
                && let Some(op) = OPS.iter().find(|op| self.starts(op))
            {
                self.i += op.len();
                out.push(Token::Op((*op).to_owned()));
                continue;
            }
            out.push(Token::Word(self.word()?));
        }
        Ok(out)
    }
}
struct Parser {
    tokens: Vec<Token>,
    i: usize,
    depth: usize,
}
impl Parser {
    fn op(&self) -> &str {
        match self.tokens.get(self.i) {
            Some(Token::Op(s)) => s,
            _ => "",
        }
    }
    fn sequence(&mut self) -> Result<Node> {
        let mut nodes = Vec::new();
        while self.i < self.tokens.len() && self.op() != ")" {
            nodes.push(self.conditional()?);
            if self.op() != ";" {
                break;
            }
            self.i += 1;
        }
        Ok(Node::List(nodes))
    }
    fn conditional(&mut self) -> Result<Node> {
        let mut nodes = vec![(String::new(), self.pipeline()?)];
        while self.op() == "&&" || self.op() == "||" {
            let op = self.op().to_owned();
            self.i += 1;
            nodes.push((op, self.pipeline()?));
        }
        Ok(Node::Conditional(nodes))
    }
    fn pipeline(&mut self) -> Result<Node> {
        let mut nodes = vec![self.command()?];
        let mut links = Vec::new();
        while self.op() == "|" || self.op() == "|&" {
            links.push(self.op() == "|&");
            self.i += 1;
            if nodes.len() >= 32 {
                return Err("Pipeline exceeds 32 stages".into());
            }
            nodes.push(self.command()?);
        }
        Ok(Node::Pipeline(nodes, links))
    }
    fn command(&mut self) -> Result<Node> {
        let mut words = Vec::new();
        let mut redirs = Vec::new();
        let group = if self.op() == "(" {
            self.depth += 1;
            if self.depth > MAX_DEPTH {
                return Err("Group nesting limit".into());
            }
            self.i += 1;
            let body = self.sequence()?;
            if self.op() != ")" {
                return Err("Unclosed command group".into());
            }
            self.i += 1;
            self.depth -= 1;
            Some(Box::new(body))
        } else {
            None
        };
        while self.i < self.tokens.len() {
            if [";", "&&", "||", "|", "|&", ")"].contains(&self.op()) {
                break;
            }
            match self.tokens[self.i].clone() {
                Token::Word(w) => {
                    if group.is_some() {
                        return Err("Unexpected word after group".into());
                    }
                    words.push(w);
                    self.i += 1;
                }
                Token::Op(op) => {
                    if ![
                        "<", ">", ">>", "1>", "1>>", "2>", "2>>", "&>", "&>>", "2>&1", "1>&2",
                        "<<<",
                    ]
                    .contains(&op.as_str())
                    {
                        return Err(format!("Unsupported operator: {op}"));
                    }
                    self.i += 1;
                    let target = if op == "2>&1" || op == "1>&2" {
                        None
                    } else {
                        match self.tokens.get(self.i) {
                            Some(Token::Word(w)) => {
                                let w = w.clone();
                                self.i += 1;
                                Some(w)
                            }
                            _ => return Err(format!("Missing target after {op}")),
                        }
                    };
                    redirs.push(Redirect(op, target));
                }
            }
        }
        if let Some(body) = group {
            return Ok(Node::Group(body, redirs));
        }
        if words.is_empty() && redirs.is_empty() {
            return Err("Expected a command".into());
        }
        if let Some(w) = words.iter().find(|w| !w.assignment())
            && let Some(name) = w.literal()
        {
            validate_command(&name)?;
        }
        Ok(Node::Command(words, redirs))
    }
}
pub fn validate_command(name: &str) -> Result<()> {
    if [
        "source", ".", "eval", "exec", "for", "while", "until", "if", "then", "else", "fi", "do",
        "done", "case", "esac", "function", "select", "jobs", "fg", "bg", "wait", "set",
        "setlocal", "endlocal", "alias", "unalias", "return", "exit", "break", "continue", "!",
    ]
    .contains(&name)
    {
        return Err(format!(
            "{name} is outside the interactive subset; use an explicit shell"
        ));
    }
    Ok(())
}
pub fn parse(text: &str) -> Result<Node> {
    parse_depth(text, 0)
}
fn parameter(body: &str, quoted: bool, depth: usize) -> Result<Part> {
    if depth > MAX_DEPTH {
        return Err("Parameter nesting limit".into());
    }
    if body == "?" || identifier(body) {
        return Ok(Part::Var(body.into(), quoted));
    }
    if let Some(name) = body.strip_prefix('#')
        && identifier(name)
    {
        return Ok(Part::Length(name.into(), quoted));
    }
    let end = body
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .ok_or("Invalid parameter")?;
    let name = &body[..end];
    if !identifier(name) {
        return Err("Invalid parameter name".into());
    }
    let tail = &body[end..];
    let op = [":-", ":+", "-", "+"]
        .into_iter()
        .find(|op| tail.starts_with(op))
        .ok_or("Unsupported parameter operator")?;
    let tokens = Lexer {
        chars: tail[op.len()..].chars().collect(),
        i: 0,
        depth,
    }
    .tokens()?;
    let words = tokens
        .into_iter()
        .map(|t| match t {
            Token::Word(w) => Ok(w),
            _ => Err("Operators are not supported inside parameter defaults".into()),
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(Part::Parameter(name.into(), op.into(), words, quoted))
}
fn parse_depth(text: &str, depth: usize) -> Result<Node> {
    if text.len() > MAX_INPUT || text.contains('\0') {
        return Err("Input exceeds 64 KiB or contains NUL".into());
    }
    if depth > MAX_DEPTH {
        return Err("Substitution nesting exceeds 16".into());
    }
    let tokens = Lexer {
        chars: text.chars().collect(),
        i: 0,
        depth,
    }
    .tokens()?;
    let mut parser = Parser {
        tokens,
        i: 0,
        depth,
    };
    let node = parser.sequence()?;
    if parser.i != parser.tokens.len() {
        return Err(format!("Unexpected operator: {}", parser.op()));
    }
    Ok(node)
}
