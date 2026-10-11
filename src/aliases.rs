//! DOSKEY compatibility is source-to-AST expansion before execution. Runtime
//! values never enter this stage. Only simple command aliases are accepted.
use crate::syntax::{self, Node, Part, Result, Word};
use std::collections::BTreeMap;
pub type Aliases = BTreeMap<String, String>;
pub fn unhex(text: &str) -> Result<String> {
    if !text.len().is_multiple_of(2) || !text.is_ascii() {
        return Err("Invalid hex transport".into());
    }
    let bytes = (0..text.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&text[i..i + 2], 16).map_err(|_| "Invalid hex transport".to_owned())
        })
        .collect::<Result<Vec<_>>>()?;
    let value = String::from_utf8(bytes).map_err(|_| "Transport must contain UTF-8")?;
    if value.contains('\0') {
        return Err("NUL in transport".into());
    }
    Ok(value)
}
pub fn read(path: &std::path::Path) -> Result<Aliases> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    if text.len() > 1024 * 1024 {
        return Err("Aliases exceed 1 MiB".into());
    }
    text.lines()
        .map(|line| {
            let (key, value) = line.split_once('\t').ok_or("Invalid alias record")?;
            Ok((unhex(key)?.to_lowercase(), unhex(value)?))
        })
        .collect()
}
fn word(w: &mut Word, aliases: &Aliases, depth: usize, budget: &mut usize) -> Result<()> {
    for p in &mut w.0 {
        match p {
            Part::Capture(n, _) | Part::Process(n) => visit(n, aliases, depth, budget)?,
            Part::Parameter(_, _, words, _) => {
                for w in words {
                    word(w, aliases, depth, budget)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}
fn template(source: &str, args: &[Word]) -> Result<String> {
    let mut out = String::new();
    let mut chars = source.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '$' {
            out.push(c);
            continue;
        }
        match chars.peek().copied().map(|c| c.to_ascii_uppercase()) {
            Some('*') => {
                chars.next();
                out.push_str(
                    &args
                        .iter()
                        .map(|w| w.1.as_str())
                        .collect::<Vec<_>>()
                        .join(" "),
                );
            }
            Some(n @ '1'..='9') => {
                chars.next();
                if let Some(w) = args.get(n as usize - '1' as usize) {
                    out.push_str(&w.1);
                }
            }
            Some('T' | 'G' | 'L' | 'B' | '$') => {
                out.push(match chars.next().unwrap().to_ascii_uppercase() {
                    'T' => ';',
                    'G' => '>',
                    'L' => '<',
                    'B' => '|',
                    _ => '$',
                });
            }
            _ => out.push('$'),
        }
        if out.len() > syntax::MAX_INPUT {
            return Err("Expanded alias exceeds 64 KiB".into());
        }
    }
    Ok(out)
}
fn simple(node: Node) -> Result<Node> {
    match node {
        Node::List(mut nodes) if nodes.len() == 1 => simple(nodes.remove(0)),
        Node::Conditional(mut nodes) if nodes.len() == 1 => simple(nodes.remove(0).1),
        Node::Pipeline(mut nodes, _) if nodes.len() == 1 => simple(nodes.remove(0)),
        n @ Node::Command(_, _) => Ok(n),
        _ => Err("Compound DOSKEY alias: use px cmd".into()),
    }
}
fn visit(node: &mut Node, aliases: &Aliases, depth: usize, budget: &mut usize) -> Result<()> {
    match node {
        Node::List(nodes) | Node::Pipeline(nodes, _) => {
            for n in nodes {
                visit(n, aliases, depth, budget)?;
            }
        }
        Node::Conditional(nodes) => {
            for (_, n) in nodes {
                visit(n, aliases, depth, budget)?;
            }
        }
        Node::Group(n, rs) => {
            visit(n, aliases, depth, budget)?;
            for r in rs {
                if let Some(w) = &mut r.1 {
                    word(w, aliases, depth, budget)?;
                }
            }
        }
        Node::Command(words, rs) => {
            let index = words.iter().position(|w| !w.assignment());
            if let Some(i) = index
                && let Some(name) = words[i].literal()
                && let Some(source) = aliases.get(&name.to_lowercase())
            {
                if depth >= 12 || *budget >= 256 {
                    return Err("Alias recursion/expansion limit".into());
                }
                *budget += 1;
                let mut expanded = simple(syntax::parse(&template(source, &words[i + 1..])?)?)?;
                visit(&mut expanded, aliases, depth + 1, budget)?;
                if let Node::Command(mut new_words, mut new_rs) = expanded {
                    let mut prefix = words[..i].to_vec();
                    prefix.append(&mut new_words);
                    new_rs.append(rs);
                    *words = prefix;
                    *rs = new_rs;
                }
            }
            for w in words {
                word(w, aliases, depth, budget)?;
            }
            for r in rs {
                if let Some(w) = &mut r.1 {
                    word(w, aliases, depth, budget)?;
                }
            }
        }
    }
    Ok(())
}
pub fn expand(node: &mut Node, aliases: &Aliases) -> Result<()> {
    visit(node, aliases, 0, &mut 0)
}
