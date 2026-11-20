// Converted from reference/upstream/tests/test_live_hooks.py.
pub mod fixtures;
#[path = "support/live_hook_fixture.rs"]
pub mod live_fixture;
mod support;

use live_fixture::{copilot_session, fixture, session, stuck, transcript, turn, warm};
use serde_json::{Value, json};
use std::fs;
use support::TempDir;
use tokeneconomizer::plugin::{cli, context::LiveContext, history, hosts, sidecall};

#[test]
fn claude_code_failure_payloads_parse_with_their_exit_code() {
    let mut payload = json!({
        "hook_event_name":"PostToolUseFailure", "tool_name":"Bash",
        "tool_input":{"command":"ls /nonexistent_dir_xyz"},
        "tool_use_id":"toolu_01", "is_interrupt":false, "duration_ms":25,
        "error":"Exit code 2\nls: cannot access '/nonexistent_dir_xyz': No such file or directory"
    });
    let (name, args, text, code) = hosts::tool_call("claude", &payload).unwrap();
    assert_eq!(name, "Bash");
    assert_eq!(args["command"], "ls /nonexistent_dir_xyz");
    assert_eq!(code, 2);
    assert!(text.contains("cannot access"));
    payload["tool_name"] = json!("Read");
    payload["error"] = json!("File does not exist.");
    assert!(hosts::tool_call("claude", &payload).unwrap().3.is_null());
}

#[test]
fn the_claude_hook_file_registers_failed_calls() {
    let manifest: Value = serde_json::from_str(include_str!(
        "../reference/upstream/plugin/hooks/hooks.json"
    ))
    .unwrap();
    let hooks = &manifest["hooks"];
    assert!(!hooks["PostToolUse"].as_array().unwrap().is_empty());
    assert_eq!(hooks["PostToolUseFailure"], hooks["PostToolUse"]);
}

#[test]
fn failed_calls_count_and_a_failing_check_after_the_last_edit_ends_the_turn_red() {
    let tmp = TempDir::new("hook-failed-call-count");
    let mut f = fixture(tmp.path(), "claude");
    turn(
        &mut f,
        "s",
        "p0",
        &[
            json!(["Edit", {"file_path":"/r/pkg/a.py","old_string":"x","new_string":"y"}, "ok"]),
            json!(["Bash", {"command":"python tests/runtests.py app"}, {"error":"Exit code 1\nFAILED (failures=1)"}]),
            json!(["Read", {"file_path":"/r/b.py"}, {"error":"File does not exist."}]),
        ],
    );
    let events = f.store.read_lines("events.jsonl", None).unwrap();
    let turns: Vec<_> = events
        .iter()
        .filter(|event| event["kind"] == "turn")
        .collect();
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0]["tool_calls"], 3);
    assert_eq!(turns[0]["resolved_proxy"], false);
}

#[test]
fn a_loop_the_agent_reports_only_as_failures_is_nudged() {
    let tmp = TempDir::new("hook-failures-loop");
    let mut f = fixture(tmp.path(), "claude");
    warm(&mut f);
    let outputs = turn(&mut f, "s", "stuck", &stuck("claude", 40));
    assert!(
        outputs
            .iter()
            .any(|out| out.get("hookSpecificOutput").is_some())
    );
}

#[test]
fn an_answer_to_a_failed_call_names_the_failure_event() {
    let tmp = TempDir::new("hook-failure-event");
    let mut f = fixture(tmp.path(), "claude");
    warm(&mut f);
    let calls = stuck("claude", 40);
    let outputs = turn(&mut f, "s", "stuck", &calls);
    let mut answered_failure = false;
    for (call, output) in calls.iter().zip(outputs) {
        if let Some(answer) = output.get("hookSpecificOutput") {
            let failed = call[2].get("error").is_some();
            let event = if failed {
                "PostToolUseFailure"
            } else {
                "PostToolUse"
            };
            assert_eq!(answer["hookEventName"], event);
            answered_failure |= failed;
        }
    }
    assert!(answered_failure);
}

#[test]
fn pi_learns_from_its_own_session_files_and_injects_at_the_next_start() {
    let tmp = TempDir::new("hook-pi-learn");
    let mut f = fixture(tmp.path(), "pi");
    let mut starts = Vec::new();
    for i in 0..5 {
        let sid = format!("pi{i}");
        let file = f.repo.join(format!("pkg/mod{i}.py"));
        let mut messages = vec![json!({"role":"user","content":format!("fix bug {i}")})];
        let usage = json!({"input":10,"output":50,"cacheRead":1000,"cacheWrite":100,"totalTokens":1160,"cost":{"total":0.001}});
        let calls = [
            (
                "edit",
                json!({"path":file,"oldText":"a","newText":"b"}),
                "ok",
                false,
            ),
            (
                "bash",
                json!({"command":"pytest -q tests/test_a.py"}),
                "bash: pytest: command not found",
                true,
            ),
            (
                "bash",
                json!({"command":"python -m pytest -q tests/test_a.py"}),
                "1 passed in 0.1s",
                false,
            ),
        ];
        for (n, (name, args, text, failed)) in calls.into_iter().enumerate() {
            let id = format!("c{name}{n}");
            messages.push(
                json!({"role":"assistant","model":"gpt-5.6-sol","usage":usage,
                    "content":[{"type":"toolCall","id":id,"name":name,"arguments":args}]
                }),
            );
            messages.push(
                json!({"role":"toolResult","toolCallId":id,"toolName":name,"isError":failed,
                "content":[{"type":"text","text":text}]}),
            );
        }
        let mut lines = vec![json!({
            "type":"session","version":3,"id":sid,"timestamp":"2026-09-30T00:00:00Z","cwd":f.repo
        }).to_string()];
        let mut parent = Value::Null;
        for (k, message) in messages.into_iter().enumerate() {
            let id = json!(format!("{i}-{k}"));
            lines.push(json!({
                "type":"message","id":id,"parentId":parent,"timestamp":"2026-09-30T00:00:01Z","message":message
            }).to_string());
            parent = id;
        }
        let path = transcript(tmp.path(), &format!("p{i}"));
        fs::write(&path, lines.join("\n") + "\n").unwrap();
        starts.push(session(&mut f, &sid, &path, "startup"));
    }
    assert!(starts[..4].iter().all(|start| *start == json!({})));
    let injected = &starts[4]["hookSpecificOutput"]["additionalContext"];
    assert!(
        injected
            .as_str()
            .unwrap()
            .contains("`python -m pytest …` works.")
    );
    assert!(
        starts[4]["systemMessage"]
            .as_str()
            .unwrap()
            .contains("/te microskills")
    );
    let ledger = f.store.read_lines("ledger.jsonl", None).unwrap();
    assert_eq!(ledger[0]["requests"], 3);
    assert_eq!(ledger[0]["priced_by"], "model");
}

#[test]
fn pi_print_mode_counts_as_headless_for_the_stop_hint() {
    let tmp = TempDir::new("hook-pi-print");
    let mut f = fixture(tmp.path(), "pi");
    f.environment
        .env
        .insert("TE_PI_MODE".into(), json!("print"));
    let reads = vec![json!(["read",{"path":"/r/a.py"},{"content":"ok"}]); 8];
    for k in 0..5 {
        turn(&mut f, "p", &format!("g{k}"), &reads);
    }
    let outputs = turn(&mut f, "p", "stuck", &stuck("pi", 40));
    let notice = outputs
        .iter()
        .filter_map(|out| out["systemMessage"].as_str())
        .find(|text| text.contains("still looks stuck"))
        .unwrap();
    assert!(notice.ends_with(hosts::profile("pi").end_run));
}

#[test]
fn only_a_signed_out_answer_pauses_the_model_calls() {
    let tmp = TempDir::new("hook-pause-auth");
    let root = tmp.path();
    sidecall::no_reply(
        "pi",
        root,
        "",
        "429 rate limited",
        &json!(1),
        history::now(),
    );
    assert!(sidecall::paused("pi", root).is_null());
    sidecall::no_reply(
        "claude",
        root,
        "Invalid API key · Please run /login",
        "",
        &json!(1),
        history::now(),
    );
    assert!(sidecall::paused("claude", root).is_object());
    assert!(sidecall::paused("pi", root).is_null());
    assert!(sidecall::resume("claude", root));
    assert!(sidecall::paused("claude", root).is_null());
}

#[test]
fn a_failed_copilot_tool_reaches_the_guard() {
    let tmp = TempDir::new("hook-copilot-failure");
    let mut f = fixture(tmp.path(), "copilot");
    let args = json!({"path":f.repo.join("nope.py")});
    copilot_session(
        &mut f,
        "f",
        &[json!(["view", args, "Path does not exist", false])],
        1,
    );
    let events = f.store.read_lines("events.jsonl", None).unwrap();
    let turns: Vec<_> = events
        .iter()
        .filter(|event| event["kind"] == "turn")
        .collect();
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0]["tool_calls"], 1);
}

#[test]
fn measure_reports_a_failed_probe_in_one_line() {
    struct BrokenProbe {
        ctx: LiveContext,
        lines: Vec<String>,
    }
    impl cli::Runtime for BrokenProbe {
        fn context(&mut self, _: &Value) -> cli::Result<LiveContext> {
            Ok(self.ctx.clone())
        }
        fn pinned_context(&mut self, _: &Value, _: &Value) -> cli::Result<LiveContext> {
            panic!("Unexpected pinned context")
        }
        fn global(&mut self, method: &str, _: Value) -> cli::Result<Value> {
            panic!("Unexpected global operation: {method}")
        }
        fn call(&mut self, _: &LiveContext, method: &str, args: Value) -> cli::Result<Value> {
            assert_eq!(method, "measure");
            assert_eq!(args, json!([["Workflow"]]));
            Err(cli::Error::new(
                "RuntimeError",
                "`claude -p` exited 1: boom",
            ))
        }
        fn say(&mut self, text: &str) -> cli::Result<()> {
            self.lines.push(text.into());
            Ok(())
        }
    }
    let tmp = TempDir::new("hook-probe-measure");
    let f = fixture(tmp.path(), "claude");
    let ctx = LiveContext::build(
        "claude",
        true,
        &f.repo,
        f.environment.env,
        &f.environment.home,
        Value::Null,
        false,
    )
    .unwrap();
    let mut probe = BrokenProbe { ctx, lines: vec![] };
    let parsed = cli::parser::parse(
        &["toolprune".into(), "measure".into(), "Workflow".into()],
        &f.repo,
        80,
    );
    assert!(parsed.exit.is_none());
    assert_eq!(cli::toolprune(&parsed.args, &mut probe).unwrap(), 1);
    assert_eq!(probe.lines, ["not measured: `claude -p` exited 1: boom"]);
}
