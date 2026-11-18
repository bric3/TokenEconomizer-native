use super::{
    Result, Runtime,
    optimizer::{capture_log, code_points},
    providers::{boolean, copy, literal},
    runner_calls::{function, invoke, member},
};
use crate::{python_json::Text, value::truthy};
use serde_json::{Value, json};

fn attr(rt: &mut dyn Runtime, object: &str, name: &str, target: &str) -> Result<()> {
    rt.call("stdout_member", json!([object, name, target]))
        .map(|_| ())
}
fn builtin(rt: &mut dyn Runtime, name: &str) -> Result<()> {
    rt.call("eval_builtin", json!([name])).map(|_| ())
}
fn call(rt: &mut dyn Runtime, callable: &str, args: &[&str], target: &str) -> Result<()> {
    copy(rt, "function", callable)?;
    invoke(rt, args, &[])?;
    copy(rt, target, "called")
}
fn apply(rt: &mut dyn Runtime, name: &str, args: &[&str], target: &str) -> Result<()> {
    function(rt, name)?;
    invoke(rt, args, &[])?;
    copy(rt, target, "called")
}
fn method(
    rt: &mut dyn Runtime,
    object: &str,
    name: &str,
    args: &[&str],
    target: &str,
) -> Result<()> {
    attr(rt, object, name, "function")?;
    invoke(rt, args, &[])?;
    copy(rt, target, "called")
}
fn append(rt: &mut dyn Runtime, list: &str, value: &str) -> Result<()> {
    rt.call("eval_append", json!([list, value])).map(|_| ())
}
fn iter(rt: &mut dyn Runtime, object: &str, target: &str) -> Result<()> {
    rt.call("workspace_iter", json!([object, target]))
        .map(|_| ())
}
fn next(rt: &mut dyn Runtime, iterator: &str, target: &str) -> Result<bool> {
    rt.call("workspace_next", json!([iterator, target]))
        .map(|v| truthy(&v))
}
fn is_none(rt: &mut dyn Runtime, object: &str) -> Result<bool> {
    rt.call("provider_is_none", json!([object]))
        .map(|v| truthy(&v))
}
fn binary(rt: &mut dyn Runtime, op: &str, a: &str, b: &str, target: &str) -> Result<()> {
    rt.call("web_binary", json!([op, a, b, target])).map(|_| ())
}
fn compare(rt: &mut dyn Runtime, op: &str, a: &str, b: &str) -> Result<bool> {
    rt.call("web_compare", json!([op, a, b]))?;
    boolean(rt, "comparison")
}
fn formatted(rt: &mut dyn Runtime, object: &str, spec: &str) -> Result<Text> {
    code_points(rt.call("task_format", json!([object, spec]))?)
}
fn push(text: &mut Text, suffix: &str) {
    text.0.extend(suffix.chars().map(u32::from));
}
fn report(rt: &mut dyn Runtime, message: Text) -> Result<()> {
    rt.call("logger_report", json!([message.0])).map(|_| ())
}
fn clock(rt: &mut dyn Runtime) -> Result<()> {
    apply(rt, "time.time", &[], "now")
}
fn catches(rt: &mut dyn Runtime, error: &super::Failure, names: &[&str]) -> Result<bool> {
    rt.call("exception_matches", json!([error, names]))
        .map(|v| truthy(&v))
}
pub(super) fn stream(rt: &mut dyn Runtime, agent: &str, web: bool) -> Result<bool> {
    let claude = agent == "claude";
    if claude {
        literal(rt, "result", json!({}))?;
    } else {
        literal(rt, "lines", json!([]))?;
    }
    clock(rt)?;
    member(rt, "task_timeout", "task_timeout")?;
    binary(rt, "add", "now", "task_timeout", "deadline")?;
    if claude {
        apply(rt, "_UsageRecovery", &[], "recovery")?;
        literal(rt, "stop_reason", Value::Null)?;
    }
    attr(rt, "proc", "stdout", "stdout")?;
    iter(rt, "stdout", "stdout_iter")?;
    while next(rt, "stdout_iter", "line")? {
        super::web_output::strip(rt, false, "line")?;
        if !boolean(rt, "line")? {
            continue;
        }
        attr(rt, "transcript_fh", "write", "write_line")?;
        literal(rt, "newline", json!("\n"))?;
        binary(rt, "add", "line", "newline", "record")?;
        call(rt, "write_line", &["record"], "written")?;
        method(rt, "transcript_fh", "flush", &[], "flushed")?;
        if claude {
            match apply(rt, "json.loads", &["line"], "event") {
                Err(error) if catches(rt, &error, &["Exception"])? => {
                    literal(rt, "event", json!({}))?
                }
                result => result?,
            }
            method(rt, "recovery", "observe", &["event"], "observed")?;
            rt.call(
                "provider_method",
                json!(["event", "get", ["type"], "event_type"]),
            )?;
            literal(rt, "result_type", json!("result"))?;
            if compare(rt, "eq", "event_type", "result_type")? {
                copy(rt, "result", "event")?;
            }
        } else {
            append(rt, "lines", "line")?;
        }
        if !is_none(rt, "guard")? {
            method(rt, "guard", "monitor", &["line"], "stop")?;
            if boolean(rt, "stop")? {
                if claude {
                    literal(rt, "stop_reason", json!("spiral_guard_stop"))?;
                }
                return Ok(true);
            }
        }
        clock(rt)?;
        if compare(rt, "gt", "now", "deadline")? {
            capture_log(rt)?;
            report(
                rt,
                Text::from(if web {
                    match agent {
                        "claude" => "webbench segment hit TE_TASK_TIMEOUT; stopping the agent",
                        "copilot" => {
                            "webbench copilot segment hit TE_TASK_TIMEOUT; stopping the agent"
                        }
                        _ => "webbench codex segment hit TE_TASK_TIMEOUT; stopping the agent",
                    }
                } else {
                    "segment hit TE_TASK_TIMEOUT; stopping the agent"
                }),
            )?;
            if claude {
                literal(rt, "stop_reason", json!("task_timeout"))?;
            }
            return Ok(true);
        }
    }
    Ok(false)
}
pub(super) fn stderr_log(rt: &mut dyn Runtime, agent: &str, web: bool) -> Result<()> {
    method(rt, "err", "strip", &[], "stripped_err")?;
    if !boolean(rt, "stripped_err")? {
        return Ok(());
    }
    if agent == "claude" {
        if boolean(rt, "result")? {
            return Ok(());
        }
    } else {
        rt.call(
            "provider_method",
            json!(["usage", "get", ["num_turns"], "turns"]),
        )?;
        if boolean(rt, "turns")? {
            return Ok(());
        }
    }
    capture_log(rt)?;
    method(rt, "err", "strip", &[], "stripped_err")?;
    rt.call(
        "logger_slice",
        json!(["stripped_err", null, 300, "err_head"]),
    )?;
    let mut message = Text::from(if web {
        match agent {
            "claude" => "webbench agent stderr: ",
            "copilot" => "webbench copilot stderr: ",
            _ => "agent stderr: ",
        }
    } else {
        "agent stderr: "
    });
    message.0.extend(formatted(rt, "err_head", "")?.0);
    report(rt, message)
}
pub(super) fn refused(rt: &mut dyn Runtime, copilot: bool, web: bool) -> Result<()> {
    builtin(rt, "RuntimeError")?;
    copy(rt, "error_constructor", "function")?;
    if copilot {
        let mut message = Text::from("copilot refused this configuration: ");
        message.0.extend(formatted(rt, "bad", "")?.0);
        push(&mut message, "\n  model=");
        member(rt, "model", "model")?;
        message.0.extend(formatted(rt, "model", "!r")?.0);
        push(&mut message, " effort=");
        member(rt, "effort", "effort")?;
        message.0.extend(formatted(rt, "effort", "!r")?.0);
        push(
            &mut message,
            if web {
                "\n  fix COPILOT_MODEL / TE_EFFORT_LEVEL."
            } else {
                "\n  fix COPILOT_MODEL / TE_EFFORT_LEVEL; COPILOT_MODEL=auto always works but does not apply an effort level."
            },
        );
        rt.call("logger_literal", json!(["error_message", message.0]))?;
    } else {
        method(rt, "err", "strip", &[], "stripped_err")?;
        rt.call(
            "logger_slice",
            json!(["stripped_err", null, 600, "err_head"]),
        )?;
        literal(
            rt,
            "error_prefix",
            json!("codex refused its config at run time:\n"),
        )?;
        binary(rt, "add", "error_prefix", "err_head", "error_message")?;
    }
    call(rt, "error_constructor", &["error_message"], "error")?;
    rt.call("task_raise", json!(["error"])).map(|_| ())
}
pub(super) fn reap(rt: &mut dyn Runtime) -> Result<()> {
    let waited = (|| {
        attr(rt, "proc", "wait", "function")?;
        literal(rt, "wait_timeout", json!(60))?;
        invoke(rt, &[], &[("timeout", "wait_timeout")])
    })();
    match waited {
        Err(error) if catches(rt, &error, &["subprocess.TimeoutExpired"])? => {
            rt.call("handler_exception", json!([error]))?;
            rt.call("handler_member", json!(["proc", "kill", "function"]))?;
            rt.call("handler_invoke", json!([[], []])).map(|_| ())
        }
        result => result,
    }
}
