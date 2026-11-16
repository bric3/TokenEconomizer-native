use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::value::{number, truthy};

pub fn scaled_thresholds(level: &str) -> Value {
    let mut thresholds = json!({
        "min_turns": 20, "edit_churn": 4, "distinct_edit_files": 3,
        "error_outputs": 6, "stagnation": 0.20, "envelope_margin": 1.25,
        "envelope_pct": 90, "envelope_min_samples": 5, "judge_confidence": 0.7,
        "backstop_mult": 2.5, "gate_min_turns": 18, "gate_error_outputs": 8,
    });
    let overrides = match level {
        "conservative" => json!({ "judge_confidence": 0.85, "envelope_margin": 1.5,
            "backstop_mult": 3.5, "stagnation": 0.30, "gate_min_turns": 24,
            "min_turns": 28, "edit_churn": 5, "error_outputs": 8, "envelope_min_samples": 7 }),
        "aggressive" => json!({ "judge_confidence": 0.55, "envelope_margin": 1.0,
            "backstop_mult": 1.8, "stagnation": 0.12, "gate_min_turns": 12,
            "min_turns": 14, "edit_churn": 3, "error_outputs": 4,
            "envelope_min_samples": 3, "nudge_on_ambiguous": true }),
        _ => json!({}),
    };
    thresholds
        .as_object_mut()
        .unwrap()
        .extend(overrides.as_object().unwrap().clone());
    thresholds
}

pub(crate) fn threshold(thr: &Value, key: &str) -> f64 {
    number(&thr[key]).unwrap_or(0.0)
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Signals {
    pub turns: u64,
    pub edit_churn: u64,
    pub distinct_edit_files: u64,
    pub error_outputs: u64,
    pub stagnation: f64,
    pub repeated_fail_sigs: u64,
    pub test_runs: u64,
    pub edit_counts: Map<String, Value>,
}

pub fn gate_for_judge(state: &Signals, thr: &Value) -> bool {
    state.turns as f64 >= threshold(thr, "gate_min_turns")
        && (state.repeated_fail_sigs >= 1
            || state.error_outputs as f64 >= threshold(thr, "gate_error_outputs"))
}

pub fn is_stuck(state: &Signals, thr: &Value) -> bool {
    state.turns as f64 >= threshold(thr, "min_turns")
        && (state.edit_churn as f64 >= threshold(thr, "edit_churn")
            || state.distinct_edit_files as f64 >= threshold(thr, "distinct_edit_files"))
        && state.error_outputs as f64 >= threshold(thr, "error_outputs")
        && state.stagnation >= threshold(thr, "stagnation")
}

pub type Judge<'a> = &'a mut dyn FnMut() -> Result<Value, String>;

pub fn assess_abort(
    state: &Signals,
    envelope: Option<f64>,
    judge: Option<Judge<'_>>,
    thr: &Value,
) -> Result<(bool, String), String> {
    let backstop = number(&thr["backstop_turns"])
        .or_else(|| envelope.map(|n| n * threshold(thr, "backstop_mult")));
    if let Some(backstop) = backstop {
        if state.turns as f64 >= backstop && is_stuck(state, thr) {
            return Ok((
                true,
                format!(
                    "backstop:{}>={backstop:.0}turns stagnation={:.2}",
                    state.turns, state.stagnation
                ),
            ));
        }
    }
    if let Some(judge) = judge {
        let over = envelope.is_some_and(|n| state.turns as f64 >= n)
            && thr.get("gate_over_envelope").is_none_or(truthy);
        if !(gate_for_judge(state, thr) || over) {
            return Ok((false, "gate-closed".into()));
        }
        let verdict = judge().unwrap_or(Value::Null);
        if !truthy(&verdict) {
            return Ok((false, "judge-throttled".into()));
        }
        let confidence = match &verdict["confidence"] {
            Value::String(s) if !s.is_empty() => s.parse::<f64>().map_err(|e| e.to_string())?,
            n => number(n).unwrap_or(0.0),
        };
        if truthy(&verdict["stuck"]) && confidence >= threshold(thr, "judge_confidence") {
            let reason = verdict["reason"]
                .as_str()
                .unwrap_or("")
                .chars()
                .take(60)
                .collect::<String>();
            return Ok((true, format!("judge-stuck@{confidence:.2}:{reason}")));
        }
        if verdict["stuck"] == false {
            return Ok((false, "judge-productive".into()));
        }
        return Ok((false, "judge-ambiguous-spared".into()));
    }
    if !is_stuck(state, thr) {
        return Ok((false, "not-stuck".into()));
    }
    if let Some(envelope) = envelope {
        if state.turns as f64 >= envelope {
            return Ok((true, format!("envelope:{}>={envelope:.0}", state.turns)));
        }
    }
    Ok((false, "stuck-but-within-envelope".into()))
}

pub fn nudge_text(state: &Signals) -> String {
    let mut busiest = None;
    let mut total = 0.0;
    for (name, count) in &state.edit_counts {
        let count = number(count).unwrap_or(0.0);
        if busiest.is_none_or(|(_, most)| count > most) {
            busiest = Some((name.as_str(), count));
        }
        total += count;
    }
    let busiest = busiest.map_or("one file", |(name, _)| name);
    format!(
        "Process check (automated): so far you've made {total} edits across {} file(s) (most to `{busiest}`) and run {} verification(s), and the same failure has recurred {} time(s) without the tests going green. This looks like a repetitive loop. Do NOT stop — instead step back and try a MATERIALLY DIFFERENT hypothesis: re-read the failing test's expectation, consider a different file or root cause than the ones you've been editing, and verify the new hypothesis directly before editing again.",
        state.distinct_edit_files, state.test_runs, state.repeated_fail_sigs
    )
}
