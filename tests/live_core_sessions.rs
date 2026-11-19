// Converted from reference/upstream/tests/test_live_core.py (Pi and opencode sessions).
pub mod fixtures;
mod support;

use fixtures::opencode_session_export;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};
use support::TempDir;
use tokeneconomizer::{
    microskills::observe::{Vocabulary, observe_task},
    trace::{calls, opencode, pi},
};

fn pi_session(path: &Path, cwd: &str, sid: &str, abandoned: bool) -> PathBuf {
    let usage = json!({"input":10,"output":50,"cacheRead":1000,"cacheWrite":100,"reasoning":5,"totalTokens":1160,"cost":{"total":0.002}});
    let mut lines = vec![
        json!({"type":"session","version":3,"id":sid,"timestamp":"2026-09-30T00:00:00Z","cwd":cwd}),
        json!({"type":"message","id":"e001","timestamp":"2026-09-30T00:00:01Z","parentId":null,"message":{"role":"user","content":"fix the bug"}}),
    ];
    for (i, (name, args, text, error)) in [
        (
            "bash",
            json!({"command":"pytest -q"}),
            "bash: pytest: command not found",
            true,
        ),
        (
            "edit",
            json!({"path":"pkg/a.py","oldText":"x","newText":"y"}),
            "ok",
            false,
        ),
        (
            "bash",
            json!({"command":"python -m pytest -q"}),
            "1 passed",
            false,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let entry = 2 + i * 2;
        let call = format!("call{}", 1 + i * 2);
        lines.push(json!({"type":"message","id":format!("e{entry:03}"),"timestamp":"2026-09-30T00:00:01Z","parentId":format!("e{:03}",entry-1),
            "message":{"role":"assistant","provider":"openai","model":"gpt-5.6-sol","usage":usage,
                "content":[{"type":"toolCall","id":call,"name":name,"arguments":args}]}}));
        lines.push(json!({"type":"message","id":format!("e{:03}",entry+1),"timestamp":"2026-09-30T00:00:01Z","parentId":format!("e{entry:03}"),
            "message":{"role":"toolResult","toolCallId":call,"toolName":name,"isError":error,"content":[{"type":"text","text":text}]}}));
    }
    if abandoned {
        lines.insert(3,json!({"type":"message","id":"zzz","parentId":"e001","timestamp":"2026-09-30T00:00:02Z",
            "message":{"role":"assistant","model":"gpt-5.6-sol","usage":usage,
                "content":[{"type":"toolCall","id":"dead","name":"bash","arguments":{"command":"rm -rf build"}}]}}));
    }
    fs::write(
        path,
        format!(
            "{}\n",
            lines
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        ),
    )
    .unwrap();
    path.into()
}

#[test]
fn a_pi_session_reads_through_the_trace_seam_on_its_active_branch() {
    let root = TempDir::new("core-pi-branch");
    let source = pi_session(&root.path().join("s.jsonl"), "/w/repo", "pi-1", true);
    let out = root.path().join("trace.jsonl");
    assert_eq!(pi::session_to_claude(&source, &out).unwrap(), Some(3));
    let trajectory: Vec<_> = calls::load_trajectory(&out)
        .into_iter()
        .map(|(name, args, _, ok)| {
            let target = if args["command"]
                .as_str()
                .is_some_and(|value| !value.is_empty())
            {
                args["command"].clone()
            } else {
                args["path"].clone()
            };
            (name, target, ok)
        })
        .collect();
    assert_eq!(
        trajectory,
        [
            (json!("bash"), json!("pytest -q"), json!(false)),
            (json!("edit"), json!("pkg/a.py"), json!(true)),
            (json!("bash"), json!("python -m pytest -q"), json!(true))
        ]
    );
    assert!(!fs::read_to_string(&out).unwrap().contains("rm -rf build"));
}

#[test]
fn a_pi_call_into_the_session_directory_is_the_plain_form() {
    let root = TempDir::new("core-pi-cwd");
    let source = pi_session(&root.path().join("s.jsonl"), "/w/repo", "pi-1", false);
    let mut lines: Vec<Value> = fs::read_to_string(&source)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    for entry in &mut lines {
        if let Some(content) = entry
            .get_mut("message")
            .and_then(|message| message.get_mut("content"))
            .and_then(Value::as_array_mut)
        {
            for block in content {
                if block["arguments"]["command"] == "pytest -q" {
                    block["arguments"]["command"] = json!("cd /w/repo && pytest -q");
                }
            }
        }
    }
    fs::write(
        &source,
        format!(
            "{}\n",
            lines
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        ),
    )
    .unwrap();
    let out = root.path().join("trace.jsonl");
    pi::session_to_claude(&source, &out).unwrap();
    assert_eq!(calls::call_cwds(&out), vec![Some("/w/repo".into()); 3]);
    let forms: Vec<_> = observe_task(&out, &Vocabulary::default())
        .unwrap()
        .obs
        .into_iter()
        .filter(|observation| observation.intent == "run_tests")
        .map(|observation| observation.form)
        .collect();
    assert_eq!(forms, ["pytest", "python -m pytest"]);
}

#[test]
fn pi_usage_maps_directly_and_keeps_pis_own_cost() {
    let root = TempDir::new("core-pi-usage");
    let source = pi_session(&root.path().join("s.jsonl"), "/w/repo", "pi-1", true);
    let usage = pi::session_usage(&source).unwrap();
    assert_eq!(usage["requests"], 3);
    assert_eq!(
        (
            usage["input_tokens"].as_f64(),
            usage["cache_read_input_tokens"].as_f64(),
            usage["cache_creation_input_tokens"].as_f64(),
            usage["output_tokens"].as_f64()
        ),
        (Some(30.0), Some(3000.0), Some(300.0), Some(150.0))
    );
    assert!((usage["cost_usd_vendor"].as_f64().unwrap() - 0.006).abs() <= 6e-9);
    assert_eq!(usage["model"], "gpt-5.6-sol");
    assert_eq!(pi::session_header(&source)["cwd"], "/w/repo");
}

fn opencode_export(path: &Path, cwd: &str) -> PathBuf {
    let tokens = json!({"input":10,"output":45,"reasoning":5,"cache":{"read":1000,"write":100}});
    let call = |id: &str, tool: &str, args: Value, mut state: Value| {
        state["input"] = args;
        json!({"info":{"id":format!("m-{id}"),"role":"assistant","modelID":"gpt-5.6-sol","path":{"cwd":cwd,"root":cwd},"cost":0.002,"tokens":tokens},
            "parts":[{"type":"tool","callID":id,"tool":tool,"state":state}]})
    };
    let messages = vec![
        json!({"info":{"id":"u1","role":"user"},"parts":[{"type":"text","text":"fix the bug"},{"type":"text","text":"(added by opencode)","synthetic":true}]}),
        call(
            "c1",
            "bash",
            json!({"command":"pytest -q"}),
            json!({"status":"error","error":"bash: pytest: command not found"}),
        ),
        call(
            "c2",
            "edit",
            json!({"filePath":"pkg/a.py","oldString":"x","newString":"y"}),
            json!({"status":"completed","output":"ok","metadata":{}}),
        ),
        call(
            "c3",
            "bash",
            json!({"command":"python -m pytest -q"}),
            json!({"status":"completed","output":"1 passed","metadata":{"exit":0}}),
        ),
    ];
    fs::write(
        path,
        json!({"info":{"id":"oc-1"},"messages":messages}).to_string(),
    )
    .unwrap();
    path.into()
}

#[test]
fn an_opencode_export_reads_through_the_trace_seam() {
    let root = TempDir::new("core-opencode-export");
    let source = opencode_export(&root.path().join("s.json"), "/w/repo");
    let out = root.path().join("t.jsonl");
    assert_eq!(opencode::session_to_claude(&source, &out).unwrap(), Some(3));
    assert_eq!(opencode::prompt_count(&source), 1);
    let text = fs::read_to_string(&out).unwrap();
    assert!(text.contains("command not found") && text.contains("python -m pytest -q"));
    assert!(
        text.contains("\"is_error\": true")
            && text.contains("fix the bug")
            && !text.contains("added by opencode")
    );
}

#[test]
fn an_opencode_list_export_and_a_missing_file() {
    let root = TempDir::new("core-opencode-list");
    let source = opencode_export(&root.path().join("s.json"), "/w/repo");
    let listed = root.path().join("l.json");
    let source: Value = serde_json::from_slice(&fs::read(&source).unwrap()).unwrap();
    fs::write(&listed, source["messages"].to_string()).unwrap();
    assert_eq!(
        opencode::session_to_claude(&listed, &root.path().join("t.jsonl")).unwrap(),
        Some(3)
    );
    assert_eq!(
        opencode::session_to_claude(&root.path().join("none.json"), &root.path().join("x"))
            .unwrap(),
        None
    );
}

#[test]
fn opencode_usage_counts_reasoning_as_output() {
    let root = TempDir::new("core-opencode-usage");
    let usage =
        opencode::session_usage(&opencode_export(&root.path().join("s.json"), "/w/repo")).unwrap();
    assert_eq!(
        (
            usage["input_tokens"].as_f64(),
            usage["output_tokens"].as_f64()
        ),
        (Some(30.0), Some(150.0))
    );
    assert_eq!(
        (
            usage["cache_read_input_tokens"].as_f64(),
            usage["cache_creation_input_tokens"].as_f64()
        ),
        (Some(3000.0), Some(300.0))
    );
    assert_eq!(usage["requests"], 3);
    assert_eq!(usage["model"], "gpt-5.6-sol");
    assert_eq!(usage["cost_usd_vendor"].as_f64(), Some(0.006));
}

#[test]
fn a_flat_opencode_2_export_reads_as_the_nested_one() {
    let root = TempDir::new("core-opencode-flat");
    let source = root.path().join("s.json");
    fs::write(&source, opencode_session_export("ses_f").to_string()).unwrap();
    let out = root.path().join("t.jsonl");
    assert_eq!(opencode::session_to_claude(&source, &out).unwrap(), Some(2));
    assert_eq!(opencode::prompt_count(&source), 1);
    let text = fs::read_to_string(&out).unwrap();
    assert!(
        text.contains("Run the tests")
            && text.contains("python -m pytest -q")
            && text.contains("1 passed")
    );
    assert!(text.contains("command not found") && text.contains("\"is_error\": true"));
    assert!(!text.contains("\"cwd\""));
    let mut data: Value = serde_json::from_slice(&fs::read(&source).unwrap()).unwrap();
    data["info"]["directory"] = json!("/w/repo");
    fs::write(&source, data.to_string()).unwrap();
    opencode::session_to_claude(&source, &out).unwrap();
    assert!(
        fs::read_to_string(&out)
            .unwrap()
            .contains("\"cwd\": \"/w/repo\"")
    );
    let usage = opencode::session_usage(&source).unwrap();
    assert_eq!(
        (
            usage["input_tokens"].as_f64(),
            usage["output_tokens"].as_f64(),
            usage["cache_read_input_tokens"].as_f64()
        ),
        (Some(18348.0), Some(70.0), Some(896.0))
    );
    assert_eq!(usage["model"], "gpt-5.6-sol");
    assert_eq!(usage["cost_usd_vendor"].as_f64(), Some(0.0056));
}
