use super::{
    Failure, Result, Runtime,
    codex_config::{integer, kind, sequence},
    settings::field,
};
use crate::value::{scalar_text, truthy};
use serde_json::{Value, json};
use std::{fs::File, io::Write, path::Path};

fn set(rt: &mut dyn Runtime, name: &str, value: Value) -> Result<()> {
    rt.call("config_set", json!([name, value])).map(|_| ())
}
fn update(rt: &mut dyn Runtime, value: Value) -> Result<()> {
    rt.call("runner_row_update", json!([value])).map(|_| ())
}
fn get(rt: &mut dyn Runtime, name: &str) -> Result<Value> {
    rt.call("runner_row_get", json!([name]))
}
pub fn run(agent: &str, args: &Value, rt: &mut dyn Runtime) -> Result<Value> {
    if !matches!(agent, "copilot" | "codex") {
        return Err(Failure::new(
            "ValueError",
            format!("unknown task wrapper: {agent}"),
        ));
    }
    if agent == "codex" {
        let caps = rt.call("config_kwarg", json!(["codex_config_capabilities", "json"]))?;
        let caps = if truthy(&caps) {
            sequence(&caps)?
        } else {
            Vec::new()
        };
        rt.call("runner_capabilities", json!(caps))?;
        set(rt, "_mcp_servers", Value::Null)?;
        set(rt, "_guard_python", json!(""))?;
    }
    let result = rt.call("runner_base_task", json!([]))?;
    let out_dir = result["out_dir"]
        .as_str()
        .ok_or_else(|| Failure::new("TypeError", "invalid task output directory"))?;
    let cwd = result["cwd"].as_str().unwrap_or(".");
    if agent == "copilot" {
        let text = rt.call("runner_row_pop", json!(["_debug_log", ""]))?;
        if truthy(&text) {
            let display = Path::new(out_dir).join("copilot_debug.log");
            let write = || -> Result<()> {
                let mut file = File::create(Path::new(cwd).join(&display))
                    .map_err(|e| Failure::for_path(e, &display))?;
                let text = text.as_str().ok_or_else(|| {
                    Failure::new(
                        "TypeError",
                        format!("write() argument must be str, not {}", kind(&text)),
                    )
                })?;
                file.write_all(text.as_bytes())
                    .map_err(|e| Failure::for_path(e, &display))
            };
            if let Err(error) = write() {
                if !error.is_a("builtins.OSError") {
                    return Err(error);
                }
                rt.call(
                    "log",
                    json!([format!(
                        "could not save copilot_debug.log: {}",
                        error.message
                    )]),
                )?;
            }
        } else {
            rt.call("log",json!(["no copilot debug log recovered -- untooling catalog will be empty and token accounting falls back to stream output only"]))?;
        }
    } else {
        let caps = field(rt, "runner", "_capabilities", "list", None)?;
        update(rt, json!({"codex_config_capabilities":caps}))?;
        let lean = field(
            rt,
            "runner",
            "_lean_instructions_tokens",
            "json",
            Some(json!(0)),
        )?;
        let lean = if truthy(&lean) {
            integer(&lean)?
        } else {
            "0".into()
        };
        // shortcut: JSON integer transport is bounded by serde_json; include large counters in the parity audit.
        let lean: Value = serde_json::from_str(&lean)
            .map_err(|e| Failure::new("OverflowError", e.to_string()))?;
        update(rt, json!({"codex_lean_instructions_tokens":lean}))?;
        if field(rt, "runner", "scaffold_name", "json", None)? == "rtk" {
            let fields = rt.call(
                "codex_function",
                json!([
                    "rtk_compliance",
                    [Path::new(out_dir).join("transcript.jsonl")],
                    {}
                ]),
            )?;
            update(rt, fields)?;
        }
        if field(rt, "runner", "scaffold_name", "json", None)? == "serena" {
            let fields = rt.call("codex_function",json!(["mcp_tool_calls",[Path::new(out_dir).join("transcript.jsonl")],{"server":"serena"}]))?;
            update(rt, fields)?;
            let sample = get(rt, "mcp_item_sample")?;
            if truthy(&sample) && !truthy(&get(rt, "mcp_tool_calls")?) {
                let sample = sample
                    .as_str()
                    .ok_or_else(|| Failure::new("TypeError", "invalid MCP item sample"))?;
                rt.call(
                    "log",
                    json!([format!(
                        "{}: MCP item with an unrecognised shape -- pin it: {}",
                        scalar_text(&args["task"]["instance_id"]),
                        sample.chars().take(200).collect::<String>()
                    )]),
                )?;
            }
        }
    }
    Ok(json!({"_runner_result":true}))
}
