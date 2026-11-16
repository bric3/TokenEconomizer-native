use std::{collections::BTreeSet, path::Path, sync::LazyLock};

use regex::Regex;
use serde_json::{Map, Value, json};

use crate::value::truthy;

pub type Catalog = Map<String, Value>;

pub fn codex_default() -> Catalog {
    serde_json::from_value(json!({
        "shell": "Run shell commands to inspect, edit, and test the repository.",
        "apply_patch": "Apply source-code file changes.", "web_search": "Search the web.",
        "view_image": "Attach a local image to context (unused on text tasks).",
        "update_plan": "Maintain a step plan (scaffolding, unused here).",
        "spawn_agent": "Spawn a collaborating subagent.", "send_input": "Send input to a collaborating subagent.",
        "resume_agent": "Resume a collaborating subagent.", "wait_agent": "Wait for a collaborating subagent.",
        "close_agent": "Close a collaborating subagent."
    })).unwrap()
}

fn prefix(text: &str, n: i64) -> String {
    let end = if n < 0 {
        text.chars()
            .count()
            .saturating_sub(n.unsigned_abs() as usize)
    } else {
        n as usize
    };
    text.chars().take(end).collect()
}

pub fn parse_flat(text: &str) -> Catalog {
    static BLOCK: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"(?s)"tools"\s*:\s*\[(.*?)\]"#).unwrap());
    static ITEM: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#""([a-z][a-z0-9_]*):[0-9a-f]{6,}""#).unwrap());
    let mut out = Catalog::new();
    for block in BLOCK.captures_iter(text) {
        for item in ITEM.captures_iter(&block[1]) {
            out.entry(&item[1]).or_insert(json!(""));
        }
    }
    out
}

pub fn parse_schemas(text: &str, max_desc: i64) -> Catalog {
    static SCHEMA: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
        r#""name":\s*"([a-z_]+)"\s*,\s*(?:"[a-z_]+":\s*(?:"[^"]*"|null|true|false|[0-9.]+)\s*,\s*)*"description":\s*""#).unwrap()
    });
    let text = text
        .replace("\\\"", "\"")
        .replace("\\n", "\n")
        .replace("\\\\", "\\");
    let mut out = Catalog::new();
    for capture in SCHEMA.captures_iter(&text) {
        if out.contains_key(&capture[1]) {
            continue;
        }
        let tail = &text[capture.get(0).unwrap().end()..];
        let window = prefix(tail, max_desc + 40);
        let description = prefix(window.split('\n').next().unwrap_or(""), max_desc);
        out.insert(capture[1].into(), description.into());
    }
    out
}

pub fn load_claude_snapshot(path: Option<&Path>) -> Value {
    match path {
        Some(path) => std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or(json!({})),
        None => serde_json::from_str(include_str!("claude_tool_catalog.json")).unwrap_or(json!({})),
    }
}

pub fn parse_claude(text: &str, max_desc: i64, snapshot: Option<&Value>) -> Catalog {
    let owned;
    let snapshot = match snapshot {
        Some(snapshot) => snapshot,
        None => {
            owned = load_claude_snapshot(None);
            &owned
        }
    };
    let mut out = Catalog::new();
    for line in text
        .lines()
        .filter(|line| line.contains("\"type\"") && line.contains("\"system\""))
    {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if event["type"] != "system" {
            continue;
        }
        if let Some(names) = event["tools"].as_array() {
            for name in names.iter().filter_map(Value::as_str) {
                out.insert(
                    name.into(),
                    prefix(snapshot[name].as_str().unwrap_or(""), max_desc).into(),
                );
            }
            break;
        }
    }
    out
}

pub fn parse_copilot_names(text: &str) -> Catalog {
    let mut names = BTreeSet::new();
    for line in text.lines() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(states) = event["data"]["promptCacheBreakState"].as_array() else {
            continue;
        };
        for state in states {
            let Some(models) = state["models"].as_object() else {
                continue;
            };
            for model in models.values() {
                if let Some(tools) = model["tools"].as_array() {
                    names.extend(
                        tools
                            .iter()
                            .filter_map(|tool| tool["name"].as_str())
                            .filter(|s| !s.is_empty())
                            .map(String::from),
                    );
                }
            }
        }
    }
    names.into_iter().map(|name| (name, json!(""))).collect()
}

fn read(path: &Path) -> String {
    std::fs::read(path)
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default()
}

pub fn catalog_sources(run_dir: &Path, agent: &str) -> Vec<(String, Catalog)> {
    if agent == "codex" {
        return vec![("codex_default".into(), codex_default())];
    }
    let transcript = read(&run_dir.join("transcript.jsonl"));
    if agent == "claude" {
        let catalog = parse_claude(&transcript, 200, None);
        return if catalog.is_empty() {
            Vec::new()
        } else {
            vec![("claude_transcript".into(), catalog)]
        };
    }
    let debug = read(&run_dir.join("copilot_debug.log"));
    [
        ("schema_debug_log", parse_schemas(&debug, 200)),
        ("usage_checkpoint", parse_copilot_names(&transcript)),
        ("flat_debug_log", parse_flat(&debug)),
    ]
    .into_iter()
    .filter(|(_, catalog)| !catalog.is_empty())
    .map(|(source, catalog)| (source.into(), catalog))
    .collect()
}

pub fn rank(catalog: &Catalog) -> (usize, usize) {
    (
        catalog.values().filter(|v| truthy(v)).count(),
        catalog.len(),
    )
}

pub fn parse_with_source(run_dir: &Path, agent: &str) -> (Catalog, Option<String>) {
    let mut best = None::<(String, Catalog)>;
    for source in catalog_sources(run_dir, agent) {
        if best
            .as_ref()
            .is_none_or(|(_, catalog)| rank(&source.1) > rank(catalog))
        {
            best = Some(source);
        }
    }
    best.map_or((Catalog::new(), None), |(name, catalog)| {
        (catalog, Some(name))
    })
}

pub fn parse(run_dir: &Path, agent: &str) -> Catalog {
    parse_with_source(run_dir, agent).0
}
