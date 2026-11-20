// Converted from reference/upstream/tests/test_grok.py.
#[path = "support/hook_fixture.rs"]
pub mod hook_fixture;
mod support;

use hook_fixture::HookFixture;
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeSet,
    fs, io,
    path::{Path, PathBuf},
};
use support::TempDir;
use tokeneconomizer::{
    accounting,
    plugin::{
        config::LiveConfig,
        hosts::{
            self,
            session::{self, Environment},
            state,
        },
        process,
        sidecall::{self, Call},
    },
    trace::{calls, grok},
};

const SID: &str = "grok-session-1";
const FACTS: &str =
    "Facts learned about this repository:\n- run tests with python -m pytest\n- no bare pytest";

fn environment(root: &Path) -> Environment {
    Environment {
        env: json!({"TOKENECONOMIZER_HOME":root.join("state")})
            .as_object()
            .unwrap()
            .clone(),
        home: root.into(),
        cwd: root.into(),
    }
}

fn payload(event: &str, repo: impl AsRef<Path>, prompt: Option<&str>, extra: Value) -> Value {
    let snake = match event {
        "SessionStart" => "session_start",
        "UserPromptSubmit" => "user_prompt_submit",
        "PostToolUse" => "post_tool_use",
        "PostToolUseFailure" => "post_tool_use_failure",
        "Stop" => "stop",
        "StopCancelled" => "stop_cancelled",
        "StopFailure" => "stop_failure",
        "SessionEnd" => "session_end",
        _ => panic!("unexpected event: {event}"),
    };
    let path = format!("/home/u/.grok/sessions/%2Fwork%2Frepo/{SID}/updates.jsonl");
    let mut out = json!({"hookEventName":snake,"hook_event_name":event,"sessionId":SID,"session_id":SID,"cwd":repo.as_ref(),"workspaceRoot":repo.as_ref(),"transcriptPath":path,"transcript_path":path});
    if let Some(prompt) = prompt {
        out["promptId"] = json!(prompt);
        out["prompt_id"] = json!(prompt);
    }
    for (key, value) in extra.as_object().unwrap() {
        out[key] = value.clone();
        if let Some(alias) = match key.as_str() {
            "toolName" => Some("tool_name"),
            "toolInput" => Some("tool_input"),
            "toolResult" => Some("tool_response"),
            _ => None,
        } {
            out[alias] = value.clone();
        }
    }
    out
}

fn prepare(payload: &Value) -> Value {
    state::prepare(
        "grok",
        payload,
        &json!({}),
        &environment(Path::new("/home/u")),
    )
    .unwrap()
}

fn tool(payload: Value) -> tokeneconomizer::trace::Call {
    hosts::tool_call("grok", &prepare(&payload)).unwrap()
}

fn bash_result(text: &str, code: i32) -> Value {
    json!({"type":"Bash","output":text.as_bytes(),"exit_code":code,"output_for_prompt":format!("exit: {code}\n{text}"),"command":"x","truncated":false,"timed_out":false,"current_dir":"/work/repo","total_bytes":text.chars().count()})
}

fn bash(command: &str, text: &str, code: i32) -> Value {
    json!({"toolName":"run_terminal_command","toolInput":{"command":command},"toolResult":bash_result(text,code)})
}

fn fixture(root: &Path) -> HookFixture {
    let mut fixture = HookFixture::new(root, "grok");
    fixture
        .environment
        .env
        .insert("TE_MICROSKILLS".into(), json!("envfacts"));
    fixture.guard_on();
    fixture
}

fn document(fixture: &HookFixture) -> Value {
    fixture
        .store
        .read(&format!("sessions/{SID}.json"), json!({}))
        .unwrap()
}

#[test]
fn grok_is_recognised_before_the_claude_fallback_with_no_host_flag() {
    let mut p = payload("PostToolUse", "/work/repo", None, json!({}));
    assert_eq!(hosts::detect(&p, &json!({}), None, &[]), "grok");
    p.as_object_mut().unwrap().remove("hookEventName");
    assert_eq!(hosts::detect(&p, &json!({}), None, &[]), "grok");
    for env in [
        json!({"GROK_HOOK_EVENT":"post_tool_use"}),
        json!({"GROK_SESSION_ID":"s"}),
    ] {
        assert_eq!(
            hosts::detect(&json!({"session_id":"s"}), &env, None, &[]),
            "grok"
        );
    }
    let p = payload("Stop", "/work/repo", None, json!({}));
    let registered: Vec<_> = hosts::HOSTS.into_iter().map(str::to_owned).collect();
    assert_eq!(
        hosts::detect(&p, &json!({}), Some("claude"), &registered),
        "claude"
    );
    assert_eq!(
        hosts::detect(&p, &json!({}), Some("grok"), &registered),
        "grok"
    );
    assert!(hosts::HOSTS.contains(&"grok"));
}

#[test]
fn claude_code_stays_claude_code_even_inside_a_grok_shell() {
    let p = json!({"session_id":"s","hook_event_name":"PostToolUse","transcript_path":"/home/u/.claude/projects/-work-repo/s.jsonl"});
    assert_eq!(
        hosts::detect(&p, &json!({"GROK_SESSION_ID":"g"}), None, &[]),
        "claude"
    );
    assert_eq!(
        hosts::detect(&json!({"session_id":"s"}), &json!({}), None, &[]),
        "claude"
    );
}

#[test]
fn a_te_command_in_a_grok_shell_is_about_grok() {
    assert_eq!(
        hosts::default_host(&json!({"GROK_SESSION_ID":"g"}), None, &[]),
        "grok"
    );
    assert_eq!(
        hosts::default_host(&json!({"GROK_SESSION_ID":"g","CLAUDECODE":"1"}), None, &[]),
        "claude"
    );
    assert_eq!(
        hosts::env_session_id("grok", &json!({"GROK_SESSION_ID":"g"})),
        "g"
    );
}

#[test]
fn stop_is_a_turns_end_only_for_end_turn_with_a_prompt_id() {
    let end = prepare(&payload(
        "Stop",
        "/work/repo",
        Some("p1"),
        json!({"reason":"end_turn"}),
    ));
    assert_eq!(end["hook_event_name"], "Stop");
    assert!(hosts::stopped_normally("grok", &end));
    assert_eq!(hosts::turn_key("grok", &end), "p1");
    for prompt in [None, Some("p1")] {
        assert_eq!(
            prepare(&payload(
                "Stop",
                "/work/repo",
                prompt,
                json!({"reason":"shutdown"})
            ))["hook_event_name"],
            ""
        );
    }
}

#[test]
fn a_cancelled_or_failed_turn_stops_abnormally() {
    for (event, reason) in [
        ("StopCancelled", Some("user_interrupt")),
        ("StopCancelled", None),
        ("StopFailure", None),
    ] {
        let extra = reason.map_or(json!({}), |reason| json!({"reason":reason}));
        let p = prepare(&payload(event, "/work/repo", Some("p1"), extra));
        assert_eq!(p["hook_event_name"], "Stop");
        assert!(!hosts::stopped_normally("grok", &p));
    }
}

#[test]
fn events_the_plugin_does_not_use_are_ignored() {
    assert_eq!(
        prepare(
            &json!({"hookEventName":"pre_tool_use","hook_event_name":"PreToolUse","sessionId":"s"})
        )["hook_event_name"],
        ""
    );
    assert_eq!(
        prepare(&json!({"sessionId":"s","hookEventName":"post_tool_use"}))["session_id"],
        "s"
    );
}

#[test]
fn a_shell_result_is_read_from_the_text_grok_showed_and_its_exit_code() {
    let call = tool(payload(
        "PostToolUse",
        "/work/repo",
        None,
        bash("rtk ls nope", "ls: nope: No such file or directory\n", 1),
    ));
    assert_eq!(call.0, "Bash");
    assert_eq!(call.1["command"], "ls nope");
    assert_eq!(call.3, 1);
    assert_eq!(call.2, "ls: nope: No such file or directory\n");
    assert_eq!(
        tool(payload(
            "PostToolUse",
            "/work/repo",
            None,
            bash("true", "", 0)
        ))
        .3,
        0
    );
    let mut timed = bash("sleep 9", "", 0);
    timed["toolResult"]["timed_out"] = json!(true);
    assert_eq!(
        tool(payload("PostToolUse", "/work/repo", None, timed)).3,
        grok::TIMEOUT_CODE
    );
}

#[test]
fn a_result_with_only_the_byte_list_is_decoded_not_listed() {
    assert_eq!(
        grok::result(&json!({"type":"Bash","output":"héllo".as_bytes(),"exit_code":0})),
        Some(("héllo".into(), Some(0)))
    );
}

#[test]
fn grok_tools_are_named_as_the_mechanisms_know_them() {
    let mapped = |name: &str, args: Value| {
        let call = tool(payload(
            "PostToolUse",
            "/work/repo",
            None,
            json!({"toolName":name,"toolInput":args,"toolResult":null}),
        ));
        (call.0, call.1)
    };
    assert_eq!(
        mapped(
            "read_file",
            json!({"target_file":"/work/repo/a.py","limit":5})
        ),
        (
            json!("Read"),
            json!({"file_path":"/work/repo/a.py","limit":5})
        )
    );
    assert_eq!(
        mapped(
            "search_replace",
            json!({"file_path":"/work/repo/a.py","old_string":"a","new_string":"b"})
        ),
        (json!("Edit"), json!({"file_path":"/work/repo/a.py"}))
    );
    assert_eq!(
        mapped("grep", json!({"pattern":"x","path":"src","head_limit":3})),
        (json!("Grep"), json!({"pattern":"x","path":"src"}))
    );
    assert_eq!(
        mapped("list_dir", json!({"target_directory":"/work/repo"})),
        (json!("Glob"), json!({"path":"/work/repo"}))
    );
    assert_eq!(
        mapped("web_search", json!({"query":"q"})),
        (json!("WebSearch"), json!({"query":"q"}))
    );
    assert_eq!(mapped("spawn_subagent", json!({})).0, "Task");
    assert_eq!(
        mapped("acme__lookup", json!({"id":1})),
        (json!("acme__lookup"), json!({"id":1}))
    );
}

#[test]
fn other_results_are_read_by_their_tag_and_a_no_match_grep_is_not_a_failure() {
    let result = |name: &str, response: Value| {
        let call = tool(payload(
            "PostToolUse",
            "/work/repo",
            None,
            json!({"toolName":name,"toolInput":{},"toolResult":response}),
        ));
        (call.2, call.3)
    };
    assert_eq!(
        result(
            "read_file",
            json!({"type":"ReadFile","FileContent":{"content":"1→hi\n","raw_output":"hi\n"}})
        ),
        ("hi\n".into(), Value::Null)
    );
    assert_eq!(
        result(
            "grep",
            json!({"type":"GrepSearch","stdout":[],"stderr":[],"exit_code":1,"match_count":0})
        ),
        ("".into(), json!(0))
    );
    assert_eq!(
        result(
            "grep",
            json!({"type":"GrepSearch","stdout":[],"stderr":b"bad regex","exit_code":2})
        )
        .1,
        2
    );
    assert_eq!(
        result(
            "web_fetch",
            json!("plain text of a result past the size limit")
        ),
        (
            "plain text of a result past the size limit".into(),
            Value::Null
        )
    );
    let failed = tool(payload(
        "PostToolUseFailure",
        "/work/repo",
        None,
        json!({"toolName":"acme__lookup","toolInput":{},"error":"MCP error"}),
    ));
    assert_eq!(failed.0, "acme__lookup");
    assert!(failed.2.contains("MCP error"));
}

#[test]
fn a_subagents_tool_events_are_not_the_sessions() {
    assert!(hosts::is_subagent(
        "grok",
        &payload(
            "PostToolUse",
            "/work/repo",
            None,
            json!({"subagentType":"explore"})
        )
    ));
    assert!(!hosts::is_subagent(
        "grok",
        &payload(
            "PostToolUse",
            "/work/repo",
            None,
            json!({"subagentType":null})
        )
    ));
}

#[test]
fn tool_events_without_a_prompt_id_stay_in_the_turn_and_the_shutdown_stop_changes_nothing() {
    let root = TempDir::new("grok-turn");
    let mut f = fixture(root.path());
    f.run(payload(
        "UserPromptSubmit",
        &f.repo,
        Some("p1"),
        json!({"prompt":"go"}),
    ));
    for i in 0..3 {
        f.run(payload(
            "PostToolUse",
            &f.repo,
            None,
            bash(&format!("echo {i}"), &format!("{i}\n"), 0),
        ));
    }
    let session = document(&f);
    assert_eq!(session["turn"]["turn_key"], "p1");
    assert_eq!(session["turn"]["cum_turns"], 3);
    assert_eq!(session["host"], "grok");
    assert!(session["turns"].as_array().unwrap().is_empty());
    f.run(payload("Stop", &f.repo, None, json!({"reason":"shutdown"})));
    assert!(!document(&f)["turn"].is_null());
    assert!(document(&f)["turns"].as_array().unwrap().is_empty());
    f.run(payload(
        "Stop",
        &f.repo,
        Some("p1"),
        json!({"reason":"end_turn","stop_hook_active":false}),
    ));
    f.run(payload("Stop", &f.repo, None, json!({"reason":"shutdown"})));
    assert!(document(&f)["turn"].is_null());
    assert_eq!(document(&f)["turns"].as_array().unwrap().len(), 1);
}

#[test]
fn a_cancelled_turn_is_recorded_as_not_resolved() {
    let root = TempDir::new("grok-cancelled");
    let mut f = fixture(root.path());
    f.run(payload(
        "UserPromptSubmit",
        &f.repo,
        Some("p1"),
        json!({"prompt":"go"}),
    ));
    f.run(payload(
        "PostToolUse",
        &f.repo,
        None,
        bash("make", "ok\n", 0),
    ));
    f.run(payload(
        "StopCancelled",
        &f.repo,
        Some("p1"),
        json!({"reason":"user_interrupt"}),
    ));
    let doc = document(&f);
    let turns = doc["turns"].as_array().unwrap();
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0]["resolved_proxy"], false);
}

fn injection(f: &HookFixture, ids: bool) {
    let mut value = json!({"rendered":FACTS});
    if ids {
        value["rendered_ids"] = json!(["a", "b"]);
    }
    f.store.write("injection.json", &value).unwrap();
}

#[test]
fn the_start_context_reaches_the_model_once_with_the_first_tool_result() {
    let root = TempDir::new("grok-start-context");
    let mut f = fixture(root.path());
    injection(&f, true);
    let start = f.run(payload(
        "SessionStart",
        &f.repo,
        None,
        json!({"source":"new"}),
    ));
    assert!(start.get("hookSpecificOutput").is_none());
    assert!(
        start["systemMessage"]
            .as_str()
            .unwrap()
            .contains("learned fact")
    );
    assert_eq!(
        f.run(payload(
            "UserPromptSubmit",
            &f.repo,
            Some("p1"),
            json!({"prompt":"go"})
        )),
        json!({})
    );
    let first = f.run(payload("PostToolUse", &f.repo, None, bash("ls", "a\n", 0)));
    assert_eq!(
        first["hookSpecificOutput"],
        json!({"hookEventName":"PostToolUse","additionalContext":FACTS})
    );
    assert!(
        f.run(payload("PostToolUse", &f.repo, None, bash("ls", "a\n", 0)))
            .get("hookSpecificOutput")
            .is_none()
    );
    assert_eq!(document(&f)["injected_chars"], FACTS.len());
}

#[test]
fn a_failed_first_call_delivers_it_under_the_failure_events_name() {
    let root = TempDir::new("grok-context-failure");
    let mut f = fixture(root.path());
    injection(&f, false);
    f.run(payload(
        "SessionStart",
        &f.repo,
        None,
        json!({"source":"new"}),
    ));
    let out = f.run(payload(
        "PostToolUseFailure",
        &f.repo,
        None,
        json!({"toolName":"acme__lookup","toolInput":{},"error":"MCP error"}),
    ));
    assert_eq!(
        out["hookSpecificOutput"]["hookEventName"],
        "PostToolUseFailure"
    );
    assert_eq!(out["hookSpecificOutput"]["additionalContext"], FACTS);
}

#[test]
fn a_subagents_tool_call_does_not_take_the_start_context() {
    let root = TempDir::new("grok-context-subagent");
    let mut f = fixture(root.path());
    injection(&f, false);
    f.run(payload(
        "SessionStart",
        &f.repo,
        None,
        json!({"source":"new"}),
    ));
    let mut subagent = bash("ls", "a\n", 0);
    subagent["subagentType"] = json!("explore");
    assert_eq!(
        f.run(payload("PostToolUse", &f.repo, None, subagent)),
        json!({})
    );
    assert!(
        f.run(payload("PostToolUse", &f.repo, None, bash("ls", "a\n", 0)))["hookSpecificOutput"]
            .get("additionalContext")
            .is_some()
    );
}

#[test]
fn a_resumed_session_is_not_given_the_context_again_and_a_session_end_drops_it() {
    let root = TempDir::new("grok-context-resume");
    let mut f = fixture(root.path());
    injection(&f, false);
    f.run(payload(
        "SessionStart",
        &f.repo,
        None,
        json!({"source":"new"}),
    ));
    let spool = f.environment.env["TOKENECONOMIZER_HOME"]
        .as_str()
        .map(Path::new)
        .unwrap()
        .join("grok-start")
        .join(format!("{SID}.txt"));
    assert_eq!(fs::read_to_string(&spool).unwrap(), FACTS);
    f.run(payload(
        "SessionEnd",
        &f.repo,
        None,
        json!({"reason":"shutdown"}),
    ));
    assert!(!spool.exists());
    for (event, extra) in [
        ("SessionStart", json!({"source":"load"})),
        ("PostToolUse", bash("ls", "a\n", 0)),
    ] {
        let mut p = payload(event, &f.repo, None, extra);
        p["sessionId"] = json!("resumed");
        p["session_id"] = json!("resumed");
        for key in ["transcriptPath", "transcript_path"] {
            p[key] = json!("/home/u/.grok/sessions/%2Fwork%2Frepo/resumed/updates.jsonl");
        }
        let out = f.run(p);
        if event == "PostToolUse" {
            assert!(out.get("hookSpecificOutput").is_none());
        }
    }
}

#[test]
fn the_start_context_gives_way_to_the_calls_own_context_within_groks_cap() {
    let root = TempDir::new("grok-context-cap");
    let env = environment(root.path());
    let p = payload("PostToolUse", "/work/repo", None, json!({}));
    let spool = state::spool(&p, &json!(env.env), &env).unwrap();
    fs::create_dir_all(spool.parent().unwrap()).unwrap();
    fs::write(&spool, "s".repeat(5000)).unwrap();
    let own = "n".repeat(9990);
    let mut out =
        json!({"hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":own}});
    state::render_grok(&p, &mut out, &json!(env.env), state::CONTEXT_CAP, &env).unwrap();
    let text = out["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(text.len() <= state::CONTEXT_CAP as usize);
    assert!(text.ends_with(&own));
    assert!(!spool.exists());
}

fn turn(f: &mut HookFixture, key: &str, calls: &[Value]) -> Vec<Value> {
    f.run(payload(
        "UserPromptSubmit",
        &f.repo,
        Some(key),
        json!({"prompt":"go"}),
    ));
    let out = calls
        .iter()
        .map(|call| f.run(payload("PostToolUse", &f.repo, None, call.clone())))
        .collect();
    f.run(payload(
        "Stop",
        &f.repo,
        Some(key),
        json!({"reason":"end_turn","stop_hook_active":false}),
    ));
    out
}

#[test]
fn a_stuck_turn_is_nudged_through_the_post_tool_use_context() {
    let root = TempDir::new("grok-stuck");
    let mut f = fixture(root.path());
    f.inline_judge = true;
    let read = json!({"toolName":"read_file","toolInput":{"target_file":"/work/repo/a.py"},"toolResult":{"type":"ReadFile","FileContent":{"raw_output":"ok\n"}}});
    for i in 0..5 {
        turn(&mut f, &format!("g{i}"), &vec![read.clone(); 8]);
    }
    let fail = "FAILED tests/test_a.py::test_x - AssertionError: assert 1 == 2\n";
    let edit = json!({"toolName":"search_replace","toolInput":{"file_path":"/work/repo/pkg/a.py","old_string":"x","new_string":"y"},"toolResult":{"type":"SearchReplace","EditsApplied":{"tool_output_for_prompt":"edited"}}});
    let stuck: Vec<_> = (0..40)
        .map(|i| {
            if i % 2 == 0 {
                edit.clone()
            } else {
                bash("pytest -q", fail, 1)
            }
        })
        .collect();
    let out = turn(&mut f, "stuck", &stuck);
    let first = out
        .iter()
        .find(|out| out.get("hookSpecificOutput").is_some())
        .unwrap();
    assert_eq!(first["hookSpecificOutput"]["hookEventName"], "PostToolUse");
    assert!(
        !first["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .is_empty()
    );
    assert!(
        f.store
            .read_lines("events.jsonl", None)
            .unwrap()
            .iter()
            .any(|event| event["kind"] == "nudge")
    );
}

fn update(kind: &str, method: &str, fields: Value) -> Value {
    let mut value = fields;
    value["sessionUpdate"] = json!(kind);
    json!({"method":method,"params":{"sessionId":SID,"update":value}})
}

fn write_session(directory: &Path, usage: Option<Value>) -> PathBuf {
    let row = |kind, fields| update(kind, "session/update", fields);
    let rows = [
        update(
            "hook_execution",
            "_x.ai/session/update",
            json!({"event_name":"user_prompt_submit"}),
        ),
        row(
            "user_message_chunk",
            json!({"content":{"type":"text","text":"fix "},"_meta":{"promptIndex":0}}),
        ),
        row(
            "user_message_chunk",
            json!({"content":{"type":"text","text":"the test"},"_meta":{"promptIndex":0}}),
        ),
        row(
            "agent_thought_chunk",
            json!({"content":{"type":"text","text":"thinking"}}),
        ),
        row(
            "tool_call",
            json!({"toolCallId":"call-1","title":"run_terminal_command","rawInput":{"command":"rtk pytest -q"}}),
        ),
        row(
            "tool_call_update",
            json!({"toolCallId":"call-1","kind":"execute","rawInput":{"variant":"Bash","command":"rtk pytest -q"}}),
        ),
        row(
            "tool_call_update",
            json!({"toolCallId":"call-1","status":"completed","rawOutput":bash_result("1 failed\n",1)}),
        ),
        row(
            "tool_call",
            json!({"toolCallId":"call-2","title":"read_file","rawInput":{"target_file":"/work/repo/a.py"}}),
        ),
        row(
            "tool_call_update",
            json!({"toolCallId":"call-2","status":"completed","rawOutput":{"type":"ReadFile","FileContent":{"raw_output":"code\n"}}}),
        ),
        row(
            "tool_call",
            json!({"toolCallId":"call-3","title":"acme__lookup","rawInput":{"id":1}}),
        ),
        row(
            "tool_call_update",
            json!({"toolCallId":"call-3","status":"failed","content":[{"type":"content","content":{"type":"text","text":"boom"}}]}),
        ),
        row(
            "agent_message_chunk",
            json!({"content":{"type":"text","text":"done"}}),
        ),
        update(
            "turn_completed",
            "_x.ai/session/update",
            json!({"stop_reason":"end_turn"}),
        ),
        row(
            "user_message_chunk",
            json!({"content":{"type":"text","text":"thanks"},"_meta":{"promptIndex":1}}),
        ),
        row(
            "agent_message_chunk",
            json!({"content":{"type":"text","text":"welcome"}}),
        ),
    ];
    let path = directory.join("updates.jsonl");
    fs::write(
        &path,
        rows.iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n",
    )
    .unwrap();
    if let Some(usage) = usage {
        fs::write(directory.join("usage.json"), usage.to_string()).unwrap();
    }
    path
}

#[test]
fn a_session_log_becomes_a_trajectory_the_mechanisms_read() {
    let root = TempDir::new("grok-trace");
    let src = write_session(root.path(), None);
    let out = root.path().join("trace.jsonl");
    assert_eq!(grok::session_to_claude(&src, &out).unwrap(), Some(3));
    assert_eq!(grok::prompt_count(&src), 2);
    assert_eq!(
        calls::load_trajectory(&out),
        vec![
            (
                json!("Bash"),
                json!({"command":"pytest -q"}),
                "1 failed\n".into(),
                json!(false)
            ),
            (
                json!("Read"),
                json!({"file_path":"/work/repo/a.py"}),
                "code\n".into(),
                json!(true)
            ),
            (
                json!("acme__lookup"),
                json!({"id":1}),
                "boom".into(),
                json!(false)
            ),
        ]
    );
    assert_eq!(
        calls::tool_usage(&out),
        [
            ("Bash".into(), 1),
            ("Read".into(), 1),
            ("acme__lookup".into(), 1)
        ]
        .into()
    );
    assert_eq!(
        session::mechanism_trace("grok", src.to_str(), root.path(), &environment(root.path()))
            .unwrap(),
        Some(root.path().join("grok_trace.jsonl"))
    );
}

#[test]
fn a_missing_or_empty_log_has_no_trace() {
    let root = TempDir::new("grok-empty-log");
    assert_eq!(
        grok::session_to_claude(&root.path().join("none.jsonl"), &root.path().join("t")).unwrap(),
        None
    );
    fs::write(root.path().join("e.jsonl"), "not json\n").unwrap();
    assert_eq!(
        grok::session_to_claude(&root.path().join("e.jsonl"), &root.path().join("t")).unwrap(),
        None
    );
    assert_eq!(
        session::mechanism_trace("grok", Some(""), root.path(), &environment(root.path())).unwrap(),
        None
    );
    assert_eq!(grok::prompt_count(&root.path().join("none.jsonl")), 0);
}

#[test]
fn session_usage_is_fresh_input_cached_reads_and_output() {
    let root = TempDir::new("grok-usage");
    let path = write_session(
        root.path(),
        Some(
            json!({"sessionId":SID,"session":{"inputTokens":40650,"outputTokens":913,"cachedReadTokens":30208,"cacheCreationTokens":0,"reasoningTokens":806,"modelCalls":2,"costUsdTicks":414660000,"turnCount":1,"primaryModelId":"grok-4.7-build"}}),
        ),
    );
    let got = session::session_usage("grok", &path).unwrap();
    assert_eq!(got["input_tokens"].as_f64(), Some(10442.0));
    assert_eq!(got["cache_read_input_tokens"].as_f64(), Some(30208.0));
    assert_eq!(got["cache_creation_input_tokens"].as_f64(), Some(0.0));
    assert_eq!(got["output_tokens"].as_f64(), Some(913.0));
    assert_eq!(got["requests"].as_f64(), Some(2.0));
    assert_eq!(got["model"], "grok-4.7-build");
    assert_eq!(got["cost_usd_vendor"].as_f64(), Some(0.041466));
    let (effective, priced_by) = accounting::effective(&got).unwrap();
    assert!(effective.is_some_and(|n| n != 0.0));
    assert_eq!(priced_by, "fallback");
    assert_eq!(
        session::session_usage("grok", &root.path().join("nowhere/updates.jsonl")).unwrap(),
        json!({})
    );
}

#[derive(Default)]
struct SideCall {
    argv: Vec<String>,
    env: Map<String, Value>,
    input: Option<String>,
    cwd: PathBuf,
    failed: bool,
}
impl sidecall::Runtime for SideCall {
    fn program(&mut self, cli: &str, _: bool, _: &Environment) -> Result<Vec<String>, String> {
        Ok(vec![cli.into()])
    }
    fn run(
        &mut self,
        argv: &[String],
        cwd: &Path,
        env: &Map<String, Value>,
        input: Option<&str>,
        _: f64,
    ) -> io::Result<process::Output> {
        self.argv = argv.to_vec();
        self.cwd = cwd.into();
        self.env = env.clone();
        self.input = input.map(str::to_owned);
        Ok(if self.failed {
            process::Output {
                code: 1,
                stdout: String::new(),
                stderr: "Not signed in".into(),
            }
        } else {
            process::Output { code:0, stdout:json!({"text":"- use python -m pytest\n","stopReason":"end_turn","usage":{"input_tokens":10319,"cache_read_input_tokens":30208,"cache_creation_input_tokens":0,"output_tokens":252},"modelUsage":{"grok-4.7-build":{}}}).to_string(), stderr:String::new() }
        })
    }
}

fn side_call(root: &Path, model: &str, runtime: &mut SideCall) -> (Value, Option<f64>) {
    sidecall::grok_call(
        &Call {
            prompt: if model.is_empty() {
                "p"
            } else {
                "propose facts"
            },
            model,
            cli: "grok",
            timeout: 180.0,
            effort: "",
            windows: cfg!(windows),
        },
        &environment(root),
        root,
        runtime,
    )
    .unwrap()
}

#[test]
fn a_grok_side_call_is_a_headless_completion_that_removes_what_acts() {
    let root = TempDir::new("grok-sidecall");
    let mut seen = SideCall::default();
    let (text, effective) = side_call(root.path(), "grok-4.7", &mut seen);
    assert_eq!(text, "- use python -m pytest");
    assert!(effective.is_some_and(|n| n > 0.0));
    assert_eq!(
        seen.argv[..5],
        ["grok", "-p", "propose facts", "--output-format", "json"]
    );
    let after =
        |flag| seen.argv[seen.argv.iter().position(|arg| arg == flag).unwrap() + 1].as_str();
    assert_eq!(after("--tools"), sidecall::GROK_TOOLS);
    assert_eq!(sidecall::GROK_TOOLS, "read_file");
    assert_eq!(after("--disallowed-tools"), "use_tool,search_tool");
    let denied: BTreeSet<_> = seen
        .argv
        .windows(2)
        .filter(|pair| pair[0] == "--deny")
        .map(|pair| pair[1].as_str())
        .collect();
    assert!(BTreeSet::from(["Bash", "Edit", "Write"]).is_subset(&denied));
    for absent in [
        "run_terminal_command",
        "search_replace",
        "--always-approve",
        "--yolo",
    ] {
        assert!(!seen.argv.iter().any(|arg| arg == absent));
    }
    assert_eq!(after("--model"), "grok-4.7");
    assert_eq!(seen.env["TE_SIDECALL"], "1");
    assert!(seen.input.is_none());
    assert_ne!(seen.cwd, std::env::current_dir().unwrap());
    let config = serde_json::to_value(LiveConfig::default()).unwrap();
    assert_eq!(
        sidecall::runner::host_call("grok", &config, &environment(root.path()))
            .unwrap()
            .unwrap()
            .method,
        "grok_call"
    );
    assert_eq!(sidecall::call_name("grok"), "grok -p");
}

#[test]
fn a_grok_side_call_with_no_reply_says_so_in_the_error_log() {
    let root = TempDir::new("grok-no-reply");
    let mut runtime = SideCall {
        failed: true,
        ..Default::default()
    };
    assert_eq!(side_call(root.path(), "", &mut runtime), (json!(""), None));
    assert!(
        fs::read_to_string(root.path().join("errors.log"))
            .unwrap()
            .contains("sidecall grok: no reply")
    );
    assert!(!sidecall::paused("grok", root.path()).is_null());
}

#[test]
fn the_json_reply_reads_the_last_object_and_tolerates_noise() {
    let (text, usage) = grok::json_reply(
        "warning: x\n{\"text\": \"hi\", \"usage\": {\"input_tokens\": 3, \"output_tokens\": 1}}\n",
    )
    .unwrap();
    assert_eq!(text, "hi");
    let usage = usage.unwrap();
    assert_eq!(usage["input_tokens"].as_f64(), Some(3.0));
    assert_eq!(usage["cache_read_input_tokens"].as_f64(), Some(0.0));
    assert_eq!(grok::json_reply("not json").unwrap(), ("".into(), None));
}

#[test]
fn the_claude_hook_file_gives_session_start_an_explicit_timeout() {
    let root = TempDir::new("grok-package-hooks");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("reference/upstream");
    let binaries = Path::new(env!("CARGO_BIN_EXE_te")).parent().unwrap();
    let plugin = tokeneconomizer::package::build(
        &source,
        Some(binaries),
        &root.path().join("marketplace"),
        false,
    )
    .unwrap();
    let path = plugin.join("hooks/hooks.json");
    let doc: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    for groups in doc["hooks"].as_object().unwrap().values() {
        for group in groups.as_array().unwrap() {
            for hook in group["hooks"].as_array().unwrap() {
                assert!(hook["timeout"].as_f64().is_some_and(|n| n > 5.0));
                assert!(!hook["command"].as_str().unwrap().contains("--host"));
            }
        }
    }
}
