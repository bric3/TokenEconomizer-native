use std::{
    fs::File,
    io::{BufWriter, Write},
    path::{Path, PathBuf},
};

use serde_json::{Map, Value, json};

use super::{
    claude,
    jsonl::records,
    time::{epoch, system_seconds},
};
use crate::value::{json_text, truthy};

pub fn claude_tool_name(name: &Value) -> Value {
    let key = name.as_str().unwrap_or("").to_lowercase();
    match key.as_str() {
        "execute" | "script" => "Bash",
        "create" | "write" => "Write",
        "edit" | "str_replace" | "applypatch" | "apply_patch" => "Edit",
        "multiedit" => "MultiEdit",
        "read" => "Read",
        "ls" => "LS",
        "grep" => "Grep",
        "glob" => "Glob",
        _ => return name.clone(),
    }
    .into()
}

pub fn session_header(path: &Path) -> Value {
    records(path)
        .find(|event| event["type"] == "session_start")
        .unwrap_or(json!({}))
}

pub fn session_cwd(path: &Path) -> Option<String> {
    session_header(path)["cwd"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(String::from)
}

pub fn log_head(path: &Path) -> Value {
    let mut head = json!({});
    let mut started = None;
    for event in records(path) {
        if event["type"] == "session_start" && !truthy(&head) {
            head = event;
        } else if event["type"] == "message" && started.is_none() {
            started = epoch(&event["timestamp"]);
        }
        if truthy(&head) && started.is_some() {
            break;
        }
    }
    let mut out = json!({});
    if let Some(cwd) = head["cwd"].as_str().filter(|s| !s.is_empty()) {
        out["cwd"] = cwd.into();
    }
    if let Some(started) = started.filter(|n| *n != 0.0) {
        out["started"] = started.into();
    }
    out
}

pub fn safe_session_id(sid: &Value) -> Option<&str> {
    claude::safe_session_id(sid)
}

pub fn is_prompt(message: &Value) -> bool {
    fn typed(text: &str) -> bool {
        !text.trim().is_empty()
            && !["<system-reminder", "<local-command", "[Request interrupted"]
                .iter()
                .any(|prefix| text.trim_start().starts_with(prefix))
    }
    if message["role"] != "user"
        || ["llm_only", "user_only"]
            .iter()
            .any(|visibility| message["visibility"] == *visibility)
    {
        return false;
    }
    let content = &message["content"];
    if let Some(text) = content.as_str() {
        return typed(text);
    }
    let Some(blocks) = content.as_array() else {
        return false;
    };
    if blocks.iter().any(|block| block["type"] == "tool_result") {
        return false;
    }
    blocks
        .iter()
        .any(|block| block["type"] == "text" && block["text"].as_str().is_some_and(typed))
}

pub fn prompt_count(path: &Path) -> usize {
    records(path)
        .filter(|event| event["type"] == "message" && is_prompt(&event["message"]))
        .count()
}

pub fn content(content: &Value) -> Vec<Value> {
    if let Some(blocks) = content.as_array() {
        return blocks
            .iter()
            .filter(|block| block.is_object())
            .cloned()
            .collect();
    }
    if let Some(text) = content.as_str().filter(|s| !s.is_empty()) {
        return vec![json!({"type": "text", "text": text})];
    }
    Vec::new()
}

pub fn claude_line(event: &Value, cwd: &Value) -> Option<Value> {
    let message = &event["message"];
    if !["user", "assistant"]
        .iter()
        .any(|role| message["role"] == *role)
    {
        return None;
    }
    let mut blocks = content(&message["content"]);
    if blocks.is_empty() {
        return None;
    }
    for block in &mut blocks {
        if block["type"] == "tool_use" {
            block["name"] = claude_tool_name(&block["name"]);
        }
    }
    let mut record =
        json!({"type": message["role"], "message": {"role": message["role"], "content": blocks}});
    if truthy(&message["modelId"]) {
        record["message"]["model"] = message["modelId"].clone();
    }
    if truthy(cwd) {
        record["cwd"] = cwd.clone();
    }
    Some(record)
}

pub fn session_to_claude(path: &Path, out_path: &Path) -> Result<Option<usize>, String> {
    if path.as_os_str().is_empty() || !path.exists() {
        return Ok(None);
    }
    let cwd = json!(session_cwd(path));
    let mut out = BufWriter::new(File::create(out_path).map_err(|e| e.to_string())?);
    let mut n = 0;
    for event in records(path).filter(|e| e["type"] == "message") {
        let Some(record) = claude_line(&event, &cwd) else {
            continue;
        };
        n += record["message"]["content"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|block| block["type"] == "tool_use")
            .count();
        writeln!(out, "{}", json_text(&record)).map_err(|e| e.to_string())?;
    }
    out.flush().map_err(|e| e.to_string())?;
    Ok(Some(n))
}

pub fn prompt_turns(path: &Path) -> impl Iterator<Item = Vec<String>> {
    let cwd = json!(session_cwd(path));
    let mut events = records(path);
    let mut current = None::<Vec<String>>;
    std::iter::from_fn(move || {
        for event in events.by_ref().filter(|e| e["type"] == "message") {
            let Some(record) = claude_line(&event, &cwd) else {
                continue;
            };
            let line = json_text(&record);
            if is_prompt(&event["message"]) {
                if let Some(previous) = current.replace(vec![line]) {
                    return Some(previous);
                }
            } else if let Some(current) = current.as_mut() {
                current.push(line);
            }
        }
        current.take()
    })
}

pub fn settings_path(transcript_path: &str) -> Option<PathBuf> {
    if transcript_path.is_empty() {
        return None;
    }
    Some(
        format!(
            "{}.settings.json",
            transcript_path
                .strip_suffix(".jsonl")
                .unwrap_or(transcript_path)
        )
        .into(),
    )
}

pub fn session_usage(path: &Path) -> Value {
    let mut out = json!({"input_tokens": null, "cache_read_input_tokens": null, "cache_creation_input_tokens": null,
        "output_tokens": null, "requests": 0, "model": null, "factory_credits": null});
    let data = settings_path(&path.to_string_lossy())
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .unwrap_or(json!({}));
    out["model"] = json!(data["model"].as_str());
    let mut models = Map::new();
    let mut requests = 0;
    for event in
        records(path).filter(|e| e["type"] == "message" && e["message"]["role"] == "assistant")
    {
        requests += 1;
        if let Some(model) = event["message"]["modelId"]
            .as_str()
            .filter(|s| !s.is_empty())
        {
            let n = models.get(model).and_then(Value::as_u64).unwrap_or(0) + 1;
            models.insert(model.into(), n.into());
        }
    }
    out["requests"] = requests.into();
    if !truthy(&out["model"]) {
        let mut most = 0;
        for (model, n) in models {
            if n.as_u64().unwrap() > most {
                most = n.as_u64().unwrap();
                out["model"] = model.into();
            }
        }
    }
    let usage = &data["tokenUsage"];
    if !usage.is_object() {
        return out;
    }
    let mut counts = Map::new();
    for (source, target) in [
        ("inputTokens", "input_tokens"),
        ("cacheReadTokens", "cache_read_input_tokens"),
        ("cacheCreationTokens", "cache_creation_input_tokens"),
        ("outputTokens", "output_tokens"),
    ] {
        counts.insert(
            target.into(),
            usage[source].as_f64().map(f64::trunc).unwrap_or(0.0).into(),
        );
    }
    if counts.values().all(|n| !truthy(n)) {
        return out;
    }
    out.as_object_mut().unwrap().extend(counts);
    if usage["factoryCredits"].is_number() {
        out["factory_credits"] = usage["factoryCredits"].clone();
    }
    out
}

pub fn sessions(root: &Path) -> Vec<Value> {
    fn visit(directory: &Path, depth: usize, rows: &mut Vec<Value>) {
        let Ok(entries) = directory.read_dir() else {
            return;
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                // os.walk's relative separator count admits two directory levels below root.
                if depth < 2 {
                    visit(&path, depth + 1, rows);
                }
                continue;
            }
            if path
                .extension()
                .is_none_or(|extension| extension != "jsonl")
            {
                continue;
            }
            let Ok(metadata) = path.metadata() else {
                continue;
            };
            let Ok(modified) = metadata.modified() else {
                continue;
            };
            if File::open(&path).is_err() {
                continue;
            }
            let mut header = json!({});
            let mut started = None;
            for event in records(&path) {
                if event["type"] == "session_start" && !truthy(&header) {
                    header = event;
                } else if event["type"] == "message" && started.is_none() {
                    started = epoch(&event["timestamp"]);
                }
                if truthy(&header) && started.is_some() {
                    break;
                }
            }
            rows.push(json!({"path": path, "id": header.get("id").filter(|v| truthy(v)).cloned().unwrap_or_else(|| json!(path.file_stem().unwrap_or_default().to_string_lossy())),
                "cwd": header["cwd"].as_str(), "version": header["version"], "title": header["title"], "parent": header["callingSessionId"],
                "started": started, "mtime": system_seconds(modified), "size": metadata.len(), "folder": directory.file_name().unwrap_or_default().to_string_lossy()}));
        }
    }
    let mut rows = Vec::new();
    visit(root, 0, &mut rows);
    rows.sort_by(|a, b| {
        a["mtime"]
            .as_f64()
            .unwrap()
            .total_cmp(&b["mtime"].as_f64().unwrap())
            .then_with(|| a["path"].as_str().cmp(&b["path"].as_str()))
    });
    rows
}
