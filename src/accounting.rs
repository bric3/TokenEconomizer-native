use num_rational::BigRational;
use num_traits::ToPrimitive;
use serde_json::{Value, json};

use crate::{
    ledger,
    pricing::{self, PricingError, Weights},
    value::{Numeric, integer, truthy},
};

pub const FALLBACK_WEIGHTS: Weights = Weights {
    input: 1.0,
    cache_read: 0.10,
    cache_write: 1.25,
    output: 6.0,
    cache_write_1h: None,
};

pub fn effective(usage: &Value) -> Result<(Option<f64>, &'static str), PricingError> {
    if !pricing::has_breakdown(usage) {
        return Ok((None, "no_usage"));
    }
    let mut record = usage.clone();
    // The live ledger resolves by model only; the experiment's agent default is separate.
    record.as_object_mut().unwrap().remove("agent");
    let (weights, priced_by) = match pricing::weights_for(&record) {
        Ok(weights) => (weights, "model"),
        Err(PricingError::UnpricedModel(_)) => (FALLBACK_WEIGHTS, "fallback"),
        Err(error) => return Err(error),
    };
    Ok((
        Some(pricing::effective_tokens(&record, Some(weights))?),
        priced_by,
    ))
}

pub fn overhead_total(overhead: &Value) -> Value {
    ledger::overhead(overhead)
}

pub fn summarize(records: &[Value]) -> Result<Value, String> {
    let active = records
        .iter()
        .filter(|r| r.is_object() && !truthy(&r["imported"]));
    let imported_sessions = records
        .iter()
        .filter(|r| r.is_object() && r["kind"] == "session" && truthy(&r["imported"]))
        .count();
    let exec_total = Numeric::sum(
        active
            .clone()
            .filter(|r| r["kind"] == "session")
            .filter_map(|r| Numeric::from_value(&r["effective_tokens"])),
    );
    let oh_total = Numeric::sum(
        active
            .clone()
            .filter(|r| r["kind"] == "session" || r["kind"] == "overhead")
            .map(|r| ledger::overhead_number(&r["overhead"])),
    );
    let mut sessions = 0usize;
    let mut priced_sessions = 0usize;
    let mut requests = 0i64;
    let mut nudges = 0i64;
    let mut aborts = 0i64;
    let mut turns = 0i64;
    let mut injected_sessions = 0usize;
    for record in active.filter(|r| r["kind"] == "session") {
        sessions += 1;
        if Numeric::from_value(&record["effective_tokens"]).is_some() {
            priced_sessions += 1;
        }
        let add = |total: i64, key: &str| {
            let value = &record[key];
            // shortcut: ledger counts use i64, extend their range in the production numeric audit.
            let count = value
                .as_i64()
                .or_else(|| value.as_str().and_then(|s| s.trim().parse().ok()))
                .or_else(|| {
                    integer(value)
                        .filter(|n| *n >= i64::MIN as f64 && *n < -(i64::MIN as f64))
                        .map(|n| n as i64)
                })
                .ok_or_else(|| format!("invalid {key} count"))?;
            total
                .checked_add(count)
                .ok_or_else(|| format!("{key} count exceeds i64"))
        };
        requests = add(requests, "requests")?;
        nudges = add(nudges, "nudges")?;
        aborts = add(aborts, "aborts")?;
        turns = add(turns, "turns")?;
        if truthy(&record["injected_chars"]) {
            injected_sessions += 1;
        }
    }
    let total = match (exec_total, oh_total) {
        (Numeric::Integer(execution), Numeric::Integer(overhead)) => {
            Numeric::Integer(execution + overhead)
        }
        _ => Numeric::Float(exec_total.as_f64() + oh_total.as_f64()),
    };
    let fraction = if total.as_f64() == 0.0 {
        0.0
    } else {
        match (oh_total, total) {
            // Round the exact integer ratio once; float division preserves zero's sign.
            (Numeric::Integer(overhead), Numeric::Integer(total)) if overhead != 0 => {
                BigRational::new(overhead.into(), total.into())
                    .to_f64()
                    .unwrap()
            }
            _ => oh_total.as_f64() / total.as_f64(),
        }
    };
    Ok(
        json!({ "sessions": sessions, "imported_sessions": imported_sessions,
        "priced_sessions": priced_sessions, "effective_tokens": exec_total.json(),
        "overhead_tokens": oh_total.json(), "overhead_frac": fraction,
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
