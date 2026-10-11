//! Small integer expression grammar. No evaluation of shell source or recursion
//! through environment values; checked i64 math makes overflow deterministic.
use crate::syntax::Result;

#[derive(Clone, Debug)]
pub enum Expr {
    Number(i64),
    Name(String),
    Unary(char, Box<Expr>),
    Binary(char, Box<Expr>, Box<Expr>),
}
impl Expr {
    pub fn evaluate(&self, get: &impl Fn(&str) -> String) -> Result<i64> {
        match self {
            Self::Number(n) => Ok(*n),
            Self::Name(n) => {
                let value = get(n);
                if value.is_empty() {
                    Ok(0)
                } else {
                    value
                        .parse()
                        .map_err(|_| format!("Arithmetic variable {n} is not an integer"))
                }
            }
            Self::Unary(op, value) => {
                let n = value.evaluate(get)?;
                if *op == '-' {
                    n.checked_neg().ok_or("Arithmetic overflow".into())
                } else {
                    Ok(n)
                }
            }
            Self::Binary(op, a, b) => {
                let a = a.evaluate(get)?;
                let b = b.evaluate(get)?;
                match op {
                    '+' => a.checked_add(b),
                    '-' => a.checked_sub(b),
                    '*' => a.checked_mul(b),
                    '/' => a.checked_div(b),
                    '%' => a.checked_rem(b),
                    _ => None,
                }
                .ok_or("Arithmetic overflow or division by zero".into())
            }
        }
    }
}
pub fn parse(text: &str) -> Result<Expr> {
    struct Parser {
        chars: Vec<char>,
        i: usize,
        nodes: usize,
    }
    impl Parser {
        fn peek(&mut self) -> Option<char> {
            while self.chars.get(self.i).is_some_and(|c| c.is_whitespace()) {
                self.i += 1;
            }
            self.chars.get(self.i).copied()
        }
        fn expr(&mut self, min: u8, depth: usize) -> Result<Expr> {
            self.nodes += 1;
            if depth > 32 || self.nodes > 256 {
                return Err("Arithmetic expression limit".into());
            }
            let c = self.peek().ok_or("Missing arithmetic operand")?;
            let mut left = if c == '+' || c == '-' {
                self.i += 1;
                Expr::Unary(c, Box::new(self.expr(3, depth + 1)?))
            } else if c == '(' {
                self.i += 1;
                let inner = self.expr(0, depth + 1)?;
                if self.peek() != Some(')') {
                    return Err("Unclosed arithmetic group".into());
                }
                self.i += 1;
                inner
            } else if c.is_ascii_digit() {
                let start = self.i;
                while self.chars.get(self.i).is_some_and(|c| c.is_ascii_digit()) {
                    self.i += 1;
                }
                Expr::Number(
                    self.chars[start..self.i]
                        .iter()
                        .collect::<String>()
                        .parse()
                        .map_err(|_| "Integer out of range")?,
                )
            } else if c.is_ascii_alphabetic() || c == '_' {
                let start = self.i;
                while self
                    .chars
                    .get(self.i)
                    .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_')
                {
                    self.i += 1;
                }
                Expr::Name(self.chars[start..self.i].iter().collect())
            } else {
                return Err("Unsupported arithmetic operand".into());
            };
            while let Some(op) = self.peek() {
                let precedence = match op {
                    '+' | '-' => 1,
                    '*' | '/' | '%' => 2,
                    _ => break,
                };
                if precedence < min {
                    break;
                }
                self.i += 1;
                // Accept the previous executor's // spelling, with truncation toward zero.
                if op == '/' && self.chars.get(self.i) == Some(&'/') {
                    self.i += 1;
                }
                let right = self.expr(precedence + 1, depth + 1)?;
                left = Expr::Binary(op, Box::new(left), Box::new(right));
            }
            Ok(left)
        }
    }
    let mut p = Parser {
        chars: text.chars().collect(),
        i: 0,
        nodes: 0,
    };
    let expr = p.expr(0, 0)?;
    if p.peek().is_some() {
        return Err("Unsupported arithmetic operator".into());
    }
    Ok(expr)
}
