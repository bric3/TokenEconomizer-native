use std::{
    collections::BTreeSet,
    fs::{File, OpenOptions},
    io::{BufWriter, Write},
    path::Path,
    sync::LazyLock,
};

use regex::Regex;
use serde_json::{Map, Value, json};

use super::{Call, jsonl::records};
use crate::value::{count, int_value, json_text, scalar_text, truthy};

pub fn shell_patch_paths(command: &str) -> Vec<String> {
    static PATCH: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?m)^\*\*\* (?:Update|Add|Delete) File:\s*(.+?)\s*$").unwrap()
    });
    static GIT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\bgit\s+apply\b").unwrap());
    static GIT_PATH: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?m)^\+\+\+ b/(.+?)\s*$").unwrap());
    let pattern = if command.contains("*** Begin Patch") {
        &*PATCH
    } else if GIT.is_match(command) {
        &*GIT_PATH
    } else {
        return Vec::new();
    };
    let mut seen = BTreeSet::new();
    pattern
        .captures_iter(command)
        .filter_map(|capture| {
            let path = capture[1].trim().to_owned();
            if !path.is_empty() && seen.insert(path.clone()) {
                Some(path)
            } else {
                None
            }
        })
        .collect()
}

pub fn exec_calls(lines: &[String]) -> Vec<Call> {
    let mut out = Vec::new();
    for line in lines {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if event["type"] != "item.completed" {
            continue;
        }
        let item = &event["item"];
        if item["type"] == "command_execution" {
            let command = item["command"].as_str().unwrap_or("");
            let output = item["aggregated_output"].as_str().unwrap_or("");
            let ok = int_value(&item["exit_code"]) == Some(0.0);
            let paths = shell_patch_paths(command);
            if !paths.is_empty() && (ok || output.contains("Success.")) {
                out.push((
                    json!("apply_patch"),
                    json!({"path": paths[0], "paths": paths, "changes": [], "via": "shell"}),
                    String::new(),
                    json!(true),
                ));
            }
            out.push((
                json!("shell"),
                json!({"command": command}),
                output.into(),
                json!(ok),
            ));
        } else if item["type"] == "file_change" {
            let changes = item
                .get("changes")
                .filter(|v| truthy(v))
                .cloned()
                .unwrap_or(json!([]));
            let paths = changes
                .as_array()
                .map(|changes| {
                    changes
                        .iter()
                        .filter(|c| c.is_object() && truthy(&c["path"]))
                        .map(|c| c["path"].clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            out.push((
                json!("apply_patch"),
                json!({"path": paths.first().cloned().unwrap_or(json!("")),
                "paths": paths, "changes": changes}),
                String::new(),
                json!(item["status"] == "completed"),
            ));
        }
    }
    out
}

pub fn command_text(command: &Value) -> String {
    fn quote(text: &str) -> String {
        if !text.is_empty()
            && text
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "_@%+=:,./-".contains(c))
        {
            text.into()
        } else {
            format!("'{}'", text.replace('\'', "'\"'\"'"))
        }
    }
    if let Some(argv) = command.as_array() {
        argv.iter()
            .map(|v| quote(&scalar_text(v)))
            .collect::<Vec<_>>()
            .join(" ")
    } else if truthy(command) {
        scalar_text(command)
    } else {
        String::new()
    }
}

pub fn rollout_item_to_exec(item: &Value) -> Option<Value> {
    let kind = match item["type"].as_str()? {
        "CommandExecution" => "command_execution",
        "FileChange" => "file_change",
        "AgentMessage" => "agent_message",
        "ImageView" => "view_image",
        "Extension" => "web_search",
        "McpToolCall" => "mcp_tool_call",
        "CollabToolCall" => "collab_tool_call",
        _ => return None,
    };
    let mut out = json!({"type": kind, "id": item["id"]});
    match kind {
        "command_execution" => {
            out["command"] = command_text(&item["command"]).into();
            out["aggregated_output"] = item
                .get("aggregated_output")
                .filter(|v| truthy(v))
                .cloned()
                .unwrap_or_else(|| {
                    json!(format!(
                        "{}{}",
                        item["stdout"].as_str().unwrap_or(""),
                        item["stderr"].as_str().unwrap_or("")
                    ))
                });
            out["exit_code"] = item["exit_code"].clone();
            out["status"] = item["status"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or("completed")
                .to_lowercase()
                .into();
        }
        "file_change" => {
            let changes = if let Some(changes) = item["changes"].as_object() {
                json!(
                    changes
                        .iter()
                        .map(|(path, change)| {
                            let mut entry = json!({"path": path});
                            if change.is_object() {
                                entry["kind"] = change["type"].clone();
                            }
                            entry
                        })
                        .collect::<Vec<_>>()
                )
            } else {
                item.get("changes")
                    .filter(|v| truthy(v))
                    .cloned()
                    .unwrap_or(json!([]))
            };
            out["changes"] = changes;
            out["status"] = item["status"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or("completed")
                .to_lowercase()
                .into();
        }
        "agent_message" => {
            out["text"] = if let Some(content) = item["content"].as_array() {
                content
                    .iter()
                    .filter(|c| c.is_object())
                    .map(|c| c["text"].as_str().unwrap_or(""))
                    .collect::<Vec<_>>()
                    .join(" ")
                    .into()
            } else {
                item.get("content")
                    .filter(|v| truthy(v))
                    .cloned()
                    .unwrap_or(json!(""))
            };
        }
        _ => {
            for (key, value) in item.as_object()? {
                if key != "type" && key != "id" {
                    out[key] = value.clone();
                }
            }
        }
    }
    Some(out)
}

pub fn rollout_to_exec(rollout_path: &Path, out_path: &Path) -> Result<Option<usize>, String> {
    if rollout_path.as_os_str().is_empty()
        || rollout_path.to_string_lossy().ends_with(".zst")
        || !rollout_path.exists()
    {
        return Ok(None);
    }
    let mut out = BufWriter::new(File::create(out_path).map_err(|e| e.to_string())?);
    let mut n = 0;
    let mut thread = None;
    for event in records(rollout_path) {
        let payload = &event["payload"];
        if event["type"] == "session_meta" && thread.is_none() {
            thread = payload
                .get("id")
                .filter(|v| truthy(v))
                .or_else(|| payload.get("session_id").filter(|v| truthy(v)))
                .cloned();
            if let Some(thread) = &thread {
                writeln!(
                    out,
                    "{}",
                    json_text(&json!({"type": "thread.started", "thread_id": thread}))
                )
                .map_err(|e| e.to_string())?;
            }
            continue;
        }
        if event["type"] != "event_msg" || payload["type"] != "item_completed" {
            continue;
        }
        if let Some(item) = rollout_item_to_exec(&payload["item"]) {
            writeln!(
                out,
                "{}",
                json_text(&json!({"type": "item.completed", "item": item}))
            )
            .map_err(|e| e.to_string())?;
            n += 1;
        }
    }
    out.flush().map_err(|e| e.to_string())?;
    Ok(Some(n))
}

pub fn capability_trace(rollout_path: &Path, out_path: &Path) -> Result<Option<usize>, String> {
    let Some(mut n) = rollout_to_exec(rollout_path, out_path)? else {
        return Ok(None);
    };
    let mut out = BufWriter::new(
        OpenOptions::new()
            .append(true)
            .open(out_path)
            .map_err(|e| e.to_string())?,
    );
    for event in records(rollout_path) {
        let payload = &event["payload"];
        if event["type"] != "response_item"
            || !["custom_tool_call", "function_call"]
                .iter()
                .any(|kind| payload["type"] == *kind)
        {
            continue;
        }
        let mut item = json!({"type": payload["type"], "name": payload["name"],
            "id": payload.get("call_id").filter(|v| truthy(v)).unwrap_or(&payload["id"]),
            "status": payload["status"].as_str().filter(|s| !s.is_empty()).unwrap_or("completed").to_lowercase()});
        let key = if payload["type"] == "custom_tool_call" {
            "input"
        } else {
            "arguments"
        };
        item[key] = payload[key].clone();
        writeln!(
            out,
            "{}",
            json_text(&json!({"type": "item.completed", "item": item}))
        )
        .map_err(|e| e.to_string())?;
        n += 1;
    }
    out.flush().map_err(|e| e.to_string())?;
    Ok(Some(n))
}

pub fn is_task_started(event: &Value) -> bool {
    event["type"] == "event_msg" && event["payload"]["type"] == "task_started"
}

pub fn prompt_count(path: &Path) -> usize {
    if records(path).any(|e| is_task_started(&e)) {
        records(path).filter(is_task_started).count()
    } else {
        records(path)
            .filter(|e| e["type"] == "turn_context")
            .count()
    }
}

pub fn session_meta(path: &Path) -> Value {
    let mut meta = Map::new();
    for event in records(path) {
        let payload = &event["payload"];
        if event["type"] == "session_meta" {
            meta.entry("cwd").or_insert_with(|| payload["cwd"].clone());
            meta.entry("cli_version")
                .or_insert_with(|| payload["cli_version"].clone());
        } else if event["type"] == "turn_context" && truthy(&payload["model"]) {
            meta.insert("model".into(), payload["model"].clone());
        }
    }
    meta.into()
}

pub fn session_usage(path: &Path) -> Result<Value, String> {
    let mut total = Value::Null;
    let mut requests = 0;
    for event in records(path) {
        let payload = &event["payload"];
        if payload["type"] == "token_count" && truthy(&payload["info"]["total_token_usage"]) {
            total = payload["info"]["total_token_usage"].clone();
            requests += 1;
        }
    }
    let meta = session_meta(path);
    if !truthy(&total) {
        return Ok(
            json!({"input_tokens": null, "cache_read_input_tokens": null,
        "cache_creation_input_tokens": null, "output_tokens": null, "requests": 0, "model": meta["model"]}),
        );
    }
    let input = count(&total["input_tokens"], "input_tokens")?;
    let cached = count(&total["cached_input_tokens"], "cached_input_tokens")?;
    let write = count(
        &total["cache_write_input_tokens"],
        "cache_write_input_tokens",
    )?;
    let reasoning = count(&total["reasoning_output_tokens"], "reasoning_output_tokens")?;
    Ok(
        json!({"input_tokens": (input - cached - write).max(0.0), "cache_read_input_tokens": cached,
        "cache_creation_input_tokens": write, "output_tokens": count(&total["output_tokens"], "output_tokens")? + reasoning,
        "reasoning_output_tokens": reasoning, "requests": requests, "model": meta["model"]}),
    )
}
