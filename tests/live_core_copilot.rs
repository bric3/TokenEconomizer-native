// Converted from reference/upstream/tests/test_live_core.py (Copilot traces and notices).
pub mod fixtures;
mod support;

use fixtures::write_copilot_events;
use serde_json::{Map, Value, json};
use std::{fs, io, path::Path};
use support::TempDir;
use tokeneconomizer::{
    microskills::observe::{Vocabulary, observe_task},
    plugin::{context::LiveContext, hook, hosts, hosts::session::Environment, process, sidecall},
    trace::{calls, copilot},
};

#[test]
fn copilot_logs_read_through_the_existing_trace_seam() {
    let root = TempDir::new("core-copilot-seam");
    let path = write_copilot_events(
        &root.path().join("s/events.jsonl"),
        &[
            json!(["bash",{"command":"pytest -q"},"1 failed\n<shellId: 0 completed with exit code 1>",true]),
            json!(["view",{"path":"/w/repo/a.py"},"x = 1",true]),
        ],
        true,
    );
    let trajectory = calls::load_trajectory(&path);
    assert_eq!(
        trajectory
            .iter()
            .map(|call| call.0.clone())
            .collect::<Vec<_>>(),
        [json!("bash"), json!("view")]
    );
    assert_eq!(trajectory[0].1["command"], "pytest -q");
    assert_eq!(
        calls::tool_usage(&path),
        [("bash".into(), 1), ("view".into(), 1)].into()
    );
    let turns = copilot::turns(&path);
    assert_eq!(turns.len(), 1);
    let turn = &turns[0];
    assert_eq!(turn[0].0, "bash");
    assert_eq!(turn[0].3, Some(1));
    assert_eq!(turn[1].3, None);
    assert_eq!(copilot::session_meta(&path)["cwd"], "/w/repo");
}

#[test]
fn a_copilot_call_into_the_session_directory_is_the_plain_form() {
    let root = TempDir::new("core-copilot-cwd");
    let path = write_copilot_events(
        &root.path().join("s/events.jsonl"),
        &[
            json!(["bash",{"command":"cd /w/repo && pytest -q"},"bash: pytest: command not found\n<shellId: 0 completed with exit code 127>",false]),
            json!(["bash",{"command":"cd /w/repo && python -m unittest -q"},"Ran 4 tests in 0.001s\nOK\n<shellId: 0 completed with exit code 0>",true]),
        ],
        true,
    );
    assert_eq!(calls::call_cwds(&path), vec![Some("/w/repo".into()); 2]);
    let forms: Vec<_> = observe_task(&path, &Vocabulary::default())
        .unwrap()
        .obs
        .into_iter()
        .filter(|observation| observation.intent == "run_tests")
        .map(|observation| observation.form)
        .collect();
    assert_eq!(forms, ["pytest", "python -m unittest"]);
}

#[test]
fn copilot_usage_is_converted_from_its_grand_input_total() {
    let root = TempDir::new("core-copilot-usage");
    let path = write_copilot_events(&root.path().join("s/events.jsonl"), &[], true);
    let usage = copilot::session_usage(&path).unwrap();
    assert_eq!(usage["input_tokens"].as_f64(), Some(1500.0));
    assert_eq!(usage["cache_read_input_tokens"].as_f64(), Some(2400.0));
    assert_eq!(usage["cache_creation_input_tokens"].as_f64(), Some(100.0));
    assert_eq!(usage["output_tokens"].as_f64(), Some(80.0));
    assert_eq!(usage["requests"].as_f64(), Some(4.0));
    assert_eq!(usage["model"], "gpt-5.4");
    assert_eq!(usage["tool_definitions_tokens"].as_f64(), Some(13149.0));
    let killed = copilot::session_usage(&write_copilot_events(
        &root.path().join("k/events.jsonl"),
        &[],
        false,
    ))
    .unwrap();
    assert!(killed["input_tokens"].is_null());
    assert_eq!(killed["model"], "gpt-5.4");
}

#[test]
fn copilot_tools_are_grouped_by_mcp_server() {
    let servers = ["github", "github-mcp-server", "acme-db"].map(str::to_owned);
    assert_eq!(
        copilot::mcp_server_of("github-mcp-server-list_issues", &servers),
        Some("github-mcp-server")
    );
    assert_eq!(
        copilot::mcp_server_of("acme-db-query", &servers),
        Some("acme-db")
    );
    assert_eq!(copilot::mcp_server_of("bash", &servers), None);
    let root = TempDir::new("core-copilot-mcp");
    let home = root.path().join("cp");
    fs::create_dir(&home).unwrap();
    fs::write(
        home.join("mcp-config.json"),
        json!({"mcpServers":{"acme-db":{}}}).to_string(),
    )
    .unwrap();
    assert_eq!(
        copilot::mcp_servers(&json!({"COPILOT_HOME":home}), root.path()),
        ["github-mcp-server".into(), "acme-db".into()].into()
    );
}

struct HookFixture(Value);
impl hook::Runtime for HookFixture {
    fn handle(&mut self, _context: &mut LiveContext, _payload: Value) -> Result<Value, String> {
        panic!("The test supplies the hook result directly");
    }
    fn run(
        &mut self,
        _payload: Value,
        _environment: &Environment,
        _hint: Option<&str>,
    ) -> Result<Value, String> {
        Ok(self.0.clone())
    }
}

#[test]
fn a_copilot_notice_is_a_progress_line_before_the_hook_output() {
    let root = TempDir::new("core-copilot-progress");
    let mut environment = Environment {
        env: json!({"TE_DISABLE":"1","TOKENECONOMIZER_HOME":root.path()})
            .as_object()
            .unwrap()
            .clone(),
        home: root.path().into(),
        cwd: root.path().into(),
    };
    assert_eq!(
        hook::main(
            &[],
            Ok(b"{}".to_vec()),
            &environment,
            1,
            &mut HookFixture(Value::Null)
        )
        .unwrap(),
        "{}\n"
    );
    environment.env.remove("TE_DISABLE");
    let mut output = hosts::notice("copilot", "hello");
    output["additionalContext"] = json!("ctx");
    let output = hook::main(
        &[],
        Ok(br#"{"session_id":"s"}"#.to_vec()),
        &environment,
        1,
        &mut HookFixture(output),
    )
    .unwrap();
    let lines: Vec<_> = output.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(
        serde_json::from_str::<Value>(lines[0]).unwrap(),
        json!({"type":"progress","message":"hello"})
    );
    assert_eq!(
        serde_json::from_str::<Value>(lines[1]).unwrap(),
        json!({"additionalContext":"ctx"})
    );
}

#[test]
fn copilot_side_calls_name_a_model_a_byok_provider_can_serve() {
    let byok = json!({"COPILOT_PROVIDER_BASE_URL":"https://api.anthropic.com","COPILOT_PROVIDER_TYPE":"anthropic"});
    let mut env = byok.as_object().unwrap().clone();
    env.insert("COPILOT_MODEL".into(), json!("auto"));
    assert_eq!(sidecall::copilot_model("", &env), "claude-haiku-4-5");
    env.insert("COPILOT_MODEL".into(), json!("claude-sonnet-5"));
    assert_eq!(sidecall::copilot_model("", &env), "claude-sonnet-5");
    assert_eq!(
        sidecall::copilot_model("claude-opus-5", byok.as_object().unwrap()),
        "claude-opus-5"
    );
    assert_eq!(
        sidecall::copilot_model("", json!({"COPILOT_MODEL":"auto"}).as_object().unwrap()),
        ""
    );
    assert_eq!(
        sidecall::copilot_model(
            "",
            json!({"COPILOT_PROVIDER_BASE_URL":"http://localhost:11434/v1"})
                .as_object()
                .unwrap()
        ),
        ""
    );
    for wire in ["anthropic", "openai"] {
        assert_eq!(sidecall::copilot_model("",json!({"COPILOT_PROVIDER_BASE_URL":"https://openrouter.ai/api","COPILOT_PROVIDER_TYPE":wire}).as_object().unwrap()),"anthropic/claude-haiku-4.5","{wire}");
    }
}

struct HelpFixture;
impl sidecall::Runtime for HelpFixture {
    fn run(
        &mut self,
        argv: &[String],
        _cwd: &Path,
        _env: &Map<String, Value>,
        _input: Option<&str>,
        _timeout: f64,
    ) -> io::Result<process::Output> {
        let help = match argv[0].as_str() {
            "new" => "  --no-session-persistence   Disable session persistence",
            "old" => "  -p",
            _ => panic!("Unexpected CLI: {argv:?}"),
        };
        Ok(process::Output {
            code: 0,
            stdout: help.into(),
            stderr: String::new(),
        })
    }
}

#[test]
fn claude_side_calls_stay_out_of_the_persons_history_where_the_cli_allows() {
    let root = TempDir::new("core-claude-persistence");
    let environment = Environment {
        env: Map::new(),
        home: root.path().into(),
        cwd: root.path().into(),
    };
    let mut cache = Map::new();
    let mut runtime = HelpFixture;
    assert_eq!(
        sidecall::claude_flags(&sidecall::claude_help(
            "new",
            &mut cache,
            &environment,
            &mut runtime
        )),
        ["--no-session-persistence"]
    );
    assert!(
        sidecall::claude_flags(&sidecall::claude_help(
            "old",
            &mut cache,
            &environment,
            &mut runtime
        ))
        .is_empty()
    );
}
