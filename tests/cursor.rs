// Converted from reference/upstream/tests/test_cursor.py.
#[path = "support/hook_fixture.rs"]
pub mod hook_fixture;
mod support;
use hook_fixture::HookFixture;

use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};
use support::TempDir;
use tokeneconomizer::{
    plugin::{
        hosts::{
            self,
            session::{self, Environment},
            state,
        },
        sidecall,
    },
    trace::{calls, cursor},
};

fn write_lines(path: &Path, rows: &[Value]) {
    fs::write(
        path,
        rows.iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n",
    )
    .unwrap();
}
#[test]
fn cursor_translates_prompt_and_tool_hook_schemas() {
    let root = TempDir::new("cursor-prepare");
    let fixture = HookFixture::new(root.path(), "cursor");
    let prompt = fixture.prepare(&json!({"hook_event_name":"beforeSubmitPrompt","conversation_id":"conv-1","generation_id":"gen-1","prompt":"Fix the bug","workspace_roots":[fixture.repo]}));
    assert_eq!(prompt["hook_event_name"], "UserPromptSubmit");
    assert_eq!(prompt["session_id"], "conv-1");
    assert_eq!(prompt["prompt_id"], "gen-1");
    assert_eq!(prompt["cwd"], json!(fixture.repo));
    assert_eq!(
        prompt["transcript_path"],
        json!(root.path().join("state/cursor-sessions/conv-1.jsonl"))
    );
    let success = fixture.prepare(&json!({"hook_event_name":"postToolUse","conversation_id":"conv-1","generation_id":"gen-1","tool_name":"Shell","tool_input":{"command":"pytest"},"tool_output":"{\"exitCode\":1,\"stdout\":\"failed\",\"stderr\":\"details\"}"}));
    assert_eq!(success["hook_event_name"], "PostToolUse");
    assert_eq!(success["tool_use_id"], "gen-1");
    assert_eq!(
        success["tool_response"],
        json!({"content":"failed\ndetails","exit_code":1,"isError":false})
    );
    let failure = fixture.prepare(&json!({"hook_event_name":"postToolUseFailure","conversation_id":"conv-1","generation_id":"gen-1","tool_name":"Shell","tool_input":{"command":"rm -rf /"},"error_message":"Denied","failure_type":"permission_denied"}));
    assert_eq!(failure["hook_event_name"], "PostToolUseFailure");
    assert_eq!(failure["error"], "Permission denied: Denied");
}

#[test]
fn cursor_plugin_declares_all_supported_hook_events() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("reference/upstream/plugin");
    let read = |path: &Path| serde_json::from_slice::<Value>(&fs::read(path).unwrap()).unwrap();
    let manifest = read(&root.join(".cursor-plugin/plugin.json"));
    let hooks = read(&root.join("hooks/cursor-hooks.json"));
    let hooks = hooks["hooks"].as_object().unwrap();
    assert_eq!(manifest["name"], "tokeneconomizer");
    assert_eq!(manifest["hooks"], "./hooks/cursor-hooks.json");
    assert_eq!(
        hooks.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        [
            "sessionStart",
            "beforeSubmitPrompt",
            "postToolUse",
            "postToolUseFailure",
            "stop",
            "sessionEnd"
        ]
        .into()
    );
    assert!(
        hooks
            .values()
            .flat_map(|entries| entries.as_array().unwrap())
            .all(|hook| {
                let command = hook["command"].as_str().unwrap();
                command.contains("${CURSOR_PLUGIN_ROOT}/scripts/te-hook")
                    && command.contains("--host cursor")
            })
    );
}

#[test]
fn cursor_events_reconstruct_tool_results_and_count_prompts() {
    let root = TempDir::new("cursor-events");
    let source = root.path().join("events.jsonl");
    let target = root.path().join("trace.jsonl");
    write_lines(
        &source,
        &[
            json!({"hook_event_name":"UserPromptSubmit","prompt":"Fix the bug"}),
            json!({"hook_event_name":"PostToolUse","tool_use_id":"t1","tool_name":"Shell","tool_input":{"command":"pytest"},"tool_response":{"content":"3 passed","exit_code":0}}),
            json!({"hook_event_name":"PostToolUseFailure","tool_use_id":"t2","tool_name":"Read","tool_input":{"file_path":"missing"},"error":"not found"}),
        ],
    );
    assert_eq!(cursor::prompt_count(&source), 1);
    assert_eq!(cursor::events_to_claude(&source, &target), Some(5));
    assert_eq!(
        calls::load_trajectory(&target)
            .into_iter()
            .map(|(name, args, _, ok)| (name, args, ok))
            .collect::<Vec<_>>(),
        [
            (json!("Bash"), json!({"command":"pytest"}), json!(true)),
            (json!("Read"), json!({"file_path":"missing"}), json!(false))
        ]
    );
}

#[test]
fn cursor_readers_skip_json_lines_that_are_not_objects() {
    let root = TempDir::new("cursor-nonobjects");
    let source = root.path().join("events.jsonl");
    let target = root.path().join("trace.jsonl");
    write_lines(
        &source,
        &[
            Value::Null,
            json!([1, 2]),
            json!("str"),
            json!(42),
            json!({"hook_event_name":"UserPromptSubmit","prompt":"Fix the bug"}),
            json!({"hook_event_name":"PostToolUse","tool_use_id":"t1","tool_name":"Shell","tool_input":{"command":"pytest"},"tool_response":{"content":"3 passed","exit_code":0}}),
        ],
    );
    assert_eq!(cursor::prompt_count(&source), 1);
    assert_eq!(cursor::events_to_claude(&source, &target), Some(3));
    assert_eq!(
        calls::load_trajectory(&target)
            .into_iter()
            .map(|(name, args, _, _)| (name, args))
            .collect::<Vec<_>>(),
        [(json!("Bash"), json!({"command":"pytest"}))]
    );
}

#[test]
fn host_stopped_normally_default() {
    for host in ["claude", "codex", "copilot", "opencode", "pi"] {
        assert!(hosts::stopped_normally(host, &json!({})), "{host}");
    }
}

#[test]
fn cursor_stop_lifecycle_status_handling() {
    for (status, normal) in [
        (Some("completed"), true),
        (Some("aborted"), false),
        (Some("error"), false),
        (None, false),
        (Some("unknown"), false),
    ] {
        let root = TempDir::new("cursor-stop");
        let mut fixture = HookFixture::new(root.path(), "cursor");
        fixture.guard_on();
        let base =
            json!({"conversation_id":"c1","generation_id":"g1","workspace_roots":[fixture.repo]});
        fixture.tool(
            &base,
            json!({"hook_event_name":"beforeSubmitPrompt","prompt":"check"}),
        );
        fixture.tool(&base,json!({"hook_event_name":"postToolUse","tool_name":"Shell","tool_use_id":"t1","tool_input":{"command":"pytest"},"tool_output":"{\"output\":\"1 passed\",\"exitCode\":0}"}));
        let mut stop = json!({"hook_event_name":"stop"});
        if let Some(status) = status {
            stop["status"] = json!(status);
        }
        assert_eq!(fixture.tool(&base, stop), json!({}));
        let session = fixture.store.read("sessions/c1.json", Value::Null).unwrap();
        assert!(!session.is_null() && session["turn"].is_null());
        assert_eq!(session["turns"].as_array().unwrap().len(), 1);
        assert_eq!(session["turns"][0]["resolved_proxy"], normal);
        assert_eq!(
            fixture.resolved_turns(),
            if normal { vec![1.0] } else { vec![] }
        );
    }
}

#[test]
fn cursor_native_shell_json_output_positive_and_negative() {
    let root = TempDir::new("cursor-shell-json");
    let mut fixture = HookFixture::new(root.path(), "cursor");
    let base = json!({"conversation_id":"c-shell","generation_id":"g-shell","workspace_roots":[fixture.repo]});
    fixture.tool(
        &base,
        json!({"hook_event_name":"beforeSubmitPrompt","prompt":"build & test"}),
    );
    for (id, command, output) in [
        ("t1", "make build", json!({"output":"ok\n","exitCode":0})),
        ("t2", "pytest -q", json!({"output":"failed\n","exitCode":1})),
    ] {
        fixture.tool(&base,json!({"hook_event_name":"postToolUse","tool_name":"Shell","tool_use_id":id,"tool_input":{"command":command},"tool_output":output.to_string()}));
    }
    let mut payload = base;
    payload["session_id"] = json!("c-shell");
    let prepared = fixture.prepare(&payload);
    let path = session::transcript("cursor", &prepared, &fixture.environment).unwrap();
    let trace =
        session::mechanism_trace("cursor", path.as_str(), root.path(), &fixture.environment)
            .unwrap()
            .unwrap();
    assert_eq!(
        calls::load_trajectory(&trace),
        [
            (
                json!("Bash"),
                json!({"command":"make build"}),
                "ok\n".into(),
                json!(true)
            ),
            (
                json!("Bash"),
                json!({"command":"pytest -q"}),
                "failed\n".into(),
                json!(false)
            )
        ]
    );
}

#[test]
fn notices_name_each_agents_side_call_command() {
    assert_eq!(
        [
            "claude", "codex", "copilot", "pi", "opencode", "cursor", "hermes"
        ]
        .map(sidecall::call_name),
        [
            "claude -p",
            "codex exec",
            "copilot -p",
            "pi -p",
            "opencode run",
            "cursor-agent -p",
            "hermes chat"
        ]
    );
}

fn agent_log(root: &Path, users: usize) -> PathBuf {
    let path = root.join("agent-transcripts/h1/h1.jsonl");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut rows = vec![];
    for _ in 0..users {
        rows.extend([
            json!({"role":"user","message":{"content":[{"type":"text","text":"q"}]}}),
            json!({"role":"assistant","message":{"content":[{"type":"text","text":"a"}]}}),
            json!({"type":"turn_ended","status":"success"}),
        ]);
    }
    write_lines(&path, &rows);
    path
}
fn headless_run(fixture: &mut HookFixture, log: &Path, end: Value) {
    let base = json!({"session_id":"h1","conversation_id":"h1","generation_id":"h1","workspace_roots":[fixture.repo]});
    fixture.tool(
        &base,
        json!({"hook_event_name":"sessionStart","transcript_path":null}),
    );
    for (id, name, input, output, transcript) in [
        (
            "call_a\nfc_1",
            "Shell",
            json!({"command":"ls","cwd":fixture.repo,"timeout":30000}),
            "{\"output\":\"\",\"exitCode\":0}",
            Value::Null,
        ),
        (
            "call_b\nfc_2",
            "Read",
            json!({"file_path":fixture.repo.join("a.py")}),
            "x = 1",
            json!(log),
        ),
    ] {
        fixture.tool(&base,json!({"hook_event_name":"postToolUse","tool_use_id":id,"tool_name":name,"tool_input":input,"tool_output":output,"transcript_path":transcript}));
    }
    let mut fields = json!({"hook_event_name":"sessionEnd","transcript_path":log});
    fields
        .as_object_mut()
        .unwrap()
        .extend(end.as_object().unwrap().clone());
    fixture.tool(&base, fields);
}
fn completed() -> Value {
    json!({"reason":"completed","final_status":"completed","duration_ms":5})
}

#[test]
fn cursor_headless_run_yields_one_turn_closed_normally_at_session_end() {
    let root = TempDir::new("cursor-headless");
    let mut fixture = HookFixture::new(root.path(), "cursor");
    fixture.guard_on();
    headless_run(&mut fixture, &agent_log(root.path(), 1), completed());
    let session = fixture.store.read("sessions/h1.json", Value::Null).unwrap();
    assert!(session["turn"].is_null());
    assert_eq!(session["turns"].as_array().unwrap().len(), 1);
    assert_eq!(session["turns"][0]["resolved_proxy"], true);
    assert_eq!(session["turns"][0]["tool_calls"], 2);
    assert_eq!(fixture.resolved_turns(), [2.0]);
    assert!(fixture.folds.iter().any(|id| id == "h1"));
}

#[test]
fn cursor_session_end_closes_open_turn_as_not_normal_unless_completed() {
    for end in [
        json!({"reason":"aborted","final_status":"aborted"}),
        json!({"reason":"user_close"}),
        json!({"duration_ms":1}),
    ] {
        let root = TempDir::new("cursor-aborted");
        let mut fixture = HookFixture::new(root.path(), "cursor");
        fixture.guard_on();
        headless_run(&mut fixture, &agent_log(root.path(), 1), end);
        let session = fixture.store.read("sessions/h1.json", Value::Null).unwrap();
        assert_eq!(session["turns"].as_array().unwrap().len(), 1);
        assert_eq!(session["turns"][0]["resolved_proxy"], false);
    }
}

#[test]
fn cursor_completed_session_end_does_not_close_a_turn_twice() {
    let root = TempDir::new("cursor-close-once");
    let mut fixture = HookFixture::new(root.path(), "cursor");
    fixture.guard_on();
    let base =
        json!({"conversation_id":"c2","generation_id":"g2","workspace_roots":[fixture.repo]});
    fixture.tool(
        &base,
        json!({"hook_event_name":"beforeSubmitPrompt","prompt":"go"}),
    );
    fixture.tool(&base,json!({"hook_event_name":"postToolUse","tool_name":"Shell","tool_use_id":"t","tool_input":{"command":"ls"},"tool_output":"{\"output\":\"\",\"exitCode\":0}"}));
    fixture.tool(
        &base,
        json!({"hook_event_name":"stop","status":"completed"}),
    );
    fixture.tool(
        &base,
        json!({"hook_event_name":"sessionEnd","reason":"completed","final_status":"completed"}),
    );
    let session = fixture.store.read("sessions/c2.json", Value::Null).unwrap();
    assert_eq!(session["turns"].as_array().unwrap().len(), 1);
    assert_eq!(fixture.resolved_turns(), [1.0]);
}

#[test]
fn cursor_prompt_count_falls_back_to_agent_transcript() {
    let root = TempDir::new("cursor-prompt-fallback");
    let mut fixture = HookFixture::new(root.path(), "cursor");
    headless_run(&mut fixture, &agent_log(root.path(), 2), completed());
    let prepared = fixture.prepare(&json!({"conversation_id":"h1"}));
    let spool = session::transcript("cursor", &prepared, &fixture.environment).unwrap();
    let spool = Path::new(spool.as_str().unwrap());
    assert_eq!(session::prompt_count("cursor", spool), Some(2));
    let output =
        session::mechanism_trace("cursor", spool.to_str(), root.path(), &fixture.environment)
            .unwrap()
            .unwrap();
    assert_eq!(
        calls::load_trajectory(&output)
            .into_iter()
            .map(|(name, args, _, _)| (name, args))
            .collect::<Vec<_>>(),
        [
            (
                json!("Bash"),
                json!({"command":"ls","cwd":fixture.repo,"timeout":30000})
            ),
            (
                json!("Read"),
                json!({"file_path":fixture.repo.join("a.py")})
            )
        ]
    );
}

#[test]
fn cursor_prompt_count_prefers_recorded_prompts_and_tolerates_gaps() {
    let root = TempDir::new("cursor-prompt-gaps");
    let spool = root.path().join("s.jsonl");
    let log = root.path().join("agent.jsonl");
    write_lines(&log, &[json!({"role":"user"})]);
    let row = json!({"hook_event_name":"PostToolUse","agent_transcript":log});
    write_lines(&spool, std::slice::from_ref(&row));
    assert_eq!(cursor::prompt_count(&spool), 1);
    write_lines(
        &spool,
        &[
            row.clone(),
            json!({"hook_event_name":"UserPromptSubmit","prompt":"p"}),
        ],
    );
    assert_eq!(cursor::prompt_count(&spool), 1);
    fs::remove_file(log).unwrap();
    write_lines(&spool, &[row]);
    assert_eq!(cursor::prompt_count(&spool), 0);
}

#[test]
fn cursor_prepare_keeps_agent_transcript_path_only_when_set() {
    let root = TempDir::new("cursor-transcript-path");
    let env = Environment {
        env: json!({"TOKENECONOMIZER_HOME":root.path()})
            .as_object()
            .unwrap()
            .clone(),
        home: root.path().into(),
        cwd: root.path().into(),
    };
    let null = state::prepare("cursor",&json!({"hook_event_name":"sessionStart","session_id":"s","conversation_id":"s","transcript_path":null}),&json!(env.env),&env).unwrap();
    assert!(null.get("agent_transcript").is_none());
    let real = state::prepare("cursor",&json!({"hook_event_name":"sessionEnd","session_id":"s","conversation_id":"s","transcript_path":"/home/u/.cursor/projects/x/agent-transcripts/s/s.jsonl"}),&json!(env.env),&env).unwrap();
    assert!(
        real["agent_transcript"]
            .as_str()
            .unwrap()
            .ends_with("/s/s.jsonl")
    );
    assert!(
        Path::new(real["transcript_path"].as_str().unwrap()).ends_with("cursor-sessions/s.jsonl")
    );
}
