// Converted from reference/upstream/experiments/tests/test_vendor_cost.py.
use serde_json::{Value, json};
use tokeneconomizer::experiments::aggregate::{
    FIELDS, usage_incomplete, vendor_cost_missing, vendor_usd,
};

fn ran(updates: Value) -> Value {
    let mut row = json!({"agent":"claude", "resolved":true, "tokens_total":400_000,
        "input_tokens":1600, "patch_bytes":900});
    row.as_object_mut()
        .unwrap()
        .extend(updates.as_object().unwrap().clone());
    row
}

#[test]
fn vendor_usd_reads_the_key_the_runners_write() {
    assert_eq!(vendor_usd(&ran(json!({"cost_usd_vendor":0.2239}))), 0.2239);
}

#[test]
fn vendor_usd_falls_back_to_the_legacy_curated_name() {
    assert_eq!(vendor_usd(&json!({"cost_usd":1.5})), 1.5);
}

#[test]
fn vendor_usd_prefers_the_runner_key_when_both_are_present() {
    assert_eq!(
        vendor_usd(&json!({"cost_usd_vendor":2.0,"cost_usd":9.9})),
        2.0
    );
}

#[test]
fn vendor_usd_is_blank_not_zero_when_absent() {
    assert_eq!(vendor_usd(&json!({"cost_usd_vendor":null})), "");
    assert_eq!(vendor_usd(&json!({})), "");
}

#[test]
fn vendor_usd_keeps_a_genuine_zero() {
    assert_eq!(vendor_usd(&json!({"cost_usd_vendor":0.0})), 0.0);
}

#[test]
fn vendor_usd_rejects_a_bool() {
    assert_eq!(vendor_usd(&json!({"cost_usd_vendor":true})), "");
}

#[test]
fn flags_a_row_that_ran_without_a_figure() {
    let (missing, reason) = vendor_cost_missing(&ran(json!({"cost_usd_vendor":null})));
    assert!(missing && reason.as_str().unwrap().contains("total_cost_usd"));
}

#[test]
fn does_not_flag_a_row_that_has_a_figure() {
    assert!(!vendor_cost_missing(&ran(json!({"cost_usd_vendor":0.5}))).0);
}

#[test]
fn does_not_flag_an_agent_that_never_reports_dollars() {
    for agent in ["copilot", "codex"] {
        assert!(
            !vendor_cost_missing(&ran(json!({"agent":agent,"cost_usd_vendor":null}))).0,
            "{agent}"
        );
    }
}

#[test]
fn does_not_flag_a_task_that_never_ran() {
    let row = json!({"agent":"claude","resolved":false,"tokens_total":0,"input_tokens":0,"patch_bytes":null,"cost_usd_vendor":null});
    assert!(!vendor_cost_missing(&row).0);
}

#[test]
fn flags_an_unresolved_row_that_still_burned_tokens() {
    assert!(
        vendor_cost_missing(&ran(
            json!({"resolved":false,"patch_bytes":0,"cost_usd_vendor":null})
        ))
        .0
    );
}

#[test]
fn flag_is_independent_of_usage_incomplete() {
    let row = ran(json!({"cost_usd_vendor":null}));
    assert!(!usage_incomplete(&row).0);
    assert!(vendor_cost_missing(&row).0);
}

#[test]
fn both_columns_are_emitted() {
    for column in [
        "cost_usd",
        "vendor_cost_missing",
        "vendor_cost_missing_reason",
    ] {
        assert!(FIELDS.contains(&column), "{column}");
    }
}
