// Converted from reference/upstream/tests/test_ledger.py.
use serde_json::{Map, Value, json};
use tokeneconomizer::ledger::{self, OVERHEAD_KEYS};

#[test]
fn overhead_sums_only_known_keys() {
    let row = json!({"optimizer_tokens": 10, "spiral_judge_tokens": 5,
        "untool_judge_tokens": 3, "not_an_overhead_key": 999});
    assert_eq!(ledger::overhead(&row), 18);
}

#[test]
fn overhead_ignores_non_numeric() {
    assert_eq!(
        ledger::overhead(&json!({"optimizer_tokens": null, "spiral_judge_tokens": "x"})),
        0
    );
}

#[test]
fn all_overhead_keys_counted() {
    let row: Map<String, Value> = OVERHEAD_KEYS
        .iter()
        .map(|key| ((*key).into(), json!(1)))
        .collect();
    assert_eq!(ledger::overhead(&json!(row)), OVERHEAD_KEYS.len());
}

#[test]
fn effective_exec_uses_breakdown() {
    let row = json!({"input_tokens": 100, "cache_read_input_tokens": 1000,
        "cache_creation_input_tokens": 0, "output_tokens": 10});
    assert_eq!(
        ledger::effective_exec(&row).unwrap(),
        100.0 + 1000.0 * 0.10 + 10.0 * 5.0
    );
}

#[test]
fn effective_exec_falls_back_to_total() {
    assert_eq!(
        ledger::effective_exec(&json!({"tokens_total": 777})).unwrap(),
        777.0
    );
}
