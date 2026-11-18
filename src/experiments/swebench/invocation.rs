use super::{
    Failure, Result, Runtime,
    optimizer::code_points,
    providers::{boolean, copy, literal},
    runner_calls::{function, invoke},
};
use crate::{python_json::Text, value::truthy};
use serde_json::{Value, json};

fn builtin(rt: &mut dyn Runtime, name: &str) -> Result<()> {
    rt.call("eval_builtin", json!([name])).map(|_| ())
}
fn append(rt: &mut dyn Runtime, slot: &str) -> Result<()> {
    rt.call("eval_append", json!(["argv", slot])).map(|_| ())
}
fn argument(rt: &mut dyn Runtime, value: &str) -> Result<()> {
    literal(rt, "argument", json!(value))?;
    append(rt, "argument")
}
fn binary(rt: &mut dyn Runtime, op: &str, a: &str, b: &str, target: &str) -> Result<()> {
    rt.call("web_binary", json!([op, a, b, target])).map(|_| ())
}
fn pair(rt: &mut dyn Runtime, flag: &str, value: &str) -> Result<()> {
    literal(rt, "flag", json!(flag))?;
    rt.call("eval_list", json!(["pair", ["flag", value]]))?;
    binary(rt, "iadd", "argv", "pair", "argv")
}
fn method(
    rt: &mut dyn Runtime,
    object: &str,
    name: &str,
    args: &[&str],
    target: &str,
) -> Result<()> {
    rt.call("stdout_member", json!([object, name, "function"]))?;
    invoke(rt, args, &[])?;
    copy(rt, target, "called")
}
fn converted(rt: &mut dyn Runtime, object: &str, target: &str) -> Result<()> {
    builtin(rt, "str")?;
    invoke(rt, &[object], &[])?;
    copy(rt, target, "called")
}
fn iter(rt: &mut dyn Runtime, object: &str, target: &str) -> Result<()> {
    rt.call("workspace_iter", json!([object, target]))
        .map(|_| ())
}
fn next(rt: &mut dyn Runtime, iterator: &str, target: &str) -> Result<bool> {
    rt.call("workspace_next", json!([iterator, target]))
        .map(|v| truthy(&v))
}
fn returned(slot: &str) -> Value {
    json!({"provider_ref":slot})
}
fn copilot(rt: &mut dyn Runtime) -> Result<Value> {
    literal(rt, "argv", json!(["copilot", "-p"]))?;
    append(rt, "prompt")?;
    for arg in [
        "--output-format",
        "json",
        "--allow-all-tools",
        "--no-color",
        "--bash-env=on",
        "--log-level",
        "all",
        "--log-dir",
    ] {
        argument(rt, arg)?;
    }
    append(rt, "log_dir")?;
    function(rt, "_ISOLATION_ARGS")?;
    copy(rt, "isolation", "function")?;
    binary(rt, "iadd", "argv", "isolation", "argv")?;
    copy(rt, "excluded", "exclude_tools")?;
    if !boolean(rt, "excluded")? {
        literal(rt, "excluded", json!([]))?;
    }
    iter(rt, "excluded", "tools")?;
    while next(rt, "tools", "tool")? {
        function(rt, "PROTECTED_TOOLS")?;
        copy(rt, "protected", "function")?;
        if !truthy(&rt.call("provider_contains", json!(["tool", "protected"]))?) {
            pair(rt, "--excluded-tools", "tool")?;
        }
    }
    if boolean(rt, "model")? {
        converted(rt, "model", "model_text")?;
        pair(rt, "--model", "model_text")?;
    }
    if boolean(rt, "effort")? {
        function(rt, "effort_applies")?;
        invoke(rt, &["model"], &[])?;
        if boolean(rt, "called")? {
            converted(rt, "effort", "effort_text")?;
            pair(rt, "--effort", "effort_text")?;
        }
    }
    if boolean(rt, "mcp_config")? {
        literal(rt, "at", json!("@"))?;
        binary(rt, "add", "at", "mcp_config", "mcp")?;
        pair(rt, "--additional-mcp-config", "mcp")?;
    }
    Ok(returned("argv"))
}
fn codex(rt: &mut dyn Runtime, resume: bool) -> Result<Value> {
    function(rt, "C_BIN")?;
    copy(rt, "binary", "function")?;
    rt.call("eval_list", json!(["argv", ["binary"]]))?;
    argument(rt, "exec")?;
    if resume {
        argument(rt, "resume")?;
        converted(rt, "session_id", "session_text")?;
        append(rt, "session_text")?;
    }
    argument(rt, "--json")?;
    if !resume {
        argument(rt, "--color")?;
        argument(rt, "never")?;
    }
    argument(rt, "--skip-git-repo-check")?;
    argument(rt, "--dangerously-bypass-approvals-and-sandbox")?;
    if boolean(rt, "hook")? {
        argument(rt, "--dangerously-bypass-hook-trust")?;
    }
    if resume {
        if boolean(rt, "model")? {
            pair(rt, "-m", "model")?;
        }
        if boolean(rt, "effort")? {
            function(rt, "json.dumps")?;
            invoke(rt, &["effort"], &[])?;
            let mut config = Text::from("model_reasoning_effort=");
            config
                .0
                .extend(code_points(rt.call("task_format", json!(["called", ""]))?)?.0);
            rt.call("logger_literal", json!(["effort_config", config.0]))?;
            pair(rt, "-c", "effort_config")?;
        }
    }
    literal(
        rt,
        "tail",
        if resume {
            json!(["-"])
        } else {
            json!(["-C", "/testbed", "-"])
        },
    )?;
    binary(rt, "add", "argv", "tail", "argv")?;
    Ok(returned("argv"))
}
fn environment(rt: &mut dyn Runtime) -> Result<Value> {
    function(rt, "C_HOME")?;
    copy(rt, "home", "function")?;
    literal(rt, "env", json!({}))?;
    rt.call("eval_field", json!(["env", "CODEX_HOME", "home"]))?;
    for (key, value) in [("HOME", "/tmp/agent-home"), ("RUST_BACKTRACE", "0")] {
        literal(rt, "value", json!(value))?;
        rt.call("eval_field", json!(["env", key, "value"]))?;
    }
    rt.call("stdout_member", json!(["env", "update", "update"]))?;
    copy(rt, "addition", "extra")?;
    if !boolean(rt, "addition")? {
        literal(rt, "addition", json!({}))?;
    }
    copy(rt, "function", "update")?;
    invoke(rt, &["addition"], &[])?;
    Ok(returned("env"))
}
fn fatal(rt: &mut dyn Runtime) -> Result<Value> {
    copy(rt, "text", "stderr_text")?;
    if !boolean(rt, "text")? {
        literal(rt, "text", json!(""))?;
    }
    method(rt, "text", "splitlines", &[], "lines")?;
    iter(rt, "lines", "lines_iter")?;
    while next(rt, "lines_iter", "line")? {
        method(rt, "line", "lower", &[], "low")?;
        builtin(rt, "any")?;
        copy(rt, "any", "function")?;
        function(rt, "_FATAL_CONFIG_ERRORS")?;
        copy(rt, "patterns", "function")?;
        rt.call(
            "eval_generator",
            json!([
                "matches",
                "patterns",
                "invocation_error_element",
                "copilot",
                "low"
            ]),
        )?;
        copy(rt, "function", "any")?;
        invoke(rt, &["matches"], &[])?;
        if boolean(rt, "called")? {
            method(rt, "line", "strip", &[], "stripped")?;
            rt.call("logger_slice", json!(["stripped", null, 300, "fatal"]))?;
            return Ok(returned("fatal"));
        }
    }
    Ok(Value::Null)
}
pub fn dispatch(operation: &str, agent: &str, rt: &mut dyn Runtime) -> Result<Value> {
    match operation {
        "build" if agent == "copilot" => copilot(rt),
        "build" => codex(rt, false),
        "resume" => codex(rt, true),
        "env" => environment(rt),
        "fatal" => fatal(rt),
        "error_element" => Ok(json!([truthy(
            &rt.call("provider_contains", json!(["element", "context"]))?
        )])),
        _ => Err(Failure::new(
            "ValueError",
            format!("unknown agent invocation helper: {operation}"),
        )),
    }
}
