// Converted from reference/upstream/tests/test_load_calls_malformed.py.
mod support;

use serde_json::{Value, json};
use std::{fs, path::PathBuf};
use support::TempDir;
use tokeneconomizer::microskills::distiller::load_calls;

fn tool_use(id: Value, name: Value, input: Value) -> Value {
    json!({"type":"assistant", "message":{"content":[{"type":"tool_use",
        "id":id, "name":name, "input":input}]}})
}

fn result(id: Value, content: Value) -> Value {
    json!({"type":"user", "message":{"content":[{"type":"tool_result",
        "tool_use_id":id, "content":content}]}})
}

fn bad_ids() -> [Value; 5] {
    [
        json!([]),
        json!({}),
        json!(["t1"]),
        json!({"x":1}),
        Value::Null,
    ]
}

fn write(root: &TempDir, lines: &[Value]) -> PathBuf {
    let path = root.path().join("t.jsonl");
    let lines: Vec<_> = lines
        .iter()
        .map(|line| {
            line.as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| line.to_string())
        })
        .collect();
    fs::write(&path, format!("{}\n", lines.join("\n"))).unwrap();
    path
}

#[test]
fn non_object_json_lines_are_skipped() {
    let root = TempDir::new("calls-non-object");
    let path = write(
        &root,
        &[
            json!("null"),
            json!("[]"),
            json!(r#""str""#),
            json!(123),
            json!(true),
            tool_use(json!("t1"), json!("Bash"), json!({"command":"pytest"})),
            result(json!("t1"), json!([{"type":"text", "text":"1 passed"}])),
        ],
    );
    assert_eq!(
        load_calls(&path),
        [(
            "bash".into(),
            json!({"command":"pytest"}),
            "1 passed".into()
        )]
    );
}

#[test]
fn non_string_tool_use_id_is_skipped() {
    for id in bad_ids() {
        let root = TempDir::new("calls-use-id");
        let path = write(
            &root,
            &[
                tool_use(id.clone(), json!("Bash"), json!({"command":"pytest"})),
                tool_use(json!("t2"), json!("Read"), json!({"file_path":"a.py"})),
            ],
        );
        assert_eq!(
            load_calls(&path),
            [("read".into(), json!({"file_path":"a.py"}), "".into())],
            "{id}"
        );
    }
}

#[test]
fn non_string_tool_result_id_is_ignored() {
    for id in bad_ids() {
        let root = TempDir::new("calls-result-id");
        let path = write(
            &root,
            &[
                tool_use(json!("t1"), json!("Bash"), json!({"command":"pytest"})),
                result(id.clone(), json!([{"type":"text", "text":"x"}])),
            ],
        );
        assert_eq!(
            load_calls(&path),
            [("bash".into(), json!({"command":"pytest"}), "".into())],
            "{id}"
        );
    }
}

#[test]
fn null_or_non_string_tool_name_does_not_crash() {
    let root = TempDir::new("calls-name");
    let path = write(
        &root,
        &[
            tool_use(json!("t1"), Value::Null, json!({"command":"pytest"})),
            tool_use(json!("t2"), json!(42), json!({"command":"pytest"})),
            tool_use(json!("t3"), json!(["x"]), json!({"command":"pytest"})),
        ],
    );
    let names: Vec<_> = load_calls(&path).into_iter().map(|call| call.0).collect();
    assert_eq!(names, ["", "", ""]);
}

#[test]
fn non_string_text_block_does_not_crash() {
    for text in [
        Value::Null,
        json!(123),
        json!(["a"]),
        json!({"k":"v"}),
        json!(true),
    ] {
        let root = TempDir::new("calls-text");
        let path = write(
            &root,
            &[
                tool_use(json!("t1"), json!("Bash"), json!({"command":"pytest"})),
                result(
                    json!("t1"),
                    json!([{"type":"text", "text":text}, {"type":"text", "text":"ok"}]),
                ),
            ],
        );
        assert!(load_calls(&path)[0].2.ends_with("ok"), "{text}");
    }
}

#[test]
fn non_dict_items_in_tool_result_content_are_skipped() {
    let root = TempDir::new("calls-content");
    let path = write(
        &root,
        &[
            tool_use(json!("t1"), json!("Bash"), json!({"command":"pytest"})),
            result(
                json!("t1"),
                json!(["stray", 3, null, {"type":"text", "text":"kept"}]),
            ),
        ],
    );
    assert_eq!(load_calls(&path)[0].2, "kept");
}

#[test]
fn non_dict_tool_input_becomes_empty_args() {
    let root = TempDir::new("calls-input");
    let path = write(
        &root,
        &[
            tool_use(json!("t1"), json!("Bash"), json!(["pytest"])),
            tool_use(json!("t2"), json!("Bash"), json!("pytest")),
        ],
    );
    let inputs: Vec<_> = load_calls(&path).into_iter().map(|call| call.1).collect();
    assert_eq!(inputs, [json!({}), json!({})]);
}
