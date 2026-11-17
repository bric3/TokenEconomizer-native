use super::{callback::CallbackTape, spiral_dispatch::failure};
use serde_json::{Value, json};
use std::cell::RefCell;
use tokeneconomizer::{
    experiments::nudge_guard::{NudgeGuard, RESUME_HEADER},
    rules,
};

fn text<'a>(args: &'a Value, key: &str) -> &'a str {
    args[key].as_str().unwrap_or("")
}

pub fn dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
    let Some(method) = op.strip_prefix("nudge_guard.") else {
        return Ok(None);
    };
    if method == "constants" {
        return Ok(Some(
            json!({"RESUME_HEADER":RESUME_HEADER,"DEFAULT_THRESHOLDS":rules::scaled_thresholds("moderate")}),
        ));
    }
    if method == "new" {
        return Ok(Some(json!(NudgeGuard::new(
            args["envelope"].as_f64(),
            args.get("thr").cloned(),
            args["nudge_budget"].as_i64().unwrap_or(1),
            args["verbose"] == true,
            args["resume_header"].as_str().map(str::to_owned),
        ))));
    }
    let mut guard: NudgeGuard =
        serde_json::from_value(args["self"].clone()).map_err(|e| e.to_string())?;
    let tape = RefCell::new(CallbackTape::new(args));
    let mut judge = |digest: &str| {
        tape.borrow_mut()
            .call(json!({"callback":"judge","digest":digest}))
            .map(|v| v["value"].clone())
    };
    let mut writer = |digest: &str| {
        tape.borrow_mut()
            .call(json!({"callback":"writer","digest":digest}))
            .map(|v| v["value"].clone())
    };
    let mut messages = Vec::new();
    let result = match method {
        "monitor" => guard
            .monitor(
                text(args, "line"),
                if args["judge"] == true {
                    Some(&mut judge)
                } else {
                    None
                },
                &mut messages,
            )
            .map(|v| json!(v)),
        "_sampled_judge" => guard.sampled_judge(
            text(args, "digest"),
            if args["judge"] == true {
                Some(&mut judge)
            } else {
                None
            },
            &mut messages,
        ),
        "_nudge_body" => guard
            .nudge_body(
                if args["writer"] == true {
                    Some(&mut writer)
                } else {
                    None
                },
                &mut messages,
            )
            .map(|v| json!(v)),
        "continuation" => guard
            .continuation(
                text(args, "base_prompt"),
                if args["writer"] == true {
                    Some(&mut writer)
                } else {
                    None
                },
                &mut messages,
            )
            .map(|v| json!(v)),
        "_stop" => Ok(json!(guard.stop(
            text(args, "action"),
            text(args, "reason"),
            &mut messages
        ))),
        _ => return Err(format!("unknown nudge guard method: {method}")),
    };
    Ok(Some(tape.into_inner().finish(
        json!({"self":guard,"value":result.unwrap_or_else(failure),"messages":messages}),
    )))
}
