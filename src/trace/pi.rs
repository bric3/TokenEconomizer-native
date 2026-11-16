use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

use serde_json::{Map, Value, json};

use super::jsonl::records;
use crate::value::{count, float_or_zero, json_text, round_decimal, truthy};

pub fn session_header(path: &Path) -> Value {
    records(path)
        .next()
        .filter(|e| e["type"] == "session")
        .unwrap_or(json!({}))
}

pub fn active_branch(entries: &[Value]) -> Vec<Value> {
    let mut by_id = HashMap::new();
    let mut leaf = None;
    for (index, entry) in entries.iter().enumerate().filter(|(_, e)| truthy(&e["id"])) {
        by_id.insert(entry["id"].to_string(), index);
        leaf = Some(index);
    }
    let mut seen = HashSet::new();
    let mut path = Vec::new();
    while let Some(index) = leaf {
        let entry = &entries[index];
        if !seen.insert(entry["id"].to_string()) {
            break;
        }
        path.push(entry.clone());
        leaf = by_id.get(&entry["parentId"].to_string()).copied();
    }
    path.reverse();
    path
}

pub fn prompt_count(path: &Path) -> usize {
    active_branch(&records(path).collect::<Vec<_>>())
        .iter()
        .filter(|e| e["type"] == "message" && e["message"]["role"] == "user")
        .count()
}

pub fn usage(usage: &Value) -> Result<Option<Value>, String> {
    if !usage.is_object() {
        return Ok(None);
    }
    Ok(Some(
        json!({"input_tokens": count(&usage["input"], "input")?,
        "cache_read_input_tokens": count(&usage["cacheRead"], "cacheRead")?,
        "cache_creation_input_tokens": count(&usage["cacheWrite"], "cacheWrite")?,
        "output_tokens": count(&usage["output"], "output")?,
        "cache_creation_1h_input_tokens": count(&usage["cacheWrite1h"], "cacheWrite1h")?,
        "cost_usd": float_or_zero(&usage["cost"]["total"])?}),
    ))
}

pub fn text(content: &Value) -> String {
    if let Some(text) = content.as_str() {
        return text.into();
    }
    content
        .as_array()
        .map(|blocks| {
            blocks
                .iter()
                .filter(|b| b["type"] == "text")
                .map(|b| b["text"].as_str().unwrap_or(""))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

pub fn session_to_claude(path: &Path, out_path: &Path) -> Result<Option<usize>, String> {
    if path.as_os_str().is_empty() || !path.exists() {
        return Ok(None);
    }
    let entries = records(path).collect::<Vec<_>>();
    let cwd = entries
        .first()
        .filter(|e| e["type"] == "session")
        .and_then(|e| e["cwd"].as_str());
    let mut out = BufWriter::new(File::create(out_path).map_err(|e| e.to_string())?);
    let mut n = 0;
    for entry in active_branch(&entries)
        .iter()
        .filter(|e| e["type"] == "message")
    {
        let message = &entry["message"];
        let event = match message["role"].as_str() {
            Some("user") => {
                json!({"type": "user", "message": {"role": "user", "content": text(&message["content"])}})
            }
            Some("assistant") => {
                let mut blocks = Vec::new();
                if let Some(content) = message["content"].as_array() {
                    for block in content {
                        if block["type"] == "toolCall" {
                            blocks.push(json!({"type": "tool_use", "id": block["id"], "name": block["name"],
                                "input": block.get("arguments").filter(|v| truthy(v)).cloned().unwrap_or(json!({}))}));
                            n += 1;
                        } else if block["type"] == "text" {
                            blocks.push(json!({"type": "text", "text": block.get("text").unwrap_or(&json!(""))}));
                        }
                    }
                }
                let mut msg = json!({"id": entry["id"], "role": "assistant", "content": blocks, "model": message["model"]});
                if let Some(mut usage) = usage(&message["usage"])? {
                    usage.as_object_mut().unwrap().remove("cost_usd");
                    msg["usage"] = usage;
                }
                let mut event = json!({"type": "assistant", "message": msg});
                if let Some(cwd) = cwd.filter(|s| !s.is_empty()) {
                    event["cwd"] = cwd.into();
                }
                event
            }
            Some("toolResult") => json!({"type": "user", "message": {"role": "user", "content": [{
                "type": "tool_result", "tool_use_id": message["toolCallId"], "content": text(&message["content"]),
                "is_error": truthy(&message["isError"])}]}}),
            _ => continue,
        };
        writeln!(out, "{}", json_text(&event)).map_err(|e| e.to_string())?;
    }
    out.flush().map_err(|e| e.to_string())?;
    Ok(Some(n))
}

pub fn json_reply(stdout: &str) -> Result<(String, Option<Value>), String> {
    let mut text_out = String::new();
    let mut totals = None::<Value>;
    for line in stdout.lines() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let message = &event["message"];
        if event["type"] != "message_end" || message["role"] != "assistant" {
            continue;
        }
        let response = text(&message["content"]);
        if !response.is_empty() {
            text_out = response;
        }
        if let Some(usage) = usage(&message["usage"])? {
            let total = totals.get_or_insert_with(|| json!({}));
            for (key, value) in usage.as_object().unwrap() {
                total[key] = (total[key].as_f64().unwrap_or(0.0) + value.as_f64().unwrap()).into();
            }
            if truthy(&message["model"]) {
                total["model"] = message["model"].clone();
            } else if total.get("model").is_none() {
                total["model"] = Value::Null;
            }
        }
    }
    Ok((text_out, totals))
}

pub fn session_usage(path: &Path) -> Result<Value, String> {
    let mut totals = json!({"input_tokens": 0, "cache_read_input_tokens": 0,
        "cache_creation_input_tokens": 0, "output_tokens": 0, "cache_creation_1h_input_tokens": 0});
    let mut cost = 0.0;
    let mut requests = 0;
    let mut models = Map::new();
    for entry in active_branch(&records(path).collect::<Vec<_>>()) {
        let usage = if entry["type"] == "message" && entry["message"]["role"] == "assistant" {
            let message = &entry["message"];
            if let Some(model) = message["model"].as_str().filter(|s| !s.is_empty()) {
                let n = models.get(model).and_then(Value::as_u64).unwrap_or(0) + 1;
                models.insert(model.into(), n.into());
            }
            usage(&message["usage"])?
        } else if ["usage", "compaction", "branch_summary"]
            .iter()
            .any(|t| entry["type"] == *t)
        {
            usage(&entry["usage"])?
        } else {
            None
        };
        if let Some(usage) = usage {
            requests += 1;
            cost += usage["cost_usd"].as_f64().unwrap();
            for (key, total) in totals.as_object_mut().unwrap() {
                *total = (total.as_f64().unwrap() + usage[key].as_f64().unwrap()).into();
            }
        }
    }
    let mut model = Value::Null;
    let mut most = 0;
    for (name, count) in models {
        if count.as_u64().unwrap() > most {
            most = count.as_u64().unwrap();
            model = name.into();
        }
    }
    totals["requests"] = requests.into();
    totals["cost_usd_vendor"] = round_decimal(cost, 6).into();
    totals["model"] = model;
    Ok(totals)
}
