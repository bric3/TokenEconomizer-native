use super::{
    Failure, Result, Runtime,
    optimizer::code_points,
    providers::{boolean, copy, literal},
    runner_calls::{function, invoke, member},
};
use crate::{python_json::Text, value::truthy};
use serde_json::{Value, json};

fn attr(rt: &mut dyn Runtime, object: &str, name: &str, target: &str) -> Result<()> {
    rt.call("stdout_member", json!([object, name, target]))
        .map(|_| ())
}
fn call(rt: &mut dyn Runtime, callable: &str, args: &[&str], target: &str) -> Result<()> {
    copy(rt, "function", callable)?;
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
fn formatted(rt: &mut dyn Runtime, slot: &str, spec: &str) -> Result<Text> {
    code_points(rt.call("task_format", json!([slot, spec]))?)
}
fn text(rt: &mut dyn Runtime, target: &str, value: Text) -> Result<()> {
    rt.call("logger_literal", json!([target, value.0]))
        .map(|_| ())
}
fn returned(slot: &str) -> Value {
    json!({"provider_ref":slot})
}
fn context(rt: &mut dyn Runtime) -> Result<Value> {
    for (target, kind, operation) in [
        ("exec", "exec", "exec"),
        ("copy", "container_copy", "copy"),
        ("read", "container_read", "read"),
    ] {
        rt.call(
            "eval_closure",
            json!([
                target,
                kind,
                format!("scaffold_{operation}"),
                [["name", "name"]],
                "swebench"
            ]),
        )?;
    }
    function(rt, "scaffolds_pkg.ScaffoldContext")?;
    copy(rt, "constructor", "function")?;
    member(rt, "scaffold_profile", "profile")?;
    function(rt, "_run")?;
    copy(rt, "host", "function")?;
    function(rt, "_log")?;
    copy(rt, "log", "function")?;
    copy(rt, "function", "constructor")?;
    invoke(
        rt,
        &[],
        &[
            ("profile", "profile"),
            ("container", "name"),
            ("task", "task"),
            ("run_dir", "run_dir_path"),
            ("exec_", "exec"),
            ("copy_in", "copy"),
            ("host_run", "host"),
            ("read_bytes", "read"),
            ("log", "log"),
        ],
    )?;
    Ok(returned("called"))
}
fn exec(rt: &mut dyn Runtime) -> Result<Value> {
    literal(rt, "cmd", json!(["docker", "exec", "-w", "/testbed"]))?;
    copy(rt, "overrides", "env")?;
    if !boolean(rt, "overrides")? {
        literal(rt, "overrides", json!({}))?;
    }
    method(rt, "overrides", "items", &[], "items")?;
    rt.call("workspace_iter", json!(["items", "iterator"]))?;
    while truthy(&rt.call("workspace_next", json!(["iterator", "entry"]))?) {
        rt.call("scaffold_pair", json!(["entry", "key", "value"]))?;
        literal(rt, "flag", json!("-e"))?;
        let mut assignment = formatted(rt, "key", "")?;
        assignment.0.push(u32::from('='));
        assignment.0.extend(formatted(rt, "value", "")?.0);
        text(rt, "assignment", assignment)?;
        rt.call("eval_list", json!(["addition", ["flag", "assignment"]]))?;
        rt.call("web_binary", json!(["iadd", "cmd", "addition", "cmd"]))?;
    }
    function(rt, "_run")?;
    copy(rt, "run", "function")?;
    literal(rt, "bash", json!("bash"))?;
    literal(rt, "lc", json!("-lc"))?;
    rt.call(
        "eval_list",
        json!(["tail", ["name", "bash", "lc", "script"]]),
    )?;
    rt.call("web_binary", json!(["add", "cmd", "tail", "args"]))?;
    copy(rt, "function", "run")?;
    invoke(rt, &["args"], &[("timeout", "timeout")])?;
    Ok(returned("called"))
}
fn copy_in(rt: &mut dyn Runtime) -> Result<Value> {
    function(rt, "_run")?;
    copy(rt, "run", "function")?;
    literal(rt, "docker", json!("docker"))?;
    literal(rt, "cp", json!("cp"))?;
    let mut destination = formatted(rt, "name", "")?;
    destination.0.push(u32::from(':'));
    destination.0.extend(formatted(rt, "container_path", "")?.0);
    text(rt, "destination", destination)?;
    rt.call(
        "eval_list",
        json!(["args", ["docker", "cp", "host_path", "destination"]]),
    )?;
    literal(rt, "timeout", json!(300))?;
    copy(rt, "function", "run")?;
    invoke(rt, &["args"], &[("timeout", "timeout")])?;
    Ok(returned("called"))
}
fn read(rt: &mut dyn Runtime) -> Result<Value> {
    function(rt, "subprocess.run")?;
    copy(rt, "run", "function")?;
    literal(rt, "docker", json!("docker"))?;
    literal(rt, "exec", json!("exec"))?;
    literal(rt, "bash", json!("bash"))?;
    literal(rt, "lc", json!("-lc"))?;
    rt.call(
        "eval_list",
        json!(["args", ["docker", "exec", "name", "bash", "lc", "script"]]),
    )?;
    function(rt, "subprocess.PIPE")?;
    copy(rt, "stdout_pipe", "function")?;
    function(rt, "subprocess.PIPE")?;
    copy(rt, "stderr_pipe", "function")?;
    copy(rt, "function", "run")?;
    invoke(
        rt,
        &["args"],
        &[
            ("stdout", "stdout_pipe"),
            ("stderr", "stderr_pipe"),
            ("timeout", "timeout"),
        ],
    )?;
    copy(rt, "proc", "called")?;
    attr(rt, "proc", "returncode", "rc")?;
    literal(rt, "zero", json!(0))?;
    rt.call("web_compare", json!(["ne", "rc", "zero"]))?;
    if boolean(rt, "comparison")? {
        function(rt, "scaffolds_pkg.ScaffoldError")?;
        copy(rt, "error_class", "function")?;
        attr(rt, "proc", "stderr", "stderr")?;
        if !boolean(rt, "stderr")? {
            rt.call("eval_bytes", json!(["stderr", []]))?;
        }
        rt.call("logger_slice", json!(["stderr", -200, null, "stderr"]))?;
        let mut message = Text::from("container script failed: ");
        message.0.extend(formatted(rt, "stderr", "!r")?.0);
        text(rt, "message", message)?;
        call(rt, "error_class", &["message"], "error")?;
        return rt.call("task_raise", json!(["error"]));
    }
    attr(rt, "proc", "stdout", "stdout")?;
    Ok(returned("stdout"))
}
pub fn dispatch(operation: &str, rt: &mut dyn Runtime) -> Result<Value> {
    match operation {
        "context" => context(rt),
        "exec" => exec(rt),
        "copy" => copy_in(rt),
        "read" => read(rt),
        _ => Err(Failure::new(
            "ValueError",
            format!("unknown SWE-bench scaffold operation: {operation}"),
        )),
    }
}
