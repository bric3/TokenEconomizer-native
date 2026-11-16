use serde_json::{Value, json};
use tokeneconomizer::plugin::hosts;

fn text<'a>(args: &'a Value, key: &str) -> &'a str {
    args[key].as_str().unwrap_or("")
}
pub(super) fn dispatch(op: &str, args: &Value) -> Result<Value, String> {
    let host = text(args, "host");
    let payload = &args["p"];
    let registered: Vec<String> = args
        .get("registered")
        .map(|v| serde_json::from_value(v.clone()))
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| hosts::HOSTS.into_iter().map(String::from).collect());
    Ok(match op {
        "host_helpers.constants" => {
            json!({"RESULT_CAP":hosts::RESULT_CAP,"PROGRESS":hosts::PROGRESS,"HOSTS":hosts::HOSTS})
        }
        "host_helpers.response_text" => json!(hosts::response_text(
            &args["resp"],
            args["cap"].as_i64().unwrap_or(hosts::RESULT_CAP as i64)
        )?),
        "host_helpers.merge_outputs" => {
            hosts::merge_outputs(args["outputs"].as_array().ok_or("expected outputs")?)?
        }
        "host_helpers._failure_response" => hosts::failure_response(payload),
        "host_helpers._exit_code" => hosts::exit_code(&args["resp"]),
        "host_helpers.claude_success_code" => json!(hosts::claude_success_code(
            text(args, "name"),
            &args["args"],
            args.get("resp")
        )?),
        "host_helpers.tool_call" => json!(hosts::tool_call(host, payload)?),
        "host_helpers.session_id" => json!(hosts::session_id(host, payload)),
        "host_helpers.turn_key" => json!(hosts::turn_key(host, payload)),
        "host_helpers.env_session_id" => json!(hosts::env_session_id(host, &args["env"])),
        "host_helpers.is_subagent" => json!(hosts::is_subagent(host, payload)),
        "host_helpers.context" => hosts::context(host, text(args, "event"), text(args, "text")),
        "host_helpers.notice" => hosts::notice(host, text(args, "text")),
        "host_helpers.relay" => json!(hosts::relay(text(args, "text"))),
        "host_helpers.stop_hint" => json!(hosts::stop_hint(
            &args["headless"],
            text(args, "interrupt"),
            text(args, "end_run")
        )),
        "host_helpers.stopped_normally" => json!(hosts::stopped_normally(host, payload)),
        "host_helpers.completed_at_end" => json!(hosts::completed_at_end(host, payload)),
        "host_helpers.default_host" => {
            json!(hosts::default_host(
                &args["env"],
                args["recent"].as_str(),
                &registered
            ))
        }
        "host_helpers.detect" => json!(hosts::detect(
            &args["payload"],
            &args["env"],
            args["hint"].as_str(),
            &registered
        )),
        _ => return Err(format!("unknown host operation: {op}")),
    })
}
