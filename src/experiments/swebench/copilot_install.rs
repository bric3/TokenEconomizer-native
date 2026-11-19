use super::{
    Failure, Result, Runtime,
    optimizer::string,
    providers::{boolean, capture, copy, literal},
};
use crate::python_json::{self, Node, Text};
use crate::value::python_space;
use serde_json::{Value, json};

const HINT: &str = "\n  install:  curl -fsSL https://gh.io/copilot-install | bash\n  sign in:  copilot   (then /login), or export COPILOT_GITHUB_TOKEN\n  override: COPILOT_HOST_BIN / COPILOT_HOST_CACHE / COPILOT_HOME";

fn path(rt: &mut dyn Runtime, method: &str, slot: &str, argument: &str) -> Result<()> {
    rt.call("host_path", json!([method, slot, argument]))
        .map(|_| ())
}
fn function(rt: &mut dyn Runtime, name: &str, slot: &str) -> Result<()> {
    rt.call("host_function", json!([name, slot])).map(|_| ())
}
fn returned(slot: &str) -> Value {
    json!({"provider_ref":slot})
}
fn directory(rt: &mut dyn Runtime, variable: &str, fallback: &str) -> Result<Value> {
    capture(rt, "directory", variable, None)?;
    if !boolean(rt, "directory")? {
        literal(rt, "directory", json!(fallback))?;
        path(rt, "expanduser", "directory", "directory")?;
    }
    Ok(returned("directory"))
}
fn binary(rt: &mut dyn Runtime) -> Result<Value> {
    capture(rt, "binary", "COPILOT_HOST_BIN", None)?;
    if boolean(rt, "binary")? {
        path(rt, "realpath", "binary", "binary")?;
        return Ok(returned("binary"));
    }
    literal(rt, "home", json!("~/.local/bin/copilot"))?;
    path(rt, "expanduser", "home", "home")?;
    rt.call("host_access", json!([]))?;
    if boolean(rt, "accessible")? {
        path(rt, "realpath", "binary", "home")?;
        return Ok(returned("binary"));
    }
    rt.call("host_which", json!([]))?;
    if !boolean(rt, "binary")? {
        return Err(Failure::new(
            "RuntimeError",
            format!("no copilot binary found.{HINT}"),
        ));
    }
    path(rt, "realpath", "binary", "binary")?;
    Ok(returned("binary"))
}
fn version(rt: &mut dyn Runtime) -> Result<Value> {
    let result: Result<Value> = (|| {
        rt.call("host_version", json!([]))?;
        rt.call("host_stream", json!(["stdout"]))?;
        if !boolean(rt, "stream")? {
            rt.call("host_stream", json!(["stderr"]))?;
            if !boolean(rt, "stream")? {
                literal(rt, "stream", json!(""))?;
            }
        }
        if let Some(text) = string(rt, "stream")? {
            let whitespace = |point: &u32| char::from_u32(*point).is_some_and(python_space);
            let start = text
                .0
                .iter()
                .position(|c| !whitespace(c))
                .unwrap_or(text.0.len());
            let end = text
                .0
                .iter()
                .rposition(|c| !whitespace(c))
                .map_or(start, |i| i + 1);
            if start == end {
                return Ok(json!("unknown"));
            }
            let first = text.0[start..end]
                .iter()
                .take_while(|c| !matches!(c, 10..=13 | 28..=30 | 133 | 8232 | 8233))
                .take(120)
                .copied()
                .collect();
            Ok(json!({"provider_points":Text(first).0}))
        } else {
            rt.call(
                "provider_method",
                json!(["stream", "strip", [], "stripped"]),
            )?;
            rt.call(
                "provider_method",
                json!(["stripped", "splitlines", [], "lines"]),
            )?;
            rt.call("host_first_version", json!([]))?;
            Ok(returned("version"))
        }
    })();
    match result {
        Err(error)
            if error.is_a("builtins.OSError")
                || error.is_a("subprocess.SubprocessError")
                || error.is_a("builtins.IndexError") =>
        {
            Ok(json!("unknown"))
        }
        result => result,
    }
}
fn require(rt: &mut dyn Runtime) -> Result<Value> {
    function(rt, "copilot_bin", "binary")?;
    let mut paths = vec!["binary"];
    if boolean(rt, "need_container_seed")? {
        function(rt, "copilot_cache", "cache")?;
        function(rt, "copilot_config", "config")?;
        paths.extend(["cache", "config"]);
    }
    let mut missing = Vec::new();
    for slot in paths {
        path(rt, "exists", "exists", slot)?;
        if !boolean(rt, "exists")? {
            missing.push(slot);
        }
    }
    if !missing.is_empty() {
        let encoded = rt.call("host_missing", json!([missing]))?;
        let Some(Node::String(ref text)) =
            python_json::parse(encoded.as_str().unwrap_or("").as_bytes(), 0)
        else {
            unreachable!()
        };
        let mut message = Text::from("copilot host install incomplete: ");
        message.0.extend(&text.0);
        message.0.extend(HINT.chars().map(u32::from));
        return Ok(json!({"provider_error":message.json()}));
    }
    Ok(returned("binary"))
}
fn environment(rt: &mut dyn Runtime) -> Result<Value> {
    literal(rt, "assembled", json!({"COPILOT_AUTO_UPDATE":"false"}))?;
    for name in ["auth_env", "provider_env"] {
        function(rt, name, "addition")?;
        rt.call("host_update", json!(["assembled", "addition"]))?;
    }
    copy(rt, "addition", "extra")?;
    if !boolean(rt, "addition")? {
        literal(rt, "addition", json!({}))?;
    }
    rt.call("host_update", json!(["assembled", "addition"]))?;
    Ok(returned("assembled"))
}
pub fn run(method: &str, rt: &mut dyn Runtime) -> Result<Value> {
    match method {
        "copilot_cache" => directory(rt, "COPILOT_HOST_CACHE", "~/.cache/copilot"),
        "copilot_config" => directory(rt, "COPILOT_HOME", "~/.copilot"),
        "copilot_bin" => binary(rt),
        "cli_version" => version(rt),
        "require_install" => require(rt),
        "run_env" => environment(rt),
        _ => unreachable!(),
    }
}
