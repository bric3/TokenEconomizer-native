use super::{EXIT_RE, RESULT_CAP, claude_success_code};
use crate::{
    trace::{self, codex, grok, hermes},
    value::{scalar_text, truthy},
};
use regex::Regex;
use serde_json::{Value, json};
use std::sync::LazyLock;

pub const PROGRESS: &str = "_progress";
pub const HOSTS: [&str; 10] = [
    "claude",
    "codex",
    "copilot",
    "droid",
    "opencode",
    "pi",
    "cursor",
    "hermes",
    "grok",
    "antigravity",
];
const RESPONSE_KEYS: [&str; 10] = [
    "stdout",
    "stderr",
    "output",
    "aggregated_output",
    "formatted_output",
    "content",
    "result",
    "error",
    "message",
    "text_result_for_llm",
];
static COPILOT_EXIT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<shellId: [^>]*?completed with exit code (-?\d+)>").unwrap());
static PATCH_FILE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\*\*\* (?:Update|Add|Delete) File:\s*(.+?)\s*$").unwrap());

fn cap_text(text: &str, cap: i64) -> String {
    let end = if cap < 0 {
        text.chars()
            .count()
            .saturating_sub(cap.unsigned_abs() as usize)
    } else {
        cap as usize
    };
    text.chars().take(end).collect()
}
pub fn response_text(resp: &Value, cap: i64) -> Result<String, String> {
    let texts = |items: &[Value], non_objects: bool| -> Result<String, String> {
        items
            .iter()
            .filter(|v| non_objects || v.is_object())
            .map(|v| {
                if v.is_object() {
                    v.get("text")
                        .unwrap_or(&json!(""))
                        .as_str()
                        .map(String::from)
                        .ok_or_else(|| "response text must be a string".into())
                } else {
                    Ok(scalar_text(v))
                }
            })
            .collect::<Result<Vec<_>, String>>()
            .map(|parts| parts.join("\n"))
    };
    let text = match resp {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Array(items) => texts(items, true)?,
        Value::Object(obj) => {
            if obj.contains_key("structuredPatch") {
                return Ok(String::new());
            }
            let mut parts = Vec::new();
            for key in RESPONSE_KEYS {
                match &resp[key] {
                    Value::String(s) if !s.is_empty() => parts.push(s.clone()),
                    Value::Array(items) => parts.push(texts(items, false)?),
                    _ => {}
                }
            }
            parts.join("\n")
        }
        _ => scalar_text(resp),
    };
    Ok(cap_text(&text, cap))
}
pub fn merge_outputs(outputs: &[Value]) -> Result<Value, String> {
    let progress = |value: &Value| -> Result<Vec<Value>, String> {
        match value {
            Value::Array(items) => Ok(items.clone()),
            Value::String(text) => Ok(text.chars().map(|c| json!(c.to_string())).collect()),
            Value::Object(items) => Ok(items.keys().map(|key| json!(key)).collect()),
            _ => Err("progress must be iterable".into()),
        }
    };
    let mut out = json!({});
    for item in outputs.iter().filter(|v| truthy(v)) {
        for (key, value) in item.as_object().ok_or("hook output must be an object")? {
            let prior = &out[key];
            let merged = match key.as_str() {
                "systemMessage" | "additionalContext" | "additional_context" if truthy(prior) => {
                    json!(format!(
                        "{}{}{}",
                        prior.as_str().ok_or("hook message must be text")?,
                        if key == "systemMessage" { "\n" } else { "\n\n" },
                        value.as_str().ok_or("hook message must be text")?
                    ))
                }
                PROGRESS => {
                    let mut lines = if truthy(prior) {
                        progress(prior)?
                    } else {
                        Vec::new()
                    };
                    lines.extend(progress(value)?);
                    json!(lines)
                }
                "hookSpecificOutput" if truthy(prior) => {
                    let mut data = prior
                        .as_object()
                        .ok_or("hookSpecificOutput must be an object")?
                        .clone();
                    data.extend(
                        value
                            .as_object()
                            .ok_or("hookSpecificOutput must be an object")?
                            .clone(),
                    );
                    if truthy(&prior["additionalContext"]) && truthy(&value["additionalContext"]) {
                        data.insert(
                            "additionalContext".into(),
                            json!(format!(
                                "{}\n\n{}",
                                prior["additionalContext"]
                                    .as_str()
                                    .ok_or("context must be text")?,
                                value["additionalContext"]
                                    .as_str()
                                    .ok_or("context must be text")?
                            )),
                        );
                    }
                    json!(data)
                }
                _ => value.clone(),
            };
            out[key] = merged;
        }
    }
    Ok(out)
}
fn parsed_code(captures: Option<regex::Captures<'_>>) -> Value {
    captures
        .and_then(|c| c[1].parse::<i64>().ok())
        .map(|code| json!(code))
        .unwrap_or(Value::Null)
}
pub fn failure_response(payload: &Value) -> Value {
    if !payload["tool_response"].is_null()
        || !payload["tool_result"].is_null()
        || payload["error"].is_null()
    {
        return Value::Null;
    }
    let text = scalar_text(&payload["error"]);
    let code = parsed_code(EXIT_RE.captures(&cap_text(&text, 400)));
    let mut result = json!({"error":text,"result_type":"failure"});
    if !code.is_null() {
        result["exit_code"] = code;
    }
    result
}
pub fn exit_code(resp: &Value) -> Value {
    if resp.is_object() {
        for key in ["exit_code", "exitCode", "returncode", "status_code"] {
            if resp[key].is_i64() || resp[key].is_u64() {
                return resp[key].clone();
            }
        }
        let meta = &resp["metadata"]["exit_code"];
        if meta.is_i64() || meta.is_u64() || meta.is_boolean() {
            return meta.clone();
        }
    }
    let text = resp.as_str().or_else(|| {
        ["output", "text_result_for_llm"]
            .into_iter()
            .find_map(|k| resp[k].as_str())
    });
    if let Some(text) = text.filter(|s| !s.is_empty()) {
        let code = parsed_code(EXIT_RE.captures(&cap_text(text, 400)));
        if !code.is_null() {
            return code;
        }
        let tail: String = text
            .chars()
            .skip(text.chars().count().saturating_sub(400))
            .collect();
        return parsed_code(COPILOT_EXIT_RE.captures(&tail));
    }
    Value::Null
}
pub fn tool_call(host: &str, payload: &Value) -> Result<trace::Call, String> {
    let original_name = scalar_text(first(&[&payload["tool_name"], &json!("")]));
    if host == "hermes" {
        let (name, args) = hermes::tool(&json!(original_name), &payload["tool_input"]);
        let resp = &payload["tool_response"];
        let (text, code, _) = hermes::result(if resp.is_object() {
            &resp["content"]
        } else {
            resp
        });
        return Ok((
            name,
            args,
            response_text(&json!(text), RESULT_CAP as i64)?,
            json!(code),
        ));
    }
    let failure = failure_response(payload);
    let resp = if truthy(&failure) {
        &failure
    } else {
        &payload[if host == "copilot" {
            "tool_result"
        } else {
            "tool_response"
        }]
    };
    if host == "grok" {
        let (name, args) = grok::tool(&json!(original_name), &payload["tool_input"]);
        if let Some((text, code)) = grok::result(resp) {
            return Ok((name, args, cap_text(&text, RESULT_CAP as i64), json!(code)));
        }
        return Ok((
            name,
            args,
            response_text(resp, RESULT_CAP as i64)?,
            exit_code(resp),
        ));
    }
    let mut name = original_name;
    let mut args = if payload["tool_input"].is_object() {
        payload["tool_input"].clone()
    } else {
        json!({})
    };
    let text = response_text(resp, RESULT_CAP as i64)?;
    let mut code = exit_code(resp);
    let low = name.to_lowercase();
    match host {
        "claude" if code.is_null() && failure.is_null() => {
            code = json!(claude_success_code(
                &name,
                &args,
                Some(&payload["tool_response"])
            )?)
        }
        "codex"
            if [
                "bash",
                "shell",
                "exec_command",
                "local_shell",
                "unified_exec",
                "container.exec",
                "exec",
            ]
            .contains(&low.as_str()) =>
        {
            name = "bash".into();
            args =
                json!({"command":codex::command_text(args.get("command").unwrap_or(&args["cmd"]))});
        }
        "droid" if ["execute", "script"].contains(&low.as_str()) => {
            name = "bash".into();
            args = json!({"command":scalar_text(first(&[args.get("command").unwrap_or(&args["script"]),&json!("")]))});
        }
        "droid"
            if ["create", "write", "edit", "multiedit", "str_replace"].contains(&low.as_str()) =>
        {
            name = if ["create", "write"].contains(&low.as_str()) {
                "create"
            } else {
                "edit"
            }
            .into();
            args["path"] = first(&[&args["file_path"], &args["path"], &json!("")]).clone();
        }
        "codex" | "droid" if low == "apply_patch" || host == "droid" && low == "applypatch" => {
            let patch = ["input", "patch", "command"]
                .into_iter()
                .find_map(|k| args[k].as_str())
                .unwrap_or("");
            let paths: Vec<_> = PATCH_FILE_RE
                .captures_iter(patch)
                .map(|c| c[1].to_string())
                .collect();
            name = if host == "codex" {
                "apply_patch"
            } else {
                "edit"
            }
            .into();
            args = json!({"path":paths.first().cloned().unwrap_or_default(),"paths":paths});
        }
        "cursor" if low == "shell" => name = "Bash".into(),
        "antigravity"
            if code.is_null()
                && payload["tool_response"].is_object()
                && truthy(&payload["tool_response"]["isError"]) =>
        {
            code = json!(1)
        }
        _ => {}
    }
    Ok((json!(name), args, text, code))
}
fn first<'a>(values: &[&'a Value]) -> &'a Value {
    values
        .iter()
        .find(|v| truthy(v))
        .copied()
        .unwrap_or_else(|| values.last().copied().unwrap())
}
pub fn session_id(host: &str, payload: &Value) -> String {
    let alias = match host {
        "cursor" => "conversation_id",
        "antigravity" => "conversationId",
        _ => "session_id",
    };
    scalar_text(first(&[
        &payload["session_id"],
        &payload[alias],
        &json!(""),
    ]))
}
pub fn turn_key(host: &str, payload: &Value) -> String {
    let keys: &[&str] = match host {
        "codex" => &["turn_id"],
        "cursor" => &["generation_id", "prompt_id"],
        "grok" => &["promptId", "prompt_id"],
        "claude" | "pi" | "opencode" | "hermes" | "antigravity" => &["prompt_id"],
        _ => &[],
    };
    keys.iter()
        .find_map(|key| payload.get(*key).filter(|v| truthy(v)))
        .map(scalar_text)
        .unwrap_or_default()
}
pub fn env_session_id(host: &str, env: &Value) -> String {
    let alias = match host {
        "claude" => "CLAUDE_CODE_SESSION_ID",
        "grok" => "GROK_SESSION_ID",
        "droid" => "FACTORY_SESSION_ID",
        _ => "TE_SESSION_ID",
    };
    scalar_text(first(&[&env["TE_SESSION_ID"], &env[alias], &json!("")]))
}
pub fn is_subagent(host: &str, payload: &Value) -> bool {
    truthy(&payload["agent_id"]) || host == "grok" && truthy(&payload["subagentType"])
}
pub fn context(host: &str, event: &str, text: &str) -> Value {
    match host {
        "cursor" => json!({"additional_context":text}),
        "antigravity" | "copilot" => json!({"additionalContext":text}),
        _ => json!({"hookSpecificOutput":{"hookEventName":event,"additionalContext":text}}),
    }
}
pub fn relay(text: &str) -> String {
    format!(
        "TokenEconomizer, a plugin the user runs, has a notice for them, and their main view does not show plugin messages. Put this line, word for word, in your next message to the user, and again in your final reply:\n\n{text}\n\nIt is for the user to act on; it does not change your task."
    )
}
pub fn notice(host: &str, text: &str) -> Value {
    match host {
        "cursor" => json!({"additional_context":relay(text)}),
        "antigravity" => json!({"additionalContext":relay(text)}),
        "copilot" => json!({PROGRESS:[text]}),
        "droid" => json!({}),
        _ => json!({"systemMessage":text}),
    }
}
pub fn stopped_normally(host: &str, payload: &Value) -> bool {
    match host {
        "cursor" => payload["status"] == "completed",
        "antigravity" => payload["terminationReason"] == "NO_TOOL_CALL",
        "grok" => payload["reason"] == "end_turn",
        _ => true,
    }
}
pub fn stop_hint<'a>(headless: &Value, interrupt: &'a str, end_run: &'a str) -> &'a str {
    if truthy(headless) { end_run } else { interrupt }
}
pub fn completed_at_end(host: &str, payload: &Value) -> bool {
    host == "cursor" && first(&[&payload["final_status"], &payload["reason"]]) == "completed"
}
pub fn default_host(env: &Value, recent: Option<&str>, registered: &[String]) -> String {
    if let Some(host) = env["TE_HOST"]
        .as_str()
        .filter(|s| registered.iter().any(|name| name == s))
    {
        return host.into();
    }
    if env["CLAUDECODE"] == "1" {
        return "claude".into();
    }
    if env["COPILOT_CLI"] == "1" {
        return "copilot".into();
    }
    if truthy(&env["CURSOR_AGENT"]) || truthy(&env["CURSOR_PROJECT_DIR"]) {
        return "cursor".into();
    }
    if truthy(&env["GROK_SESSION_ID"]) {
        return "grok".into();
    }
    if truthy(&env["FACTORY_SESSION_ID"]) || truthy(&env["DROID_PLUGIN_ROOT"]) {
        return "droid".into();
    }
    recent
        .filter(|s| registered.iter().any(|name| name == s))
        .unwrap_or("claude")
        .into()
}
pub fn detect(payload: &Value, env: &Value, hint: Option<&str>, registered: &[String]) -> String {
    if let Some(host) = hint.filter(|s| registered.iter().any(|name| name == s)) {
        return host.into();
    }
    if truthy(&payload["conversationId"])
        && scalar_text(first(&[&payload["transcriptPath"], &json!("")]))
            .replace('\\', "/")
            .contains("/antigravity")
    {
        return "antigravity".into();
    }
    let tp = scalar_text(first(&[&payload["transcript_path"], &json!("")]));
    let sp = tp.replace('\\', "/");
    if payload.get("hookEventName").is_some()
        || sp.contains("/.grok/sessions/")
        || tp.is_empty() && (truthy(&env["GROK_HOOK_EVENT"]) || truthy(&env["GROK_SESSION_ID"]))
    {
        return "grok".into();
    }
    if payload.get("turn_id").is_some()
        || sp.rsplit('/').next().unwrap_or("").starts_with("rollout-")
        || sp.contains("/.codex/")
    {
        return "codex".into();
    }
    if sp.contains("/session-state/") && tp.ends_with("events.jsonl") {
        return "copilot".into();
    }
    if sp.contains("/.factory/sessions/") || tp.is_empty() && truthy(&env["DROID_PLUGIN_ROOT"]) {
        return "droid".into();
    }
    if tp.is_empty() && truthy(&env["COPILOT_PLUGIN_ROOT"]) {
        return "copilot".into();
    }
    "claude".into()
}
