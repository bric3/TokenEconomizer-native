use super::spiral_dispatch::failure;
use serde_json::{Value, json};
use std::path::Path;
use tokeneconomizer::experiments::usage::{
    claude::{self, UsageRecovery},
    codex, copilot,
};

fn text<'a>(args: &'a Value, key: &str) -> &'a str {
    args[key].as_str().unwrap_or("")
}

pub fn dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
    let Some(method) = op.strip_prefix("experiment.usage.") else {
        return Ok(None);
    };
    let result = (|| -> Result<Value, String> {
        Ok(match method {
            "claude._merge_usage" => claude::merge_usage(&args["a"], &args["b"])?,
            "claude._row_from_result" => claude::row_from_result(&args["result"])?,
            "claude.session_output_tokens" => {
                json!(claude::session_output_tokens(text(args, "text"))?)
            }
            "claude.recovery.new" => json!(UsageRecovery::default()),
            "claude.recovery.observe" | "claude.recovery.synthesize" => {
                let mut recovery: UsageRecovery =
                    serde_json::from_value(args["self"].clone()).map_err(|e| e.to_string())?;
                let mut messages = Vec::new();
                let value = if method.ends_with("observe") {
                    recovery.observe(&args["ev"]).map(|()| Value::Null)
                } else {
                    recovery.synthesize(
                        &args["reason"],
                        args["session_text"].as_str(),
                        &mut messages,
                    )
                };
                json!({"self":recovery,"value":value.unwrap_or_else(failure),"messages":messages})
            }
            "codex.row_usage" => codex::row_usage(text(args, "stream_text"))?,
            "codex.merge_usage" => codex::merge_usage(&args["a"], &args["b"])?,
            "copilot.sum_openai_usage" => copilot::sum_openai_usage(text(args, "log_text"))?,
            "copilot.read_log_dir" => {
                let path = Path::new(text(args, "log_dir"));
                let path = if path.is_relative() {
                    Path::new(text(args, "process_cwd")).join(path)
                } else {
                    path.to_path_buf()
                };
                json!(copilot::read_log_dir(&path))
            }
            "copilot.turns_from_stream" => {
                json!(copilot::turns_from_stream(text(args, "stream_text")))
            }
            "copilot.stream_output_tokens" => {
                copilot::stream_output_tokens(text(args, "stream_text"))?
            }
            "copilot.row_usage" => {
                copilot::row_usage(text(args, "stream_text"), text(args, "log_text"))?
            }
            "copilot.merge_usage" => copilot::merge_usage(&args["a"], &args["b"])?,
            _ => return Err(format!("unknown usage operation: {method}")),
        })
    })();
    Ok(Some(result.unwrap_or_else(failure)))
}
