use std::io::{self, BufRead, Write};
use std::path::Path;

#[path = "bridge/callback.rs"]
mod callback;
#[path = "bridge/guard.rs"]
mod guard_dispatch;
#[path = "bridge/microskills.rs"]
mod microskills_dispatch;
#[path = "bridge/proposer.rs"]
mod proposer_dispatch;
#[path = "bridge/storage.rs"]
mod storage_dispatch;
#[path = "bridge/trace.rs"]
mod trace_dispatch;

use serde_json::{Value, json};
use tokeneconomizer::{
    accounting,
    config::{EconomizerConfig, InjectionConfig, SpiralConfig, ToolingConfig},
    envelope::{self, EnvelopeTracker},
    ledger,
    lifecycle::Lifecycle,
    names, pricing,
    rules::{self, Signals},
    trace::{claude, jsonl},
};

fn dispatch(request: &Value) -> Result<Value, String> {
    let args = &request["args"];
    if let Some(value) = proposer_dispatch::dispatch(request["op"].as_str().unwrap_or(""), args)? {
        return Ok(value);
    }
    if let Some(value) = microskills_dispatch::dispatch(request["op"].as_str().unwrap_or(""), args)?
    {
        return Ok(value);
    }
    if let Some(value) = storage_dispatch::dispatch(request["op"].as_str().unwrap_or(""), args)? {
        return Ok(value);
    }
    if let Some(value) = guard_dispatch::dispatch(request["op"].as_str().unwrap_or(""), args)? {
        return Ok(value);
    }
    if let Some(value) = trace_dispatch::dispatch(request["op"].as_str().unwrap_or(""), args)? {
        return Ok(value);
    }
    let result = match request["op"].as_str().unwrap_or("") {
        "trace.jsonl.lines" => json!(
            jsonl::lines(Path::new(args["path"].as_str().ok_or("expected path")?))
                .collect::<Vec<_>>()
        ),
        "trace.jsonl.records" => json!(
            jsonl::records(Path::new(args["path"].as_str().ok_or("expected path")?))
                .collect::<Vec<_>>()
        ),
        "trace.claude.claude_calls" => {
            let lines: Vec<String> =
                serde_json::from_value(args["lines"].clone()).map_err(|e| e.to_string())?;
            json!(claude::calls(&lines))
        }
        "trace.claude._is_prompt" => json!(claude::is_prompt(&args["ev"])),
        "trace.claude._is_notification" => json!(claude::is_notification(&args["ev"])),
        "trace.claude.claude_safe_session_id" => json!(claude::safe_session_id(&args["sid"])),
        "trace.claude.claude_project_folder" => {
            json!(claude::project_folder(args["path"].as_str().unwrap_or("")))
        }
        "trace.claude.claude_prompt_count"
        | "trace.claude.claude_prompt_turns"
        | "trace.claude.claude_subagent_files"
        | "trace.claude.claude_session_usage"
        | "trace.claude.claude_deferred_tools" => {
            let path = Path::new(args["transcript_path"].as_str().unwrap_or(""));
            match request["op"].as_str().unwrap() {
                "trace.claude.claude_prompt_count" => json!(claude::prompt_count(path)),
                "trace.claude.claude_prompt_turns" => {
                    json!(claude::prompt_turns(path).collect::<Vec<_>>())
                }
                "trace.claude.claude_subagent_files" => json!(claude::subagent_files(path)),
                "trace.claude.claude_session_usage" => claude::session_usage(
                    path,
                    args["include_subagents"].as_bool().unwrap_or(true),
                )?,
                _ => json!(claude::deferred_tools(path)),
            }
        }
        "trace.claude.claude_turn_interrupted" | "trace.claude.claude_turn_answered" => {
            let lines: Vec<String> =
                serde_json::from_value(args["turn_lines"].clone()).map_err(|e| e.to_string())?;
            if request["op"] == "trace.claude.claude_turn_interrupted" {
                json!(claude::turn_interrupted(&lines))
            } else {
                json!(claude::turn_answered(&lines))
            }
        }
        "rules.scaled_thresholds" => {
            rules::scaled_thresholds(args["level"].as_str().unwrap_or("default"))
        }
        "envelope.base_thresholds" => envelope::base_thresholds(
            args["level"].as_str().unwrap_or("default"),
            &args["overrides"],
        ),
        "envelope._percentile" => {
            let xs: Vec<f64> =
                serde_json::from_value(args["xs"].clone()).map_err(|e| e.to_string())?;
            json!(envelope::percentile(
                &xs,
                args["p"].as_f64().ok_or("expected percentile")?
            ))
        }
        "envelope.new" => json!(EnvelopeTracker::new(
            args["level"].as_str().unwrap_or("default"),
            args["margin"].as_f64(),
            args["pct"].as_f64(),
            args["min_samples"].as_i64(),
            args["frozen_envelope"].as_f64()
        )),
        "envelope.envelope"
        | "envelope.update"
        | "envelope.tracker_thresholds"
        | "envelope.thresholds" => {
            let source = if args["tracker"].is_null() {
                &args["self"]
            } else {
                &args["tracker"]
            };
            let mut tracker: EnvelopeTracker =
                serde_json::from_value(source.clone()).map_err(|e| e.to_string())?;
            match request["op"].as_str().unwrap() {
                "envelope.envelope" => json!(tracker.envelope()),
                "envelope.tracker_thresholds" => tracker.thresholds(args["max_turns"].as_f64()),
                "envelope.thresholds" => envelope::thresholds(&tracker, &args["overrides"]),
                _ => {
                    tracker.update(
                        args["turns"].as_f64().ok_or("expected turns")?,
                        args["resolved"].as_bool().unwrap_or(false),
                        Some(args["snapshot"].clone()),
                    );
                    json!(tracker)
                }
            }
        }
        "rules.gate_for_judge" | "rules.is_stuck" | "rules.nudge_text" | "rules.assess_abort" => {
            let state: Signals =
                serde_json::from_value(args["state"].clone()).map_err(|e| e.to_string())?;
            let thr = args["thr"]
                .as_object()
                .filter(|m| !m.is_empty())
                .cloned()
                .map(Value::Object)
                .unwrap_or_else(|| rules::scaled_thresholds("default"));
            match request["op"].as_str().unwrap() {
                "rules.gate_for_judge" => json!(rules::gate_for_judge(&state, &thr)),
                "rules.is_stuck" => json!(rules::is_stuck(&state, &thr)),
                "rules.nudge_text" => json!(rules::nudge_text(&state)),
                _ => {
                    let mut called = false;
                    let mut judge = || {
                        called = true;
                        Ok(args["judge_result"].clone())
                    };
                    let result = rules::assess_abort(
                        &state,
                        args["envelope"].as_f64(),
                        if args["judge"] == true {
                            Some(&mut judge)
                        } else {
                            None
                        },
                        &thr,
                    )?;
                    if called && args.get("judge_result").is_none() {
                        json!({"callback": "judge"})
                    } else {
                        json!(result)
                    }
                }
            }
        }
        "config.InjectionConfig" => {
            let cfg: InjectionConfig =
                serde_json::from_value(args.clone()).map_err(|e| e.to_string())?;
            cfg.validate()?;
            json!(cfg)
        }
        "config.ToolingConfig" => {
            let cfg: ToolingConfig =
                serde_json::from_value(args.clone()).map_err(|e| e.to_string())?;
            json!(cfg)
        }
        "config.SpiralConfig" => {
            let cfg: SpiralConfig =
                serde_json::from_value(args.clone()).map_err(|e| e.to_string())?;
            cfg.validate()?;
            json!(cfg)
        }
        "config.injection_paid" => {
            let cfg: InjectionConfig =
                serde_json::from_value(args.clone()).map_err(|e| e.to_string())?;
            json!(cfg.paid())
        }
        "config.tooling_enabled" => {
            let cfg: ToolingConfig =
                serde_json::from_value(args.clone()).map_err(|e| e.to_string())?;
            json!(cfg.enabled())
        }
        "config.spiral_enabled" => {
            let cfg: SpiralConfig =
                serde_json::from_value(args.clone()).map_err(|e| e.to_string())?;
            json!(cfg.enabled())
        }
        "config.needs_optimizer" => {
            let cfg: EconomizerConfig =
                serde_json::from_value(args["config"].clone()).map_err(|e| e.to_string())?;
            json!(cfg.needs_optimizer())
        }
        "config.from_args" => json!(EconomizerConfig::from_args(args)?),
        "pricing.resolve_model" => json!(pricing::resolve_model(&args["rec"])?),
        "pricing.weights_for" => json!(pricing::weights_for(&args["rec"])?),
        "pricing.has_breakdown" => json!(pricing::has_breakdown(&args["rec"])),
        "pricing.cache_write_1h_tokens" => json!(pricing::cache_write_1h_tokens(&args["usage"])),
        "pricing.effective_tokens" => {
            let weights = if args["weights"].is_null() {
                None
            } else {
                Some(serde_json::from_value(args["weights"].clone()).map_err(|e| e.to_string())?)
            };
            json!(pricing::effective_tokens(&args["rec"], weights)?)
        }
        "pricing.approx_tokens" => json!(pricing::approx_tokens(
            args["text"].as_str().ok_or("expected text")?
        )),
        "ledger.overhead" => json!(ledger::overhead(&args["row"])),
        "ledger.effective_exec" => json!(ledger::effective_exec(&args["row"])?),
        "accounting.effective" => json!(accounting::effective(&args["usage"])?),
        "accounting.overhead_total" => json!(accounting::overhead_total(&args["overhead"])),
        "accounting.summarize" => {
            accounting::summarize(args["records"].as_array().ok_or("expected records")?)?
        }
        "accounting.fmt_tokens" => {
            let n = match &args["n"] {
                Value::Null => None,
                Value::String(s) => Some(s.parse::<f64>().map_err(|e| e.to_string())?),
                Value::Bool(b) => Some(u8::from(*b) as f64),
                n => Some(n.as_f64().ok_or("expected token count")?),
            };
            json!(accounting::fmt_tokens(n))
        }
        "names.repo_filename" => json!(names::repo_filename(
            args["repo"].as_str().ok_or("expected repository label")?
        )),
        op if op.starts_with("lifecycle.") => {
            let mut life: Lifecycle =
                serde_json::from_value(args["self"].clone()).map_err(|e| e.to_string())?;
            match op {
                "lifecycle._drifted_confirms" => json!(life.drifted_confirms()),
                "lifecycle.score" => {
                    json!(life.score(args["now"].as_i64().ok_or("expected task index")?))
                }
                "lifecycle.firing_rate" => json!(life.firing_rate()),
                "lifecycle.injectable" => json!(life.injectable()),
                "lifecycle.update" => {
                    life.update(
                        args["now"].as_i64().ok_or("expected task index")?,
                        args["verdict"].as_str().unwrap_or(""),
                        args["fired"].as_bool().unwrap_or(false),
                        args["exercised"].as_bool().unwrap_or(false),
                        serde_json::from_value(args["files"].clone()).map_err(|e| e.to_string())?,
                    );
                    json!(life)
                }
                _ => return Err(format!("unknown operation {op}")),
            }
        }
        op => return Err(format!("unknown operation {op}")),
    };
    Ok(result)
}

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    for line in stdin.lock().lines() {
        let response = match serde_json::from_str::<Value>(&line?) {
            Ok(request) => match dispatch(&request) {
                Ok(value) => json!({"result": value}),
                Err(error) => json!({"error": error}),
            },
            Err(error) => json!({"error": error.to_string()}),
        };
        serde_json::to_writer(&mut stdout, &response)?;
        stdout.write_all(b"\n")?;
        stdout.flush()?;
    }
    Ok(())
}
