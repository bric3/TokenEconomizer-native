// Converted from reference/upstream/tests/test_cache_write_1h.py.
mod support;

use serde_json::{Value, json};
use std::{fs, path::PathBuf};
use support::TempDir;
use tokeneconomizer::{
    accounting,
    pricing::{self, Weights},
    trace::{claude, pi},
};

const BASE: f64 = 100.0 + 10_000.0 * 0.10 + 10.0 * 5.0;

fn row(cache_write: i64, one_hour: Option<Value>, model: &str) -> Value {
    let mut row = json!({"input_tokens":100, "cache_read_input_tokens":10_000,
        "cache_creation_input_tokens":cache_write, "output_tokens":10, "model":model});
    if let Some(one_hour) = one_hour {
        row["cache_creation_1h_input_tokens"] = one_hour;
    }
    row
}

fn message(mid: &str, cache_write: i64, split: Option<Value>) -> Vec<Value> {
    let mut usage = json!({"input_tokens":10, "cache_read_input_tokens":1000,
        "cache_creation_input_tokens":cache_write, "output_tokens":50});
    if let Some(split) = split {
        usage["cache_creation"] = split;
    }
    (0..2)
        .map(|block| {
            json!({"type":"assistant", "message":{
        "id":mid, "model":"claude-opus-4-8", "role":"assistant", "usage":usage,
        "content":[{"type":"text", "text":format!("block {block}")}]}})
        })
        .collect()
}

fn transcript(root: &TempDir, entries: Vec<Value>) -> PathBuf {
    let path = root.path().join("s.jsonl");
    let lines: Vec<_> = entries.iter().map(Value::to_string).collect();
    fs::write(&path, format!("{}\n", lines.join("\n"))).unwrap();
    path
}

#[test]
fn one_hour_cache_writes_are_priced_at_twice_input() {
    assert_eq!(
        pricing::effective_tokens(&row(1000, Some(json!(1000)), "claude-opus-4-8"), None).unwrap(),
        BASE + 1000.0 * 2.0
    );
}

#[test]
fn a_mixed_row_prices_each_share_at_its_own_rate() {
    assert_eq!(
        pricing::effective_tokens(&row(1000, Some(json!(400)), "claude-opus-4-8"), None).unwrap(),
        BASE + 600.0 * 1.25 + 400.0 * 2.0
    );
}

#[test]
fn a_row_without_the_split_is_priced_as_before() {
    for one_hour in [None, Some(json!(0))] {
        assert_eq!(
            pricing::effective_tokens(&row(1000, one_hour, "claude-opus-4-8"), None).unwrap(),
            BASE + 1000.0 * 1.25
        );
    }
}

#[test]
fn a_claude_id_outside_the_table_gets_the_one_hour_rate_too() {
    assert_eq!(
        pricing::effective_tokens(&row(1000, Some(json!(1000)), "claude-opus-4-5"), None).unwrap(),
        BASE + 1000.0 * 2.0
    );
}

#[test]
fn the_one_hour_share_is_never_more_than_the_cache_writes() {
    for (value, expected) in [(json!(5000), 1000.0), (json!(-5), 0.0), (json!("x"), 0.0)] {
        assert_eq!(
            pricing::cache_write_1h_tokens(&row(1000, Some(value), "claude-opus-4-8")),
            expected
        );
    }
    assert_eq!(pricing::cache_write_1h_tokens(&Value::Null), 0.0);
}

#[test]
fn a_weight_table_without_a_one_hour_rate_uses_its_cache_write_rate() {
    assert_eq!(
        pricing::effective_tokens(&row(1000, Some(json!(1000)), "gpt-5.6-sol"), None).unwrap(),
        100.0 + 10_000.0 * 0.10 + 1000.0 * 1.25 + 10.0 * 6.0
    );
    let explicit = Weights {
        input: 1.0,
        cache_read: 0.0,
        cache_write: 1.0,
        output: 1.0,
        cache_write_1h: None,
    };
    assert_eq!(
        pricing::effective_tokens(
            &row(1000, Some(json!(1000)), "claude-opus-4-8"),
            Some(explicit)
        )
        .unwrap(),
        100.0 + 1000.0 + 10.0
    );
}

#[test]
fn the_split_is_read_from_an_anthropic_usage_object() {
    assert_eq!(
        pricing::cache_write_1h_tokens(&json!({"cache_creation_input_tokens":248,
        "cache_creation":{"ephemeral_5m_input_tokens":148, "ephemeral_1h_input_tokens":100}})),
        100.0
    );
    assert_eq!(
        pricing::cache_write_1h_tokens(&json!({"cache_creation_input_tokens":248,
        "cache_creation":"not an object"})),
        0.0
    );
}

#[test]
fn a_claude_session_reports_its_one_hour_cache_writes() {
    let root = TempDir::new("claude-cache");
    let entries = message(
        "m1",
        300,
        Some(json!({"ephemeral_5m_input_tokens":0, "ephemeral_1h_input_tokens":300})),
    )
    .into_iter()
    .chain(message(
        "m2",
        200,
        Some(json!({"ephemeral_5m_input_tokens":150, "ephemeral_1h_input_tokens":50})),
    ))
    .chain(message("m3", 100, None))
    .collect();
    let usage = claude::session_usage(&transcript(&root, entries), true).unwrap();
    assert_eq!(usage["cache_creation_input_tokens"].as_f64(), Some(600.0));
    assert_eq!(
        usage["cache_creation_1h_input_tokens"].as_f64(),
        Some(350.0)
    );
}

#[test]
fn a_split_on_a_message_that_reports_no_cache_writes_adds_nothing() {
    let root = TempDir::new("claude-empty-cache");
    let entries = message(
        "m1",
        0,
        Some(json!({"ephemeral_5m_input_tokens":0, "ephemeral_1h_input_tokens":2948})),
    )
    .into_iter()
    .chain(message(
        "m2",
        80,
        Some(json!({"ephemeral_5m_input_tokens":0, "ephemeral_1h_input_tokens":"x"})),
    ))
    .collect();
    let usage = claude::session_usage(&transcript(&root, entries), true).unwrap();
    assert_eq!(usage["cache_creation_input_tokens"].as_f64(), Some(80.0));
    assert_eq!(usage["cache_creation_1h_input_tokens"].as_f64(), Some(0.0));
}

#[test]
fn a_session_is_priced_with_its_one_hour_cache_writes() {
    let root = TempDir::new("claude-cache-price");
    let path = transcript(
        &root,
        message(
            "m1",
            1000,
            Some(json!({"ephemeral_5m_input_tokens":0, "ephemeral_1h_input_tokens":1000})),
        ),
    );
    assert_eq!(
        accounting::effective(&claude::session_usage(&path, true).unwrap()).unwrap(),
        (
            Some(10.0 + 1000.0 * 0.10 + 1000.0 * 2.0 + 50.0 * 5.0),
            "model"
        )
    );
}

#[test]
fn a_claude_side_call_is_priced_from_the_usage_object_it_returns() {
    let usage = json!({"input_tokens":10, "cache_read_input_tokens":0, "output_tokens":0,
        "cache_creation_input_tokens":1000, "model":"claude-haiku-4-5",
        "cache_creation":{"ephemeral_5m_input_tokens":0, "ephemeral_1h_input_tokens":1000}});
    assert_eq!(
        accounting::effective(&usage).unwrap(),
        (Some(10.0 + 1000.0 * 2.0), "model")
    );
}

#[test]
fn an_unpriced_model_keeps_the_fallback_cache_write_rate() {
    let usage = json!({"input_tokens":10, "cache_read_input_tokens":0, "output_tokens":0,
        "cache_creation_input_tokens":1000, "cache_creation_1h_input_tokens":1000, "model":"some-new-model"});
    assert_eq!(
        accounting::effective(&usage).unwrap(),
        (Some(10.0 + 1000.0 * 1.25), "fallback")
    );
}

fn pi_usage(cache_write: i64, one_hour: i64) -> Value {
    json!({"input":10, "output":50, "cacheRead":1000, "cacheWrite":cache_write,
        "cacheWrite1h":one_hour, "totalTokens":1060+cache_write, "cost":{"total":0.001}})
}

#[test]
fn a_pi_session_reports_its_one_hour_cache_writes() {
    let root = TempDir::new("pi-cache");
    let mut entries = vec![
        json!({"type":"session", "version":3, "id":"pi-1", "cwd":"/w/repo"}),
        json!({"type":"message", "id":"e1", "parentId":null, "message":{"role":"user", "content":"fix the bug"}}),
    ];
    for (i, (cache_write, one_hour)) in [(1000, 1000), (200, 50)].into_iter().enumerate() {
        let id = i + 2;
        entries.push(
            json!({"type":"message", "id":format!("e{id}"), "parentId":format!("e{}",id-1),
            "message":{"role":"assistant", "provider":"anthropic", "model":"claude-opus-4-8",
            "usage":pi_usage(cache_write,one_hour), "content":[{"type":"text", "text":"ok"}]}}),
        );
    }
    let usage = pi::session_usage(&transcript(&root, entries)).unwrap();
    assert_eq!(usage["cache_creation_input_tokens"].as_f64(), Some(1200.0));
    assert_eq!(
        usage["cache_creation_1h_input_tokens"].as_f64(),
        Some(1050.0)
    );
    assert_eq!(
        accounting::effective(&usage).unwrap(),
        (
            Some(20.0 + 2000.0 * 0.10 + 150.0 * 1.25 + 1050.0 * 2.0 + 100.0 * 5.0),
            "model"
        )
    );
}

#[test]
fn a_pi_side_call_is_priced_with_its_one_hour_cache_writes() {
    let run = json!({"type":"message_end", "message":{"role":"assistant", "model":"claude-haiku-4-5",
        "usage":pi_usage(1000,1000), "content":[{"type":"text", "text":"- use python -m pytest"}]}});
    let (_, usage) = pi::json_reply(&run.to_string()).unwrap();
    let mut usage = usage.unwrap();
    usage.as_object_mut().unwrap().remove("cost_usd");
    assert_eq!(
        accounting::effective(&usage).unwrap(),
        (
            Some(10.0 + 1000.0 * 0.10 + 1000.0 * 2.0 + 50.0 * 5.0),
            "model"
        )
    );
}

#[test]
fn a_cache_hit_is_billed_at_the_models_own_fraction() {
    for (model, ratio) in [
        ("claude-opus-5-5", 0.05),
        ("claude-sonnet-5-5", 0.05),
        ("claude-fable-5-1", 0.025),
        ("claude-fable-5-2", 0.025),
        ("claude-mythos-5-1", 0.025),
        ("claude-mythos-5-2", 0.025),
        ("claude-sonnet-5.5", 0.05),
        ("anthropic/claude-opus-5.5", 0.05),
        ("claude-opus-5-5-20261001", 0.05),
    ] {
        assert_eq!(
            pricing::weights_for(&row(1000, None, model))
                .unwrap()
                .cache_read,
            ratio,
            "{model}"
        );
        assert_eq!(
            pricing::effective_tokens(&row(0, None, model), None).unwrap(),
            100.0 + 10_000.0 * ratio + 10.0 * 5.0,
            "{model}"
        );
    }
}

#[test]
fn the_other_claude_models_keep_the_usual_cache_hit_fraction() {
    for model in [
        "claude-opus-5",
        "claude-opus-4-8",
        "claude-sonnet-5",
        "claude-sonnet-4-6",
        "claude-haiku-5-5",
        "claude-haiku-4-5",
        "claude-fable-5",
        "claude-mythos-5",
        "claude-opus-4.5",
    ] {
        assert_eq!(
            pricing::weights_for(&row(1000, None, model))
                .unwrap()
                .cache_read,
            0.10,
            "{model}"
        );
        assert_eq!(
            pricing::effective_tokens(&row(0, None, model), None).unwrap(),
            BASE,
            "{model}"
        );
    }
}

#[test]
fn a_session_on_opus_5_5_is_priced_with_its_cheaper_cache_hits() {
    let root = TempDir::new("claude-cheaper-cache");
    let entries = vec![
        json!({"type":"assistant", "message":{"id":"m1", "model":"claude-opus-5-5", "role":"assistant",
        "usage":{"input_tokens":10, "cache_read_input_tokens":1000, "cache_creation_input_tokens":0, "output_tokens":50},
        "content":[{"type":"text", "text":"x"}]}}),
    ];
    assert_eq!(
        accounting::effective(&claude::session_usage(&transcript(&root, entries), true).unwrap())
            .unwrap(),
        (Some(10.0 + 1000.0 * 0.05 + 50.0 * 5.0), "model")
    );
}
