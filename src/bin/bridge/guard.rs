use std::{cell::RefCell, path::Path};

use serde_json::{Map, Value, json};
use tokeneconomizer::loopguard::{
    shell_edits,
    state::{self, SpiralState},
    turn::{self, BackgroundJudge, FoldOptions, Judge, TurnGuard},
};

struct CallbackTape {
    replies: Vec<Value>,
    index: usize,
    pending: Option<Value>,
}

impl CallbackTape {
    fn call(
        &mut self,
        kind: &str,
        guard: Option<&TurnGuard>,
        digest: Option<&str>,
    ) -> Result<Value, String> {
        if let Some(reply) = self.replies.get(self.index) {
            self.index += 1;
            if let Some(error) = reply["error"].as_str() {
                return Err(error.into());
            }
            return Ok(reply.clone());
        }
        self.pending = Some(json!({"callback": kind, "guard": guard, "digest": digest}));
        Err("callback pending".into())
    }
}

struct BackgroundTransport<'a>(&'a RefCell<CallbackTape>);

impl BackgroundJudge for BackgroundTransport<'_> {
    fn poll(&mut self, guard: &mut TurnGuard) -> Result<(bool, Value), String> {
        let reply = self.0.borrow_mut().call("poll", Some(guard), None)?;
        *guard = serde_json::from_value(reply["guard"].clone()).map_err(|e| e.to_string())?;
        Ok((
            reply["value"][0].as_bool().unwrap_or(false),
            reply["value"][1].clone(),
        ))
    }

    fn start(&mut self, guard: &mut TurnGuard, digest: &str) -> Result<Value, String> {
        let reply = self
            .0
            .borrow_mut()
            .call("start", Some(guard), Some(digest))?;
        *guard = serde_json::from_value(reply["guard"].clone()).map_err(|e| e.to_string())?;
        Ok(reply["value"].clone())
    }
}

fn turn_method(op: &str, args: &Value) -> Result<Value, String> {
    let mut guard: TurnGuard =
        serde_json::from_value(args["self"].clone()).map_err(|e| e.to_string())?;
    let tape = RefCell::new(CallbackTape {
        replies: args["replies"].as_array().cloned().unwrap_or_default(),
        index: 0,
        pending: None,
    });
    let mut sampled = |digest: &str| {
        tape.borrow_mut()
            .call("judge", None, Some(digest))
            .map(|reply| reply["value"].clone())
    };
    let mut writer = |digest: &str| {
        tape.borrow_mut()
            .call("writer", None, Some(digest))
            .map(|reply| reply["value"].clone())
    };
    let mut background = BackgroundTransport(&tape);
    let value = match op {
        "turn.to_dict" => guard.to_dict(),
        "turn.resolved_proxy" => {
            json!(guard.resolved_proxy(args["stopped_normally"].as_bool().unwrap_or(true)))
        }
        "turn.verification_state" => json!(guard.verification_state()),
        "turn.live_digest" => json!(guard.live_digest(args["digest"].as_str().unwrap_or(""))),
        "turn.summary" => guard.summary(args["stopped_normally"].as_bool().unwrap_or(true)),
        "turn.after_nudge" => {
            guard.after_nudge();
            Value::Null
        }
        "turn.nudge_body" => json!(guard.nudge_body(
            args["envelope"].as_f64(),
            if args["writer"] == true {
                Some(&mut writer)
            } else {
                None
            }
        )),
        "turn.fold" => {
            let judge = match args["judge"].as_str() {
                Some("sampled") => Some(Judge::Sampled(&mut sampled)),
                Some("background") => Some(Judge::Background(&mut background)),
                _ => None,
            };
            json!(guard.fold(
                args["name"].as_str().unwrap_or(""),
                &args["args"],
                args["result"].as_str().unwrap_or(""),
                args["exit_code"].as_i64(),
                FoldOptions {
                    envelope: args["envelope"].as_f64(),
                    thresholds: args.get("thr"),
                    judge,
                    nudge_budget: args["nudge_budget"].as_u64().unwrap_or(1)
                }
            )?)
        }
        _ => return Err(format!("unknown turn method: {op}")),
    };
    if let Some(pending) = tape.into_inner().pending {
        return Ok(pending);
    }
    Ok(json!({"guard": guard, "value": value}))
}

pub fn dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
    let value = match op {
        "turn.new" => json!(TurnGuard::new(
            args["turn_key"].as_str().unwrap_or(""),
            &args["data"]
        )?),
        "turn.state_to_dict" => args["st"].clone(),
        "turn.state_from_dict" => json!(
            serde_json::from_value::<SpiralState>(if args["d"].is_object() {
                args["d"].clone()
            } else {
                json!({})
            })
            .map_err(|e| e.to_string())?
        ),
        "turn.to_dict"
        | "turn.resolved_proxy"
        | "turn.verification_state"
        | "turn.live_digest"
        | "turn.summary"
        | "turn.after_nudge"
        | "turn.nudge_body"
        | "turn.fold" => turn_method(op, args)?,
        "turn.exit_status_covers" => json!(turn::exit_status_covers(
            args["cmd"].as_str().unwrap_or("")
        )?),
        "turn.status_of_tests" => json!(turn::status_of_tests(args["cmd"].as_str().unwrap_or(""))?),
        "turn.runner_verdict" => json!(turn::runner_verdict(args["text"].as_str().unwrap_or(""))),
        "turn._pieces" => json!(turn::pieces(args["cmd"].as_str().unwrap_or(""))?),
        "turn.verify_search" => json!(
            turn::VERIFY_RE
                .is_match(args["text"].as_str().unwrap_or(""))
                .map_err(|e| e.to_string())?
        ),
        "shell_edits.shell_edit_targets" => json!(shell_edits::shell_edit_targets(
            args["cmd"].as_str().unwrap_or("")
        )?),
        "shell_edits.shell_revert_targets" => json!(shell_edits::shell_revert_targets(
            args["cmd"].as_str().unwrap_or("")
        )?),
        "shell_edits._split_heredocs" => json!(shell_edits::split_heredocs(
            args["cmd"].as_str().unwrap_or("")
        )?),
        "shell_edits.fold_shell_edits" => {
            let mut state: SpiralState =
                serde_json::from_value(args["st"].clone()).map_err(|e| e.to_string())?;
            let names = serde_json::from_value::<Vec<String>>(args["names"].clone())
                .map_err(|e| e.to_string())?;
            shell_edits::fold_shell_edits(&mut state, &names);
            json!(state)
        }
        "state.new" => json!(SpiralState::default()),
        "state.canon_tool" => json!(state::canon_tool(args["name"].as_str().unwrap_or(""))),
        "state.basename" => {
            if args["p"].is_null() {
                json!("")
            } else {
                json!(state::basename(args["p"].as_str().ok_or("expected path")?))
            }
        }
        "state._failure_sig" => json!(state::failure_sig(args["text"].as_str().unwrap_or(""))),
        "state.completed_calls" => {
            let mut pending: Map<String, Value> =
                serde_json::from_value(args["pend"].clone()).map_err(|e| e.to_string())?;
            let calls = state::completed_calls(args["line"].as_str().unwrap_or(""), &mut pending);
            json!({"calls": calls, "pend": pending})
        }
        "state.state_from_trajectory" => {
            let state = if let Some(trajectory) = args.get("trajectory") {
                state::state_from_calls(
                    &serde_json::from_value::<Vec<_>>(trajectory.clone())
                        .map_err(|e| e.to_string())?,
                )
            } else {
                state::state_from_trajectory(Path::new(
                    args["path"].as_str().ok_or("expected path")?,
                ))
            };
            json!(state)
        }
        "state.edit_churn"
        | "state.distinct_edit_files"
        | "state.stagnation"
        | "state.update"
        | "state.recent_digest"
        | "state.snapshot" => {
            let mut state: SpiralState =
                serde_json::from_value(args["self"].clone()).map_err(|e| e.to_string())?;
            match op {
                "state.edit_churn" => json!(state.edit_churn()),
                "state.distinct_edit_files" => json!(state.distinct_edit_files()),
                "state.stagnation" => json!(state.stagnation()),
                "state.recent_digest" => json!(
                    state
                        .recent_digest(args["n"].as_i64().unwrap_or(24), args["envelope"].as_f64())
                ),
                "state.snapshot" => state.snapshot(),
                _ => {
                    state.update(
                        args["tool_name"].as_str().unwrap_or(""),
                        &args["args"],
                        args["result_text"].as_str().unwrap_or(""),
                        args["failed"].as_bool(),
                    );
                    json!(state)
                }
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(value))
}
