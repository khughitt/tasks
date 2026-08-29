use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Scalar(String),
    List(Vec<String>),
    /// Emitted verbatim by `serialize`; `parse` never yields it (it reads `2` as `Scalar("2")`).
    Raw(String),
}

const RESERVED: &str = "\"[],#:\\";

fn bad(detail: impl Into<String>) -> Error {
    Error::Parse { file: "<frontmatter>".into(), detail: detail.into() }
}

pub fn parse(text: &str) -> Result<Vec<(String, Value)>> {
    let mut out: Vec<(String, Value)> = Vec::new();
    for (n, line) in text.lines().enumerate() {
        let n = n + 1;
        let Some((key, rest)) = line.split_once(':') else {
            return Err(bad(format!("line {n}: expected `key: value`")));
        };
        if key.is_empty() || !key.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
            return Err(bad(format!("line {n}: bad key {key:?}")));
        }
        let Some(rest) = rest.strip_prefix(' ') else {
            return Err(bad(format!("line {n}: expected one space after `{key}:`")));
        };
        if rest.is_empty() {
            return Err(bad(format!("line {n}: empty value for {key}; use \"\"")));
        }
        if out.iter().any(|(k, _)| k == key) {
            return Err(bad(format!("line {n}: duplicate key {key}")));
        }
        let value = if let Some(inner) = rest.strip_prefix('[') {
            let inner = inner.strip_suffix(']').ok_or_else(|| bad(format!("line {n}: unterminated list")))?;
            Value::List(parse_items(inner).map_err(|d| bad(format!("line {n}: {d}")))?)
        } else {
            Value::Scalar(parse_scalar(rest).map_err(|d| bad(format!("line {n}: {d}")))?)
        };
        out.push((key.to_string(), value));
    }
    Ok(out)
}

fn parse_items(inner: &str) -> std::result::Result<Vec<String>, String> {
    if inner.is_empty() { return Ok(vec![]); }
    let mut items = Vec::new();
    let mut rest = inner;
    loop {
        let (item, after) = if rest.starts_with('"') {
            take_quoted(rest)?
        } else {
            let end = rest.find(',').unwrap_or(rest.len());
            (parse_scalar(&rest[..end])?, &rest[end..])
        };
        items.push(item);
        if after.is_empty() { return Ok(items); }
        rest = after.strip_prefix(", ").ok_or_else(|| format!("expected `, ` before {after:?}"))?;
        if rest.trim().is_empty() { return Err("trailing comma".into()); }
    }
}

fn parse_scalar(tok: &str) -> std::result::Result<String, String> {
    if tok.starts_with('"') {
        let (s, after) = take_quoted(tok)?;
        if !after.is_empty() { return Err(format!("unexpected text after quoted string: {after:?}")); }
        return Ok(s);
    }
    if tok.is_empty() { return Err("empty bare scalar; use \"\"".into()); }
    if tok.trim() != tok { return Err(format!("bare scalar {tok:?} has surrounding whitespace")); }
    if tok.chars().any(|c| RESERVED.contains(c)) {
        return Err(format!("bare scalar {tok:?} contains a reserved character; quote it"));
    }
    Ok(tok.to_string())
}

/// Parses a leading `"..."`; returns the unescaped string and the remainder after the closing quote.
fn take_quoted(s: &str) -> std::result::Result<(String, &str), String> {
    let mut out = String::new();
    let mut chars = s[1..].char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some((_, '"')) => out.push('"'),
                Some((_, '\\')) => out.push('\\'),
                other => return Err(format!("bad escape {other:?}")),
            },
            '"' => return Ok((out, &s[1 + i + 1..])),
            c => out.push(c),
        }
    }
    Err("unterminated quoted string".into())
}

fn quote(s: &str) -> String {
    let mut q = String::with_capacity(s.len() + 2);
    q.push('"');
    for c in s.chars() {
        if c == '"' || c == '\\' { q.push('\\'); }
        q.push(c);
    }
    q.push('"');
    q
}

fn needs_quotes(s: &str) -> bool {
    s.is_empty()
        || s.trim() != s
        || s.chars().any(|c| RESERVED.contains(c))
        || matches!(s, "true" | "false" | "null" | "~" | "yes" | "no")
        || s.parse::<f64>().is_ok()
}

fn render_scalar(s: &str) -> String {
    if needs_quotes(s) { quote(s) } else { s.to_string() }
}

pub fn serialize(pairs: &[(String, Value)]) -> String {
    let mut out = String::new();
    for (k, v) in pairs {
        out.push_str(k);
        out.push_str(": ");
        match v {
            Value::Scalar(s) => out.push_str(&render_scalar(s)),
            Value::Raw(s) => out.push_str(s),
            Value::List(items) => {
                let rendered: Vec<String> = items.iter().map(|i| render_scalar(i)).collect();
                out.push('[');
                out.push_str(&rendered.join(", "));
                out.push(']');
            }
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> Value { Value::Scalar(v.into()) }

    #[test]
    fn parses_scalars_and_lists() {
        let text = "id: sci-4f2a9c\ntitle: \"Bank: the ledger\"\ndepends: [sci-91be03, fam-0c3d7e]\ntags: []\n";
        let pairs = parse(text).unwrap();
        assert_eq!(pairs, vec![
            ("id".into(), s("sci-4f2a9c")),
            ("title".into(), s("Bank: the ledger")),
            ("depends".into(), Value::List(vec!["sci-91be03".into(), "fam-0c3d7e".into()])),
            ("tags".into(), Value::List(vec![])),
        ]);
    }

    #[test]
    fn rejects_duplicates_and_bad_lines() {
        assert!(parse("a: 1\na: 2\n").is_err());
        assert!(parse("a:1\n").is_err());
        assert!(parse("a: x: y\n").is_err());
        assert!(parse("a:\n").is_err());
        assert!(parse("- item\n").is_err());
        assert!(parse("a: [b, c\n").is_err());
        assert!(parse("a: [b,]\n").is_err());
        assert!(parse("a: [b,,c]\n").is_err());
        assert!(parse("a: [ ]\n").is_err());
        assert!(parse("a: [ a]\n").is_err());
        assert!(parse("a: [a ]\n").is_err());
        assert!(parse("a: [a , b]\n").is_err());
        assert!(parse("a: [\"a\" , b]\n").is_err());
        assert!(parse("a: [a,  b]\n").is_err());
        assert!(parse("a: [a,b]\n").is_err());
    }

    #[test]
    fn roundtrips_with_quoting() {
        let pairs = vec![
            ("title".into(), s("Task 3: emit \"the\" row")),
            ("owner".into(), s("keith")),
            ("empty".into(), s("")),
            ("num".into(), s("42")),
            ("tags".into(), Value::List(vec!["a b".into(), "c,d".into()])),
        ];
        let text = serialize(&pairs);
        assert_eq!(text, "title: \"Task 3: emit \\\"the\\\" row\"\nowner: keith\nempty: \"\"\nnum: \"42\"\ntags: [a b, \"c,d\"]\n");
        assert_eq!(parse(&text).unwrap(), pairs);
        assert_eq!(serialize(&[("priority".into(), Value::Raw("2".into()))]), "priority: 2\n");
    }
}
