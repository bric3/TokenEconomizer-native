use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

use serde_json::{Map, Value, json};

use crate::value::{count, float_or_zero, json_text, round_decimal, scalar_text, truthy};

fn content_text(content: &Value) -> String {
    if let Some(blocks) = content.as_array() {
        blocks
            .iter()
            .filter(|b| b.is_object())
            .map(|b| b["text"].as_str().unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n")
    } else if truthy(content) {
        scalar_text(content)
    } else {
        String::new()
    }
}

pub fn nested(message: &Value, cwd: Option<&str>) -> Value {
    if message.get("info").is_some() || !["user", "assistant"].iter().any(|t| message["type"] == *t)
    {
        return message.clone();
    }
    let mut info = json!({"id": message["id"], "role": message["type"]});
    if message["type"] == "user" {
        return json!({"info": info, "parts": [{"type": "text", "text": if truthy(&message["text"]) { scalar_text(&message["text"]) } else { String::new() }}]});
    }
    info["modelID"] = message["model"]["id"].clone();
    info["cost"] = message["cost"].clone();
    info["tokens"] = message["tokens"].clone();
    if let Some(cwd) = cwd.filter(|s| !s.is_empty()) {
        info["path"] = json!({"cwd": cwd});
    }
    let mut parts = Vec::new();
    if let Some(content) = message["content"].as_array() {
        for part in content.iter().filter(|p| p.is_object()) {
            if part["type"] == "text" {
                parts.push(json!({"type": "text", "text": if truthy(&part["text"]) { scalar_text(&part["text"]) } else { String::new() }}));
            } else if part["type"] == "tool" {
                let state = &part["state"];
                let mut normalized = json!({"status": state["status"], "input": state.get("input").filter(|v| truthy(v)).cloned().unwrap_or(json!({}))});
                if state["status"] == "error" {
                    let error = if state["error"].is_object() {
                        &state["error"]["message"]
                    } else {
                        &state["error"]
                    };
                    let error = if truthy(error) {
                        scalar_text(error)
                    } else {
                        content_text(&state["content"])
                    };
                    normalized["error"] = error.into();
                } else {
                    normalized["output"] = content_text(&state["content"]).into();
                }
                parts.push(json!({"type": "tool", "callID": part["id"], "tool": part["name"], "state": normalized}));
            }
        }
    }
    json!({"info": info, "parts": parts})
}

pub fn load(path: &Path) -> (Value, Vec<Value>) {
    let Some(data) = std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
    else {
        return (json!({}), Vec::new());
    };
    if let Some(messages) = data.as_array() {
        return (
            json!({}),
            messages
                .iter()
                .filter(|m| m.is_object())
                .map(|m| nested(m, None))
                .collect(),
        );
    }
    let info = data["info"]
        .as_object()
        .cloned()
        .map(Value::Object)
        .unwrap_or(json!({}));
    let messages = data["messages"]
        .as_array()
        .map(|messages| {
            messages
                .iter()
                .filter(|m| m.is_object())
                .map(|m| nested(m, info["directory"].as_str()))
                .collect()
        })
        .unwrap_or_default();
    (info, messages)
}

pub fn text(message: &Value) -> String {
    message["parts"]
        .as_array()
        .map(|parts| {
            parts
                .iter()
                .filter(|p| {
                    p["type"] == "text" && !truthy(&p["synthetic"]) && !truthy(&p["ignored"])
                })
                .map(|p| p["text"].as_str().unwrap_or(""))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

pub fn session_info(path: &Path) -> Value {
    load(path).0
}

pub fn prompt_count(path: &Path) -> usize {
    load(path)
        .1
        .iter()
        .filter(|m| m["info"]["role"] == "user" && !text(m).trim().is_empty())
        .count()
}

pub fn usage(info: &Value) -> Result<Option<Value>, String> {
    let tokens = &info["tokens"];
    if !tokens.is_object() {
        return Ok(None);
    }
    Ok(Some(
        json!({"input_tokens": count(&tokens["input"], "input")?,
        "cache_read_input_tokens": count(&tokens["cache"]["read"], "cache.read")?,
        "cache_creation_input_tokens": count(&tokens["cache"]["write"], "cache.write")?,
        "output_tokens": count(&tokens["output"], "output")? + count(&tokens["reasoning"], "reasoning")?,
        "cost_usd": float_or_zero(&info["cost"])?}),
    ))
}

pub fn session_to_claude(path: &Path, out_path: &Path) -> Result<Option<usize>, String> {
    if path.as_os_str().is_empty() || !path.exists() {
        return Ok(None);
    }
    let (_, messages) = load(path);
    if messages.is_empty() {
        return Ok(None);
    }
    let mut out = BufWriter::new(File::create(out_path).map_err(|e| e.to_string())?);
    let mut n = 0;
    for message in messages {
        let info = &message["info"];
        if info["role"] == "user" {
            let text = text(&message);
            if !text.trim().is_empty() {
                writeln!(
                    out,
                    "{}",
                    json_text(
                        &json!({"type": "user", "message": {"role": "user", "content": text}})
                    )
                )
                .map_err(|e| e.to_string())?;
            }
            continue;
        }
        if info["role"] != "assistant" {
            continue;
        }
        let mut blocks = Vec::new();
        let mut results = Vec::new();
        if let Some(parts) = message["parts"].as_array() {
            for part in parts {
                if part["type"] == "text" && truthy(&part["text"]) {
                    blocks.push(json!({"type": "text", "text": part["text"]}));
                } else if part["type"] == "tool" {
                    let state = &part["state"];
                    blocks.push(json!({"type": "tool_use", "id": part["callID"], "name": part["tool"],
                        "input": state.get("input").filter(|v| truthy(v)).cloned().unwrap_or(json!({}))}));
                    n += 1;
                    let key = if state["status"] == "completed" {
                        "output"
                    } else if state["status"] == "error" {
                        "error"
                    } else {
                        continue;
                    };
                    results.push((
                        part["callID"].clone(),
                        if truthy(&state[key]) {
                            scalar_text(&state[key])
                        } else {
                            String::new()
                        },
                        key == "error",
                    ));
                }
            }
        }
        let mut msg = json!({"id": info["id"], "role": "assistant", "content": blocks, "model": info["modelID"]});
        if let Some(mut usage) = usage(info)? {
            usage.as_object_mut().unwrap().remove("cost_usd");
            msg["usage"] = usage;
        }
        let mut event = json!({"type": "assistant", "message": msg});
        if let Some(cwd) = info["path"]["cwd"].as_str().filter(|s| !s.is_empty()) {
            event["cwd"] = cwd.into();
        }
        writeln!(out, "{}", json_text(&event)).map_err(|e| e.to_string())?;
        for (id, text, error) in results {
            writeln!(
                out,
                "{}",
                json_text(
                    &json!({"type": "user", "message": {"role": "user", "content": [{
                "type": "tool_result", "tool_use_id": id, "content": text, "is_error": error}]}})
                )
            )
            .map_err(|e| e.to_string())?;
        }
    }
    out.flush().map_err(|e| e.to_string())?;
    Ok(Some(n))
}

pub fn session_usage(path: &Path) -> Result<Value, String> {
    let mut totals = json!({"input_tokens": 0, "cache_read_input_tokens": 0, "cache_creation_input_tokens": 0, "output_tokens": 0});
    let mut cost = 0.0;
    let mut requests = 0;
    let mut models = Map::new();
    for message in load(path).1 {
        let info = &message["info"];
        if info["role"] != "assistant" {
            continue;
        }
        let Some(usage) = usage(info)? else {
            continue;
        };
        requests += 1;
        cost += usage["cost_usd"].as_f64().unwrap();
        for (key, total) in totals.as_object_mut().unwrap() {
            *total = (total.as_f64().unwrap() + usage[key].as_f64().unwrap()).into();
        }
        if let Some(model) = info["modelID"].as_str().filter(|s| !s.is_empty()) {
            let n = models.get(model).and_then(Value::as_u64).unwrap_or(0) + 1;
            models.insert(model.into(), n.into());
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

pub fn run_reply(stdout: &str) -> Result<(String, Option<Value>, String), String> {
    let mut text = String::new();
    let mut totals = None::<Value>;
    let mut session_id = String::new();
    for line in stdout.lines() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if !event.is_object() {
            continue;
        }
        if session_id.is_empty() && truthy(&event["sessionID"]) {
            session_id = scalar_text(&event["sessionID"]);
        }
        let part = &event["part"];
        if event["type"] == "text" {
            if let Some(response) = part["text"].as_str() {
                text.push_str(response);
            }
        } else if event["type"] == "step_finish"
            && let Some(usage) = usage(part)?
        {
            let total = totals.get_or_insert_with(|| json!({}));
            for (key, value) in usage.as_object().unwrap() {
                total[key] = (total[key].as_f64().unwrap_or(0.0) + value.as_f64().unwrap()).into();
            }
        }
    }
    Ok((text.trim().into(), totals, session_id))
}
