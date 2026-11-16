use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    path::{Path, PathBuf},
    sync::LazyLock,
};

use regex::Regex;
use serde_json::{Map, Value, json};

use super::jsonl::{lines, records};
use super::{
    calls as trajectories,
    jsonl::TextReader,
    time::{epoch, system_seconds},
};
use crate::value::{integer, truthy};

pub const SYNTHETIC_MODEL: &str = "<synthetic>";
pub const HEAD_BYTES: usize = 8 << 20;
pub const LINE_BYTES: usize = 1 << 20;
pub const HEAD_LINES: usize = 200;
pub const USAGE_KEYS: [&str; 5] = [
    "input_tokens",
    "cache_read_input_tokens",
    "cache_creation_input_tokens",
    "output_tokens",
    "cache_creation_1h_input_tokens",
];

pub type Call = (Value, Value, String, bool, Option<String>);

pub fn projects_dir(env: &Value, home: &Path) -> PathBuf {
    let base = env["CLAUDE_CONFIG_DIR"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let key = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
            env[key]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| home.to_path_buf())
                .join(".claude")
        });
    base.join("projects")
}

pub fn history_logs(projects_dir: &Path) -> Vec<(String, PathBuf, f64, u64)> {
    let Ok(folders) = projects_dir.read_dir() else {
        return Vec::new();
    };
    let mut folders = folders.filter_map(Result::ok).collect::<Vec<_>>();
    folders.sort_by_key(|entry| entry.file_name());
    let mut out = Vec::new();
    for folder in folders {
        let Ok(entries) = folder.path().read_dir() else {
            continue;
        };
        let mut entries = entries.filter_map(Result::ok).collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.ends_with(".jsonl") || name.starts_with("agent-") {
                continue;
            }
            let path = entry.path();
            let Ok(metadata) = std::fs::metadata(&path) else {
                continue;
            };
            let Ok(modified) = metadata.modified() else {
                continue;
            };
            if metadata.is_file() {
                out.push((
                    folder.file_name().to_string_lossy().into_owned(),
                    path,
                    system_seconds(modified),
                    metadata.len(),
                ));
            }
        }
    }
    out
}

pub fn log_head(path: &Path) -> Value {
    let mut out = json!({"session_id": null, "cwd": null, "started": null, "entrypoint": ""});
    let Ok(mut reader) = TextReader::open(path) else {
        return out;
    };
    let mut left = HEAD_BYTES;
    for _ in 0..HEAD_LINES {
        let Ok(line) = reader.readline(left.min(LINE_BYTES)) else {
            break;
        };
        if line.is_empty() {
            break;
        }
        let n = line.chars().count();
        left -= n;
        if !line.ends_with('\n') && n >= LINE_BYTES {
            while left > 0 {
                let Ok(rest) = reader.readline(left.min(LINE_BYTES)) else {
                    break;
                };
                left -= rest.chars().count();
                if rest.is_empty() || rest.ends_with('\n') {
                    break;
                }
            }
            continue;
        }
        let Ok(event) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if !event.is_object() {
            continue;
        }
        if out["session_id"].is_null() && event["sessionId"].is_string() {
            out["session_id"] = event["sessionId"].clone();
        }
        if out["cwd"].is_null() && event["cwd"].as_str().is_some_and(|s| !s.is_empty()) {
            out["cwd"] = event["cwd"].clone();
        }
        if out["started"].is_null() {
            out["started"] = json!(epoch(&event["timestamp"]));
        }
        if !truthy(&out["entrypoint"])
            && let Some(entrypoint) = event["entrypoint"].as_str()
        {
            out["entrypoint"] = entrypoint.trim().into();
        }
        if ["session_id", "cwd", "started", "entrypoint"]
            .iter()
            .all(|key| truthy(&out[*key]))
        {
            break;
        }
    }
    out
}

pub fn tool_usage(transcript: &Path, include_subagents: bool) -> BTreeMap<String, usize> {
    let mut files = vec![transcript.to_path_buf()];
    if include_subagents {
        files.extend(subagent_files(transcript));
    }
    let mut counts = BTreeMap::new();
    for file in files {
        for (tool, n) in trajectories::tool_usage(&file) {
            *counts.entry(tool).or_default() += n;
        }
    }
    counts
}

pub fn calls(lines: &[String]) -> Vec<Call> {
    let mut order = Vec::new();
    let mut seen = HashMap::new();
    let mut results = HashMap::new();
    for line in lines {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if !event.is_object() {
            continue;
        }
        let event_type = event["type"].as_str().unwrap_or("assistant");
        if !["assistant", "user"].contains(&event_type) {
            continue;
        }
        let Some(content) = event["message"]["content"].as_array() else {
            continue;
        };
        for block in content.iter().filter(|b| b.is_object()) {
            if block["type"] == "tool_use" {
                let id = block["id"].to_string();
                if seen.contains_key(&id) {
                    continue;
                }
                let args = block.get("input").cloned().unwrap_or(json!({}));
                let cwd = event["cwd"].as_str().map(String::from);
                seen.insert(id.clone(), (block["name"].clone(), args, cwd));
                order.push(id);
            } else if block["type"] == "tool_result" {
                let content = &block["content"];
                let text = if let Some(blocks) = content.as_array() {
                    blocks
                        .iter()
                        .filter(|b| b.is_object())
                        .map(|b| b["text"].as_str().unwrap_or(""))
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    content.as_str().unwrap_or("").into()
                };
                results.insert(
                    block["tool_use_id"].to_string(),
                    (text, !truthy(&block["is_error"])),
                );
            }
        }
    }
    order
        .into_iter()
        .map(|id| {
            let (name, args, cwd) = seen.remove(&id).unwrap();
            let (text, success) = results.remove(&id).unwrap_or((String::new(), true));
            (name, args, text, success, cwd)
        })
        .collect()
}

pub fn is_prompt(event: &Value) -> bool {
    if event["type"] != "user"
        || ["isMeta", "isSidechain", "isCompactSummary"]
            .iter()
            .any(|key| truthy(&event[*key]))
    {
        return false;
    }
    let content = &event["message"]["content"];
    let text = if let Some(blocks) = content.as_array() {
        if blocks.iter().any(|b| b["type"] == "tool_result") {
            return false;
        }
        blocks
            .iter()
            .filter(|b| b["type"] == "text")
            .map(|b| b["text"].as_str().unwrap_or(""))
            .collect::<Vec<_>>()
            .join(" ")
    } else if let Some(text) = content.as_str() {
        text.into()
    } else {
        return false;
    };
    !text.starts_with("<local-command") && !text.starts_with("[Request interrupted")
}

pub fn is_notification(event: &Value) -> bool {
    event["origin"]["kind"] == "task-notification"
}

pub fn prompt_count(path: &Path) -> usize {
    records(path)
        .filter(|event| is_prompt(event) && !is_notification(event))
        .count()
}

pub fn safe_session_id(sid: &Value) -> Option<&str> {
    static SAFE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\A[A-Za-z0-9][A-Za-z0-9_-]{0,127}\z").unwrap());
    sid.as_str().filter(|sid| SAFE.is_match(sid))
}

pub fn project_folder(path: &str) -> String {
    path.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

pub fn prompt_turns(path: &Path) -> impl Iterator<Item = Vec<String>> {
    let mut lines = lines(path);
    let mut current = None::<Vec<String>>;
    std::iter::from_fn(move || {
        for line in lines.by_ref() {
            let Ok(event) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if !event.is_object() {
                continue;
            }
            if is_prompt(&event) {
                if let Some(previous) = current.replace(vec![line]) {
                    return Some(previous);
                }
            } else if !truthy(&event["isSidechain"])
                && let Some(current) = current.as_mut()
            {
                current.push(line);
            }
        }
        current.take()
    })
}

pub fn turn_interrupted(lines: &[String]) -> bool {
    lines
        .iter()
        .filter(|line| line.contains("[Request interrupted"))
        .any(|line| {
            let Ok(event) = serde_json::from_str::<Value>(line) else {
                return false;
            };
            if event["type"] != "user" {
                return false;
            }
            let content = &event["message"]["content"];
            if let Some(text) = content.as_str() {
                return text.starts_with("[Request interrupted");
            }
            content.as_array().is_some_and(|blocks| {
                blocks
                    .iter()
                    .filter(|b| b["type"] == "text")
                    .map(|b| b["text"].as_str().unwrap_or(""))
                    .collect::<Vec<_>>()
                    .join(" ")
                    .starts_with("[Request interrupted")
            })
        })
}

pub fn turn_answered(lines: &[String]) -> bool {
    for line in lines.iter().rev() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if !event.is_object() || !["assistant", "user"].iter().any(|t| event["type"] == *t) {
            continue;
        }
        if event["type"] != "assistant" {
            return false;
        }
        let content = &event["message"]["content"];
        if let Some(text) = content.as_str() {
            return !text.trim().is_empty();
        }
        return content.as_array().is_some_and(|blocks| {
            blocks.iter().any(|b| {
                b["type"] == "text" && b["text"].as_str().is_some_and(|s| !s.trim().is_empty())
            })
        });
    }
    false
}

pub fn subagent_files(transcript_path: &Path) -> Vec<PathBuf> {
    let directory = transcript_path.with_extension("").join("subagents");
    let Ok(entries) = directory.read_dir() else {
        return Vec::new();
    };
    let mut files = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "jsonl"))
        .collect::<Vec<_>>();
    files.sort();
    files
}

pub fn session_usage(transcript_path: &Path, include_subagents: bool) -> Result<Value, String> {
    let mut files = vec![transcript_path.to_path_buf()];
    if include_subagents {
        files.extend(subagent_files(transcript_path));
    }
    let mut by_id = HashMap::<String, [f64; 5]>::new();
    let mut models = Map::<String, Value>::new();
    let mut anonymous = 0;
    for path in &files {
        for line in lines(path).filter(|line| line.contains("\"usage\"")) {
            let Ok(event) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if event["type"] != "assistant" {
                continue;
            }
            let message = &event["message"];
            let usage = &message["usage"];
            if !usage.is_object() || message["model"] == SYNTHETIC_MODEL {
                continue;
            }
            let id = if truthy(&message["id"]) {
                message["id"].to_string()
            } else {
                anonymous += 1;
                json!(format!("anon-{}-{anonymous}", path.display())).to_string()
            };
            let mut counts = [0.0; 5];
            for (i, key) in USAGE_KEYS[..4].iter().enumerate() {
                counts[i] = integer(&usage[*key]).ok_or_else(|| format!("invalid {key} count"))?;
            }
            counts[4] = integer(&usage["cache_creation"]["ephemeral_1h_input_tokens"])
                .unwrap_or(0.0)
                .min(counts[2])
                .max(0.0);
            by_id.insert(id, counts);
            if let Some(model) = message["model"].as_str().filter(|s| !s.is_empty()) {
                let count = models.get(model).and_then(Value::as_u64).unwrap_or(0) + 1;
                models.insert(model.into(), count.into());
            }
        }
    }
    let mut output = Map::new();
    for (i, key) in USAGE_KEYS.iter().enumerate() {
        output.insert(
            (*key).into(),
            by_id.values().map(|counts| counts[i]).sum::<f64>().into(),
        );
    }
    // On a tie the first model encountered wins, matching insertion-ordered Python dicts.
    let mut model = Value::Null;
    let mut most = 0;
    for (name, count) in models {
        if count.as_u64().unwrap_or(0) > most {
            most = count.as_u64().unwrap();
            model = name.into();
        }
    }
    output.insert("requests".into(), by_id.len().into());
    output.insert("model".into(), model);
    output.insert(
        "files".into(),
        files.iter().filter(|p| p.exists()).count().into(),
    );
    Ok(output.into())
}

pub fn deferred_tools(transcript_path: &Path) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for line in lines(transcript_path).filter(|line| line.contains("\"deferred_tools_delta\"")) {
        let Ok(event) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if let Some(added) = event["attachment"]["addedNames"].as_array() {
            names.extend(added.iter().filter_map(Value::as_str).map(String::from));
        }
        if let Some(removed) = event["attachment"]["removedNames"].as_array() {
            for name in removed.iter().filter_map(Value::as_str) {
                names.remove(name);
            }
        }
    }
    names
}
