use super::{
    RESULT_CAP, exit_code, is_subagent,
    session::{self, Environment, Error, Result},
    session_id,
};
use crate::{
    plugin::paths,
    trace::{antigravity, claude},
    value::{int_value, json_text, json_text_utf8, scalar_text, truthy},
};
use regex::Regex;
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::LazyLock,
};

pub const STATE_DIR: &str = "antigravity-sessions";
pub const STATE_KEEP_S: f64 = 30.0 * 86400.0;
pub const CONTEXT_CAP: i64 = 10000;

fn text(value: &Value) -> String {
    if truthy(value) {
        scalar_text(value)
    } else {
        String::new()
    }
}
fn take(value: &str, cap: usize) -> String {
    value.chars().take(cap).collect()
}
fn integer(value: &Value) -> Result<i64> {
    // shortcut: state counters use i64, extend their range during the numeric parity audit.
    if let Some(n) = value.as_i64() {
        return Ok(n);
    }
    if let Some(n) = value.as_str().and_then(|s| s.trim().parse::<i64>().ok()) {
        return Ok(n);
    }
    int_value(value)
        .filter(|n| *n >= i64::MIN as f64 && *n < -(i64::MIN as f64))
        .map(|n| n as i64)
        .ok_or_else(|| Error::Invalid(format!("invalid state counter: {value}")))
}
fn private_dir(path: &Path) -> io::Result<()> {
    let mut dirs = fs::DirBuilder::new();
    dirs.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        dirs.mode(0o700);
    }
    dirs.create(path)
}
fn private_file(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}
pub fn prepare(
    host: &str,
    payload: &Value,
    env: &Value,
    environment: &Environment,
) -> Result<Value> {
    let mut p = if truthy(payload) {
        payload
            .as_object()
            .cloned()
            .ok_or_else(|| Error::Invalid("hook payload must be an object".into()))?
    } else {
        Default::default()
    };
    if host == "grok" {
        let raw = p
            .get("hookEventName")
            .filter(|v| truthy(v))
            .or_else(|| p.get("hook_event_name"))
            .map(text)
            .unwrap_or_default();
        let mut event = match raw.as_str() {
            "session_start" | "SessionStart" => "SessionStart",
            "user_prompt_submit" | "UserPromptSubmit" => "UserPromptSubmit",
            "post_tool_use" | "PostToolUse" => "PostToolUse",
            "post_tool_use_failure" | "PostToolUseFailure" => "PostToolUseFailure",
            "stop" | "Stop" => "Stop",
            "stop_cancelled" | "StopCancelled" => "StopCancelled",
            "stop_failure" | "StopFailure" => "StopFailure",
            "session_end" | "SessionEnd" => "SessionEnd",
            _ => "",
        };
        for (alias, own) in [
            ("session_id", "sessionId"),
            ("transcript_path", "transcriptPath"),
            ("tool_name", "toolName"),
            ("tool_input", "toolInput"),
            ("tool_response", "toolResult"),
            ("prompt_id", "promptId"),
        ] {
            if p.get(alias).is_none_or(|v| v.is_null() || v == "") {
                p.insert(alias.into(), p.get(own).cloned().unwrap_or(Value::Null));
            }
        }
        if !p.get("session_id").is_some_and(truthy) {
            p.insert(
                "session_id".into(),
                env.get("GROK_SESSION_ID")
                    .filter(|v| truthy(v))
                    .cloned()
                    .unwrap_or(json!("")),
            );
        }
        if event == "Stop" {
            if p.get("reason").is_none_or(|v| v != "end_turn")
                || !p.get("prompt_id").is_some_and(truthy)
            {
                event = "";
            }
        } else if event == "StopCancelled" || event == "StopFailure" {
            p.insert(
                "reason".into(),
                json!(if event == "StopFailure" {
                    "api_error".into()
                } else {
                    p.get("reason")
                        .filter(|v| truthy(v))
                        .map(scalar_text)
                        .unwrap_or("cancelled".into())
                }),
            );
            event = "Stop";
        }
        p.insert("hook_event_name".into(), json!(event));
        return Ok(json!(p));
    }
    if host != "cursor" {
        return Ok(json!(p));
    }
    let sid = p
        .get("conversation_id")
        .filter(|v| truthy(v))
        .or_else(|| p.get("session_id").filter(|v| truthy(v)))
        .cloned()
        .unwrap_or(json!(""));
    p.insert("session_id".into(), sid.clone());
    let cwd = if let Some(cwd) = p
        .get("cwd")
        .filter(|v| truthy(v))
        .or_else(|| env.get("CURSOR_PROJECT_DIR").filter(|v| truthy(v)))
    {
        cwd.clone()
    } else {
        let root = match p.get("workspace_roots").filter(|v| truthy(v)) {
            Some(Value::Array(a)) => a.first().cloned().unwrap_or(json!("")),
            Some(Value::String(s)) => {
                json!(s.chars().next().map(|c| c.to_string()).unwrap_or_default())
            }
            Some(_) => return Err(Error::Invalid("workspace_roots must be indexable".into())),
            None => json!(""),
        };
        if truthy(&root) {
            root
        } else {
            json!(environment.cwd)
        }
    };
    p.insert("cwd".into(), cwd);
    let event = match p.get("hook_event_name").and_then(Value::as_str) {
        Some("sessionStart") => "SessionStart",
        Some("beforeSubmitPrompt") => "UserPromptSubmit",
        Some("postToolUse") => "PostToolUse",
        Some("postToolUseFailure") => "PostToolUseFailure",
        Some("stop") => "Stop",
        Some("sessionEnd") => "SessionEnd",
        _ => "",
    };
    p.insert("hook_event_name".into(), json!(event));
    if truthy(&sid) {
        if let Some(agent) = p
            .get("transcript_path")
            .and_then(Value::as_str)
            .filter(|s| s.ends_with(".jsonl"))
        {
            p.insert("agent_transcript".into(), json!(agent));
        }
        let sid = sid
            .as_str()
            .ok_or_else(|| Error::Invalid("session id must be a path".into()))?;
        let path = paths::state_root(
            env.as_object()
                .ok_or_else(|| Error::Invalid("environment must be an object".into()))?,
            &environment.home,
        )?
        .join("cursor-sessions")
        .join(format!("{}.jsonl", session::basename(sid)));
        p.insert("transcript_path".into(), json!(path));
    }
    if event == "SessionStart" {
        p.insert("source".into(), json!("startup"));
    }
    if event == "UserPromptSubmit" {
        p.insert(
            "prompt_id".into(),
            p.get("generation_id")
                .filter(|v| truthy(v))
                .cloned()
                .unwrap_or(json!("")),
        );
    }
    if event == "PostToolUse" || event == "PostToolUseFailure" {
        p.insert(
            "tool_use_id".into(),
            p.get("tool_use_id")
                .filter(|v| truthy(v))
                .or_else(|| p.get("generation_id").filter(|v| truthy(v)))
                .cloned()
                .unwrap_or(json!("")),
        );
        if event == "PostToolUse" {
            let raw = p.get("tool_output").cloned().unwrap_or(Value::Null);
            let output = raw
                .as_str()
                .and_then(|s| serde_json::from_str::<Value>(s).ok())
                .unwrap_or(raw);
            let response = if output.is_object() {
                let content = ["stdout", "stderr", "output"]
                    .into_iter()
                    .filter_map(|k| output.get(k).filter(|v| !v.is_null()))
                    .map(scalar_text)
                    .collect::<Vec<_>>()
                    .join("\n");
                json!({"content":content,"exit_code":exit_code(&output),"isError":truthy(&output["isError"])||truthy(&output["is_error"])})
            } else {
                json!({"content":text(&output)})
            };
            p.insert("tool_response".into(), response);
        } else {
            let mut error = p
                .get("error_message")
                .filter(|v| truthy(v))
                .or_else(|| p.get("error").filter(|v| truthy(v)))
                .cloned()
                .unwrap_or(json!("tool failed"));
            if p.get("failure_type")
                .is_some_and(|v| v == "permission_denied")
            {
                error = json!(format!("Permission denied: {}", scalar_text(&error)));
            }
            p.insert("error".into(), error);
        }
    }
    Ok(json!(p))
}
pub fn record_event(payload: &Value, environment: &Environment) -> Result<()> {
    let event = payload["hook_event_name"].as_str().unwrap_or("");
    if ![
        "UserPromptSubmit",
        "PostToolUse",
        "PostToolUseFailure",
        "SessionEnd",
    ]
    .contains(&event)
    {
        return Ok(());
    }
    let path = session::transcript("cursor", payload, environment)?;
    let Some(path) = path.as_str().filter(|s| !s.is_empty()) else {
        return Ok(());
    };
    let path = environment.path(Path::new(path));
    if event == "SessionEnd" && !path.exists() {
        return Ok(());
    }
    let mut row = json!({});
    for key in [
        "hook_event_name",
        "prompt",
        "cwd",
        "tool_use_id",
        "generation_id",
        "tool_name",
        "tool_input",
        "error",
        "agent_transcript",
    ] {
        if let Some(value) = payload.get(key).filter(|v| !v.is_null() && *v != "") {
            row[key] = value.clone();
        }
    }
    if let Some(input) = row.get_mut("tool_input").and_then(Value::as_object_mut) {
        for value in input.values_mut() {
            if let Some(s) = value.as_str() {
                *value = json!(take(s, RESULT_CAP));
            }
        }
    }
    if let Some(s) = row["error"].as_str() {
        row["error"] = json!(take(s, RESULT_CAP));
    }
    if payload["tool_response"].is_object() {
        let mut response = payload["tool_response"].clone();
        if let Some(s) = response["content"].as_str() {
            response["content"] = json!(take(s, RESULT_CAP));
        }
        row["tool_response"] = response;
    }
    if let Some(parent) = path.parent() {
        private_dir(parent)?;
    }
    row.sort_all_objects();
    let line = format!("{}\n", json_text_utf8(&row));
    OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)?
        .write_all(line.as_bytes())?;
    let _ = private_file(&path);
    Ok(())
}
pub fn state_path(env: &Value, cid: &str, dir: &str, environment: &Environment) -> Result<PathBuf> {
    Ok(paths::state_root(
        env.as_object()
            .ok_or_else(|| Error::Invalid("environment must be an object".into()))?,
        &environment.home,
    )?
    .join(dir)
    .join(format!("{cid}.json")))
}
pub fn load(env: &Value, cid: &str, dir: &str, environment: &Environment) -> Result<Option<Value>> {
    let value = fs::read(environment.path(&state_path(env, cid, dir, environment)?))
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
    Ok(value.filter(Value::is_object))
}
pub fn save(
    env: &Value,
    cid: &str,
    st: &Value,
    dir: &str,
    environment: &Environment,
) -> Result<()> {
    let path = environment.path(&state_path(env, cid, dir, environment)?);
    if let Some(parent) = path.parent() {
        private_dir(parent)?;
    }
    let tmp = PathBuf::from(format!(
        "{}.{}.tmp",
        path.to_string_lossy(),
        std::process::id()
    ));
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&tmp)?;
    file.write_all(json_text(st).as_bytes())?;
    drop(file);
    fs::rename(tmp, path)?;
    Ok(())
}
pub fn prune(env: &Value, dir: &str, keep: f64, environment: &Environment) -> Result<()> {
    let folder = environment
        .path(&state_path(env, "x", dir, environment)?)
        .parent()
        .unwrap()
        .to_path_buf();
    let _ = (|| -> io::Result<()> {
        for entry in fs::read_dir(folder)? {
            let path = entry?.path();
            if crate::plugin::history::now() - session::modified(&path)? > keep {
                fs::remove_file(path)?;
            }
        }
        Ok(())
    })();
    Ok(())
}
pub fn fresh(path: Option<&Path>) -> Value {
    json!({"offset":path.and_then(|p|fs::metadata(p).ok()).map_or(0,|m|m.len()),"last_step_index":-1,"turn_seq":0})
}
pub fn expand(
    payload: &Value,
    env: &Value,
    dir: &str,
    keep: f64,
    environment: &Environment,
) -> Result<Vec<Value>> {
    let event = payload["hook_event_name"].as_str().unwrap_or("");
    let sid = json!(session_id("antigravity", payload));
    let Some(cid) = claude::safe_session_id(&sid)
        .filter(|_| ["SessionStart", "PreInvocation", "Stop"].contains(&event))
    else {
        return Ok(Vec::new());
    };
    let path = session::transcript("antigravity", payload, environment)?;
    let source = path
        .as_str()
        .ok_or_else(|| Error::Invalid("transcript path must be text".into()))?;
    let input = environment.path(Path::new(source));
    let base = json!({"session_id":cid,"cwd":session::cwd("antigravity",payload,env,&environment.cwd),"transcript_path":path});
    if event == "SessionStart" {
        prune(env, dir, keep, environment)?;
        if load(env, cid, dir, environment)?.is_none() {
            save(env, cid, &fresh(Some(&input)), dir, environment)?;
        }
        let mut out = base;
        out["hook_event_name"] = json!("SessionStart");
        out["source"] = json!("startup");
        return Ok(vec![out]);
    }
    let first = event == "PreInvocation" && !truthy(&payload["invocationNum"]);
    let (mut st, items) = match load(env, cid, dir, environment)? {
        None => (fresh(Some(&input)), Vec::new()),
        Some(mut st) => {
            let offset = integer(st.get("offset").filter(|v| truthy(v)).unwrap_or(&json!(0)))?;
            let offset = u64::try_from(offset).map_err(|e| Error::Invalid(e.to_string()))?;
            let last = integer(st.get("last_step_index").unwrap_or(&json!(-1)))?;
            let (items, offset, last) =
                antigravity::read(&input, offset, last, first || event == "Stop");
            st["offset"] = json!(offset);
            st["last_step_index"] = json!(last);
            (st, items)
        }
    };
    let seq = st
        .get("turn_seq")
        .ok_or_else(|| Error::Invalid("missing turn_seq".into()))?;
    let key = format!("{cid}:{}", scalar_text(seq));
    let mut out = Vec::new();
    for item in items.iter().filter(|i| i["kind"] == "tool") {
        let mut p = base.clone();
        p.as_object_mut().unwrap().extend(json!({"hook_event_name":"PostToolUse","prompt_id":key,"tool_name":item["name"],"tool_input":item["args"],"tool_response":{"content":take(item["content"].as_str().unwrap_or(""),RESULT_CAP),"exit_code":item["exit_code"],"isError":item["is_error"]}}).as_object().unwrap().clone());
        out.push(p);
    }
    if first {
        let next = integer(
            st.get("turn_seq")
                .filter(|v| truthy(v))
                .unwrap_or(&json!(0)),
        )?
        .checked_add(1)
        .ok_or_else(|| Error::Invalid("state counter exceeds i64".into()))?;
        st["turn_seq"] = json!(next);
        let mut p = base.clone();
        p["hook_event_name"] = json!("UserPromptSubmit");
        p["prompt"] = json!(text(&payload["lastUserInput"]));
        p["prompt_id"] = json!(format!("{cid}:{}", scalar_text(&st["turn_seq"])));
        out.push(p);
    } else if event == "Stop" {
        let mut p = base;
        p["hook_event_name"] = json!("Stop");
        p["terminationReason"] = payload["terminationReason"].clone();
        out.push(p);
    }
    save(env, cid, &st, dir, environment)?;
    Ok(out)
}
pub fn render_antigravity(payload: &Value, out: &Value) -> Result<Value> {
    let parts = [
        &out["hookSpecificOutput"]["additionalContext"],
        &out["additionalContext"],
        &out["systemMessage"],
    ];
    let text = parts
        .into_iter()
        .filter(|v| truthy(v))
        .map(|v| {
            v.as_str()
                .ok_or_else(|| Error::Invalid("context must be text".into()))
        })
        .collect::<Result<Vec<_>>>()?
        .join("\n\n");
    let event = payload["hook_event_name"].as_str().unwrap_or("");
    Ok(if text.is_empty() || event == "Stop" {
        json!({})
    } else if event == "SessionStart" {
        json!({"injectSteps":[{"systemMessage":{"systemMessage":text}}]})
    } else {
        json!({"injectSteps":[{"ephemeralMessage":text}]})
    })
}
pub fn spool(payload: &Value, env: &Value, environment: &Environment) -> Result<PathBuf> {
    static UNSAFE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^\p{L}\p{N}_.-]").unwrap());
    let sid = session_id("grok", payload);
    Ok(paths::state_root(
        env.as_object()
            .ok_or_else(|| Error::Invalid("environment must be an object".into()))?,
        &environment.home,
    )?
    .join("grok-start")
    .join(format!("{}.txt", UNSAFE.replace_all(&sid, "_"))))
}
pub fn render_grok(
    payload: &Value,
    out: &mut Value,
    env: &Value,
    cap: i64,
    environment: &Environment,
) -> Result<()> {
    let event = payload["hook_event_name"].as_str().unwrap_or("");
    if session_id("grok", payload).is_empty()
        || ![
            "SessionStart",
            "PostToolUse",
            "PostToolUseFailure",
            "SessionEnd",
        ]
        .contains(&event)
    {
        return Ok(());
    }
    let path = environment.path(&spool(payload, env, environment)?);
    if event == "SessionStart" {
        let mut remove = false;
        let text = if let Some(hso) = out
            .get_mut("hookSpecificOutput")
            .and_then(Value::as_object_mut)
        {
            let text = hso.remove("additionalContext").unwrap_or(json!(""));
            remove = hso.keys().all(|k| k == "hookEventName");
            text
        } else {
            json!("")
        };
        if remove {
            out.as_object_mut()
                .ok_or_else(|| Error::Invalid("hook output must be an object".into()))?
                .remove("hookSpecificOutput");
        }
        let text = if truthy(&text) {
            Some(
                text.as_str()
                    .ok_or_else(|| Error::Invalid("context must be text".into()))?,
            )
        } else {
            None
        };
        let _ = (|| -> io::Result<()> {
            if let Some(parent) = path.parent() {
                private_dir(parent)?;
            }
            if let Some(text) = text {
                fs::write(&path, text)?;
                private_file(&path)?;
            } else {
                fs::remove_file(&path)?;
            }
            Ok(())
        })();
        return Ok(());
    }
    if event == "SessionEnd" || is_subagent("grok", payload) {
        if event == "SessionEnd" {
            let _ = fs::remove_file(path);
        }
        return Ok(());
    }
    let claimed = PathBuf::from(format!("{}.{}", path.to_string_lossy(), std::process::id()));
    let start = (|| -> Result<String> {
        fs::rename(&path, &claimed)?;
        let raw = fs::read(&claimed)?;
        let start = String::from_utf8(raw).map_err(|e| Error::Invalid(e.to_string()))?;
        fs::remove_file(&claimed)?;
        Ok(start.replace("\r\n", "\n").replace('\r', "\n"))
    })();
    let start = match start {
        Ok(start) => start,
        Err(Error::Io(_)) => return Ok(()),
        Err(e) => return Err(e),
    };
    if out.get("hookSpecificOutput").is_none() {
        out["hookSpecificOutput"] = json!({});
    }
    let hso = &mut out["hookSpecificOutput"];
    if !hso.is_object() {
        return Err(Error::Invalid(
            "hookSpecificOutput must be an object".into(),
        ));
    }
    let rest = hso
        .get("additionalContext")
        .filter(|v| truthy(v))
        .map(|v| {
            v.as_str()
                .ok_or_else(|| Error::Invalid("context must be text".into()))
        })
        .transpose()?
        .unwrap_or("");
    if !start.trim().is_empty() {
        let room = cap
            .saturating_sub(rest.chars().count() as i64)
            .saturating_sub(if rest.is_empty() { 0 } else { 2 })
            .max(0) as usize;
        let text = format!(
            "{}{}",
            take(&start, room),
            if rest.is_empty() {
                String::new()
            } else {
                format!("\n\n{rest}")
            }
        );
        hso["hookEventName"] = json!(event);
        hso["additionalContext"] = json!(text);
    } else if !truthy(hso) {
        out.as_object_mut().unwrap().remove("hookSpecificOutput");
    }
    Ok(())
}
