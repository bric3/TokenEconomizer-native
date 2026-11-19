// Converted from reference/upstream/tests/test_store_read_lines.py.
mod support;

use serde_json::json;
use std::fs;
use support::TempDir;
use tokeneconomizer::{accounting, store::Store};

fn write(store: &Store, name: &str, lines: &[&str]) {
    fs::create_dir_all(&store.base).unwrap();
    fs::write(store.path(&[name]), format!("{}\n", lines.join("\n"))).unwrap();
}

#[test]
fn read_lines_skips_non_object_json() {
    let root = TempDir::new("store-objects");
    let store = Store::new(&root.path().join("st")).unwrap();
    write(
        &store,
        "ledger.jsonl",
        &[
            r#"{"n":1}"#,
            "null",
            "[]",
            r#""str""#,
            "3",
            r#"{"n":2}"#,
            "null",
        ],
    );
    assert_eq!(
        store.read_lines("ledger.jsonl", None).unwrap(),
        [json!({"n":1}), json!({"n":2})]
    );
}

#[test]
fn read_lines_limit_counts_only_object_records() {
    let root = TempDir::new("store-limit");
    let store = Store::new(&root.path().join("st")).unwrap();
    write(
        &store,
        "ledger.jsonl",
        &[r#"{"n":1}"#, r#"{"n":2}"#, "null"],
    );
    assert_eq!(
        store.read_lines("ledger.jsonl", Some(1)).unwrap(),
        [json!({"n":2})]
    );
}

#[test]
fn summarize_tolerates_non_object_records() {
    let root = TempDir::new("store-summary");
    let store = Store::new(&root.path().join("st")).unwrap();
    let valid = json!({"kind":"session", "session_id":"a", "imported":false});
    write(
        &store,
        "ledger.jsonl",
        &["null", "[]", r#""str""#, &valid.to_string()],
    );
    let expected = accounting::summarize(std::slice::from_ref(&valid)).unwrap();
    assert_eq!(
        accounting::summarize(&store.read_lines("ledger.jsonl", None).unwrap()).unwrap(),
        expected
    );
    assert_eq!(
        accounting::summarize(&[json!(null), json!([]), json!("str"), valid]).unwrap(),
        expected
    );
}
