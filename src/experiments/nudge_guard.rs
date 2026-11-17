use crate::{
    loopguard::{
        state::{SpiralState, completed_calls},
        turn::TextCallback,
    },
    rules,
    value::{number, python_repr, scalar_text, truthy},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

pub const RESUME_HEADER: &str = include_str!("nudge_guard/resume-header.txt");

fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "NoneType",
        Value::Bool(_) => "bool",
        Value::String(_) => "str",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                "int"
            } else {
                "float"
            }
        }
    }
}
fn object(value: &Value) -> Result<&Map<String, Value>, String> {
    value.as_object().ok_or_else(|| {
        format!(
            "AttributeError: '{}' object has no attribute 'get'",
            type_name(value)
        )
    })
}
// shortcut: malformed token totals preserve the error type; audit exact diagnostics before production parity.
fn tokens(value: Option<&Value>) -> Result<f64, String> {
    let Some(value) = value.filter(|v| truthy(v)) else {
        return Ok(0.0);
    };
    number(value).ok_or_else(|| format!("TypeError: invalid token total: {}", type_name(value)))
}

#[derive(Serialize, Deserialize)]
struct Sampling {
    #[serde(rename = "_judge_last")]
    last: i64,
    #[serde(rename = "_judge_gap")]
    gap: i64,
    #[serde(rename = "_judge_calls")]
    calls: u64,
    judge_tokens: f64,
    #[serde(rename = "_JUDGE_BASE_GAP")]
    base: i64,
    #[serde(rename = "_JUDGE_MAX_GAP")]
    max: i64,
}
impl Default for Sampling {
    fn default() -> Self {
        Self {
            last: -999,
            gap: 5,
            calls: 0,
            judge_tokens: 0.0,
            base: 5,
            max: 20,
        }
    }
}
impl Sampling {
    fn sample(
        &mut self,
        turns: u64,
        digest: &str,
        judge: Option<TextCallback<'_>>,
        segment: u64,
        verbose: bool,
        messages: &mut Vec<String>,
    ) -> Result<Value, String> {
        let Some(judge) = judge else {
            return Ok(Value::Null);
        };
        if turns as i64 - self.last < self.gap {
            return Ok(Value::Null);
        }
        self.last = turns as i64;
        self.calls += 1;
        let value = judge(digest).unwrap_or(Value::Null);
        let verdict = if truthy(&value) {
            Some(object(&value)?)
        } else {
            None
        };
        if let Some(verdict) = verdict {
            self.judge_tokens += tokens(verdict.get("_tokens"))?;
        }
        self.gap = if verdict.is_some_and(|v| v.get("stuck") == Some(&Value::Bool(false))) {
            self.max.min(self.gap + self.base)
        } else {
            self.base
        };
        if verbose && let Some(verdict) = verdict {
            let text = |key: &str| scalar_text(verdict.get(key).unwrap_or(&Value::Null));
            let reason = scalar_text(verdict.get("reason").unwrap_or(&json!("")))
                .chars()
                .take(50)
                .collect::<String>();
            messages.push(format!("  [nudge-judge] seg{segment} turn {turns} (call #{}, next>={}): stuck={} conf={} {reason}", self.calls, self.gap, text("stuck"), text("confidence")));
        }
        Ok(value)
    }
    fn reset(&mut self) {
        self.last = -999;
        self.gap = self.base;
    }
}

#[derive(Serialize, Deserialize)]
// shortcut: counters use native integer ranges; audit larger values before declaring production parity.
pub struct NudgeGuard {
    pub envelope: Option<f64>,
    pub thr: Value,
    pub nudge_budget: i64,
    pub verbose: bool,
    pub resume_header: String,
    pub nudge_writer_tokens: f64,
    pub st: SpiralState,
    pub pend: Map<String, Value>,
    pub cum_turns: u64,
    pub segment: u64,
    pub nudges_done: u64,
    pub aborted: bool,
    pub pending_action: Option<String>,
    pub events: Vec<Value>,
    #[serde(flatten)]
    sampling: Sampling,
}

impl NudgeGuard {
    pub fn new(
        envelope: Option<f64>,
        thr: Option<Value>,
        nudge_budget: i64,
        verbose: bool,
        resume_header: Option<String>,
    ) -> Self {
        Self {
            envelope,
            thr: thr
                .filter(truthy)
                .unwrap_or_else(|| rules::scaled_thresholds("moderate")),
            nudge_budget,
            verbose,
            resume_header: resume_header.unwrap_or_else(|| RESUME_HEADER.into()),
            nudge_writer_tokens: 0.0,
            st: SpiralState::default(),
            pend: Map::new(),
            cum_turns: 0,
            segment: 0,
            nudges_done: 0,
            aborted: false,
            pending_action: None,
            events: Vec::new(),
            sampling: Sampling::default(),
        }
    }
    pub fn judge_tokens(&self) -> f64 {
        self.sampling.judge_tokens
    }
    pub fn sampled_judge(
        &mut self,
        digest: &str,
        judge: Option<TextCallback<'_>>,
        messages: &mut Vec<String>,
    ) -> Result<Value, String> {
        self.sampling.sample(
            self.st.turns,
            digest,
            judge,
            self.segment,
            self.verbose,
            messages,
        )
    }
    pub fn monitor(
        &mut self,
        line: &str,
        mut judge: Option<TextCallback<'_>>,
        messages: &mut Vec<String>,
    ) -> Result<bool, String> {
        for (name, args, result) in completed_calls(line, &mut self.pend) {
            self.st.update(&name, &args, &result, None);
            self.cum_turns += 1;
            let signals = self.st.signals();
            let backstop = number(&self.thr["backstop_turns"]).or_else(|| {
                self.envelope
                    .map(|env| env * rules::threshold(&self.thr, "backstop_mult"))
            });
            if let Some(backstop) = backstop
                && self.cum_turns as f64 >= backstop
                && rules::is_stuck(&signals, &self.thr)
            {
                return Ok(self.stop(
                    "abort",
                    &format!("cum-backstop:{}>={backstop:.0}t", self.cum_turns),
                    messages,
                ));
            }
            let (abort, reason) = if let Some(judge) = judge.as_mut() {
                let state = &self.st;
                let (envelope, segment, verbose) = (self.envelope, self.segment, self.verbose);
                let sampling = &mut self.sampling;
                let mut assess = || {
                    sampling.sample(
                        state.turns,
                        &state.recent_digest(24, envelope),
                        Some(&mut **judge),
                        segment,
                        verbose,
                        messages,
                    )
                };
                rules::assess_abort(&signals, envelope, Some(&mut assess), &self.thr)?
            } else {
                rules::assess_abort(&signals, self.envelope, None, &self.thr)?
            };
            if !abort {
                if truthy(&self.thr["nudge_on_ambiguous"])
                    && reason == "judge-ambiguous-spared"
                    && (self.nudges_done as i64) < self.nudge_budget
                {
                    return Ok(self.stop("nudge", "ambiguous-past-envelope", messages));
                }
                continue;
            }
            let action = if (self.nudges_done as i64) < self.nudge_budget {
                "nudge"
            } else {
                "abort"
            };
            return Ok(self.stop(action, &reason, messages));
        }
        Ok(false)
    }
    pub fn stop(&mut self, action: &str, reason: &str, messages: &mut Vec<String>) -> bool {
        self.pending_action = Some(action.into());
        let snapshot = self.st.snapshot();
        self.events.push(json!({"segment":self.segment,"action":action,"turn_in_segment":self.st.turns,"cum_turns":self.cum_turns,
            "envelope":self.envelope,"reason":reason,"snapshot":snapshot}));
        if action == "abort" {
            self.aborted = true;
        }
        if self.verbose {
            messages.push(format!(
                "  [nudge-guard] seg{} {} at turn {} (cum {}) ({reason}) {}",
                self.segment,
                action.to_uppercase(),
                self.st.turns,
                self.cum_turns,
                python_repr(&snapshot)
            ));
        }
        true
    }
    pub fn nudge_body(
        &mut self,
        writer: Option<TextCallback<'_>>,
        messages: &mut Vec<String>,
    ) -> Result<String, String> {
        if let Some(writer) = writer {
            let value = writer(&self.st.recent_digest(24, self.envelope)).unwrap_or(Value::Null);
            if truthy(&value) {
                let value = object(&value)?;
                self.nudge_writer_tokens += tokens(value.get("_tokens"))?;
                let empty = json!("");
                let text = value.get("text").filter(|v| truthy(v)).unwrap_or(&empty);
                let text = text.as_str().ok_or_else(|| {
                    format!(
                        "AttributeError: '{}' object has no attribute 'strip'",
                        type_name(text)
                    )
                })?;
                let text = text
                    .trim_matches(|c: char| c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}'));
                if !text.is_empty() {
                    if self.verbose {
                        messages.push(format!(
                            "  [nudge-writer] seg{} targeted redirect ({} chars, +{} tok)",
                            self.segment,
                            text.chars().count(),
                            scalar_text(value.get("_tokens").unwrap_or(&Value::Null))
                        ));
                    }
                    return Ok(text.into());
                }
            }
        }
        Ok(rules::nudge_text(&self.st.signals()))
    }
    pub fn continuation(
        &mut self,
        base_prompt: &str,
        writer: Option<TextCallback<'_>>,
        messages: &mut Vec<String>,
    ) -> Result<Option<String>, String> {
        if self.pending_action.as_deref() != Some("nudge") {
            return Ok(None);
        }
        let text = self.nudge_body(writer, messages)?;
        self.nudges_done += 1;
        self.st = SpiralState::default();
        self.pend.clear();
        self.pending_action = None;
        self.segment += 1;
        self.sampling.reset();
        Ok(Some(format!("{base_prompt}{}{text}", self.resume_header)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cumulative_backstop_aborts_with_nudges_remaining() {
        let mut guard = NudgeGuard::new(Some(10.0), None, 3, false, None);
        guard.thr["backstop_turns"] = json!(1);
        guard.st.turns = 24;
        guard.st.edit_counts.insert("file.rs".into(), json!(5));
        guard.st.error_outputs = 10;
        guard.st.test_runs = 10;
        guard.st.repeated_fail_sigs = 9;
        let line = json!({"type":"item.completed","item":{"type":"command_execution","command":"pytest","aggregated_output":"FAILED fixture"}}).to_string();
        assert!(guard.monitor(&line, None, &mut Vec::new()).unwrap());
        assert!(guard.aborted);
        assert_eq!(guard.nudges_done, 0);
        assert_eq!(guard.pending_action.as_deref(), Some("abort"));
        assert_eq!(guard.events[0]["reason"], "cum-backstop:1>=1t");
        assert_eq!(
            guard.continuation("task", None, &mut Vec::new()).unwrap(),
            None
        );
    }

    #[test]
    fn continuation_resets_segment_and_retains_costs_and_history() {
        let mut guard = NudgeGuard::new(None, None, 1, false, Some(String::new()));
        let mut messages = Vec::new();
        let mut judge = |_: &str| Ok(json!({"stuck":false,"_tokens":7}));
        guard.st.turns = 20;
        guard.cum_turns = 35;
        guard
            .pend
            .insert("\"pending\"".into(), json!({"name":"edit","args":{}}));
        guard
            .sampled_judge("digest", Some(&mut judge), &mut messages)
            .unwrap();
        guard.stop("nudge", "fixture", &mut messages);
        let mut writer = |_: &str| Ok(json!({"text":"  Try a focused check.  ","_tokens":3}));
        assert_eq!(
            guard
                .continuation("task: ", Some(&mut writer), &mut messages)
                .unwrap()
                .as_deref(),
            Some("task: Try a focused check.")
        );
        assert_eq!(
            (
                guard.st.turns,
                guard.cum_turns,
                guard.segment,
                guard.nudges_done
            ),
            (0, 35, 1, 1)
        );
        assert!(guard.pend.is_empty());
        assert_eq!(guard.pending_action, None);
        assert_eq!(guard.events.len(), 1);
        assert_eq!(
            (
                guard.judge_tokens(),
                guard.nudge_writer_tokens,
                guard.sampling.calls
            ),
            (7.0, 3.0, 1)
        );
        assert_eq!((guard.sampling.last, guard.sampling.gap), (-999, 5));
        assert_eq!(
            guard
                .continuation("task", Some(&mut writer), &mut messages)
                .unwrap(),
            None
        );
    }
}
