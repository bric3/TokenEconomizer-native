use super::{
    Result, Runtime,
    optimizer::{capture_log, code_points, string},
    providers::{boolean, capture, equal, literal},
};
use crate::python_json::Text;
use serde_json::{Value, json};

fn field(rt: &mut dyn Runtime, name: &str, slot: &str) -> Result<()> {
    rt.call("logger_field", json!([name, slot])).map(|_| ())
}
fn slice(
    rt: &mut dyn Runtime,
    slot: &str,
    start: Option<isize>,
    end: Option<usize>,
    target: &str,
) -> Result<()> {
    if let Some(value) = string(rt, slot)? {
        let start = start.map_or(0, |n| value.0.len().saturating_sub(n.unsigned_abs()));
        let end = end.unwrap_or(value.0.len()).min(value.0.len());
        let sliced = Text(value.0[start..end].to_vec());
        rt.call("logger_literal", json!([target, sliced.0]))?;
    } else {
        rt.call("logger_slice", json!([slot, start, end, target]))?;
    }
    field(rt, target, target)
}
fn length(rt: &mut dyn Runtime, slot: &str, target: &str) -> Result<()> {
    if let Some(value) = string(rt, slot)? {
        literal(rt, target, json!(value.0.len()))?;
    } else {
        rt.call("logger_length", json!([slot, target]))?;
    }
    field(rt, target, target)
}
fn row(rt: &mut dyn Runtime) -> Result<()> {
    rt.call("logger_capture_write", json!([]))?;
    rt.call("logger_timestamp", json!(["%Y-%m-%dT%H:%M:%SZ"]))?;
    rt.call("logger_context", json!([]))?;
    rt.call("logger_kind", json!([]))?;
    for name in ["kind", "model", "usage", "stop_reason"] {
        field(rt, name, name)?;
    }
    rt.call(
        "logger_equal",
        json!(["stop_reason", "max_tokens", "truncated"]),
    )?;
    field(rt, "truncated", "truncated")?;
    slice(rt, "prompt", None, Some(400), "prompt_head")?;
    slice(rt, "prompt", Some(-400), None, "prompt_tail")?;
    length(rt, "prompt", "prompt_chars")?;
    slice(rt, "text", None, Some(4000), "response")?;
    length(rt, "text", "response_chars")?;
    // The codec sees the retained objects after callbacks have finished mutating them.
    let encoded = rt.call("logger_encode", json!([]))?;
    let mut line = encoded.as_str().unwrap().to_owned();
    line.push('\n');
    rt.call("logger_write", json!([line]))?;
    Ok(())
}
fn write(rt: &mut dyn Runtime) -> Result<()> {
    rt.call("logger_capture_makedirs", json!([]))?;
    rt.call("host_path", json!(["dirname", "parent", "path"]))?;
    rt.call("logger_makedirs", json!([]))?;
    rt.call("logger_open", json!([]))?;
    let result = row(rt);
    rt.call("logger_close", json!([result.err()]))?;
    Ok(())
}
pub fn record(rt: &mut dyn Runtime) -> Result<Value> {
    rt.call("logger_path", json!([]))?;
    if !boolean(rt, "path")? {
        return Ok(Value::Null);
    }
    match write(rt) {
        Ok(()) => {}
        Err(error) if error.is_a("builtins.Exception") => {
            capture_log(rt)?;
            let detail = code_points(rt.call("logger_error_format", json!([error]))?)?;
            let mut message = Text::from("could not write optimizer log: ");
            message.0.extend(&detail.0);
            rt.call("logger_report", json!([message.0]))?;
        }
        Err(error) => return Err(error),
    }
    Ok(Value::Null)
}
pub fn verbose(rt: &mut dyn Runtime) -> Result<Value> {
    capture(rt, "verbose", "TE_VERBOSE", Some("1"))?;
    if equal(rt, "verbose", "1")? {
        rt.call("logger_capture_print", json!([]))?;
        let detail = code_points(rt.call("logger_format", json!(["msg"]))?)?;
        let mut message = Text::from("[te_harness] ");
        message.0.extend(&detail.0);
        rt.call("logger_print", json!([message.0]))?;
        rt.call("logger_flush", json!([]))?;
    }
    Ok(Value::Null)
}
