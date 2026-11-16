use serde_json::{Value, json};

use crate::{
    ledger,
    pricing::{self, Weights},
    value::{integer, number, truthy},
};

pub const FALLBACK_WEIGHTS: Weights = Weights {
    input: 1.0,
    cache_read: 0.10,
    cache_write: 1.25,
    output: 6.0,
    cache_write_1h: None,
};

pub fn effective(usage: &Value) -> (Option<f64>, &'static str) {
    if !pricing::has_breakdown(usage) {
        return (None, "no_usage");
    }
    let mut record = usage.clone();
    // The live ledger resolves by model only; the experiment's agent default is separate.
    record.as_object_mut().unwrap().remove("agent");
    match pricing::effective_tokens(&record, None) {
        Ok(value) => (Some(value), "model"),
        Err(_) => (
            pricing::effective_tokens(&record, Some(FALLBACK_WEIGHTS)).ok(),
            "fallback",
        ),
    }
}

pub fn overhead_total(overhead: &Value) -> f64 {
    ledger::overhead(overhead)
}

pub fn summarize(records: &[Value]) -> Result<Value, String> {
    let mut sessions = 0;
    let mut imported_sessions = 0;
    let mut priced_sessions = 0;
    let mut exec_total = 0.0;
    let mut oh_total = 0.0;
    let mut requests = 0.0;
    let mut nudges = 0.0;
    let mut aborts = 0.0;
    let mut turns = 0.0;
    let mut injected_sessions = 0;
    for record in records.iter().filter(|r| r.is_object()) {
        let session = record["kind"] == "session";
        if truthy(&record["imported"]) {
            if session {
                imported_sessions += 1;
            }
            continue;
        }
        if session || record["kind"] == "overhead" {
            oh_total += overhead_total(&record["overhead"]);
        }
        if !session {
            continue;
        }
        sessions += 1;
        if let Some(effective) = number(&record["effective_tokens"]) {
            priced_sessions += 1;
            exec_total += effective;
        }
        let count = |key: &str| integer(&record[key]).ok_or_else(|| format!("invalid {key} count"));
        requests += count("requests")?;
        nudges += count("nudges")?;
        aborts += count("aborts")?;
        turns += count("turns")?;
        if truthy(&record["injected_chars"]) {
            injected_sessions += 1;
        }
    }
    let total = exec_total + oh_total;
    Ok(
        json!({ "sessions": sessions, "imported_sessions": imported_sessions,
        "priced_sessions": priced_sessions, "effective_tokens": exec_total,
        "overhead_tokens": oh_total, "overhead_frac": if total != 0.0 { oh_total / total } else { 0.0 },
        "requests": requests, "nudges": nudges, "aborts": aborts, "turns": turns,
        "injected_sessions": injected_sessions }),
    )
}

pub fn fmt_tokens(n: Option<f64>) -> String {
    let Some(n) = n else {
        return "n/a".into();
    };
    for (unit, divisor) in [("B", 1e9), ("M", 1e6), ("k", 1e3)] {
        if n.abs() >= divisor {
            return format!("{:.1}{unit}", n / divisor);
        }
    }
    format!("{n:.0}")
}
