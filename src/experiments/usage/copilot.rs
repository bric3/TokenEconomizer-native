use super::*;
use crate::trace::jsonl::TextReader;
use std::{fs, path::Path};

pub fn sum_openai_usage(text: &str) -> Result<Value, String> {
    let (mut index, mut blocks) = (0, 0_u64);
    let (mut prompt, mut cached, mut created, mut completion, mut total) =
        (json!(0), json!(0), json!(0), json!(0), json!(0));
    const KEY: &str = "\"usage\": {";
    while let Some(offset) = text.get(index..).and_then(|t| t.find(KEY)) {
        let start = index + offset + KEY.len() - 1;
        let mut depth = 0_usize;
        let mut end = text.len();
        for (offset, byte) in text.as_bytes()[start..].iter().enumerate() {
            if *byte == b'{' {
                depth += 1;
            }
            if *byte == b'}' {
                depth -= 1;
                if depth == 0 {
                    end = start + offset + 1;
                    break;
                }
            }
        }
        index = end;
        let Ok(usage) = serde_json::from_str::<Value>(&text[start..end]) else {
            continue;
        };
        let usage = fields(&usage, "get")?;
        if !usage.contains_key("prompt_tokens") && !usage.contains_key("completion_tokens") {
            continue;
        }
        blocks += 1;
        prompt = add(&prompt, &or_zero(get(usage, "prompt_tokens")))?;
        completion = add(&completion, &or_zero(get(usage, "completion_tokens")))?;
        total = add(&total, &or_zero(get(usage, "total_tokens")))?;
        let details = fields_or_empty(get(usage, "prompt_tokens_details"))?;
        cached = add(&cached, &or_zero(get(details, "cached_tokens")))?;
        created = add(&created, &or_zero(get(details, "cache_creation_tokens")))?;
    }
    if blocks == 0 {
        return Ok(Value::Null);
    }
    Ok(
        json!({"input_tokens":nonnegative(subtract(&subtract(&prompt, &cached)?, &created)?),"cache_read_input_tokens":cached,"cache_creation_input_tokens":created,"output_tokens":completion,"tokens_total":if truthy(&total) {total} else {add(&prompt,&completion)?},"n_api_calls":blocks}),
    )
}

pub fn read_log_dir(root: &Path) -> String {
    let Ok(entries) = fs::read_dir(root) else {
        return String::new();
    };
    let mut paths = entries
        .filter_map(Result::ok)
        .filter(|e| {
            let name = e.file_name();
            let name = name.as_encoded_bytes();
            !name.starts_with(b".") && name.ends_with(b".log")
        })
        .map(|e| e.path())
        .collect::<Vec<_>>();
    paths.sort();
    let mut out = String::new();
    for path in paths {
        let read = (|| -> std::io::Result<String> {
            let mut reader = TextReader::open(&path)?;
            let mut text = String::new();
            loop {
                let line = reader.readline(usize::MAX)?;
                if line.is_empty() {
                    break;
                }
                text.push_str(&line);
            }
            Ok(text)
        })();
        match read {
            Ok(text) => out.push_str(&text),
            Err(_) => break,
        }
    }
    out
}

pub fn turns_from_stream(stream: &str) -> u64 {
    lines(stream)
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|ev| ev.get("type").is_some_and(|v| v == "assistant.turn_start"))
        .count() as u64
}

pub fn stream_output_tokens(stream: &str) -> Result<Value, String> {
    let mut total = json!(0);
    for line in lines(stream) {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let ev = fields(&event, "get")?;
        let data = fields_or_empty(get(ev, "data"))?;
        for key in ["outputTokens", "output_tokens"] {
            let value = get(data, key);
            if is_int(value) {
                total = add(&total, value)?;
            }
        }
    }
    Ok(total)
}

pub fn row_usage(stream: &str, log: &str) -> Result<Value, String> {
    let mut usage = sum_openai_usage(log)?;
    let turns = turns_from_stream(stream);
    if truthy(&usage) {
        usage["num_turns"] = json!(turns);
        usage["tokens_source"] = json!("debug_log");
        usage["stream_output_tokens"] = stream_output_tokens(stream)?;
        return Ok(usage);
    }
    let output = stream_output_tokens(stream)?;
    Ok(
        json!({"input_tokens":null,"cache_read_input_tokens":null,"cache_creation_input_tokens":null,"output_tokens":if truthy(&output) {output} else {Value::Null},"tokens_total":null,"num_turns":turns,"tokens_source":"stream_output_only","n_api_calls":null}),
    )
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
        "tokens_total",
        "num_turns",
        "n_api_calls",
        "stream_output_tokens",
    ] {
        let (x, y) = (get(a, key), get(b, key));
        out.insert(
            key.into(),
            if x.is_null() {
                y.clone()
            } else if y.is_null() {
                x.clone()
            } else {
                add(x, y)?
            },
        );
    }
    out.insert(
        "tokens_source".into(),
        json!(
            if get(a, "tokens_source") == "debug_log" && get(b, "tokens_source") == "debug_log" {
                "debug_log"
            } else {
                "mixed"
            }
        ),
    );
    Ok(json!(out))
}
