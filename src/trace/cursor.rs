use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
};

use serde_json::{Value, json};

use super::jsonl::records;
use crate::value::{json_text_utf8, scalar_text, truthy};

pub fn response_text(response: &Value) -> String {
    if let Some(text) = response.as_str() {
        return text.into();
    }
    if response.is_object() {
        return ["content", "stdout", "stderr", "output", "error"]
            .iter()
            .filter_map(|key| response.get(*key).filter(|v| !v.is_null()))
            .map(scalar_text)
            .collect::<Vec<_>>()
            .join("\n");
    }
    if truthy(response) {
        scalar_text(response)
    } else {
        String::new()
    }
}

pub fn exit_code(response: &Value) -> Option<i64> {
    ["exit_code", "exitCode", "returncode", "status_code"]
        .iter()
        .find_map(|key| response[*key].as_i64())
}

pub fn events_to_claude(path: &Path, out_path: &Path) -> Option<usize> {
    let mut out = Vec::new();
    for event in records(path) {
        let name = event["hook_event_name"].as_str().unwrap_or("");
        if name == "UserPromptSubmit" {
            if let Some(prompt) = event["prompt"].as_str().filter(|s| !s.trim().is_empty()) {
                out.push(json!({"type": "user", "message": {"content": [{"type": "text", "text": prompt}]}}));
            }
        } else if ["PostToolUse", "PostToolUseFailure"].contains(&name) {
            let id = event
                .get("tool_use_id")
                .filter(|v| truthy(v))
                .or_else(|| event.get("generation_id").filter(|v| truthy(v)))
                .map(scalar_text)
                .unwrap_or_else(|| out.len().to_string());
            let tool = event
                .get("tool_name")
                .filter(|v| truthy(v))
                .map(scalar_text)
                .unwrap_or("unknown".into());
            let tool = if tool.to_lowercase() == "shell" {
                "Bash".into()
            } else {
                tool
            };
            let args = event["tool_input"]
                .as_object()
                .cloned()
                .map(Value::Object)
                .unwrap_or(json!({}));
            out.push(
                json!({"type": "assistant", "cwd": event["cwd"], "message": {"content": [{
                "type": "tool_use", "id": id, "name": tool, "input": args}]}}),
            );
            let response = if name == "PostToolUseFailure" {
                json!({"content": event.get("error").filter(|v| truthy(v)).cloned().unwrap_or(json!("tool failed")), "isError": true})
            } else {
                event["tool_response"].clone()
            };
            let failed =
                name == "PostToolUseFailure" || exit_code(&response).is_some_and(|code| code != 0);
            out.push(json!({"type": "user", "message": {"content": [{"type": "tool_result", "tool_use_id": id,
                "content": response_text(&response), "is_error": failed}]}}));
        }
    }
    let mut file = BufWriter::new(File::create(out_path).ok()?);
    for record in &out {
        writeln!(file, "{}", json_text_utf8(record)).ok()?;
    }
    file.flush().ok()?;
    Some(out.len())
}

pub fn prompt_count(path: &Path) -> usize {
    let mut count = 0;
    let mut agent = None;
    for event in records(path) {
        count += usize::from(event["hook_event_name"] == "UserPromptSubmit");
        if let Some(path) = event["agent_transcript"].as_str() {
            agent = Some(path.to_owned());
        }
    }
    let Some(agent) = agent.filter(|s| !s.is_empty()) else {
        return count;
    };
    let agent = Path::new(&agent);
    if count > 0
        || (agent
            .canonicalize()
            .ok()
            .zip(path.canonicalize().ok())
            .is_some_and(|(a, b)| a == b))
    {
        return count;
    }
    count + records(agent).filter(|e| e["role"] == "user").count()
}
