// Converted from reference/upstream/tests/test_live_core.py (host payloads and replies).
mod support;

use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use support::TempDir;
use tokeneconomizer::{
    plugin::hosts::{
        self,
        session::{self, Environment, NativeRuntime},
    },
    trace::opencode,
};

fn headless(host: &str, env: Value) -> bool {
    let environment = Environment {
        env: Map::new(),
        home: PathBuf::new(),
        cwd: PathBuf::new(),
    };
    session::headless(
        host,
        &env,
        None,
        &environment,
        0,
        false,
        &mut NativeRuntime {
            environment: &environment,
        },
    )
    .unwrap()
}

#[test]
fn host_detection() {
    let registered = hosts::HOSTS.map(str::to_owned);
    assert_eq!(
        hosts::detect(&json!({"turn_id":"t"}), &json!({}), None, &registered),
        "codex"
    );
    assert_eq!(
        hosts::detect(
            &json!({"transcript_path":"/h/.codex/sessions/2026/rollout-a.jsonl"}),
            &json!({}),
            None,
            &registered
        ),
        "codex"
    );
    assert_eq!(
        hosts::detect(
            &json!({"transcript_path":"/h/.claude/projects/x/s.jsonl"}),
            &json!({}),
            None,
            &registered
        ),
        "claude"
    );
    assert_eq!(
        hosts::detect(
            &json!({"turn_id":"t"}),
            &json!({}),
            Some("claude"),
            &registered
        ),
        "claude"
    );
    assert_eq!(
        hosts::detect(
            &json!({"transcript_path":"/h/.copilot/session-state/s/events.jsonl"}),
            &json!({}),
            None,
            &registered
        ),
        "copilot"
    );
    let env = json!({"COPILOT_PLUGIN_ROOT":"/p","COPILOT_CLI":"1"});
    assert_eq!(
        hosts::detect(&json!({"session_id":"s"}), &env, None, &registered),
        "copilot"
    );
    assert_eq!(
        hosts::detect(
            &json!({"session_id":"s","transcript_path":"/h/.claude/projects/x/s.jsonl"}),
            &env,
            None,
            &registered
        ),
        "claude"
    );
    assert_eq!(
        hosts::detect(&json!({"session_id":"s"}), &json!({}), None, &registered),
        "claude"
    );
}

#[test]
fn the_te_command_defaults_to_the_agent_whose_shell_runs_it() {
    let registered = hosts::HOSTS.map(str::to_owned);
    assert_eq!(hosts::default_host(&json!({}), None, &registered), "claude");
    assert_eq!(
        hosts::default_host(&json!({"COPILOT_CLI":"1"}), None, &registered),
        "copilot"
    );
    assert_eq!(
        hosts::default_host(
            &json!({"COPILOT_CLI":"1","CLAUDECODE":"1"}),
            None,
            &registered
        ),
        "claude"
    );
    assert_eq!(
        hosts::default_host(
            &json!({"TE_HOST":"codex","CLAUDECODE":"1"}),
            None,
            &registered
        ),
        "codex"
    );
    assert_eq!(
        hosts::default_host(&json!({"TE_HOST":"nonsense"}), None, &registered),
        "claude"
    );
    assert_eq!(
        hosts::default_host(&json!({}), Some("pi"), &registered),
        "pi"
    );
    assert_eq!(
        hosts::default_host(&json!({"CLAUDECODE":"1"}), Some("pi"), &registered),
        "claude"
    );
}

#[test]
fn the_copilot_payloads_observed_live_are_read() {
    let post = json!({"hook_event_name":"PostToolUse","session_id":"fcb5fc6b","timestamp":"2026-10-08T04:27:38.418Z","cwd":"/w/repo","tool_name":"Bash",
        "tool_input":{"command":"python3 -c 'import sys; sys.exit(3)'","description":"run it"},
        "tool_result":{"result_type":"success","text_result_for_llm":"boom\n<shellId: 0 completed with exit code 3>"}});
    let failure = json!({"hook_event_name":"PostToolUseFailure","session_id":"10bbc978","timestamp":"2026-10-08T04:27:51.286Z","cwd":"/w/repo",
        "tool_name":"Read","tool_input":{"path":"/w/repo/nope.py"},"error":"Path does not exist"});
    let host = hosts::detect(
        &post,
        &json!({"COPILOT_PLUGIN_ROOT":"/p"}),
        None,
        &hosts::HOSTS.map(str::to_owned),
    );
    assert_eq!(host, "copilot");
    let (name, args, text, code) = hosts::tool_call(&host, &post).unwrap();
    assert_eq!(
        (name, args["description"].clone(), code),
        (json!("Bash"), json!("run it"), json!(3))
    );
    assert!(text.starts_with("boom"));
    let (name, args, text, code) = hosts::tool_call(&host, &failure).unwrap();
    assert_eq!(
        (name, args["path"].clone(), code),
        (json!("Read"), json!("/w/repo/nope.py"), Value::Null)
    );
    assert!(text.contains("Path does not exist"));
    assert_eq!(
        hosts::context(&host, "SessionStart", "x"),
        json!({"additionalContext":"x"})
    );
    let merged = hosts::merge_outputs(&[
        hosts::context(&host, "SessionStart", "a"),
        hosts::notice(&host, "n"),
        hosts::context(&host, "SessionStart", "b"),
    ])
    .unwrap();
    assert_eq!(
        merged,
        json!({"additionalContext":"a\n\nb","_progress":["n"]})
    );
}

#[test]
fn a_copilot_session_log_is_found_from_the_session_id() {
    let root = TempDir::new("core-copilot-log");
    let environment = Environment {
        env: json!({"COPILOT_HOME":root.path().join("cp")})
            .as_object()
            .unwrap()
            .clone(),
        home: root.path().into(),
        cwd: root.path().into(),
    };
    assert_eq!(
        session::transcript("copilot", &json!({"session_id":"abc"}), &environment).unwrap(),
        json!(root.path().join("cp/session-state/abc/events.jsonl"))
    );
    assert_eq!(
        session::transcript(
            "copilot",
            &json!({"session_id":"abc","transcript_path":"/x/events.jsonl"}),
            &environment
        )
        .unwrap(),
        "/x/events.jsonl"
    );
    assert_eq!(
        session::cwd(
            "copilot",
            &json!({}),
            &json!({"COPILOT_PROJECT_DIR":"/w/repo"}),
            Path::new("")
        ),
        "/w/repo"
    );
}

#[test]
fn claude_tool_results_and_outputs() {
    let (name, args, text, code) = hosts::tool_call(
        "claude",
        &json!({"tool_name":"Bash","tool_input":{"command":"pytest"},
        "tool_response":{"stdout":"1 failed","stderr":"Traceback","interrupted":false}}),
    )
    .unwrap();
    assert_eq!(
        (name, args["command"].clone(), code),
        (json!("Bash"), json!("pytest"), json!(0))
    );
    assert!(text.contains("1 failed") && text.contains("Traceback"));
    assert_eq!(
        hosts::context("claude", "PostToolUse", "x"),
        json!({"hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":"x"}})
    );
    assert!(headless(
        "claude",
        json!({"CLAUDE_CODE_SESSION_ATTENDED":"0","CLAUDE_CODE_ENTRYPOINT":"sdk-cli"})
    ));
    assert!(!headless(
        "claude",
        json!({"CLAUDE_CODE_SESSION_ATTENDED":"1","CLAUDE_CODE_ENTRYPOINT":"sdk-cli"})
    ));
    assert!(headless(
        "claude",
        json!({"CLAUDE_CODE_ENTRYPOINT":"sdk-cli"})
    ));
    assert!(!headless("claude", json!({})));
}

#[test]
fn the_codex_posttooluse_payload_observed_live_is_read() {
    let payload = json!({"session_id":"01a0f5e1","turn_id":"t1","model":"gpt-5.6-sol","permission_mode":"default","hook_event_name":"PostToolUse",
        "cwd":"/w/repo","transcript_path":"/h/.codex/sessions/2026/09/30/rollout-x.jsonl","tool_name":"Bash","tool_use_id":"call_1",
        "tool_input":{"command":"echo one"},"tool_response":"one\n"});
    let host = hosts::detect(&payload, &json!({}), None, &hosts::HOSTS.map(str::to_owned));
    assert_eq!(host, "codex");
    assert_eq!(
        hosts::tool_call(&host, &payload).unwrap(),
        (
            json!("bash"),
            json!({"command":"echo one"}),
            "one\n".into(),
            Value::Null
        )
    );
}

#[test]
fn codex_tool_calls_normalize() {
    let (name, args, _, code) = hosts::tool_call(
        "codex",
        &json!({"tool_name":"exec_command","tool_input":{"command":["bash","-lc","go test ./..."]},
        "tool_response":{"output":"FAIL","exit_code":1}}),
    )
    .unwrap();
    assert_eq!(
        (name, args["command"].clone(), code),
        (json!("bash"), json!("bash -lc 'go test ./...'"), json!(1))
    );
    let (name,args,_,_)=hosts::tool_call("codex",&json!({"tool_name":"apply_patch","tool_input":{"input":"*** Begin Patch\n*** Update File: src/a.py\n@@\n-x\n+y\n*** End Patch"}})).unwrap();
    assert_eq!(name, "apply_patch");
    assert_eq!(args["paths"], json!(["src/a.py"]));
    let (_, _, _, code) = hosts::tool_call(
        "codex",
        &json!({"tool_name":"shell","tool_input":{"command":"ls"},
        "tool_response":"Exit code: 2\nWall time: 0.1s\nOutput:\n"}),
    )
    .unwrap();
    assert_eq!(code, 2);
}

#[test]
fn merge_outputs_concatenates_contexts_and_notices() {
    let merged = hosts::merge_outputs(&[
        hosts::context("claude", "SessionStart", "a"),
        hosts::notice("claude", "n1"),
        hosts::context("claude", "SessionStart", "b"),
        hosts::notice("claude", "n2"),
        Value::Null,
        json!({}),
    ])
    .unwrap();
    assert_eq!(merged["hookSpecificOutput"]["additionalContext"], "a\n\nb");
    assert_eq!(merged["systemMessage"], "n1\nn2");
}

#[test]
fn the_pi_host_reads_what_the_extension_sends() {
    let (name,args,text,code)=hosts::tool_call("pi",&json!({"tool_name":"bash","tool_input":{"command":"go test"},
        "tool_response":{"content":"FAIL\nCommand exited with code 1","isError":true,"exit_code":1}})).unwrap();
    assert_eq!(
        (name, args["command"].clone(), code),
        (json!("bash"), json!("go test"), json!(1))
    );
    assert!(text.contains("FAIL"));
    assert!(headless("pi", json!({"TE_PI_MODE":"print"})));
    assert!(!headless("pi", json!({"TE_PI_MODE":"tui"})));
    assert_eq!(
        hosts::detect(
            &json!({"turn_id":"t"}),
            &json!({}),
            Some("pi"),
            &hosts::HOSTS.map(str::to_owned)
        ),
        "pi"
    );
}

#[test]
fn the_opencode_host_reads_what_the_plugin_sends() {
    let (name, args, text, code) = hosts::tool_call(
        "opencode",
        &json!({"tool_name":"bash","tool_input":{"command":"go test"},
        "tool_response":{"content":"FAIL","isError":true,"exit_code":1}}),
    )
    .unwrap();
    assert_eq!(
        (name, args["command"].clone(), code),
        (json!("bash"), json!("go test"), json!(1))
    );
    assert!(text.contains("FAIL"));
    assert!(headless("opencode", json!({"TE_OPENCODE_MODE":"run"})));
    assert!(!headless("opencode", json!({"TE_OPENCODE_MODE":"tui"})));
    assert_eq!(
        hosts::turn_key("opencode", &json!({"prompt_id":"s:1"})),
        "s:1"
    );
    assert_eq!(
        hosts::detect(
            &json!({}),
            &json!({}),
            Some("opencode"),
            &hosts::HOSTS.map(str::to_owned)
        ),
        "opencode"
    );
}

#[test]
fn an_opencode_run_reply_is_read_from_its_json_events() {
    let events = [
        json!({"type":"step_start","sessionID":"ses_x","part":{"type":"step-start"}}),
        json!({"type":"text","sessionID":"ses_x","part":{"type":"text","text":"po"}}),
        json!({"type":"text","sessionID":"ses_x","part":{"type":"text","text":"ng"}}),
        json!({"type":"step_finish","sessionID":"ses_x","part":{"type":"step-finish","cost":0.5,
            "tokens":{"input":10,"output":4,"reasoning":1,"cache":{"read":7,"write":2}}}}),
    ];
    let output = format!(
        "{}\nnot json\n",
        events
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
    let (text, usage, sid) = opencode::run_reply(&output).unwrap();
    assert_eq!((text.as_str(), sid.as_str()), ("pong", "ses_x"));
    let expected = json!({"input_tokens":10,"cache_read_input_tokens":7,"cache_creation_input_tokens":2,"output_tokens":5,"cost_usd":0.5});
    let usage = usage.unwrap();
    assert_eq!(
        usage.as_object().unwrap().len(),
        expected.as_object().unwrap().len()
    );
    for (key, value) in expected.as_object().unwrap() {
        assert_eq!(usage[key].as_f64(), value.as_f64(), "{key}");
    }
    assert_eq!(
        opencode::run_reply("").unwrap(),
        ("".into(), None, "".into())
    );
}
