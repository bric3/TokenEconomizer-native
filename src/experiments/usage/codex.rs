use super::*;
use crate::value::scalar_text;
use std::collections::BTreeMap;

const USAGE_KEYS: [&str; 5] = [
    "input_tokens",
    "cached_input_tokens",
    "cache_write_input_tokens",
    "output_tokens",
    "reasoning_output_tokens",
];

pub fn row_usage(stream: &str) -> Result<Value, String> {
    let mut totals = Map::from_iter(USAGE_KEYS.into_iter().map(|key| (key.into(), json!(0))));
    let (mut steps, mut saw_turn, mut saw_usage) = (0_u64, false, false);
    let mut calls = BTreeMap::<&str, u64>::new();
    let mut thread = json!("");
    let mut errors = Vec::new();
    for line in lines(stream) {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let event = fields(&event, "get")?;
        match get(event, "type").as_str() {
            Some("item.completed") => {
                let item = fields_or_empty(get(event, "item"))?;
                match get(item, "type").as_str() {
                    Some("command_execution") => {
                        *calls.entry("shell").or_default() += 1;
                        steps += 1;
                    }
                    Some("file_change") => {
                        *calls.entry("apply_patch").or_default() += 1;
                        steps += 1;
                    }
                    Some("agent_message") => {
                        steps += 1;
                    }
                    _ => {}
                }
            }
            Some("turn.completed") => {
                saw_turn = true;
                let raw = get(event, "usage");
                if truthy(raw) {
                    saw_usage = true;
                    let usage = fields(raw, "get")?;
                    for key in USAGE_KEYS {
                        totals.insert(key.into(), add(&totals[key], &int(get(usage, key))?)?);
                    }
                }
            }
            Some("thread.started") => {
                if truthy(get(event, "thread_id")) {
                    thread = get(event, "thread_id").clone();
                }
            }
            Some("error") => {
                errors.push(
                    scalar_text(&if truthy(get(event, "message")) {
                        get(event, "message").clone()
                    } else {
                        json!("")
                    })
                    .chars()
                    .take(200)
                    .collect::<String>(),
                );
            }
            _ => {}
        }
        for key in ["thread_id", "session_id", "conversation_id"] {
            if !truthy(&thread) && get(event, key).is_string() {
                thread = get(event, key).clone();
            }
        }
    }
    let mut row = json!({"num_turns":if steps == 0 {None} else {Some(steps)}, "turns_basis":"codex_steps", "tool_calls":calls,"thread_id":thread,"is_error":!errors.is_empty() && !saw_turn,"cost_usd_vendor":null});
    if let Some(error) = errors.first() {
        row["api_error_status"] = json!(error);
    }
    if saw_usage {
        row["input_tokens"] = nonnegative(subtract(
            &subtract(&totals["input_tokens"], &totals["cached_input_tokens"])?,
            &totals["cache_write_input_tokens"],
        )?);
        row["cache_read_input_tokens"] = totals["cached_input_tokens"].clone();
        row["cache_creation_input_tokens"] = totals["cache_write_input_tokens"].clone();
        row["output_tokens"] = add(&totals["output_tokens"], &totals["reasoning_output_tokens"])?;
        row["reasoning_output_tokens"] = totals["reasoning_output_tokens"].clone();
        row["tokens_source"] = json!("stream_usage");
        row["tokens_total"] = [
            "input_tokens",
            "cache_read_input_tokens",
            "cache_creation_input_tokens",
            "output_tokens",
        ]
        .into_iter()
        .try_fold(json!(0), |total, key| add(&total, &row[key]))?;
    } else {
        for key in [
            "input_tokens",
            "cache_read_input_tokens",
            "cache_creation_input_tokens",
            "output_tokens",
            "tokens_total",
        ] {
            row[key] = Value::Null;
        }
        row["tokens_source"] = json!(if saw_turn {
            "no_usage_event"
        } else {
            "no_turn_event"
        });
    }
    Ok(row)
}

pub fn merge_usage(a: &Value, b: &Value) -> Result<Value, String> {
    let (a, b) = (fields_or_empty(a)?, fields_or_empty(b)?);
    if a.is_empty() {
        return Ok(json!(b));
    }
    if b.is_empty() {
        return Ok(json!(a));
    }
    let mut out = a.clone();
    for key in [
        "input_tokens",
        "cache_read_input_tokens",
        "cache_creation_input_tokens",
        "output_tokens",
        "reasoning_output_tokens",
        "tokens_total",
    ] {
        let (x, y) = (get(a, key), get(b, key));
        out.insert(
            key.into(),
            if x.is_null() || y.is_null() {
                Value::Null
            } else {
                add(&or_zero(x), &or_zero(y))?
            },
        );
    }
    let (x, y) = (get(a, "num_turns"), get(b, "num_turns"));
    out.insert(
        "num_turns".into(),
        if x.is_null() && y.is_null() {
            Value::Null
        } else {
            add(&or_zero(x), &or_zero(y))?
        },
    );
    let mut calls = fields_or_empty(get(a, "tool_calls"))?.clone();
    for (key, n) in fields_or_empty(get(b, "tool_calls"))? {
        calls.insert(
            key.clone(),
            add(&calls.get(key).cloned().unwrap_or(json!(0)), n)?,
        );
    }
    let sorted = calls.into_iter().collect::<BTreeMap<_, _>>();
    out.insert("tool_calls".into(), json!(sorted));
    out.insert(
        "thread_id".into(),
        [get(a, "thread_id"), get(b, "thread_id")]
            .into_iter()
            .find(|v| truthy(v))
            .cloned()
            .unwrap_or(json!("")),
    );
    let sources = [get(a, "tokens_source"), get(b, "tokens_source")]
        .into_iter()
        .filter(|v| truthy(v))
        .collect::<Vec<_>>();
    let source = if !sources.is_empty() && sources.iter().all(|v| **v == "stream_usage") {
        json!("stream_usage")
    } else {
        for value in &sources {
            if value.is_array() || value.is_object() {
                return Err(unhashable(value, "set element"));
            }
        }
        let sources = sources
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
        if sources.is_empty() {
            Value::Null
        } else {
            json!(sources.join("+"))
        }
    };
    out.insert("tokens_source".into(), source);
    out.insert(
        "is_error".into(),
        json!(truthy(get(a, "is_error")) || truthy(get(b, "is_error"))),
    );
    out.insert("cost_usd_vendor".into(), Value::Null);
    Ok(json!(out))
}
