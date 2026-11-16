use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::rules::{scaled_thresholds, threshold};
use crate::value::round_decimal;

pub const ENVELOPE_HISTORY: usize = 500;
pub const ENVELOPE_KEYS: [&str; 3] = ["envelope_margin", "envelope_pct", "envelope_min_samples"];

pub fn percentile(xs: &[f64], p: f64) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    let mut xs = xs.to_vec();
    xs.sort_by(f64::total_cmp);
    let i = (p / 100.0) * (xs.len() - 1) as f64;
    let lo = i as usize;
    if lo + 1 >= xs.len() {
        return xs.last().copied();
    }
    Some(xs[lo] + (i - lo as f64) * (xs[lo + 1] - xs[lo]))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EnvelopeTracker {
    pub level: String,
    pub base_thr: Value,
    pub margin: f64,
    pub pct: f64,
    pub min_samples: i64,
    pub frozen_envelope: Option<f64>,
    pub resolved_turns: Vec<f64>,
    pub resolved_snaps: Vec<Value>,
}

impl EnvelopeTracker {
    pub fn new(
        level: &str,
        margin: Option<f64>,
        pct: Option<f64>,
        min_samples: Option<i64>,
        frozen_envelope: Option<f64>,
    ) -> Self {
        let base_thr = scaled_thresholds(level);
        Self {
            level: level.into(),
            margin: margin
                .filter(|n| *n != 0.0)
                .unwrap_or_else(|| threshold(&base_thr, "envelope_margin")),
            pct: pct.unwrap_or_else(|| threshold(&base_thr, "envelope_pct")),
            min_samples: min_samples
                .unwrap_or_else(|| threshold(&base_thr, "envelope_min_samples") as i64),
            frozen_envelope,
            base_thr,
            resolved_turns: Vec::new(),
            resolved_snaps: Vec::new(),
        }
    }

    pub fn update(&mut self, turns: f64, resolved: bool, snapshot: Option<Value>) {
        if resolved && turns != 0.0 {
            self.resolved_turns.push(turns);
            if let Some(snapshot) = snapshot.filter(crate::value::truthy) {
                self.resolved_snaps.push(snapshot);
            }
        }
    }

    pub fn envelope(&self) -> Option<f64> {
        if (self.resolved_turns.len() as i64) < self.min_samples {
            return self.frozen_envelope;
        }
        percentile(&self.resolved_turns, self.pct).map(|n| n * self.margin)
    }

    pub fn thresholds(&self, max_turns: Option<f64>) -> Value {
        let mut thr = self.base_thr.clone();
        if let Some(env) = self.envelope() {
            let backstop = env * threshold(&thr, "backstop_mult");
            thr["backstop_turns"] = max_turns
                .filter(|n| *n != 0.0)
                .map_or(backstop, |n| backstop.min(0.9 * n))
                .into();
        }
        if (self.resolved_turns.len() as i64) < self.min_samples {
            return thr;
        }
        let p90 = percentile(&self.resolved_turns, 90.0).unwrap_or(0.0);
        let clamp = |min: f64, max: f64| (0.6 * p90).round_ties_even().min(max).max(min) as i64;
        thr["gate_min_turns"] = clamp(6.0, threshold(&self.base_thr, "gate_min_turns")).into();
        thr["min_turns"] = clamp(8.0, threshold(&self.base_thr, "min_turns")).into();
        if !self.resolved_snaps.is_empty() {
            let signal = |key: &str| {
                percentile(
                    &self
                        .resolved_snaps
                        .iter()
                        .map(|s| threshold(s, key))
                        .collect::<Vec<_>>(),
                    90.0,
                )
                .unwrap_or(0.0)
            };
            for key in ["edit_churn", "distinct_edit_files", "error_outputs"] {
                thr[key] = threshold(&self.base_thr, key)
                    .max((signal(key) + 1.0).ceil())
                    .into();
            }
            thr["gate_error_outputs"] = threshold(&self.base_thr, "gate_error_outputs")
                .max(threshold(&thr, "error_outputs"))
                .into();
            thr["stagnation"] = threshold(&self.base_thr, "stagnation")
                .max(round_decimal(signal("stagnation"), 3))
                .into();
        }
        thr
    }
}

pub fn thresholds(tracker: &EnvelopeTracker, overrides: &Value) -> Value {
    let mut thr = tracker.thresholds(None);
    if let Some(overrides) = overrides.as_object() {
        for (key, value) in overrides {
            if !ENVELOPE_KEYS.contains(&key.as_str()) {
                thr[key] = value.clone();
            }
        }
        if overrides.contains_key("backstop_mult")
            && let Some(envelope) = tracker.envelope()
        {
            thr["backstop_turns"] = (envelope * threshold(&thr, "backstop_mult")).into();
        }
    }
    thr
}

pub fn base_thresholds(level: &str, overrides: &Value) -> Value {
    let mut thr = scaled_thresholds(level);
    if let Some(overrides) = overrides.as_object() {
        thr.as_object_mut().unwrap().extend(overrides.clone());
    }
    thr
}
