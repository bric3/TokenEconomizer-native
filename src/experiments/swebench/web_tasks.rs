use super::{
    Result, Runtime,
    providers::{boolean, literal},
    runner_calls::member,
    web_output::strip,
};
use crate::value::truthy;
use serde_json::{Value, json};

fn read(rt: &mut dyn Runtime) -> Result<()> {
    literal(rt, "output", json!([]))?;
    rt.call("stdout_iter", json!([]))?;
    while truthy(&rt.call("stdout_next", json!([]))?) {
        strip(rt, false, "stripped")?;
        if boolean(rt, "stripped")? {
            rt.call("tasks_decode", json!([]))?;
            rt.call(
                "provider_method",
                json!(["decoded", "get", ["id"], "identifier"]),
            )?;
            rt.call("tasks_append", json!([]))?;
        }
    }
    Ok(())
}
fn opened(rt: &mut dyn Runtime) -> Result<Value> {
    rt.call("tasks_open", json!([]))?;
    let result = read(rt);
    rt.call("tasks_exit", json!([result.as_ref().err()]))?;
    if let Err(error) = result {
        if !boolean(rt, "exit_result")? {
            return Err(error);
        }
        return Ok(Value::Null);
    }
    Ok(json!({"provider_ref":"output"}))
}
pub fn load(rt: &mut dyn Runtime) -> Result<Value> {
    rt.call("context_capture_path", json!(["join", "join"]))?;
    member(rt, "repo", "repo")?;
    literal(rt, "projects", json!("projects"))?;
    member(rt, "project", "project")?;
    literal(rt, "tasks", json!("tasks.jsonl"))?;
    rt.call(
        "context_apply_path",
        json!(["join", "path", ["repo", "projects", "project", "tasks"]]),
    )?;
    match opened(rt) {
        Err(error) if error.is_a("builtins.OSError") => Ok(json!([])),
        result => result,
    }
}
