use super::{
    Failure, Result, Runtime,
    providers::{boolean, capture, copy, literal},
    runner_calls::{function, invoke},
};
use crate::value::truthy;
use serde_json::{Value, json};

fn returned(slot: &str) -> Value {
    json!({"provider_ref":slot})
}
fn binary(rt: &mut dyn Runtime) -> Result<Value> {
    capture(rt, "binary", "TE_CLAUDE_BIN", None)?;
    if !boolean(rt, "binary")? {
        function(rt, "shutil.which")?;
        literal(rt, "name", json!("claude"))?;
        invoke(rt, &["name"], &[])?;
        copy(rt, "binary", "called")?;
    }
    if !boolean(rt, "binary")? {
        return Err(Failure::new(
            "RuntimeError",
            "no claude binary found; set TE_CLAUDE_BIN",
        ));
    }
    function(rt, "os.path.realpath")?;
    invoke(rt, &["binary"], &[])?;
    Ok(returned("called"))
}
fn version(rt: &mut dyn Runtime) -> Result<Value> {
    let result = (|| {
        function(rt, "_run")?;
        copy(rt, "run", "function")?;
        literal(rt, "flag", json!("--version"))?;
        rt.call("eval_list", json!(["command", ["binary", "flag"]]))?;
        literal(rt, "timeout", json!(30))?;
        copy(rt, "function", "run")?;
        invoke(rt, &["command"], &[("timeout", "timeout")])?;
        rt.call("stdout_member", json!(["called", "stdout", "stdout"]))?;
        rt.call("stdout_member", json!(["stdout", "strip", "function"]))?;
        invoke(rt, &[], &[])?;
        if !boolean(rt, "called")? {
            literal(rt, "called", json!("unknown"))?;
        }
        Ok(returned("called"))
    })();
    if let Err(error) = &result
        && truthy(&rt.call("exception_matches", json!([error, ["Exception"]]))?)
    {
        return Ok(json!("unknown"));
    }
    result
}
fn grader(rt: &mut dyn Runtime) -> Result<Value> {
    function(rt, "swebench_repo")?;
    invoke(rt, &[], &[])?;
    copy(rt, "repo", "called")?;
    function(rt, "sys.path")?;
    copy(rt, "path", "function")?;
    if !truthy(&rt.call("provider_contains", json!(["repo", "path"]))?) {
        rt.call("stdout_member", json!(["path", "insert", "function"]))?;
        literal(rt, "zero", json!(0))?;
        invoke(rt, &["zero", "repo"], &[])?;
    }
    for (module, name, slot) in [
        (
            "swebench.harness.test_spec.test_spec",
            "make_test_spec",
            "spec",
        ),
        ("swebench.harness.grading", "get_eval_report", "report"),
    ] {
        rt.call("runner_import", json!([module, name, slot]))?;
    }
    rt.call("eval_tuple", json!(["imports", ["spec", "report"]]))?;
    Ok(returned("imports"))
}
pub fn run(operation: &str, rt: &mut dyn Runtime) -> Result<Value> {
    match operation {
        "claude_bin" => binary(rt),
        "cli_version" => version(rt),
        "_import_swebench" => grader(rt),
        _ => Err(Failure::new(
            "ValueError",
            "unknown SWE-bench host operation",
        )),
    }
}
