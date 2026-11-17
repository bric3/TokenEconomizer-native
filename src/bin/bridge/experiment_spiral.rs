use super::callback::{CallbackTape, RunnerTransport};
use serde_json::{Value, json};
use std::{cell::RefCell, path::Path};
use tokeneconomizer::{
    experiments::{
        codex_spiral::{self as cs, EnvelopeTracker, Event, Policy, ResumeGuard, State},
        spiral_stage::SpiralStage,
    },
    runner::Runner,
};

fn text<'a>(args: &'a Value, key: &str) -> &'a str {
    args[key].as_str().unwrap_or("")
}
fn decode<T: serde::de::DeserializeOwned>(value: &Value) -> Result<T, String> {
    serde_json::from_value(value.clone()).map_err(|e| e.to_string())
}
pub(super) fn failure(error: String) -> Value {
    let (kind, message) = error
        .split_once(": ")
        .filter(|(kind, _)| {
            ["TypeError", "AttributeError", "IndexError", "ValueError"].contains(kind)
        })
        .unwrap_or(("ValueError", &error));
    json!({"raised":kind,"message":message})
}
fn codex(method: &str, args: &Value) -> Result<Value, String> {
    Ok(match method {
        "constants" => {
            json!({"PROFILE_VERSION":cs::PROFILE_VERSION,"DEFENSIVE_PROFILE_VERSION":cs::DEFENSIVE_PROFILE_VERSION,"CODEX_DEFAULT_PROFILE":cs::CODEX_DEFAULT_PROFILE,"DEFAULT_POLICY":Policy::default(),"SHIPPED_POLICY":Policy::shipped(),"DEFENSIVE_POLICY":Policy::defensive()})
        }
        "policy.at_threshold" => json!(
            decode::<Policy>(&args["self"])?
                .at_threshold(args["turns"].as_f64().ok_or("expected threshold")?)
        ),
        "shipped_policy" => json!(
            Policy::shipped().at_threshold(args["threshold"].as_f64().ok_or("expected threshold")?)
        ),
        "defensive_policy" => json!(
            Policy::defensive()
                .at_threshold(args["threshold"].as_f64().ok_or("expected threshold")?)
        ),
        "profile_policy" => json!(cs::profile_policy(
            text(args, "profile"),
            args["threshold"].as_f64().ok_or("expected threshold")?
        )),
        "profile_version" => json!(cs::profile_version(text(args, "profile"))),
        "_normalize" => json!(cs::normalize(
            text(args, "text"),
            args["limit"].as_i64().unwrap_or(180)
        )),
        "_command_fingerprint" => json!(cs::command_fingerprint(text(args, "command"))),
        "green_is_trustworthy" => json!(cs::green_is_trustworthy(text(args, "command"))),
        "verification_scopes" => json!(cs::verification_scopes(text(args, "command"))),
        "is_test_command" => json!(cs::is_test_command(text(args, "command"))),
        "is_verification_command" => json!(cs::is_verification_command(text(args, "command"))),
        "is_research_command" => json!(cs::is_research_command(text(args, "command"))),
        "failure_signatures" => json!(cs::failure_signatures(text(args, "output"))),
        "patch_paths" => json!(cs::patch_paths(text(args, "command"))),
        "shell_patch_applied" => json!(cs::shell_patch_applied(
            text(args, "command"),
            text(args, "output"),
            &args["exit_code"]
        )),
        "parse_events" => json!(cs::parse_events(text(args, "line"))?),
        "iterator" => {
            let mut cursor: cs::EventCursor = if args["self"].is_null() {
                cs::EventCursor::new(args["first_thread_only"] == true)
            } else {
                decode(&args["self"])?
            };
            let events = cursor.feed(text(args, "line"))?;
            json!({"events":events,"self":cursor})
        }
        "replay_events" => {
            let (decision, state) = cs::replay_events(
                decode::<Vec<Event>>(&args["events"])?.into_iter().map(Ok),
                &decode(&args["policy"])?,
            )?;
            json!({"decision":decision,"state":state})
        }
        "_percentile" => json!(cs::percentile(
            &decode::<Vec<f64>>(&args["values"])?,
            args["pct"].as_f64().ok_or("expected percentile")?
        )?),
        "nudge_text" => json!(cs::nudge_text(text(args, "reason"))),
        "state.new" => json!(State::new(args["recent_window"].as_i64().unwrap_or(12))?),
        "guard.new" => json!(ResumeGuard::new(
            decode(&args["policy"])?,
            args["verbose"] == true
        )?),
        "tracker.new" => {
            let mut value = serde_json::to_value(EnvelopeTracker::default()).unwrap();
            value
                .as_object_mut()
                .unwrap()
                .extend(args["fields"].as_object().cloned().unwrap_or_default());
            json!(decode::<EnvelopeTracker>(&value)?)
        }
        _ if method.starts_with("state.") => {
            let mut state: State = decode(&args["self"])?;
            let value = match method {
                "state.update" => {
                    state.update(&decode(&args["event"])?);
                    Value::Null
                }
                "state.decision" => json!(state.decision(&decode(&args["policy"])?)),
                "state.snapshot" => state.snapshot(),
                "state.distinct_edit_files" => json!(state.distinct_edit_files()),
                "state.edit_churn" => json!(state.edit_churn()),
                "state.green_after_edit" => json!(state.green_after_edit()),
                "state.currently_green" => json!(state.currently_green()),
                "state.repeated_failure_count" => json!(state.repeated_failure_count()),
                _ => return Err(format!("unknown Codex state method: {method}")),
            };
            json!({"self":state,"value":value})
        }
        _ if method.starts_with("tracker.") => {
            let mut tracker: EnvelopeTracker = decode(&args["self"])?;
            let value = match method {
                "tracker.threshold" => json!(tracker.threshold()?),
                "tracker.snapshot" => tracker.snapshot()?,
                "tracker.update" => {
                    tracker.update(&args["tool_turns"], args["resolved"] == true)?;
                    Value::Null
                }
                _ => return Err(format!("unknown Codex tracker method: {method}")),
            };
            json!({"self":tracker,"value":value})
        }
        _ if method.starts_with("guard.") => {
            let mut guard: ResumeGuard = decode(&args["self"])?;
            let mut messages = Vec::new();
            let value = match method {
                "guard.monitor" => json!(guard.monitor(text(args, "line"), &mut messages)?),
                "guard.continuation" => json!(guard.continuation()),
                "guard.telemetry" => guard.telemetry(),
                "guard.segment" => json!(guard.nudges_done),
                _ => return Err(format!("unknown Codex guard method: {method}")),
            };
            json!({"self":guard,"value":value,"messages":messages})
        }
        _ => return Err(format!("unknown Codex spiral operation: {method}")),
    })
}
pub fn dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
    if let Some(method) = op.strip_prefix("codex_spiral.") {
        return Ok(Some(codex(method, args).unwrap_or_else(failure)));
    }
    let Some(method) = op.strip_prefix("spiral_stage.") else {
        return Ok(None);
    };
    let tape = RefCell::new(CallbackTape::new(args));
    let mut runner = RunnerTransport(&tape);
    let mut snapshot = |path: &Path| {
        tape.borrow_mut()
            .call(json!({"callback":"snapshot","path":path}))
            .map(|v| v["value"].clone())
    };
    let result = (|| {
        if method == "new" {
            return Ok(
                json!({"self":SpiralStage::new(decode(&args["cfg"])?, args["max_turns"].as_f64(), args["optimizer_model"].as_str().unwrap_or("agent"), args["agent"].as_str().map(str::to_owned))}),
            );
        }
        let mut stage: SpiralStage = decode(&args["self"])?;
        let mut srow = args["srow"].clone();
        let result = match method {
            "run_kwargs" => stage.run_kwargs(),
            "after_task" => stage
                .after_task(
                    &args["row"],
                    Path::new(text(args, "run_dir_path")),
                    &mut srow,
                    if args["adapter"] == true {
                        Some(&mut snapshot)
                    } else {
                        None
                    },
                )
                .map(|v| json!(v)),
            "judge" => Ok(stage.judge(
                text(args, "digest"),
                if args["runner"] == true {
                    Some(&mut runner as &mut dyn Runner)
                } else {
                    None
                },
            )),
            "nudge_writer" => Ok(stage.nudge_writer(
                text(args, "digest"),
                if args["runner"] == true {
                    Some(&mut runner as &mut dyn Runner)
                } else {
                    None
                },
            )),
            _ => return Err(format!("unknown spiral stage operation: {method}")),
        };
        Ok(json!({"self":stage,"srow":srow,"value":result.unwrap_or_else(failure)}))
    })();
    Ok(Some(tape.into_inner().finish_result(result)?))
}
