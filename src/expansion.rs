//! Bounded literal brace expansion and quote-aware pathname expansion.
use crate::syntax::{Part, Result, Word};
use std::{fs, path::Path};
pub const MAX_WORDS: usize = 4096;
const MAX_BRACES: usize = 1000;
type Pattern = Vec<(char, bool)>; // bool: protected by quotes/escape/tilde.

pub fn variants(word: &Word, enabled: bool) -> Result<Vec<Vec<Part>>> {
    let mut out = vec![Vec::new()];
    for part in &word.0 {
        let choices = match part {
            Part::Lit(s, false) if enabled => braces(s, 0)?
                .into_iter()
                .map(|s| Part::Lit(s, false))
                .collect(),
            _ => vec![part.clone()],
        };
        if out.len().saturating_mul(choices.len()) > MAX_BRACES {
            return Err("Too many brace expansions".into());
        }
        out = out
            .into_iter()
            .flat_map(|parts| {
                choices.iter().map(move |p| {
                    let mut next = parts.clone();
                    next.push(p.clone());
                    next
                })
            })
            .collect();
    }
    Ok(out)
}
fn braces(s: &str, depth: usize) -> Result<Vec<String>> {
    if depth > 32 {
        return Err("Brace nesting limit".into());
    }
    let Some(start) = s.find('{') else {
        return Ok(vec![s.into()]);
    };
    let mut level = 0;
    let mut end = None;
    let mut commas = Vec::new();
    for (i, c) in s.char_indices().skip_while(|(i, _)| *i < start) {
        match c {
            '{' => level += 1,
            '}' => {
                level -= 1;
                if level == 0 {
                    end = Some(i);
                    break;
                }
            }
            ',' if level == 1 => commas.push(i),
            _ => {}
        }
    }
    let Some(end) = end else {
        return Ok(vec![s.into()]);
    };
    let body = &s[start + 1..end];
    let choices: Vec<String> = if !commas.is_empty() {
        let mut points = vec![start];
        points.extend(commas);
        points.push(end);
        points
            .windows(2)
            .map(|p| s[p[0] + 1..p[1]].into())
            .collect()
    } else {
        let values = body
            .split("..")
            .map(str::parse::<i64>)
            .collect::<std::result::Result<Vec<_>, _>>();
        let Ok(values) = values else {
            return Ok(vec![s.into()]);
        };
        if !(2..=3).contains(&values.len()) {
            return Ok(vec![s.into()]);
        }
        let (a, b) = (i128::from(values[0]), i128::from(values[1]));
        let step = i128::from(*values.get(2).unwrap_or(&1)).abs();
        if step == 0 || (b - a).abs() / step >= MAX_BRACES as i128 {
            return Err("Brace range too large or zero step".into());
        }
        let direction = if b >= a { step } else { -step };
        (0..=(b - a).abs() / step)
            .map(|i| (a + i * direction).to_string())
            .collect()
    };
    let mut out = Vec::new();
    for choice in choices {
        out.extend(braces(
            &format!("{}{choice}{}", &s[..start], &s[end + 1..]),
            depth + 1,
        )?);
        if out.len() > MAX_BRACES {
            return Err("Too many brace expansions".into());
        }
    }
    Ok(out)
}
pub fn append(pattern: &mut Pattern, text: &str, protected: bool) {
    pattern.extend(text.chars().map(|c| (c, protected)));
}
fn magic(p: &[(char, bool)]) -> bool {
    p.iter().any(|(c, q)| !q && "*?[".contains(*c))
}
fn plain(p: &[(char, bool)]) -> String {
    p.iter().map(|(c, _)| c).collect()
}
fn matches(pattern: &[(char, bool)], name: &str) -> bool {
    let text: Vec<char> = name.to_lowercase().chars().collect();
    // Dynamic programming prevents exponential backtracking for many stars.
    let mut row = vec![false; text.len() + 1];
    row[0] = true;
    let mut i = 0;
    while i < pattern.len() {
        let (c, q) = pattern[i];
        if c == '*' && !q {
            for j in 1..row.len() {
                row[j] |= row[j - 1];
            }
            i += 1;
            continue;
        }
        let mut class = None;
        if c == '[' && !q {
            let mut close = i + 1;
            if pattern.get(close).is_some_and(|p| p.0 == '!') {
                close += 1;
            }
            if pattern.get(close).is_some_and(|p| p.0 == ']') {
                close += 1;
            }
            while close < pattern.len() && (pattern[close].0 != ']' || pattern[close].1) {
                close += 1;
            }
            if close < pattern.len() {
                class = Some((i + 1, close));
            }
        }
        let mut next = vec![false; row.len()];
        for j in 0..text.len() {
            let ok = if let Some((start, end)) = class {
                let negated = pattern[start].0 == '!';
                let mut k = start + usize::from(negated);
                let mut found = false;
                while k < end {
                    let a = pattern[k].0.to_ascii_lowercase();
                    if k + 2 < end && pattern[k + 1] == ('-', false) {
                        found |= a <= text[j] && text[j] <= pattern[k + 2].0.to_ascii_lowercase();
                        k += 3;
                    } else {
                        found |= a == text[j];
                        k += 1;
                    }
                }
                found != negated
            } else {
                (!q && c == '?') || c.to_lowercase().next() == Some(text[j])
            };
            next[j + 1] = row[j] && ok;
        }
        row = next;
        i = class.map_or(i + 1, |(_, end)| end + 1);
    }
    row[text.len()]
}
pub fn glob(pattern: &[(char, bool)], cwd: &Path) -> Result<Vec<String>> {
    if !magic(pattern) {
        return Ok(vec![plain(pattern)]);
    }
    if pattern
        .windows(2)
        .any(|p| p[0] == ('*', false) && p[1] == ('*', false))
    {
        return Err("Recursive ** globs are not enabled".into());
    }
    fn walk(
        p: &[(char, bool)],
        cwd: &Path,
        scanned: &mut usize,
        depth: usize,
    ) -> Result<Vec<String>> {
        if depth > 128 {
            return Err("Glob path depth limit".into());
        }
        if !magic(p) {
            let value = plain(p);
            return Ok(if value.is_empty() || cwd.join(&value).exists() {
                vec![value]
            } else {
                vec![]
            });
        }
        let split = p.iter().rposition(|(c, _)| *c == '/' || *c == '\\');
        let (parent, name, sep) = if let Some(i) = split {
            (&p[..i], &p[i + 1..], Some(p[i].0))
        } else {
            (&p[..0], p, None)
        };
        let parents = if parent.is_empty()
            && let Some(separator) = sep
        {
            vec![separator.to_string()]
        } else {
            walk(parent, cwd, scanned, depth + 1)?
        };
        let mut out = Vec::new();
        for base in parents {
            let prefix = if base.is_empty() {
                String::new()
            } else if base.ends_with(['/', '\\']) {
                base.clone()
            } else {
                format!("{base}{}", sep.unwrap_or('/'))
            };
            if !magic(name) {
                let path = format!("{prefix}{}", plain(name));
                let full = cwd.join(&path);
                if full.exists() && (!name.is_empty() || full.is_dir()) {
                    out.push(path);
                }
            } else {
                let entries = match fs::read_dir(cwd.join(&base)) {
                    Ok(e) => e,
                    Err(e)
                        if matches!(
                            e.kind(),
                            std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                        ) =>
                    {
                        continue;
                    }
                    Err(e) => return Err(format!("Glob directory: {e}")),
                };
                for entry in entries {
                    *scanned += 1;
                    if *scanned > 100_000 {
                        return Err("Glob directory scan limit".into());
                    }
                    let entry = entry.map_err(|e| e.to_string())?;
                    let text = entry.file_name().to_string_lossy().into_owned();
                    if text.starts_with('.') && name.first().is_none_or(|p| p.0 != '.') {
                        continue;
                    }
                    if matches(name, &text) {
                        out.push(format!("{prefix}{text}"));
                    }
                    if out.len() > MAX_WORDS {
                        return Err("Too many glob matches".into());
                    }
                }
            }
        }
        Ok(out)
    }
    let mut found = walk(pattern, cwd, &mut 0, 0)?;
    found.sort();
    if found.is_empty() {
        found.push(plain(pattern));
    }
    Ok(found)
}
