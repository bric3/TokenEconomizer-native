use super::input::{self, Node, Text};
use crate::python_json::{self, ParseError};
use indexmap::IndexMap;
use serde_json::json;
use std::{fs, path::Path};

fn map(node: &mut Node) -> Result<&mut IndexMap<Text, Node>, String> {
    let kind = node.kind();
    match node {
        Node::Dict(items) => Ok(items),
        _ => Err(format!(
            "AttributeError: '{kind}' object has no attribute 'setdefault'"
        )),
    }
}
pub fn install_hook(
    agent: &str,
    path: &Path,
    command: &str,
    int_max_digits: usize,
) -> Result<(), String> {
    let (event, entry) = match agent {
        "claude" | "codex" => (
            "PreToolUse",
            json!({"matcher":"Bash","hooks":[{"type":"command","command":command}]}),
        ),
        "copilot" => (
            "preToolUse",
            json!({"matcher":"bash","type":"command","command":command}),
        ),
        _ => return Err(format!("ValueError: unsupported guard agent: {agent}")),
    };
    merge_entry(path, event, Node::from(&entry), int_max_digits, true)
}

pub fn install_entry(
    path: &Path,
    event: &str,
    entry_json: &[u8],
    int_max_digits: usize,
) -> Result<(), String> {
    let document =
        std::str::from_utf8(entry_json).map_err(|error| format!("UnicodeDecodeError: {error}"))?;
    let entry = python_json::parse_result(entry_json, int_max_digits).map_err(|error| {
        let kind = if matches!(error, ParseError::Syntax { .. }) {
            "JSONDecodeError"
        } else {
            "ValueError"
        };
        format!("{kind}: {}", error.message(document))
    })?;
    merge_entry(path, event, entry, int_max_digits, false)
}

fn merge_entry(
    path: &Path,
    event: &str,
    entry: Node,
    int_max_digits: usize,
    recognize_identical: bool,
) -> Result<(), String> {
    let parent = path.parent().unwrap_or(Path::new(""));
    if parent.as_os_str().is_empty() {
        return Err("FileNotFoundError: [Errno 2] No such file or directory: ''".into());
    }
    fs::create_dir_all(parent).map_err(|e| format!("OSError: {e}"))?;
    let mut current = fs::read(path)
        .ok()
        .filter(|bytes| std::str::from_utf8(bytes).is_ok())
        .and_then(|bytes| input::parse(&bytes, int_max_digits))
        .unwrap_or_else(|| Node::Dict(IndexMap::new()));
    let hooks = map(&mut current)?
        .entry(Text::from("hooks"))
        .or_insert_with(|| Node::Dict(IndexMap::new()));
    let pre = map(hooks)?
        .entry(Text::from(event))
        .or_insert_with(|| Node::List(Vec::new()));
    let seen = match pre {
        // Native executable names use hyphens; an identical entry also makes installation idempotent.
        Node::List(items) => items.iter().any(|item| {
            recognize_identical && item == &entry || item.json(false).contains("net_guard")
        }),
        Node::Dict(items) => items.keys().any(|key| key.json().contains("net_guard")),
        Node::String(_) => false,
        other => {
            return Err(format!(
                "TypeError: '{}' object is not iterable",
                other.kind()
            ));
        }
    };
    if !seen {
        match pre {
            Node::List(items) => items.push(entry),
            other => {
                return Err(format!(
                    "AttributeError: '{}' object has no attribute 'append'",
                    other.kind()
                ));
            }
        }
    }
    // Validate the hook structure before truncating the existing file, as the source installer does.
    fs::write(path, current.json(true)).map_err(|e| format!("OSError: {e}"))
}
