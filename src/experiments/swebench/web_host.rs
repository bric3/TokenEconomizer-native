use super::{
    Failure, Result, Runtime,
    optimizer::{capture_log, code_points, nonnegative_difference},
    providers::{boolean, capture, copy, literal},
};
use crate::python_json::Text;
use serde_json::{Value, json};
use std::path::PathBuf;

fn returned(slot: &str) -> Value {
    json!({"provider_ref":slot})
}
fn constant(rt: &mut dyn Runtime, slot: &str, name: &str) -> Result<()> {
    rt.call("provider_constant", json!([slot, name]))
        .map(|_| ())
}
fn compare(rt: &mut dyn Runtime, operation: &str, a: &str, b: &str) -> Result<bool> {
    rt.call("web_compare", json!([operation, a, b]))?;
    boolean(rt, "comparison")
}
fn floor(rt: &mut dyn Runtime, minimum: i32) -> Result<Value> {
    literal(rt, "floor", json!(minimum))?;
    if !compare(rt, "gt", "value", "floor")? {
        copy(rt, "value", "floor")?;
    }
    Ok(returned("value"))
}
fn decimal(rt: &mut dyn Runtime, slot: &str) -> Result<Option<String>> {
    rt.call("web_decimal", json!([slot]))
        .map(|value| value.as_str().map(str::to_owned))
}
fn subtract(rt: &mut dyn Runtime) -> Result<()> {
    if let Some(a) = decimal(rt, "abort")?
        && let Some(b) = decimal(rt, "headroom")?
        && !a.starts_with('-')
        && !b.starts_with('-')
        && (a.len(), &a) >= (b.len(), &b)
    {
        rt.call(
            "web_integer_literal",
            json!(["cap", nonnegative_difference(&a, &b)]),
        )?;
    } else {
        rt.call("web_binary", json!(["sub", "abort", "headroom", "cap"]))?;
    }
    Ok(())
}
fn millis(rt: &mut dyn Runtime) -> Result<()> {
    if let Some(value) = decimal(rt, "abort")? {
        rt.call(
            "web_integer_literal",
            json!([
                "millis",
                if value == "0" {
                    value
                } else {
                    format!("{value}000")
                }
            ]),
        )?;
    } else {
        literal(rt, "thousand", json!(1000))?;
        rt.call("web_binary", json!(["mul", "abort", "thousand", "millis"]))?;
    }
    rt.call("web_stringify", json!(["millis", "millis_text"]))
        .map(|_| ())
}
fn formatted(rt: &mut dyn Runtime, slot: &str) -> Result<Text> {
    code_points(rt.call("logger_format", json!([slot]))?)
}
fn append(text: &mut Text, suffix: &str) {
    text.0.extend(suffix.chars().map(u32::from));
}
fn log(rt: &mut dyn Runtime, text: Text) -> Result<()> {
    rt.call("logger_report", json!([text.0])).map(|_| ())
}
fn repo(rt: &mut dyn Runtime) -> Result<Value> {
    capture(rt, "path", "TE_WEBBENCH_REPO", None)?;
    if boolean(rt, "path")? {
        rt.call("host_path", json!(["realpath", "path", "path"]))?;
        return Ok(returned("path"));
    }
    capture(rt, "projects", "TOKENECONOMIZER_WEBBENCH_PROJECTS", None)?;
    if boolean(rt, "projects")? {
        rt.call("context_capture_path", json!(["realpath", "realpath"]))?;
        rt.call("context_capture_path", json!(["dirname", "dirname"]))?;
        rt.call(
            "provider_method",
            json!(["projects", "rstrip", ["/"], "stripped"]),
        )?;
        rt.call(
            "context_apply_path",
            json!(["dirname", "parent", ["stripped"]]),
        )?;
        rt.call(
            "context_apply_path",
            json!(["realpath", "path", ["parent"]]),
        )?;
        return Ok(returned("path"));
    }
    Err(Failure::new(
        "RuntimeError",
        "no Web-Bench checkout: set TE_WEBBENCH_REPO or TOKENECONOMIZER_WEBBENCH_PROJECTS (see the repo's .env.example)",
    ))
}
fn node(rt: &mut dyn Runtime) -> Result<Value> {
    capture(rt, "node", "TE_NODE_BIN", None)?;
    if !boolean(rt, "node")? {
        rt.call("web_which", json!(["node", "node"]))?;
        if !boolean(rt, "node")? {
            return Err(Failure::new(
                "RuntimeError",
                "no `node` on PATH and TE_NODE_BIN is unset; the Web-Bench evaluator is a Node program",
            ));
        }
    }
    Ok(returned("node"))
}
fn abort(rt: &mut dyn Runtime) -> Result<Value> {
    let converted: Result<()> = (|| {
        capture(rt, "raw", "TE_WEBBENCH_UNDICI_ABORT", None)?;
        if !boolean(rt, "raw")? {
            constant(rt, "raw", "WEBBENCH_UNDICI_ABORT_DEFAULT_S")?;
        }
        rt.call("web_integer", json!(["raw", "value"]))?;
        Ok(())
    })();
    if let Err(error) = converted {
        if !error.is_a("builtins.TypeError") && !error.is_a("builtins.ValueError") {
            return Err(error);
        }
        constant(rt, "value", "WEBBENCH_UNDICI_ABORT_DEFAULT_S")?;
    }
    floor(rt, 60)
}
fn timeout(rt: &mut dyn Runtime) -> Result<Value> {
    capture(rt, "raw", "TE_WEBBENCH_TASK_TIMEOUT", None)?;
    if !boolean(rt, "raw")? {
        capture(rt, "raw", "TE_TASK_TIMEOUT", None)?;
    }
    let converted: Result<()> = (|| {
        if boolean(rt, "raw")? {
            rt.call("web_integer", json!(["raw", "value"]))?;
        } else {
            constant(rt, "value", "WEBBENCH_TASK_TIMEOUT_DEFAULT")?;
        }
        Ok(())
    })();
    if let Err(error) = converted {
        if !error.is_a("builtins.TypeError") && !error.is_a("builtins.ValueError") {
            return Err(error);
        }
        constant(rt, "value", "WEBBENCH_TASK_TIMEOUT_DEFAULT")?;
    }
    rt.call("web_abort", json!([]))?;
    constant(rt, "headroom", "WEBBENCH_TASK_TIMEOUT_HEADROOM_S")?;
    subtract(rt)?;
    if compare(rt, "gt", "value", "cap")? {
        capture_log(rt)?;
        let mut message = Text::from("webbench: TE_TASK_TIMEOUT=");
        message.0.extend(formatted(rt, "value")?.0);
        append(&mut message, "s is above the evaluator's ");
        message.0.extend(formatted(rt, "abort")?.0);
        append(
            &mut message,
            "s request abort, which would lose whole attempts and truncate the stream -- clamping to ",
        );
        message.0.extend(formatted(rt, "cap")?.0);
        append(
            &mut message,
            "s. Set TE_WEBBENCH_TASK_TIMEOUT to choose a lower value for this domain.",
        );
        log(rt, message)?;
        copy(rt, "value", "cap")?;
    }
    floor(rt, 1)
}
fn undici_env(rt: &mut dyn Runtime) -> Result<Value> {
    rt.call("web_abort", json!([]))?;
    constant(rt, "default", "WEBBENCH_UNDICI_ABORT_DEFAULT_S")?;
    if compare(rt, "le", "abort", "default")? {
        return Ok(returned("output"));
    }
    let assets = crate::experiments::assets::installed()?.unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/experiments/assets")
    });
    rt.call(
        "web_uri",
        json!([assets.join("webbench_undici_preload.mjs")]),
    )?;
    literal(rt, "prefix", json!("--import "))?;
    rt.call("web_binary", json!(["add", "prefix", "uri", "opt"]))?;
    capture(rt, "previous", "NODE_OPTIONS", Some(""))?;
    if boolean(rt, "previous")? {
        literal(rt, "space", json!(" "))?;
        rt.call("web_binary", json!(["add", "previous", "space", "options"]))?;
        rt.call("web_binary", json!(["add", "options", "opt", "options"]))?;
        rt.call(
            "provider_method",
            json!(["options", "strip", [], "options"]),
        )?;
    } else {
        copy(rt, "options", "opt")?;
    }
    rt.call("provider_collect", json!(["NODE_OPTIONS", "options"]))?;
    millis(rt)?;
    rt.call(
        "provider_collect",
        json!(["TE_WEBBENCH_UNDICI_ABORT_MS", "millis_text"]),
    )?;
    if !boolean(rt, "repo")? {
        capture(rt, "repo", "TOKENECONOMIZER_WEBBENCH_REPO", Some(""))?;
    }
    rt.call("web_stringify", json!(["repo", "repo_text"]))?;
    rt.call("provider_collect", json!(["TE_WEBBENCH_REPO", "repo_text"]))?;
    Ok(returned("output"))
}
fn namespace(rt: &mut dyn Runtime) -> Result<Value> {
    let ran = rt.call("web_run",json!([["unshare","--user","--map-root-user","--mount","bash","-c","getent hosts github.com | head -1"],{"capture_output":true,"text":true,"timeout":60}]));
    if let Err(error) = ran {
        if !error.is_a("builtins.OSError") && !error.is_a("subprocess.TimeoutExpired") {
            return Err(error);
        }
        capture_log(rt)?;
        log(rt,Text::from(format!("codex webbench: cannot create a user+mount namespace ({}) -- the answer-key hosts block is NOT available on this host",error.kind).as_str()))?;
        return Ok(json!(false));
    }
    rt.call("web_returncode", json!([]))?;
    literal(rt, "zero", json!(0))?;
    let failed = compare(rt, "ne", "returncode", "zero")?;
    capture_log(rt)?;
    log(
        rt,
        Text::from(if failed {
            "codex webbench: unprivileged user namespaces are refused on this host (check /proc/sys/kernel/unprivileged_userns_clone) -- the answer-key hosts block is NOT available"
        } else {
            "codex webbench: answer-key hosts block armed through a user+mount namespace (there is no container here; the PreToolUse hook is the second layer)"
        }),
    )?;
    Ok(json!(!failed))
}
pub fn run(method: &str, rt: &mut dyn Runtime) -> Result<Value> {
    match method {
        "webbench_repo" => repo(rt),
        "node_bin" => node(rt),
        "webbench_undici_abort" => abort(rt),
        "webbench_task_timeout" => timeout(rt),
        "webbench_undici_env" => undici_env(rt),
        "_probe_namespace_guard" => namespace(rt),
        _ => Err(Failure::new(
            "ValueError",
            format!("unknown Web-Bench host helper: {method}"),
        )),
    }
}
