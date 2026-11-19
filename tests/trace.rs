// Converted from reference/upstream/tests/test_trace.py.
pub mod fixtures;
mod support;

use fixtures::write_claude_transcript;
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};
use support::TempDir;
use tokeneconomizer::{
    experiments::aggregate,
    loopguard::state::completed_calls,
    microskills::distiller,
    toolprune::codex::inspect_usage,
    trace::{calls, catalog, claude, codex, copilot, pi},
};

fn jsonl(path: &Path, records: &[Value]) -> PathBuf {
    let lines: Vec<_> = records.iter().map(Value::to_string).collect();
    fs::write(path, format!("{}\n", lines.join("\n"))).unwrap();
    path.into()
}

#[test]
fn load_trajectory_claude() {
    let root = TempDir::new("trace-claude");
    let path = write_claude_transcript(
        &root.path().join("t.jsonl"),
        &[
            json!(["bash", {"command":"python -m pytest tests/test_x.py"}, "2 passed"]),
            json!(["edit", {"file_path":"pkg/mod.py"}, "ok"]),
        ],
    );
    let names: Vec<_> = calls::load_trajectory(&path)
        .into_iter()
        .map(|call| call.0)
        .collect();
    assert!(names.contains(&json!("bash")) && names.contains(&json!("edit")));
}

#[test]
fn a_claude_request_blob_repeating_the_conversation_adds_no_call() {
    let root = TempDir::new("trace-request-blob");
    let use_block =
        json!({"type":"tool_use", "id":"toolu_1", "name":"Bash", "input":{"command":"pytest -q"}});
    let result = json!({"type":"tool_result", "tool_use_id":"toolu_1",
        "content":"/bin/bash: line 1: pytest: command not found", "is_error":true});
    let lines = [
        json!({"type":"assistant", "cwd":"/w/repo", "message":{"role":"assistant", "content":[use_block]}}),
        json!({"type":"user", "message":{"role":"user", "content":[result]}}),
        json!({"type":"api-request-blob", "message":{"role":"assistant", "content":[use_block]}}),
        json!({"type":"api-request-blob", "message":{"role":"user", "content":[result]}}),
        json!({"type":"assistant", "message":{"role":"assistant", "content":[use_block]}}),
    ];
    let path = root.path().join("s.jsonl");
    fs::write(
        &path,
        lines
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .unwrap();
    let observed: Vec<_> = calls::load_trajectory(&path)
        .into_iter()
        .map(|call| (call.0, call.1["command"].clone(), call.3))
        .collect();
    assert_eq!(
        observed,
        [(json!("Bash"), json!("pytest -q"), json!(false))]
    );
    assert_eq!(calls::call_cwds(&path), [Some("/w/repo".into())]);
    assert_eq!(calls::tool_usage(&path), [("Bash".into(), 1)].into());
    assert_eq!(distiller::load_calls(&path).len(), 1);
}

#[test]
fn tool_usage_counts() {
    let root = TempDir::new("trace-tool-count");
    let path = write_claude_transcript(
        &root.path().join("t.jsonl"),
        &[
            json!(["bash", {"command":"ls"}, ""]),
            json!(["bash", {"command":"pwd"}, ""]),
            json!(["edit", {"file_path":"a.py"}, ""]),
        ],
    );
    assert!(calls::tool_usage(&path).get("bash").copied().unwrap_or(0) >= 2);
}

#[test]
fn parse_catalog_codex_default() {
    let cat = catalog::parse(Path::new("/nonexistent"), "codex");
    assert!(cat.contains_key("shell") && cat.contains_key("apply_patch"));
}

#[test]
fn parse_catalog_missing_returns_empty() {
    let root = TempDir::new("trace-missing-catalog");
    assert!(catalog::parse(root.path(), "claude").is_empty());
}

const FLAT_LOG: &str = "{\n  \"tools\": [\n    \"bash:66921ccc\",\n    \"view:0963ceac\",\n    \"edit:9e640c64\"\n  ]\n}";
fn schema_log() -> String {
    json!({"body":json!({"tools":[{"name":"bash","title":null,"description":"Run a shell command"},
        {"name":"view","description":"Read a file"}]}).to_string()})
    .to_string()
}
fn checkpoint(names: &[&str]) -> Value {
    let tools: Vec<_> = names.iter().map(|name| json!({"name":name})).collect();
    json!({"type":"session.usage_checkpoint", "data":{"promptCacheBreakState":[{"models":{"m":{"tools":tools}}}]}})
}

#[test]
fn flat_catalog_parses_name_fingerprint_form() {
    let cat = catalog::parse_flat(FLAT_LOG);
    assert_eq!(
        cat.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        ["bash", "view", "edit"].into()
    );
    assert!(cat.values().all(|value| value == ""));
    assert!(catalog::parse_schemas(FLAT_LOG, 500).is_empty());
}

#[test]
fn schema_catalog_still_wins_and_keeps_descriptions() {
    let root = TempDir::new("trace-schema-catalog");
    fs::write(
        root.path().join("copilot_debug.log"),
        format!("{}\n{FLAT_LOG}", schema_log()),
    )
    .unwrap();
    let (cat, source) = catalog::parse_with_source(root.path(), "copilot");
    assert_eq!(source.as_deref(), Some("schema_debug_log"));
    assert!(
        cat["bash"]
            .as_str()
            .unwrap()
            .starts_with("Run a shell command")
    );
}

#[test]
fn flat_used_when_schemas_and_checkpoint_are_gone() {
    let root = TempDir::new("trace-flat-catalog");
    fs::write(root.path().join("copilot_debug.log"), FLAT_LOG).unwrap();
    fs::write(
        root.path().join("transcript.jsonl"),
        r#"{"type":"assistant.message","data":{}}"#,
    )
    .unwrap();
    let (cat, source) = catalog::parse_with_source(root.path(), "copilot");
    assert_eq!(source.as_deref(), Some("flat_debug_log"));
    assert_eq!(
        cat.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        ["bash", "view", "edit"].into()
    );
}

#[test]
fn usage_checkpoint_source_is_still_read() {
    let root = TempDir::new("trace-checkpoint");
    fs::write(
        root.path().join("transcript.jsonl"),
        checkpoint(&["bash", "view"]).to_string(),
    )
    .unwrap();
    let (cat, source) = catalog::parse_with_source(root.path(), "copilot");
    assert_eq!(source.as_deref(), Some("usage_checkpoint"));
    assert_eq!(
        cat.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        ["bash", "view"].into()
    );
}

#[test]
fn no_source_reports_none_not_empty_catalog() {
    let root = TempDir::new("trace-no-catalog");
    fs::write(root.path().join("copilot_debug.log"), "nothing useful here").unwrap();
    let (cat, source) = catalog::parse_with_source(root.path(), "copilot");
    assert!(cat.is_empty());
    assert_eq!(source, None);
}

fn p1p2(row: Value) -> (Option<bool>, Option<bool>) {
    // Recover the per-row decisions from the aggregate's single-row counts.
    let cell = aggregate::summarize(&[row])
        .unwrap()
        .into_values()
        .next()
        .unwrap();
    if cell["graded_pn"] == 0 {
        (None, None)
    } else {
        (Some(cell["p1"] == 1), Some(cell["p2"] == 1))
    }
}

#[test]
fn explicit_attempt_fields_win() {
    assert_eq!(
        p1p2(json!({"resolved":true,"resolved_p1":false,"resolved_p2":true,"webbench_attempts":2})),
        (Some(false), Some(true))
    );
}

#[test]
fn legacy_row_second_attempt_pass_is_not_pass_at_1() {
    assert_eq!(
        p1p2(json!({"resolved":true,"webbench_attempts":2})),
        (Some(false), Some(true))
    );
}

#[test]
fn legacy_row_first_attempt_pass() {
    assert_eq!(
        p1p2(json!({"resolved":true,"webbench_attempts":1})),
        (Some(true), Some(true))
    );
}

#[test]
fn legacy_row_failed_every_attempt() {
    assert_eq!(
        p1p2(json!({"resolved":false,"webbench_attempts":2})),
        (Some(false), Some(false))
    );
}

#[test]
fn unreached_task_counts_in_neither_metric() {
    assert_eq!(
        p1p2(json!({"resolved":false,"webbench_attempts":0})),
        (None, None)
    );
}

#[test]
fn single_attempt_domain_collapses_to_resolved() {
    assert_eq!(p1p2(json!({"resolved":true})), (Some(true), Some(true)));
    assert_eq!(p1p2(json!({"resolved":false})), (Some(false), Some(false)));
}

fn user(content: Value) -> Value {
    json!({"type":"user", "message":{"role":"user", "content":content}})
}

#[test]
fn a_claude_session_counts_the_prompts_the_person_gave() {
    let root = TempDir::new("trace-prompts");
    let mut meta = user(json!(
        "Caveat: the messages below were generated by the user"
    ));
    meta["isMeta"] = json!(true);
    let mut summary = user(json!("This session is being continued..."));
    summary["isCompactSummary"] = json!(true);
    let mut sidechain = user(json!("subagent task"));
    sidechain["isSidechain"] = json!(true);
    let path = jsonl(
        &root.path().join("c.jsonl"),
        &[
            user(json!("fix the test")),
            user(json!([{"type":"tool_result","tool_use_id":"t1","content":"ok"}])),
            user(json!([{"type":"text","text":"and the docs"},{"type":"image","source":{}}])),
            user(json!(
                "<command-name>/tokeneconomizer:status</command-name>"
            )),
            user(json!("<local-command-stdout>done</local-command-stdout>")),
            user(json!("[Request interrupted by user]")),
            meta,
            summary,
            sidechain,
            json!({"type":"assistant", "message":{"role":"assistant","content":"done"}}),
        ],
    );
    assert_eq!(claude::prompt_count(&path), 3);
}

#[test]
fn a_claude_background_task_notification_is_no_prompt() {
    let root = TempDir::new("trace-task-notification");
    let note = json!({"type":"user", "promptSource":"system", "turnOrigin":"task_notification",
        "origin":{"kind":"task-notification","producer":"session-task"},"message":{"role":"user","content":
        "<task-notification>\n<task-id>b1</task-id>\n<status>completed</status>\n<summary>Background command \"pytest\" completed (exit code 0)</summary>\n</task-notification>"}});
    let path = jsonl(
        &root.path().join("c.jsonl"),
        &[
            user(json!("run the tests in the background")),
            json!({"type":"assistant","message":{"role":"assistant","content":"started"}}),
            note,
            json!({"type":"assistant","message":{"role":"assistant","content":"they pass"}}),
        ],
    );
    assert_eq!(claude::prompt_count(&path), 1);
    assert_eq!(claude::prompt_turns(&path).count(), 2);
}

#[test]
fn a_codex_session_counts_one_prompt_per_turn() {
    let root = TempDir::new("trace-codex-prompts");
    let turn = [
        json!({"type":"turn_context","payload":{"turn_id":"t"}}),
        json!({"type":"event_msg","payload":{"type":"task_started"}}),
        json!({"type":"response_item","payload":{"type":"message","role":"user"}}),
    ];
    assert_eq!(
        codex::prompt_count(&jsonl(
            &root.path().join("r.jsonl"),
            &[turn.as_slice(); 3].concat()
        )),
        3
    );
    let older: Vec<_> = turn
        .into_iter()
        .filter(|row| row["type"] != "event_msg")
        .collect();
    assert_eq!(
        codex::prompt_count(&jsonl(
            &root.path().join("o.jsonl"),
            &[older.as_slice(); 2].concat()
        )),
        2
    );
}

#[test]
fn a_copilot_session_counts_its_user_messages() {
    let root = TempDir::new("trace-copilot-prompts");
    let path = jsonl(
        &root.path().join("events.jsonl"),
        &[
            json!({"type":"session.start","data":{}}),
            json!({"type":"user.message","data":{}}),
            json!({"type":"tool.execution_start","data":{}}),
            json!({"type":"user.message","data":{}}),
        ],
    );
    assert_eq!(copilot::prompt_count(&path), 2);
}

#[test]
fn a_pi_session_counts_the_prompts_on_its_active_branch() {
    let root = TempDir::new("trace-pi-prompts");
    let message = |id: &str, parent: Option<&str>, role: &str| json!({"type":"message","id":id,"parentId":parent,"message":{"role":role}});
    let path = jsonl(
        &root.path().join("pi.jsonl"),
        &[
            json!({"type":"session","id":"s"}),
            message("a", None, "user"),
            message("b", Some("a"), "assistant"),
            message("x", Some("b"), "user"),
            message("y", Some("x"), "assistant"),
            message("c", Some("b"), "user"),
            message("d", Some("c"), "toolResult"),
            message("e", Some("d"), "assistant"),
        ],
    );
    assert_eq!(pi::prompt_count(&path), 2);
}

const NON_OBJECT_LINES: [&str; 4] = [
    "null",
    "42",
    r#""text""#,
    r#"["usage", "session_meta", "turn_context", "token_count", "response_item", "session.start", "type", "system"]"#,
];
fn around_non_objects(records: &[Value]) -> Vec<String> {
    NON_OBJECT_LINES
        .into_iter()
        .map(str::to_owned)
        .chain(records.iter().map(Value::to_string))
        .chain(NON_OBJECT_LINES.into_iter().map(str::to_owned))
        .collect()
}

#[test]
fn a_non_object_line_does_not_discard_the_calls_around_it() {
    let claude = json!({"type":"assistant","message":{"content":[{"type":"tool_use","id":"t","name":"Bash","input":{"command":"ls"}}]}});
    let codex = json!({"type":"item.completed","item":{"type":"command_execution","command":"ls","exit_code":0}});
    let start = json!({"type":"tool.execution_start","data":{"toolCallId":"c","toolName":"bash","arguments":{"command":"ls"}}});
    let done = json!({"type":"tool.execution_complete","data":{"toolCallId":"c","success":true}});
    let observed: Vec<_> = claude::calls(&around_non_objects(&[claude]))
        .into_iter()
        .map(|call| (call.0, call.1))
        .collect();
    assert_eq!(observed, [(json!("Bash"), json!({"command":"ls"}))]);
    let observed: Vec<_> = codex::exec_calls(&around_non_objects(&[codex]))
        .into_iter()
        .map(|call| (call.0, call.1))
        .collect();
    assert_eq!(observed, [(json!("shell"), json!({"command":"ls"}))]);
    let observed: Vec<_> = copilot::event_calls(&around_non_objects(&[start, done]))
        .into_iter()
        .map(|call| (call.0, call.1))
        .collect();
    assert_eq!(observed, [(json!("bash"), json!({"command":"ls"}))]);
}

#[test]
fn a_non_object_line_does_not_stop_a_session_from_being_read() {
    let root = TempDir::new("trace-non-object-session");
    let claude = json!({"type":"assistant","message":{"id":"m1","usage":{"input_tokens":5}}});
    let meta = json!({"type":"session_meta","payload":{"cwd":"/w","cli_version":"1.0"}});
    let count = json!({"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":10,"cached_input_tokens":4,"output_tokens":3}}}});
    let start = json!({"type":"session.start","data":{"context":{"cwd":"/repo"}}});
    let path = root.path().join("claude.jsonl");
    fs::write(&path, around_non_objects(&[claude]).join("\n")).unwrap();
    assert_eq!(
        claude::session_usage(&path, true).unwrap()["input_tokens"].as_f64(),
        Some(5.0)
    );
    let path = root.path().join("rollout.jsonl");
    fs::write(&path, around_non_objects(&[meta, count]).join("\n")).unwrap();
    assert_eq!(
        codex::session_meta(&path),
        json!({"cwd":"/w","cli_version":"1.0"})
    );
    let usage = codex::session_usage(&path).unwrap();
    assert_eq!(
        (
            usage["input_tokens"].as_f64(),
            usage["output_tokens"].as_f64(),
            usage["requests"].as_f64()
        ),
        (Some(6.0), Some(3.0), Some(1.0))
    );
    assert_eq!(
        copilot::session_cwd(&around_non_objects(&[start])).as_deref(),
        Some("/repo")
    );
}

#[test]
fn a_non_object_line_does_not_stop_a_rollout_from_being_converted() {
    let root = TempDir::new("trace-non-object-rollout");
    let done = json!({"type":"event_msg","payload":{"type":"item_completed","item":{"type":"CommandExecution","command":["ls"],"exit_code":0}}});
    let call = json!({"type":"response_item","payload":{"type":"function_call","name":"x","call_id":"c","arguments":"{}"}});
    let path = root.path().join("rollout.jsonl");
    let out = root.path().join("exec.jsonl");
    fs::write(&path, around_non_objects(&[done, call]).join("\n")).unwrap();
    assert_eq!(codex::rollout_to_exec(&path, &out).unwrap(), Some(1));
    assert_eq!(codex::capability_trace(&path, &out).unwrap(), Some(2));
}

#[test]
fn a_non_object_line_does_not_empty_a_tool_catalog() {
    let system = json!({"type":"system","tools":["Bash"]});
    let text = around_non_objects(&[system]).join("\n");
    assert_eq!(
        catalog::parse_claude(&text, 500, Some(&json!({}))),
        json!({"Bash":""}).as_object().unwrap().clone()
    );
    let text = around_non_objects(&[checkpoint(&["view"])]).join("\n");
    assert_eq!(
        catalog::parse_copilot_names(&text),
        json!({"view":""}).as_object().unwrap().clone()
    );
}

#[test]
fn the_loop_guard_and_the_capability_audit_skip_a_non_object_line() {
    for line in NON_OBJECT_LINES {
        assert!(completed_calls(line, &mut Map::new()).is_empty(), "{line}");
    }
    let root = TempDir::new("trace-non-object-audit");
    let path = root.path().join("exec.jsonl");
    fs::write(
        &path,
        around_non_objects(&[json!({"type":"error","message":"Reconnecting... 1/5"})]).join("\n"),
    )
    .unwrap();
    assert_eq!(inspect_usage(&path)["reconnects"], 1);
}
