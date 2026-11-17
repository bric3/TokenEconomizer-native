use super::codex_spiral::{self, EnvelopeTracker};
use crate::{
    config::SpiralConfig,
    envelope,
    loopguard::judge,
    runner::Runner,
    value::{int_value, number, truthy},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;

pub type SnapshotReader<'a> = dyn FnMut(&Path) -> Result<Value, String> + 'a;

#[derive(Serialize, Deserialize)]
pub struct SpiralStage {
    pub cfg: SpiralConfig,
    pub max_turns: Option<f64>,
    pub optimizer_model: String,
    pub agent: Option<String>,
    pub envelope: envelope::EnvelopeTracker,
    pub spiral_judge: bool,
    pub spiral_nudge_writer: bool,
    pub codex_tracker: Option<EnvelopeTracker>,
}
impl SpiralStage {
    pub const NAME: &str = "spiral";
    pub fn new(
        cfg: SpiralConfig,
        max_turns: Option<f64>,
        model: &str,
        agent: Option<String>,
    ) -> Self {
        Self {
            envelope: envelope::EnvelopeTracker::new(
                &cfg.aggr,
                None,
                None,
                None,
                cfg.frozen_envelope,
            ),
            spiral_judge: cfg.guard && cfg.judge,
            spiral_nudge_writer: cfg.nudge && cfg.nudge_llm,
            codex_tracker: (cfg.codex_spiral && agent.as_deref() == Some("codex"))
                .then(EnvelopeTracker::default),
            cfg,
            max_turns,
            optimizer_model: model.into(),
            agent,
        }
    }
    pub fn run_kwargs(&self) -> Result<Value, String> {
        // Bindings replace callback flags with callables for judge() and nudge_writer().
        let mut out = json!({});
        if self.cfg.guard {
            out = json!({"spiral_guard":self.cfg.guard,"abort_envelope":self.envelope.envelope(),
                "spiral_thr":self.envelope.thresholds(self.max_turns),"spiral_judge":if self.spiral_judge { Some(true) } else { None },
                "spiral_nudge":self.cfg.nudge,"spiral_nudge_writer":if self.spiral_nudge_writer { Some(true) } else { None }});
        }
        if let Some(tracker) = &self.codex_tracker {
            out["codex_spiral_policy"] = json!(codex_spiral::profile_policy(
                &self.cfg.codex_spiral_profile,
                tracker.threshold()?
            ));
        }
        Ok(out)
    }
    pub fn judge(&self, digest: &str, runner: Option<&mut dyn Runner>) -> Value {
        judge::llm_stuck_judge(digest, runner, &self.optimizer_model)
    }
    pub fn nudge_writer(&self, digest: &str, runner: Option<&mut dyn Runner>) -> Value {
        judge::llm_nudge_writer(digest, runner, &self.optimizer_model)
    }
    pub fn after_task(
        &mut self,
        row: &Value,
        run_dir: &Path,
        srow: &mut Value,
        snapshot: Option<&mut SnapshotReader<'_>>,
    ) -> Result<i64, String> {
        let cfg = &self.cfg;
        let or_zero = |field: &str| {
            row.get(field)
                .filter(|v| truthy(v))
                .cloned()
                .unwrap_or(json!(0))
        };
        if cfg.guard {
            srow["aborted"] = json!(truthy(&row["aborted"]));
            srow["spiral_judge_tokens"] = or_zero("spiral_judge_tokens");
            if truthy(&row["spiral_abort"]) {
                srow["spiral_abort"] = row["spiral_abort"].clone();
            }
            if cfg.nudge {
                srow["spiral_nudges"] = row.get("spiral_nudges").cloned().unwrap_or(json!(0));
                if truthy(&row["spiral_events"]) {
                    srow["spiral_events"] = row["spiral_events"].clone();
                }
                if cfg.nudge_llm {
                    srow["spiral_nudge_writer_tokens"] = or_zero("spiral_nudge_writer_tokens");
                }
            }
            let resolved = truthy(&row["resolved"]);
            let snap = if resolved {
                snapshot.and_then(|get| get(&run_dir.join("transcript.jsonl")).ok())
            } else {
                None
            };
            if resolved {
                let turns =
                    number(&or_zero("num_turns")).ok_or("TypeError: num_turns must be numeric")?;
                self.envelope.update(turns, true, snap);
            }
        }
        if let Some(tracker) = &mut self.codex_tracker {
            srow["codex_spiral_active"] = json!(true);
            srow["codex_spiral_profile"] =
                json!(codex_spiral::profile_version(&cfg.codex_spiral_profile));
            srow["codex_spiral_nudges"] =
                row.get("codex_spiral_nudges").cloned().unwrap_or(json!(0));
            if truthy(&row["codex_spiral_events"]) {
                srow["codex_spiral_events"] = row["codex_spiral_events"].clone();
            }
            srow["spiral_nudge_writer_tokens"] = or_zero("codex_spiral_paid_judge_tokens");
            let tool_turns = row.get("codex_spiral_tool_turns").filter(|v| !v.is_null());
            srow["codex_spiral_envelope_unit"] = json!(if tool_turns.is_some() {
                "tool_events"
            } else {
                "steps_fallback"
            });
            srow["codex_spiral_tool_turns"] = tool_turns.cloned().unwrap_or(Value::Null);
            srow["codex_spiral_threshold"] = json!(tracker.threshold()?);
            let fallback = or_zero("num_turns");
            let turns = tool_turns.unwrap_or(&fallback);
            let turns = codex_spiral::integer(turns)
                .and_then(|v| int_value(&v))
                .ok_or("ValueError: invalid Codex tool-turn count")?;
            tracker.update(&json!(turns as i64), truthy(&row["resolved"]))?;
        }
        Ok(0)
    }
}
