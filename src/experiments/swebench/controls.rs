use super::{Failure, Result, Runtime, codex_config::integer};
use crate::value::truthy;
use serde_json::{Value, json};

fn capture(rt: &mut dyn Runtime, key: &str, default: Option<Value>) -> Result<()> {
    let mut args = vec![json!(key)];
    if let Some(value) = default {
        args.push(value);
    }
    rt.call("control_capture", json!(args)).map(|_| ())
}
fn boolean(rt: &mut dyn Runtime, key: &str) -> Result<bool> {
    rt.call("control_bool", json!([key])).map(|v| truthy(&v))
}
fn reference(name: &str, key: &str) -> Value {
    json!([name, "ref", key])
}
fn literal(name: &str, value: Value) -> Value {
    json!([name, "value", value])
}
fn budget(rt: &mut dyn Runtime, int_max_digits: usize) -> Result<Value> {
    let input = rt.call("control_integer_input", json!(["spiral_nudge_budget"]))?;
    match input["kind"].as_str().unwrap_or("") {
        "int" => Ok(reference("nudge_budget", "spiral_nudge_budget")),
        "external" => {
            rt.call("control_external_int", json!(["spiral_nudge_budget"]))?;
            Ok(reference("nudge_budget", "spiral_nudge_budget"))
        }
        kind => {
            let mut number = integer(&input["value"]).map_err(|mut e| {
                if kind == "tuple" {
                    e.message = e.message.replace("'list'", "'tuple'");
                }
                e
            })?;
            if let Some(text) = input["value"].as_str()
                && int_max_digits != 0
                && text.chars().filter(|c| c.is_numeric()).count() > int_max_digits
            {
                return Err(Failure::new(
                    "ValueError",
                    format!(
                        "Exceeds the limit ({int_max_digits} digits) for integer string conversion: value has {} digits; use sys.set_int_max_str_digits() to increase the limit",
                        text.chars().filter(|c| c.is_numeric()).count()
                    ),
                ));
            }
            if number == "-0" {
                number = "0".into();
            }
            Ok(json!(["nudge_budget", "integer", number]))
        }
    }
}
pub fn accepts(param: &Value, rt: &mut dyn Runtime) -> Result<Value> {
    let parameters = match rt.call("control_signature", json!([])) {
        Ok(value) => value,
        Err(error) if error.is_a("builtins.TypeError") || error.is_a("builtins.ValueError") => {
            return Ok(json!(false));
        }
        Err(error) => return Err(error),
    };
    if matches!(param, Value::Array(_) | Value::Object(_)) {
        return Ok(json!(false));
    }
    Ok(json!(
        parameters.as_array().is_some_and(|p| p.contains(param))
    ))
}
pub fn build(agent: &str, int_max_digits: usize, rt: &mut dyn Runtime) -> Result<Value> {
    let (kind, mut args) = if agent == "codex" {
        capture(rt, "codex_spiral_policy", None)?;
        if !truthy(&rt.call("control_present", json!(["codex_spiral_policy"]))?) {
            return Ok(Value::Null);
        }
        ("codex", vec![reference("policy", "codex_spiral_policy")])
    } else {
        capture(rt, "spiral_guard", None)?;
        if !boolean(rt, "spiral_guard")? {
            return Ok(Value::Null);
        }
        for key in ["abort_envelope", "spiral_judge", "spiral_thr"] {
            capture(rt, key, None)?;
        }
        capture(rt, "spiral_nudge", None)?;
        let default = if boolean(rt, "spiral_nudge")? { 1 } else { 0 };
        capture(rt, "spiral_nudge_budget", Some(json!(default)))?;
        let budget = budget(rt, int_max_digits)?;
        capture(rt, "spiral_nudge_writer", None)?;
        (
            "nudge",
            vec![
                reference("envelope", "abort_envelope"),
                reference("judge", "spiral_judge"),
                reference("thr", "spiral_thr"),
                budget,
                reference("nudge_writer", "spiral_nudge_writer"),
            ],
        )
    };
    let verbose = rt.call("control_env", json!(["TE_VERBOSE", "1"]))? == "1";
    args.push(literal("verbose", json!(verbose)));
    if kind == "nudge"
        && truthy(&rt.call("config_function", json!(["guard_accepts", ["never_abort"]]))?)
    {
        capture(rt, "spiral_never_abort", None)?;
        args.push(literal(
            "never_abort",
            json!(boolean(rt, "spiral_never_abort")?),
        ));
    }
    rt.call("control_new", json!([kind, args]))?;
    Ok(json!({"_control_result":true}))
}
pub fn row(agent: &str, rt: &mut dyn Runtime) -> Result<Value> {
    if agent == "codex" {
        rt.call("control_telemetry", json!([]))?;
        rt.call("control_row_value", json!(["aborted", false]))?;
        if truthy(&rt.call("control_snapshot", json!(["snapshot"]))?) {
            rt.call(
                "control_snapshot_get",
                json!(["codex_spiral_tool_turns", "tool_turns"]),
            )?;
        } else {
            rt.call(
                "control_row_value",
                json!(["codex_spiral_tool_turns", null]),
            )?;
        }
        rt.call(
            "control_row_items",
            json!([
                ["codex_spiral_nudges", "nudges"],
                ["codex_spiral_events", "events"],
                ["codex_spiral_thread_captured", "thread_id_captured"],
                [
                    "codex_spiral_paid_judge_tokens",
                    "paid_judge_effective_tokens"
                ],
                ["codex_spiral_policy", "policy"]
            ]),
        )?;
    } else {
        rt.call(
            "control_row_attributes",
            json!([
                ["aborted", "aborted"],
                ["spiral_abort", "aborted"],
                ["spiral_nudges", "nudges_done"],
                ["spiral_abort_suppressed", "abort_suppressed", false],
                ["spiral_events", "events"],
                ["spiral_judge_tokens", "judge_tokens"],
                ["spiral_nudge_writer_tokens", "nudge_writer_tokens"]
            ]),
        )?;
    }
    Ok(json!({"_control_row":true}))
}
pub fn dispatch(method: &str, args: &Value, rt: &mut dyn Runtime) -> Result<Value> {
    if method == "accepts" {
        return accepts(&args["param"], rt);
    }
    let agent = args["agent"].as_str().unwrap_or("");
    if !matches!(agent, "claude" | "copilot" | "codex") {
        return Err(Failure::new(
            "ValueError",
            format!("unknown controller agent: {agent}"),
        ));
    }
    match method {
        "build" => build(
            agent,
            args["int_max_digits"].as_u64().unwrap_or(4300) as usize,
            rt,
        ),
        "row" => row(agent, rt),
        "continuation" => {
            let params = if agent == "codex" {
                json!([])
            } else {
                json!(["prompt"])
            };
            rt.call("control_continue", params)?;
            Ok(json!({"_control_result":true}))
        }
        _ => Err(Failure::new(
            "ValueError",
            format!("unknown controller operation: {method}"),
        )),
    }
}
