use std::path::Path;

use serde_json::{Value, json};
use tokeneconomizer::{
    lifecycle,
    microskills::observe::{self, Observation, Vocabulary},
};

fn text<'a>(args: &'a Value, key: &str) -> &'a str {
    args[key].as_str().unwrap_or("")
}
fn optional<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args[key].as_str()
}
fn strings(args: &Value, key: &str) -> Result<Vec<String>, String> {
    serde_json::from_value(args[key].clone()).map_err(|e| e.to_string())
}
fn observation(args: &Value) -> Result<Observation, String> {
    serde_json::from_value(args["o"].clone()).map_err(|e| e.to_string())
}

pub fn dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
    let vocab = || {
        if args["vocab"].is_null() {
            Ok(Vocabulary::default())
        } else {
            serde_json::from_value::<Vocabulary>(args["vocab"].clone()).map_err(|e| e.to_string())
        }
    };
    let value = match op {
        "lifecycle.edited_files" | "lifecycle.edited_pyfiles" => {
            let calls: Vec<(String, Value, Value)> =
                serde_json::from_value(args["calls"].clone()).map_err(|e| e.to_string())?;
            json!(lifecycle::edited_files(
                calls.iter().map(|(tool, args, _)| (tool.as_str(), args)),
                op == "lifecycle.edited_pyfiles"
            ))
        }
        "observe.unwrap" => json!(observe::unwrap(text(args, "cmd"))),
        "observe.segments" => json!(observe::segments(text(args, "cmd"))?),
        "observe._tokens" => json!(text(args, "seg").split_whitespace().collect::<Vec<_>>()),
        "observe.repository_top" => json!(observe::repository_top(text(args, "start"))),
        "observe._cd_target" => json!(observe::cd_target(
            text(args, "cur"),
            text(args, "target"),
            optional(args, "root"),
            optional(args, "top")
        )),
        "observe._shell_path" => json!(observe::shell_path(text(args, "p"))),
        "observe._shell_isabs" => json!(observe::shell_isabs(text(args, "p"))),
        "observe.classify_command" => json!(observe::classify_command(
            text(args, "cmd"),
            args["call_index"].as_u64().unwrap_or(0) as usize,
            &vocab()?,
            text(args, "cwd"),
            optional(args, "root"),
            optional(args, "top")
        )?),
        "observe._runner_script" => {
            let rest = strings(args, "rest")?;
            json!(observe::runner_script(
                &rest.iter().map(String::as_str).collect::<Vec<_>>(),
                &vocab()?
            ))
        }
        "observe._probe_kind" => {
            let rest = strings(args, "args")?;
            json!(observe::probe_kind(
                text(args, "sub"),
                &rest.iter().map(String::as_str).collect::<Vec<_>>()
            ))
        }
        "observe.error_class" => json!(observe::error_class(text(args, "output"))),
        "observe._exit_code_hint" => json!(observe::exit_code_hint(text(args, "output"))),
        "observe._label_failed" => json!(observe::label_failed(
            &strings(args, "labels")?,
            text(args, "out")
        )),
        "observe._test_labels" => json!(observe::test_labels(&observation(args)?)),
        "observe.label_rule" => json!(observe::label_rule(text(args, "a"), text(args, "b"))),
        "observe._env_calls" => json!(observe::env_calls(
            text(args, "cmd"),
            text(args, "out"),
            args["success"].as_bool().unwrap_or(false)
        )?),
        "observe.form_failed" => json!(observe::form_failed(&observation(args)?)),
        "observe.form_worked" => json!(observe::form_worked(&observation(args)?)),
        "observe.observe_task" => json!(observe::observe_task(
            Path::new(text(args, "transcript_path")),
            &vocab()?
        )?),
        "observe._error_line" => json!(observe::error_line(text(args, "out"), text(args, "err"))),
        "observe._base" => json!(observe::base(text(args, "path"))),
        "observe._not_found_name" => json!(observe::not_found_name(text(args, "line"))?),
        "observe.is_bare" => json!(observe::is_bare(text(args, "form"))),
        _ => return Ok(None),
    };
    Ok(Some(value))
}
