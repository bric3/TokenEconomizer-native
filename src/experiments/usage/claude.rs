use super::*;
use crate::value::scalar_text;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub fn merge_usage(a: &Value, b: &Value) -> Result<Value, String> {
    if !truthy(a) {
        return Ok(if truthy(b) { b.clone() } else { json!({}) });
    }
    if !truthy(b) {
        return Ok(a.clone());
    }
    let (a, b) = (fields(a, "get")?, fields(b, "get")?);
    let mut out = b.clone();
    let (ua, ub) = (
        fields_or_empty(get(a, "usage"))?,
        fields_or_empty(get(b, "usage"))?,
    );
    let mut usage = Map::new();
    for key in ua.keys().chain(ub.keys()) {
        if usage.contains_key(key) {
            continue;
        }
        if ua.get(key).is_none_or(is_numeric) && ub.get(key).is_none_or(is_numeric) {
            usage.insert(
                key.clone(),
                add(&or_zero(get(ua, key)), &or_zero(get(ub, key)))?,
            );
        }
    }
    out.insert("usage".into(), json!(usage));
    for key in ["num_turns", "total_cost_usd", "duration_ms"] {
        if is_numeric(get(a, key)) && is_numeric(get(b, key)) {
            out.insert(key.into(), add(get(a, key), get(b, key))?);
        }
    }
    for key in [
        "usage_incomplete",
        "output_tokens_missing",
        "num_turns_recovered",
        "output_tokens_missing_turn",
    ] {
        out.insert(
            key.into(),
            json!(truthy(get(a, key)) || truthy(get(b, key))),
        );
    }
    let exact = [
        get(a, "output_tokens_exact_turns"),
        get(b, "output_tokens_exact_turns"),
    ];
    if exact.iter().any(|v| is_int(v)) {
        let total = exact
            .into_iter()
            .filter(|v| is_int(v))
            .try_fold(json!(0), |n, v| add(&n, v))?;
        out.insert("output_tokens_exact_turns".into(), total);
    }
    let reasons = [
        get(a, "usage_incomplete_reason"),
        get(b, "usage_incomplete_reason"),
    ]
    .into_iter()
    .filter(|v| truthy(v))
    .collect::<Vec<_>>();
    if !reasons.is_empty() {
        let reasons = reasons
            .iter()
            .enumerate()
            .map(|(i, v)| {
                v.as_str().ok_or_else(|| {
                    format!(
                        "TypeError: sequence item {i}: expected str instance, {} found",
                        python_type(v)
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        out.insert("usage_incomplete_reason".into(), json!(reasons.join("+")));
    }
    Ok(json!(out))
}

pub fn session_output_tokens(text: &str) -> Result<(Value, Vec<Value>), String> {
    let mut per_message = IndexMap::<String, (Value, Value)>::new();
    for line in lines(text) {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let event = fields_or_empty(&event)?;
        let Some(message) = get(event, "message").as_object() else {
            continue;
        };
        let id = get(message, "id");
        let Some(usage) = get(message, "usage").as_object() else {
            continue;
        };
        let out = get(usage, "output_tokens");
        if !truthy(id) || out.is_boolean() || !is_int(out) {
            continue;
        }
        let key = id_key(id)?;
        if let Some(pair) = per_message.get_mut(&key) {
            pair.1 = out.clone();
        } else {
            per_message.insert(key, (id.clone(), out.clone()));
        }
    }
    let total = per_message
        .values()
        .try_fold(json!(0), |n, (_, v)| add(&n, v))?;
    Ok((total, per_message.into_values().map(|(id, _)| id).collect()))
}

mod snapshots {
    use super::*;
    pub fn serialize<S: serde::Serializer>(
        value: &IndexMap<String, (Value, Value)>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(value.values())
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<IndexMap<String, (Value, Value)>, D::Error> {
        let pairs = Vec::<(Value, Value)>::deserialize(deserializer)?;
        let mut map = IndexMap::<String, (Value, Value)>::new();
        for (id, usage) in pairs {
            let key = id_key(&id).map_err(serde::de::Error::custom)?;
            if let Some(pair) = map.get_mut(&key) {
                pair.1 = usage;
            } else {
                map.insert(key, (id, usage));
            }
        }
        Ok(map)
    }
}

#[derive(Default, Serialize, Deserialize)]
pub struct UsageRecovery {
    #[serde(rename = "_by_msg", with = "snapshots")]
    by_msg: IndexMap<String, (Value, Value)>,
}
impl UsageRecovery {
    pub fn observe(&mut self, event: &Value) -> Result<(), String> {
        let ev = fields_or_empty(event)?;
        if get(ev, "type") != "assistant" {
            return Ok(());
        }
        let message = fields_or_empty(get(ev, "message"))?;
        let (usage, id) = (get(message, "usage"), get(message, "id"));
        if truthy(usage) && truthy(id) {
            let key = id_key(id)?;
            if let Some(pair) = self.by_msg.get_mut(&key) {
                pair.1 = usage.clone();
            } else {
                self.by_msg.insert(key, (id.clone(), usage.clone()));
            }
        }
        Ok(())
    }
    pub fn synthesize(
        &self,
        reason: &Value,
        session_text: Option<&str>,
        messages: &mut Vec<String>,
    ) -> Result<Value, String> {
        if self.by_msg.is_empty() {
            return Ok(json!({}));
        }
        let mut totals = Map::new();
        for (_, usage) in self.by_msg.values() {
            for (key, value) in fields(usage, "items")? {
                if value.is_number() {
                    totals.insert(key.clone(), add(&or_zero(get(&totals, key)), value)?);
                }
            }
        }
        let mut recovered = Map::new();
        for key in [
            "input_tokens",
            "cache_creation_input_tokens",
            "cache_read_input_tokens",
        ] {
            recovered.insert(key.into(), int(get(&totals, key))?);
        }
        recovered.insert("output_tokens".into(), json!(0));
        let (mut got_output, mut exact_turns, mut inflight) = (false, 0, true);
        if let Some(text) = session_text.filter(|s| !s.is_empty()) {
            match session_output_tokens(text) {
                Ok((total, ids)) if !ids.is_empty() => {
                    exact_turns = ids.len();
                    let keys = ids.iter().map(id_key).collect::<Result<BTreeSet<_>, _>>()?;
                    inflight = self.by_msg.keys().any(|key| !keys.contains(key));
                    recovered.insert("output_tokens".into(), total);
                    got_output = true;
                }
                Err(error) => messages.push(format!(
                    "session-log usage read failed ({}: {})",
                    error.split_once(": ").map_or("ValueError", |p| p.0),
                    error.split_once(": ").map_or(error.as_str(), |p| p.1)
                )),
                _ => {}
            }
        }
        let complete = got_output && !inflight;
        let reason_text = scalar_text(reason);
        if got_output {
            messages.push(format!("segment ended without a result event ({reason_text}); output_tokens recovered EXACTLY from the CLI session log: {} over {exact_turns} completed turns{}", scalar_text(&recovered["output_tokens"]), if complete { "" } else { " -- one in-flight turn unaccounted, row stays flagged" }));
        } else {
            messages.push(format!("segment ended without a result event ({reason_text}); recovered usage from {} assistant messages: in={} cw={} cr={} out=UNRECOVERABLE -- row flagged usage_incomplete", self.by_msg.len(), scalar_text(&recovered["input_tokens"]), scalar_text(&recovered["cache_creation_input_tokens"]), scalar_text(&recovered["cache_read_input_tokens"])));
        }
        Ok(
            json!({"type":"result","usage":recovered,"num_turns":self.by_msg.len(),"usage_incomplete":!complete,"usage_recovered_from":if got_output {"session_log"} else {"transcript"},"usage_incomplete_reason":if complete {Value::Null} else {reason.clone()},"output_tokens_missing":!got_output,"output_tokens_exact_turns":if got_output {exact_turns} else {0},"output_tokens_missing_turn":if got_output {inflight} else {true},"num_turns_recovered":true}),
        )
    }
}

pub fn row_from_result(result: &Value) -> Result<Value, String> {
    let result = fields_or_empty(result)?;
    let usage = fields_or_empty(get(result, "usage"))?;
    let mut out = Map::new();
    for key in [
        "input_tokens",
        "cache_read_input_tokens",
        "cache_creation_input_tokens",
        "output_tokens",
    ] {
        let value = or_zero(get(usage, key));
        out.insert(key.into(), value);
    }
    let total = add(
        &add(
            &add(&out["input_tokens"], &out["cache_read_input_tokens"])?,
            &out["cache_creation_input_tokens"],
        )?,
        &out["output_tokens"],
    )?;
    out.insert("tokens_total".into(), total);
    out.insert("num_turns".into(), or_zero(get(result, "num_turns")));
    for key in [
        "is_error",
        "usage_incomplete",
        "output_tokens_missing",
        "num_turns_recovered",
        "output_tokens_missing_turn",
    ] {
        out.insert(key.into(), json!(truthy(get(result, key))));
    }
    for key in ["api_error_status", "usage_incomplete_reason"] {
        out.insert(key.into(), get(result, key).clone());
    }
    out.insert(
        "cost_usd_vendor".into(),
        get(result, "total_cost_usd").clone(),
    );
    out.insert(
        "output_tokens_exact_turns".into(),
        int(get(result, "output_tokens_exact_turns"))?,
    );
    Ok(json!(out))
}
