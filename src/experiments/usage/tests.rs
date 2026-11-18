use super::*;

#[test]
fn grand_input_is_decomposed_without_rounding_large_counters() {
    let row = codex::row_usage(r#"{"type":"turn.completed","usage":{"input_tokens":9007199254740993,"cached_input_tokens":60,"cache_write_input_tokens":10,"output_tokens":5,"reasoning_output_tokens":2}}"#).unwrap();
    assert_eq!(row["input_tokens"], json!(9007199254740923_u64));
    assert_eq!(row["cache_read_input_tokens"], 60);
    assert_eq!(row["cache_creation_input_tokens"], 10);
    assert_eq!(row["output_tokens"], 7);
    assert_eq!(row["tokens_total"], json!(9007199254741000_u64));
}

#[test]
fn each_agent_keeps_its_own_unknown_segment_rule() {
    let known =
        json!({"input_tokens":30,"tokens_total":107,"num_turns":2,"tokens_source":"stream_usage"});
    let unknown = json!({"input_tokens":null,"tokens_total":null,"num_turns":null,"tokens_source":"no_turn_event"});
    let row = codex::merge_usage(&known, &unknown).unwrap();
    assert!(row["input_tokens"].is_null());
    assert!(row["tokens_total"].is_null());
    assert_eq!(row["tokens_source"], "stream_usage+no_turn_event");
    let row = copilot::merge_usage(&known, &unknown).unwrap();
    assert_eq!(row["input_tokens"], 30);
    assert_eq!(row["tokens_total"], 107);
    assert_eq!(row["tokens_source"], "mixed");
}

#[test]
fn recovery_clears_degradation_only_after_every_observed_message_is_in_the_log() {
    let mut recovery = claude::UsageRecovery::default();
    for (id, input) in [("m1", 10), ("m1", 20), ("m2", 5)] {
        recovery.observe(&json!({"type":"assistant","message":{"id":id,"usage":{"input_tokens":input,"output_tokens":999}}})).unwrap();
    }
    let log = "{\"message\":{\"id\":\"m1\",\"usage\":{\"output_tokens\":5}}}\n{\"message\":{\"id\":\"m1\",\"usage\":{\"output_tokens\":7}}}";
    let partial = recovery
        .synthesize(&json!("task_timeout"), Some(log), &mut Vec::new())
        .unwrap();
    assert_eq!(partial["usage"]["input_tokens"], 25);
    assert_eq!(partial["usage"]["output_tokens"], 7);
    assert_eq!(partial["num_turns"], 2);
    assert_eq!(partial["output_tokens_exact_turns"], 1);
    assert_eq!(partial["usage_incomplete"], true);
    assert_eq!(partial["output_tokens_missing_turn"], true);
    let complete_log = format!(
        "{log}\n{}",
        json!({"message":{"id":"m2","usage":{"output_tokens":4}}})
    );
    let complete = recovery
        .synthesize(
            &json!("spiral_guard_stop"),
            Some(&complete_log),
            &mut Vec::new(),
        )
        .unwrap();
    assert_eq!(complete["usage"]["output_tokens"], 11);
    assert_eq!(complete["output_tokens_exact_turns"], 2);
    assert_eq!(complete["usage_incomplete"], false);
    assert!(complete["usage_incomplete_reason"].is_null());
}

#[test]
fn large_numeric_message_ids_remain_distinct() {
    let mut recovery = claude::UsageRecovery::default();
    for id in [1e100, 2e100] {
        recovery
            .observe(&json!({"type":"assistant","message":{"id":id,"usage":{"input_tokens":10}}}))
            .unwrap();
    }
    let row = recovery
        .synthesize(&json!("timeout"), None, &mut Vec::new())
        .unwrap();
    assert_eq!(row["num_turns"], 2);
    assert_eq!(row["usage"]["input_tokens"], 20);
}
