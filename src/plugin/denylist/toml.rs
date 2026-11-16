use regex::Regex;
use std::sync::LazyLock;

static HEADER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"^\s*\[\s*([A-Za-z0-9_.\-\"]+)\s*\]\s*(#.*)?$"#).unwrap());
static ANY_HEADER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*\[").unwrap());

pub fn lines(text: &str) -> Vec<String> {
    if text.contains("\r\n") {
        return lines(&text.replace("\r\n", "\n"));
    }
    let mut lines: Vec<_> = text
        .split([
            '\n', '\r', '\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2028}',
            '\u{2029}',
        ])
        .map(String::from)
        .collect();
    if text.is_empty() {
        return Vec::new();
    }
    if lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines
}
pub fn sections(lines: &[String]) -> Vec<(Option<String>, usize, usize)> {
    let (mut out, mut current, mut start) = (Vec::new(), None, 0);
    for (i, line) in lines.iter().enumerate() {
        if ANY_HEADER.is_match(line) {
            out.push((current, start, i));
            current = Some(
                HEADER
                    .captures(line)
                    .map(|m| m[1].to_string())
                    .unwrap_or("[[array]]".into()),
            );
            start = i + 1;
        }
    }
    out.push((current, start, lines.len()));
    out
}
pub fn key_set(lines: &[String], table: Option<&str>, key: &str) -> bool {
    let key = Regex::new(&format!(r"^{}\s*=", regex::escape(key))).unwrap();
    let dotted = table.map(|table| {
        Regex::new(&format!(
            r"^{}\.{}",
            regex::escape(table),
            key.as_str().trim_start_matches('^')
        ))
        .unwrap()
    });
    for (t, a, b) in sections(lines) {
        for line in &lines[a..b] {
            let line = line.trim();
            if t.as_deref() == table && key.is_match(line) {
                return true;
            }
            if t.is_none() && dotted.as_ref().is_some_and(|r| r.is_match(line)) {
                return true;
            }
        }
    }
    false
}
pub fn insert(
    text: &str,
    table: Option<&str>,
    key: &str,
    value: &str,
    tag: &str,
) -> (String, bool) {
    let mut content = lines(text);
    if key_set(&content, table, key) {
        return (text.into(), false);
    }
    if table.is_some_and(|t| {
        let rx = Regex::new(&format!(r"^\s*{}\s*=", regex::escape(t))).unwrap();
        content.iter().any(|line| rx.is_match(line))
    }) {
        return (text.into(), false);
    }
    let line = format!("{key} = {value}  {tag}");
    let secs = sections(&content);
    let first = secs.get(1).map(|(_, a, _)| a - 1).unwrap_or(content.len());
    if let Some(table) = table {
        let header = secs
            .iter()
            .find(|(t, _, _)| t.as_deref() == Some(table))
            .map(|(_, a, _)| *a);
        let rx = Regex::new(&format!(r"^\s*{}\.\w+\s*=", regex::escape(table))).unwrap();
        if let Some(at) = header {
            content.insert(at, line);
        } else if content[..first].iter().any(|s| rx.is_match(s)) {
            content.insert(first, format!("{table}.{line}"));
        } else {
            if content.last().is_some_and(|s| !s.trim().is_empty()) {
                content.push(String::new());
            }
            content.extend([format!("[{table}]  {tag}"), line]);
        }
    } else {
        content.insert(first, line);
    }
    (content.join("\n") + "\n", true)
}
pub fn remove(text: &str, tags: &[String]) -> String {
    let content = lines(text);
    let marked = |line: &str| tags.iter().any(|tag| line.trim_end().ends_with(tag));
    let mut keep = Vec::new();
    for (i, line) in content.iter().enumerate() {
        if !marked(line) {
            keep.push(line.clone());
            continue;
        }
        if HEADER.is_match(line) {
            let foreign = content[i + 1..]
                .iter()
                .take_while(|line| !ANY_HEADER.is_match(line))
                .any(|line| !line.trim().is_empty() && !marked(line));
            if foreign {
                keep.push(line.clone());
            }
        }
    }
    while keep.last().is_some_and(|line| line.trim().is_empty()) {
        keep.pop();
    }
    if keep.is_empty() {
        String::new()
    } else {
        keep.join("\n") + "\n"
    }
}
pub fn valid(text: &str) -> bool {
    text.parse::<::toml::Table>().is_ok()
}
