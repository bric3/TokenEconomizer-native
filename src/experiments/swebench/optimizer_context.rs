use super::{
    Result, Runtime,
    optimizer::{code_points, string},
    providers::{boolean, capture, literal},
};
use crate::value::truthy;
use serde_json::{Value, json};

fn log_path(rt: &mut dyn Runtime) -> Result<Value> {
    capture(rt, "path", "TE_OPTIMIZER_LOG", None)?;
    if boolean(rt, "path")? {
        return Ok(json!({"provider_ref":"path"}));
    }
    capture(rt, "root", "TOKENECONOMIZER_RESULTS", None)?;
    if !boolean(rt, "root")? {
        return Ok(Value::Null);
    }
    rt.call("context_log_path", json!([]))?;
    Ok(json!({"provider_ref":"path"}))
}
fn capture_path(rt: &mut dyn Runtime, method: &str, target: &str) -> Result<()> {
    rt.call("context_capture_path", json!([method, target]))
        .map(|_| ())
}
fn apply_path(rt: &mut dyn Runtime, function: &str, target: &str, slots: &[&str]) -> Result<()> {
    rt.call("context_apply_path", json!([function, target, slots]))
        .map(|_| ())
}
fn cell(rt: &mut dyn Runtime) -> Result<Value> {
    // Nested path calls capture the outer callable before evaluating their arguments.
    capture_path(rt, "dirname", "outer_dirname")?;
    rt.call("host_path", json!(["normpath", "normal", "run_dir_path"]))?;
    apply_path(rt, "outer_dirname", "parent", &["normal"])?;
    capture_path(rt, "join", "join")?;
    capture_path(rt, "basename", "outer_basename")?;
    rt.call("host_path", json!(["dirname", "arm_parent", "parent"]))?;
    apply_path(rt, "outer_basename", "arm", &["arm_parent"])?;
    rt.call("host_path", json!(["basename", "cell", "parent"]))?;
    apply_path(rt, "join", "cell_key", &["arm", "cell"])?;
    Ok(json!({"provider_ref":"cell_key"}))
}
fn set(rt: &mut dyn Runtime) -> Result<Value> {
    rt.call("context_clear", json!([]))?;
    rt.call("context_capture_update", json!([]))?;
    let none = rt.call("context_fields", json!([]))?;
    let selected: Vec<usize> = none
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .filter_map(|(index, value)| (!truthy(value)).then_some(index))
        .collect();
    rt.call("context_update", json!([selected]))?;
    Ok(Value::Null)
}
fn kind(rt: &mut dyn Runtime) -> Result<Value> {
    if !boolean(rt, "prompt")? {
        literal(rt, "prompt", json!(""))?;
    }
    let head = if let Some(mut head) = string(rt, "prompt")? {
        head.0.truncate(600);
        rt.call("context_head", json!([head.0]))?;
        Some(head)
    } else {
        rt.call("context_slice_head", json!([]))?;
        None
    };
    rt.call("context_kinds", json!([]))?;
    loop {
        let next = rt.call("context_next_kind", json!([]))?;
        if !truthy(&next[0]) {
            return Ok(Value::Null);
        }
        let found = if let Some(head) = &head
            && !next[1].is_null()
        {
            let signature = code_points(next[1].clone())?;
            signature.0.is_empty()
                || head
                    .0
                    .windows(signature.0.len())
                    .any(|window| window == signature.0)
        } else {
            truthy(&rt.call("provider_contains", json!(["signature", "head"]))?)
        };
        if found {
            return Ok(json!({"provider_ref":"kind"}));
        }
    }
}
pub fn run(method: &str, rt: &mut dyn Runtime) -> Result<Value> {
    match method {
        "optimizer_log_path" => log_path(rt),
        "_cell_of" => cell(rt),
        "set_optimizer_context" => set(rt),
        "optimizer_call_kind" => kind(rt),
        _ => unreachable!(),
    }
}
