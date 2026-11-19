use super::{
    context::LiveContext,
    hosts::{self, session::Environment, state},
    paths,
};
use crate::value::python_space;
use crate::value::{json_text, scalar_text, truthy};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{fs, io::Write, path::Path};

#[derive(Serialize, Deserialize)]
pub struct Host {
    pub name: String,
    pub keeps_history: bool,
    pub prepare: bool,
    pub record_event: bool,
    pub expand: bool,
    pub render: bool,
}
impl Host {
    pub fn builtin(name: String) -> Self {
        Self {
            keeps_history: hosts::profile(&name).keeps_history,
            prepare: matches!(name.as_str(), "cursor" | "grok"),
            record_event: name == "cursor",
            expand: name == "antigravity",
            render: matches!(name.as_str(), "antigravity" | "grok"),
            name,
        }
    }
}
pub trait Runtime {
    fn detect(
        &mut self,
        payload: &Value,
        environment: &Environment,
        hint: Option<&str>,
    ) -> Result<Host, String> {
        Ok(Host::builtin(hosts::detect(
            payload,
            &json!(environment.env),
            hint,
            &hosts::HOSTS.map(String::from),
        )))
    }
    fn host(
        &mut self,
        host: &Host,
        method: &str,
        payload: &Value,
        environment: &Environment,
        render: Option<(&LiveContext, &Value)>,
    ) -> Result<Value, String> {
        host_method(host, method, payload, environment, environment, render)
    }
    fn handle(&mut self, ctx: &mut LiveContext, payload: Value) -> Result<Value, String>;
    fn run(
        &mut self,
        payload: Value,
        environment: &Environment,
        hint: Option<&str>,
    ) -> Result<Value, String> {
        run(payload, environment, hint, self)
    }
}

pub fn host_method(
    host: &Host,
    method: &str,
    payload: &Value,
    environment: &Environment,
    process: &Environment,
    render: Option<(&LiveContext, &Value)>,
) -> Result<Value, String> {
    let env = json!(environment.env);
    let result = match method {
        "cwd" => {
            return Ok(hosts::session::cwd(
                &host.name,
                payload,
                &env,
                &environment.cwd,
            ));
        }
        "session_id" => return Ok(json!(hosts::session_id(&host.name, payload))),
        "prepare" => state::prepare(&host.name, payload, &env, process),
        "record_event" => state::record_event(payload, process).map(|_| Value::Null),
        "expand" => state::expand(
            payload,
            &env,
            state::STATE_DIR,
            state::STATE_KEEP_S,
            process,
        )
        .map(|events| json!(events)),
        "render" => {
            let (ctx, output) = render.ok_or("missing hook rendering context")?;
            let mut out = output.clone();
            if host.name == "antigravity" {
                state::render_antigravity(payload, &out)
            } else {
                state::render_grok(
                    payload,
                    &mut out,
                    &json!(ctx.env),
                    state::CONTEXT_CAP,
                    process,
                )
                .map(|_| out)
            }
        }
        _ => return Err(format!("unknown hook host method: {method}")),
    };
    result.map_err(|e| e.to_string())
}

pub fn run<R: Runtime + ?Sized>(
    mut payload: Value,
    environment: &Environment,
    hint: Option<&str>,
    runtime: &mut R,
) -> Result<Value, String> {
    let host = runtime.detect(&payload, environment, hint)?;
    if host.prepare {
        payload = runtime.host(&host, "prepare", &payload, environment, None)?;
    }
    let cwd = runtime.host(&host, "cwd", &payload, environment, None)?;
    let sid = runtime.host(&host, "session_id", &payload, environment, None)?;
    let cwd = cwd.as_str().ok_or("host working directory must be text")?;
    let mut ctx = LiveContext::build(
        &host.name,
        host.keeps_history,
        &environment.path(Path::new(cwd)),
        environment.env.clone(),
        &environment.home,
        if truthy(&sid) { sid } else { Value::Null },
        false,
    )?;
    if !truthy(&ctx.config["enabled"]) {
        return Ok(json!({}));
    }
    if host.record_event {
        runtime.host(&host, "record_event", &payload, environment, None)?;
    }
    let out = if host.expand {
        let events = runtime.host(&host, "expand", &payload, environment, None)?;
        let mut outputs = Vec::new();
        for event in events.as_array().ok_or("host events must be an array")? {
            outputs.push(runtime.handle(&mut ctx, event.clone())?);
        }
        hosts::merge_outputs(&outputs)?
    } else {
        runtime.handle(&mut ctx, payload.clone())?
    };
    if host.render {
        runtime.host(&host, "render", &payload, environment, Some((&ctx, &out)))
    } else {
        Ok(out)
    }
}

pub fn enabled_flag(value: &Value) -> bool {
    matches!(
        scalar_text(if truthy(value) { value } else { &Value::Null })
            .trim_matches(python_space)
            .to_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

pub fn record_error(environment: &Environment, error: &str, pid: u32) {
    let _ = (|| -> Result<(), Box<dyn std::error::Error>> {
        let root = paths::state_root(&environment.env, &environment.home)?;
        fs::create_dir_all(&root)?;
        let mut log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join("errors.log"))?;
        writeln!(
            log,
            "--- {} pid={pid}",
            chrono::Local::now().format("%Y-%m-%dT%H:%M:%S")
        )?;
        write!(log, "{error}")?;
        if !error.ends_with('\n') {
            writeln!(log)?;
        }
        Ok(())
    })();
}

pub fn main<R: Runtime + ?Sized>(
    argv: &[String],
    input: Result<Vec<u8>, String>,
    environment: &Environment,
    pid: u32,
    runtime: &mut R,
) -> Result<String, String> {
    let argument = |flag: &str| {
        argv.iter()
            .position(|v| v == flag)
            .and_then(|i| argv.get(i + 1))
            .map(String::as_str)
    };
    let out = (|| {
        let input = input?;
        let raw = std::str::from_utf8(&input).map_err(|e| e.to_string())?;
        if enabled_flag(environment.env.get("TE_SIDECALL").unwrap_or(&Value::Null))
            || enabled_flag(environment.env.get("TE_DISABLE").unwrap_or(&Value::Null))
        {
            return Ok(json!({}));
        }
        let mut payload: Value = if raw.trim_matches(python_space).is_empty() {
            json!({})
        } else {
            // shortcut: serde_json limits numbers and nesting, reconcile them in the production parity audit.
            serde_json::from_str(raw).map_err(|e| e.to_string())?
        };
        let Some(object) = payload.as_object_mut() else {
            return Ok(json!({}));
        };
        if let Some(event) = argument("--event").filter(|s| !s.is_empty()) {
            object.entry("hook_event_name").or_insert(json!(event));
        }
        let out = runtime.run(payload, environment, argument("--host"))?;
        paths::secure_root(&environment.env, &environment.home)?;
        Ok::<_, String>(if truthy(&out) { out } else { json!({}) })
    })();
    let mut out = out.unwrap_or_else(|error| {
        record_error(environment, &error, pid);
        json!({})
    });
    let progress = out
        .as_object_mut()
        .ok_or("hook output must be an object")?
        .shift_remove(hosts::PROGRESS);
    let mut lines = String::new();
    if let Some(progress) = progress.filter(truthy) {
        let entries: Vec<Value> = match progress {
            Value::Array(values) => values,
            Value::String(text) => text.chars().map(|c| json!(c.to_string())).collect(),
            Value::Object(values) => values.into_iter().map(|(key, _)| json!(key)).collect(),
            _ => return Err("hook progress must be iterable".into()),
        };
        for message in entries {
            lines.push_str(&json_text(&json!({"type":"progress","message":message})));
            lines.push('\n');
        }
    }
    lines.push_str(&json_text(&out));
    lines.push('\n');
    Ok(lines)
}
