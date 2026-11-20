// Converted from reference/upstream/experiments/tests/test_usage_recovery.py.
use serde_json::{Value, json};
use tokeneconomizer::experiments::usage::claude::{
    UsageRecovery, merge_usage, row_from_result, session_output_tokens,
};

fn assistant(id: &str, input: i32, cache_write: i32, cache_read: i32, output: i32) -> Value {
    json!({"type":"assistant","message":{"id":id,"usage":{"input_tokens":input,"cache_creation_input_tokens":cache_write,"cache_read_input_tokens":cache_read,"output_tokens":output}}})
}

fn synthesize(events: &[Value], reason: &str, session: Option<&str>) -> Value {
    let mut recovery = UsageRecovery::default();
    for event in events {
        recovery.observe(event).unwrap();
    }
    recovery
        .synthesize(&json!(reason), session, &mut Vec::new())
        .unwrap()
}

fn healthy() -> Value {
    json!({"type":"result","num_turns":4,"usage":{"input_tokens":10,"cache_creation_input_tokens":20,"cache_read_input_tokens":30,"output_tokens":40}})
}

fn session_lines(pairs: &[(&str, i32)]) -> String {
    pairs.iter().flat_map(|(id, output)| ["a", "b"].map(|uuid|
        json!({"type":"assistant","uuid":format!("{id}-{uuid}"),"message":{"id":id,"usage":{"output_tokens":output}}}).to_string()
    )).collect::<Vec<_>>().join("\n")
}

#[test]
fn killed_stream_is_not_recorded_as_free() {
    let result = synthesize(
        &[
            assistant("m1", 2, 4000, 3000, 5),
            assistant("m2", 2, 500, 7000, 9),
        ],
        "spiral_guard_stop",
        None,
    );
    let row = row_from_result(&result).unwrap();
    assert_eq!(row["input_tokens"], 4);
    assert_eq!(row["cache_creation_input_tokens"], 4500);
    assert_eq!(row["cache_read_input_tokens"], 10000);
    assert!(row["tokens_total"].as_f64().unwrap() > 0.0);
    assert_eq!(row["num_turns"], 2);
    assert_eq!(row["usage_incomplete"], true);
    assert_eq!(row["usage_incomplete_reason"], "spiral_guard_stop");
}

#[test]
fn usage_snapshots_are_deduplicated_on_message_id() {
    let result = synthesize(
        &[
            assistant("m1", 2, 100, 1000, 1),
            assistant("m1", 2, 400, 5000, 7),
            assistant("m2", 2, 50, 6000, 3),
        ],
        "task_timeout",
        None,
    );
    let row = row_from_result(&result).unwrap();
    assert_eq!(row["cache_creation_input_tokens"], 450);
    assert_eq!(row["cache_read_input_tokens"], 11000);
    assert_eq!(row["num_turns"], 2);
}

#[test]
fn output_tokens_are_reported_missing_not_estimated() {
    let result = synthesize(
        &[assistant("m1", 2, 100, 1000, 5)],
        "spiral_guard_stop",
        None,
    );
    let row = row_from_result(&result).unwrap();
    assert_eq!(row["output_tokens"], 0);
    assert_eq!(row["output_tokens_missing"], true);
}

#[test]
fn segment_that_never_ran_stays_unflagged() {
    assert_eq!(synthesize(&[], "spiral_guard_stop", None), json!({}));
    assert_eq!(
        synthesize(
            &[json!({"type":"system"}), json!({"type":"user"})],
            "x",
            None
        ),
        json!({})
    );
}

#[test]
fn healthy_row_carries_the_flags_as_false() {
    let row = row_from_result(&json!({"type":"result","num_turns":12,"usage":{"input_tokens":10,"cache_creation_input_tokens":20,"cache_read_input_tokens":30,"output_tokens":40}})).unwrap();
    assert_eq!(row["usage_incomplete"], false);
    assert_eq!(row["output_tokens_missing"], false);
    assert_eq!(row["num_turns_recovered"], false);
    assert_eq!(row["output_tokens"], 40);
}

#[test]
fn flags_survive_a_spiral_merge() {
    let killed = synthesize(
        &[assistant("m1", 2, 100, 1000, 3)],
        "spiral_guard_stop",
        None,
    );
    let merged = row_from_result(&merge_usage(&healthy(), &killed).unwrap()).unwrap();
    assert_eq!(merged["usage_incomplete"], true);
    assert_eq!(merged["num_turns"], 5);
    assert_eq!(merged["cache_read_input_tokens"], 1030);
    assert_eq!(
        row_from_result(&merge_usage(&killed, &healthy()).unwrap()).unwrap()["usage_incomplete"],
        true
    );
}

#[test]
fn merge_of_two_healthy_segments_stays_clean() {
    assert_eq!(
        row_from_result(&merge_usage(&healthy(), &healthy()).unwrap()).unwrap()["usage_incomplete"],
        false
    );
}

#[test]
fn merge_reason_names_every_degraded_segment() {
    let a = synthesize(&[assistant("m1", 2, 1, 1, 1)], "spiral_guard_stop", None);
    let b = synthesize(&[assistant("m2", 2, 1, 1, 1)], "task_timeout", None);
    assert_eq!(
        merge_usage(&a, &b).unwrap()["usage_incomplete_reason"],
        "spiral_guard_stop+task_timeout"
    );
}

#[test]
fn session_log_parser_deduplicates_the_double_write() {
    let text = session_lines(&[("m1", 421)]);
    assert_eq!(text.lines().count(), 2);
    assert_eq!(
        session_output_tokens(&text).unwrap(),
        (json!(421), vec![json!("m1")])
    );
}

#[test]
fn session_log_parser_never_raises_on_junk() {
    for junk in [
        "",
        "not json",
        r#"{"no":"message"}"#,
        r#"{"message":{"id":"m"}}"#,
        r#"{"message":{"id":"m","usage":{"output_tokens":true}}}"#,
    ] {
        assert_eq!(
            session_output_tokens(junk).unwrap(),
            (json!(0), Vec::new()),
            "{junk}"
        );
    }
}

#[test]
fn output_tokens_recovered_exactly_when_every_turn_completed() {
    let result = synthesize(
        &[
            assistant("m1", 2, 4000, 3000, 5),
            assistant("m2", 2, 500, 7000, 9),
        ],
        "spiral_guard_stop",
        Some(&session_lines(&[("m1", 1200), ("m2", 800)])),
    );
    let row = row_from_result(&result).unwrap();
    assert_eq!(row["output_tokens"], 2000);
    assert_eq!(row["output_tokens_exact_turns"], 2);
    assert_eq!(row["output_tokens_missing_turn"], false);
    assert_eq!(row["usage_incomplete"], false);
    assert_eq!(row["output_tokens_missing"], false);
    assert!(row["usage_incomplete_reason"].is_null());
}

#[test]
fn an_in_flight_turn_keeps_the_row_flagged() {
    let result = synthesize(
        &[
            assistant("m1", 2, 10, 20, 5),
            assistant("m2", 2, 10, 20, 7),
            assistant("m3", 2, 10, 20, 3),
        ],
        "task_timeout",
        Some(&session_lines(&[("m1", 1200), ("m2", 800)])),
    );
    let row = row_from_result(&result).unwrap();
    assert_eq!(row["output_tokens"], 2000);
    assert_eq!(row["output_tokens_exact_turns"], 2);
    assert_eq!(row["output_tokens_missing_turn"], true);
    assert_eq!(row["usage_incomplete"], true);
    assert_eq!(row["usage_incomplete_reason"], "task_timeout");
}

#[test]
fn no_session_log_falls_back_to_a_flagged_zero() {
    for text in [None, Some(""), Some("garbage")] {
        let result = synthesize(
            &[assistant("m1", 2, 4000, 3000, 5)],
            "spiral_guard_stop",
            text,
        );
        let row = row_from_result(&result).unwrap();
        assert_eq!(row["output_tokens"], 0);
        assert_eq!(row["usage_incomplete"], true);
        assert_eq!(row["output_tokens_missing"], true);
        assert_eq!(row["output_tokens_exact_turns"], 0);
    }
}

#[test]
fn exact_turns_add_across_spiral_segments_while_flags_or() {
    let clean = synthesize(
        &[assistant("m1", 2, 10, 20, 5)],
        "spiral_guard_stop",
        Some(&session_lines(&[("m1", 500)])),
    );
    let partial = synthesize(
        &[assistant("m2", 2, 10, 20, 5), assistant("m3", 2, 10, 20, 5)],
        "spiral_guard_stop",
        Some(&session_lines(&[("m2", 700)])),
    );
    let merged = merge_usage(&clean, &partial).unwrap();
    assert_eq!(merged["output_tokens_exact_turns"], 2);
    assert_eq!(merged["output_tokens_missing_turn"], true);
    assert_eq!(merged["usage_incomplete"], true);
}
