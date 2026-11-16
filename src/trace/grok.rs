use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
    sync::LazyLock,
};

use regex::Regex;
use serde_json::{Map, Value, json};

use super::{calls::strip_rtk_prefix, jsonl::records};
use crate::value::{count, json_text, json_text_utf8, round_decimal, scalar_text, truthy};

pub const TIMEOUT_CODE: i64 = 124;

pub fn tool(name: &Value, args: &Value) -> (Value, Value) {
    let args = args
        .as_object()
        .cloned()
        .map(Value::Object)
        .unwrap_or(json!({}));
    let first = |keys: &[&str]| {
        keys.iter()
            .find_map(|key| args.get(*key).filter(|v| truthy(v)))
            .cloned()
            .unwrap_or(json!(""))
    };
    match name.as_str().unwrap_or("") {
        "run_terminal_command" | "run_terminal_cmd" => (
            json!("Bash"),
            json!({"command": strip_rtk_prefix(&if truthy(&args["command"]) { scalar_text(&args["command"]) } else { String::new() })}),
        ),
        "read_file" => {
            let mut out = json!({"file_path": first(&["target_file", "file_path"])});
            for key in ["offset", "limit"] {
                if !args[key].is_null() {
                    out[key] = args[key].clone();
                }
            }
            (json!("Read"), out)
        }
        "search_replace" => (
            json!("Edit"),
            json!({"file_path": first(&["file_path", "target_file"])}),
        ),
        "grep" => {
            let mut out = json!({"pattern": first(&["pattern"]), "path": first(&["path"])});
            if truthy(&args["glob"]) {
                out["glob"] = args["glob"].clone();
            }
            (json!("Grep"), out)
        }
        "list_dir" => (
            json!("Glob"),
            json!({"path": first(&["target_directory", "path"])}),
        ),
        "web_fetch" => (json!("WebFetch"), args),
        "web_search" => (json!("WebSearch"), args),
        "spawn_subagent" => (json!("Task"), args),
        _ => (
            if truthy(name) {
                name.clone()
            } else {
                json!("")
            },
            args,
        ),
    }
}

pub fn bytes_text(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return text.into();
    }
    let Some(bytes) = value.as_array() else {
        return String::new();
    };
    let bytes = bytes
        .iter()
        .map(|b| match b {
            Value::Bool(b) => Some(u8::from(*b)),
            n => n.as_u64().and_then(|n| u8::try_from(n).ok()),
        })
        .collect::<Option<Vec<_>>>();
    bytes
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default()
}

pub fn result(response: &Value) -> Option<(String, Option<i64>)> {
    static EXIT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\A\s*exit: (-?\d+)[ \t]*(?:\r?\n|\z)").unwrap());
    let kind = response["type"].as_str()?;
    let mut code = match &response["exit_code"] {
        Value::Bool(b) => Some(i64::from(*b)),
        n => n.as_i64(),
    };
    if kind == "Bash" {
        let mut text = response["output_for_prompt"]
            .as_str()
            .map(String::from)
            .unwrap_or_else(|| bytes_text(&response["output"]));
        if let Some(matched) = EXIT.captures(&text) {
            code = code.or_else(|| matched[1].parse().ok());
            text = text[matched.get(0).unwrap().end()..].into();
        }
        if truthy(&response["timed_out"]) && code.is_none_or(|code| code == 0) {
            code = Some(TIMEOUT_CODE);
        }
        return Some((text, code));
    }
    if kind == "GrepSearch" {
        let text = [
            bytes_text(&response["stdout"]),
            bytes_text(&response["stderr"]),
        ]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
        return Some((text, code.map(|code| if code == 1 { 0 } else { code })));
    }
    for body in response.as_object()?.values().filter(|v| v.is_object()) {
        for key in ["raw_output", "tool_output_for_prompt", "content"] {
            if let Some(text) = body[key].as_str() {
                return Some((text.into(), code));
            }
        }
    }
    let mut sorted = response.clone();
    sorted.sort_all_objects();
    Some((json_text(&sorted), code))
}

pub fn content_text(content: &Value) -> String {
    content
        .as_array()
        .map(|blocks| {
            blocks
                .iter()
                .filter_map(|b| b["content"]["text"].as_str())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

type Scan = (Vec<(String, Value)>, Map<String, Value>, Map<String, Value>);

fn scan(path: &Path) -> Scan {
    fn flush(prompt: &mut Vec<String>, events: &mut Vec<(String, Value)>) {
        if !prompt.is_empty() {
            events.push(("user".into(), prompt.join("").into()));
        }
        prompt.clear();
    }
    let mut events = Vec::new();
    let mut calls = Map::<String, Value>::new();
    let mut results = Map::new();
    let mut prompt = Vec::new();
    let mut prompt_index = Value::Null;
    for record in records(path) {
        let update = &record["params"]["update"];
        let kind = update["sessionUpdate"].as_str().unwrap_or("");
        if kind == "user_message_chunk" {
            if !prompt.is_empty() && update["_meta"]["promptIndex"] != prompt_index {
                flush(&mut prompt, &mut events);
            }
            prompt_index = update["_meta"]["promptIndex"].clone();
            if let Some(text) = update["content"]["text"].as_str() {
                prompt.push(text.into());
            }
            continue;
        }
        if [
            "tool_call",
            "tool_call_update",
            "agent_message_chunk",
            "turn_completed",
        ]
        .contains(&kind)
        {
            flush(&mut prompt, &mut events);
        }
        if !truthy(&update["toolCallId"]) {
            continue;
        }
        let key = scalar_text(&update["toolCallId"]);
        if kind == "tool_call" {
            calls.insert(key, json!({"name": update.get("title").filter(|v| truthy(v)).map(scalar_text).unwrap_or_default(),
                "args": update["rawInput"].as_object().cloned().map(Value::Object).unwrap_or(json!({}))}));
            events.push(("call".into(), update["toolCallId"].clone()));
        } else if kind == "tool_call_update" {
            if let Some(call) = calls.get_mut(&key)
                && let Some(input) = update["rawInput"].as_object()
            {
                for (key, value) in input {
                    if key != "variant" {
                        call["args"][key] = value.clone();
                    }
                }
            }
            if truthy(&update["status"]) {
                results.insert(key, update.clone());
            }
        }
    }
    flush(&mut prompt, &mut events);
    (events, calls, results)
}

pub fn session_to_claude(path: &Path, out_path: &Path) -> Result<Option<usize>, String> {
    let (events, calls, results) = scan(path);
    if events.is_empty() {
        return Ok(None);
    }
    let mut n = 0;
    let mut out = Vec::new();
    for (kind, value) in events {
        if kind == "user" {
            out.push(json!({"type": "user", "message": {"role": "user", "content": value}}));
            continue;
        }
        let key = scalar_text(&value);
        let info = &calls[&key];
        let update = results.get(&key);
        let (name, args) = tool(&info["name"], &info["args"]);
        n += 1;
        let (text, code) = update
            .map(|update| {
                result(&update["rawOutput"]).unwrap_or_else(|| {
                    (
                        update["rawOutput"]
                            .as_str()
                            .map(String::from)
                            .unwrap_or_else(|| content_text(&update["content"])),
                        None,
                    )
                })
            })
            .unwrap_or_default();
        let mut event = json!({"type": "assistant", "message": {"role": "assistant", "content": [{"type": "tool_use", "id": value, "name": name, "input": args}]}});
        if let Some(cwd) = update
            .and_then(|u| u["rawOutput"]["current_dir"].as_str())
            .filter(|s| !s.is_empty())
        {
            event["cwd"] = cwd.into();
        }
        out.push(event);
        if let Some(update) = update {
            let failed = update["status"] == "failed" || code.is_some_and(|code| code != 0);
            out.push(json!({"type": "user", "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": value, "content": text, "is_error": failed}]}}));
        }
    }
    let mut file = BufWriter::new(File::create(out_path).map_err(|e| e.to_string())?);
    for record in out {
        writeln!(file, "{}", json_text_utf8(&record)).map_err(|e| e.to_string())?;
    }
    file.flush().map_err(|e| e.to_string())?;
    Ok(Some(n))
}

pub fn prompt_count(path: &Path) -> usize {
    scan(path)
        .0
        .iter()
        .filter(|(kind, text)| {
            kind == "user" && text.as_str().is_some_and(|s| !s.trim().is_empty())
        })
        .count()
}

pub fn session_usage(path: &Path) -> Result<Value, String> {
    let file = path.parent().unwrap_or(Path::new("")).join("usage.json");
    let Some(data) = std::fs::read(file)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
    else {
        return Ok(json!({}));
    };
    let session = &data["session"];
    if !session.is_object() || session["inputTokens"].is_null() {
        return Ok(json!({}));
    }
    let cached = count(&session["cachedReadTokens"], "cachedReadTokens")?;
    Ok(
        json!({"input_tokens": (count(&session["inputTokens"], "inputTokens")? - cached).max(0.0),
        "cache_read_input_tokens": cached, "cache_creation_input_tokens": count(&session["cacheCreationTokens"], "cacheCreationTokens")?,
        "output_tokens": count(&session["outputTokens"], "outputTokens")?, "requests": count(&session["modelCalls"], "modelCalls")?,
        "model": session["primaryModelId"], "cost_usd_vendor": round_decimal(count(&session["costUsdTicks"], "costUsdTicks")? / 1e10, 6)}),
    )
}

pub fn json_reply(stdout: &str) -> Result<(String, Option<Value>), String> {
    let stdout = stdout.trim();
    let data = std::iter::once(stdout)
        .chain(stdout.lines().rev())
        .find_map(|line| serde_json::from_str::<Value>(line).ok())
        .unwrap_or(Value::Null);
    if !data.is_object() {
        return Ok((String::new(), None));
    }
    let text = data["text"].as_str().unwrap_or("").trim().to_owned();
    let usage = &data["usage"];
    if !usage.is_object() || usage["input_tokens"].is_null() {
        return Ok((text, None));
    }
    Ok((
        text,
        Some(
            json!({"input_tokens": count(&usage["input_tokens"], "input_tokens")?,
        "cache_read_input_tokens": count(&usage["cache_read_input_tokens"], "cache_read_input_tokens")?,
        "cache_creation_input_tokens": count(&usage["cache_creation_input_tokens"], "cache_creation_input_tokens")?,
        "output_tokens": count(&usage["output_tokens"], "output_tokens")?,
        "model": data["modelUsage"].as_object().and_then(|m| m.keys().next())}),
        ),
    ))
}
