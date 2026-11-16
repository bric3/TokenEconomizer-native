use std::{
    collections::{BTreeSet, HashMap},
    path::Path,
    sync::LazyLock,
};

use regex::Regex;
use serde_json::{Map, Value, json};

use super::{Call, jsonl::records};
use crate::value::{count, json_text, scalar_text, truthy};

pub fn event_calls(lines: &[String]) -> Vec<Call> {
    let mut out = Vec::new();
    let mut pending = HashMap::new();
    for line in lines {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if !event.is_object() {
            continue;
        }
        let data = &event["data"];
        if event["type"] == "tool.execution_start" {
            pending.insert(
                data["toolCallId"].to_string(),
                (
                    data["toolName"].clone(),
                    data.get("arguments").cloned().unwrap_or(json!({})),
                ),
            );
        } else if event["type"] == "tool.execution_complete" {
            let (name, args) = pending
                .get(&data["toolCallId"].to_string())
                .cloned()
                .unwrap_or((json!("?"), json!({})));
            out.push((
                name,
                args,
                data["result"]["content"].as_str().unwrap_or("").into(),
                data["success"].clone(),
            ));
        }
    }
    out
}

pub fn session_cwd(lines: &[String]) -> Option<String> {
    for line in lines.iter().filter(|l| l.contains("\"session.start\"")) {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if !event.is_object() {
            continue;
        }
        return event["data"]["context"]["cwd"].as_str().map(String::from);
    }
    None
}

pub fn session_meta(path: &Path) -> Value {
    for event in records(path) {
        if event["type"] == "session.start" {
            let data = &event["data"];
            return json!({"cwd": data["context"]["cwd"], "model": data["selectedModel"],
                "cli_version": data["copilotVersion"], "session_id": data["sessionId"]});
        }
    }
    json!({})
}

pub fn session_usage(path: &Path) -> Result<Value, String> {
    let mut totals = [0.0; 4];
    let mut requests = 0.0;
    let mut models = Map::new();
    let mut seen = false;
    let mut tool_defs = None;
    for event in records(path).filter(|e| e["type"] == "session.shutdown") {
        let data = &event["data"];
        if let Some(metrics) = data["modelMetrics"].as_object() {
            for (model, metric) in metrics {
                let usage = &metric["usage"];
                let input = count(&usage["inputTokens"], "inputTokens")?;
                let read = count(&usage["cacheReadTokens"], "cacheReadTokens")?;
                let write = count(&usage["cacheWriteTokens"], "cacheWriteTokens")?;
                totals[0] += (input - read - write).max(0.0);
                totals[1] += read;
                totals[2] += write;
                totals[3] += count(&usage["outputTokens"], "outputTokens")?;
                let n = count(&metric["requests"]["count"], "requests")?;
                requests += n;
                let previous = models.get(model).and_then(Value::as_f64).unwrap_or(0.0);
                models.insert(model.clone(), json!(previous + n));
                seen = true;
            }
        }
        if !data["toolDefinitionsTokens"].is_null() {
            tool_defs = Some(count(
                &data["toolDefinitionsTokens"],
                "toolDefinitionsTokens",
            )?);
        }
    }
    if !seen {
        return Ok(
            json!({"input_tokens": null, "cache_read_input_tokens": null,
            "cache_creation_input_tokens": null, "output_tokens": null, "requests": 0,
            "model": session_meta(path)["model"]}),
        );
    }
    let mut model = Value::Null;
    let mut most = f64::NEG_INFINITY;
    for (name, n) in models {
        if n.as_f64().unwrap() > most {
            most = n.as_f64().unwrap();
            model = name.into();
        }
    }
    Ok(
        json!({"input_tokens": totals[0], "cache_read_input_tokens": totals[1],
        "cache_creation_input_tokens": totals[2], "output_tokens": totals[3],
        "requests": requests, "model": model, "tool_definitions_tokens": tool_defs}),
    )
}

pub type TurnCall = (String, Value, String, Option<i64>);

pub fn turns(path: &Path) -> Vec<Vec<TurnCall>> {
    static EXIT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"<shellId: [^>]*?completed with exit code (-?\d+)>").unwrap());
    let mut turns = Vec::new();
    let mut current = None::<Vec<TurnCall>>;
    let mut pending = HashMap::new();
    for event in records(path) {
        let data = &event["data"];
        if event["type"] == "user.message" {
            if let Some(previous) = current.replace(Vec::new()).filter(|t| !t.is_empty()) {
                turns.push(previous);
            }
            pending.clear();
        } else if event["type"] == "tool.execution_start" && current.is_some() {
            let name = if truthy(&data["toolName"]) {
                scalar_text(&data["toolName"])
            } else {
                String::new()
            };
            let args = data["arguments"]
                .as_object()
                .cloned()
                .map(Value::Object)
                .unwrap_or(json!({}));
            pending.insert(data["toolCallId"].to_string(), (name, args));
        } else if event["type"] == "tool.execution_complete" {
            let Some(current) = current.as_mut() else {
                continue;
            };
            let Some((name, args)) = pending.remove(&data["toolCallId"].to_string()) else {
                continue;
            };
            let result = &data["result"];
            let mut text = if result.is_object() || !truthy(result) {
                let content = &result["content"];
                content
                    .as_str()
                    .map(String::from)
                    .unwrap_or_else(|| json_text(content))
            } else {
                scalar_text(result)
            };
            if data.get("success").is_some_and(|s| !truthy(s))
                && (data["error"].is_string() || data["error"].is_object())
            {
                text.push('\n');
                text.push_str(&json_text(&data["error"]));
            }
            let tail = text
                .chars()
                .rev()
                .take(400)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>();
            let code = EXIT.captures(&tail).and_then(|c| c[1].parse::<i64>().ok());
            current.push((name, args, text, code));
        }
    }
    if let Some(current) = current.filter(|t| !t.is_empty()) {
        turns.push(current);
    }
    turns
}

pub fn prompt_count(path: &Path) -> usize {
    records(path)
        .filter(|e| e["type"] == "user.message")
        .count()
}

pub fn mcp_servers(env: &Value, home: &Path) -> BTreeSet<String> {
    let root = env["COPILOT_HOME"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(Path::new)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| home.join(".copilot"));
    let mut names = BTreeSet::from(["github-mcp-server".into()]);
    if let Ok(bytes) = std::fs::read(root.join("mcp-config.json")) {
        if let Ok(config) = serde_json::from_slice::<Value>(&bytes) {
            if let Some(servers) = config["mcpServers"].as_object() {
                names.extend(servers.keys().cloned());
            }
        }
    }
    names
}

pub fn mcp_server_of<'a>(tool: &str, servers: &'a [String]) -> Option<&'a str> {
    let mut best = None::<&str>;
    for server in servers {
        if tool.starts_with(&format!("{server}-")) && best.is_none_or(|b| server.len() > b.len()) {
            best = Some(server);
        }
    }
    best
}
