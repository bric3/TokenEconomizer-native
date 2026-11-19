// Converted from reference/upstream/tests/test_claude_verbose_reply.py.
pub mod fixtures;
mod support;

use fixtures::write_claude_transcript;
use serde_json::{Map, Value, json};
use std::{
    collections::VecDeque,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use support::TempDir;
use tokeneconomizer::{
    microskills::distiller::{Distiller, ProposeContext},
    plugin::{
        config::LiveConfig,
        hosts::session::Environment,
        process,
        sidecall::{
            self, Call, Failure,
            runner::{Context, RoutedRunner, State},
        },
        toolcheck,
    },
    runner::Runner,
};

fn result() -> Value {
    json!({"type":"result","subtype":"success","is_error":false,"result":"OK","usage":{"input_tokens":12,"output_tokens":3},"modelUsage":{"claude-haiku-4-5":{}}})
}
fn verbose() -> Vec<Value> {
    vec![
        json!({"type":"system","subtype":"init","tools":[]}),
        json!({"type":"assistant","message":{"content":[{"type":"text","text":"OK"}]}}),
        json!({"type":"assistant","message":{"content":[]}}),
        json!({"type":"rate_limit_event"}),
        result(),
    ]
}
fn environment(root: &Path) -> Environment {
    Environment {
        env: Map::new(),
        home: root.into(),
        cwd: root.into(),
    }
}
struct Claude(String);
impl sidecall::Runtime for Claude {
    fn run(
        &mut self,
        argv: &[String],
        _cwd: &Path,
        _env: &Map<String, Value>,
        _input: Option<&str>,
        _timeout: f64,
    ) -> io::Result<process::Output> {
        Ok(process::Output {
            code: 0,
            stdout: if argv.iter().any(|a| a == "--help") {
                String::new()
            } else {
                self.0.clone()
            },
            stderr: String::new(),
        })
    }
}
impl toolcheck::Runtime for Claude {}
fn call(stdout: String, root: &Path) -> (Value, Option<f64>) {
    sidecall::claude_call(
        &Call {
            prompt: "hi",
            cli: "claude",
            model: "",
            effort: "",
            timeout: 180.0,
            windows: cfg!(windows),
        },
        &environment(root),
        root,
        &mut Map::new(),
        &mut Claude(stdout),
    )
    .unwrap()
}

#[test]
fn claude_json_reads_the_result_object() {
    let events = verbose();
    for (stdout, expected) in [
        (Some(result().to_string()), result()),
        (Some(json!(events).to_string()), result()),
        (Some(json!(events[..4]).to_string()), json!({})),
        (
            Some(json!([result(),{"type":"result","result":"later"}]).to_string()),
            json!({"type":"result","result":"later"}),
        ),
        (Some(json!(["text", 3, null]).to_string()), json!({})),
        (Some(json!("OK").to_string()), json!({})),
        (Some("not json".into()), json!({})),
        (Some(String::new()), json!({})),
        (None, json!({})),
    ] {
        let actual = if let Some(stdout) = stdout {
            sidecall::claude_json(&stdout)
        } else {
            // The native bridge owns the nullable input boundary; the parser takes &str.
            let mut child = Command::new(env!("CARGO_BIN_EXE_te-bridge"))
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap();
            writeln!(
                child.stdin.take().unwrap(),
                "{}",
                json!({"op":"sidecall.claude_json","args":{"stdout":null}})
            )
            .unwrap();
            let output = child.wait_with_output().unwrap();
            assert!(output.status.success());
            let response: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert!(response.get("error").is_none(), "{response}");
            response["result"]["value"].clone()
        };
        assert_eq!(actual, expected);
    }
}

#[test]
fn a_verbose_side_call_returns_its_reply_and_usage() {
    let root = TempDir::new("verbose-side-reply");
    let (text, eff) = call(json!(verbose()).to_string(), root.path());
    assert_eq!(text, "OK");
    assert!(eff.is_some_and(|e| e > 0.0));
}

#[test]
fn a_verbose_side_call_without_a_result_is_no_reply() {
    let root = TempDir::new("verbose-side-missing");
    assert_eq!(call(json!(verbose()[..4]).to_string(), root.path()).0, "");
    assert!(
        fs::read_to_string(root.path().join("errors.log"))
            .unwrap()
            .contains("sidecall claude: no reply")
    );
}

#[test]
fn the_measure_probe_reads_a_verbose_reply() {
    let root = TempDir::new("verbose-measure");
    let mut value = result();
    value["usage"] = json!({"input_tokens":100});
    value["total_cost_usd"] = json!(0.01);
    let mut runtime = Claude(json!([{"type":"system"},value]).to_string());
    let mut checks = toolcheck::Checks {
        environment: environment(root.path()),
        windows: cfg!(windows),
        help: Map::new(),
    };
    let (tokens, cost) = checks
        .prompt_tokens(&[], "claude", 180.0, &mut runtime)
        .unwrap();
    assert_eq!(tokens.as_f64(), Some(100.0));
    assert_eq!(cost.as_f64(), Some(0.01));
}

struct Proposer {
    replies: VecDeque<Result<(String, Option<f64>), String>>,
    calls: usize,
}
impl Runner for Proposer {
    fn run(&mut self, _prompt: &str, _model: &str) -> Result<(String, Option<f64>), String> {
        self.calls += 1;
        self.replies.pop_front().expect("Unexpected proposer call")
    }
}
fn session(root: &Path, n: usize) -> PathBuf {
    write_claude_transcript(
        &root.join(format!("s{n}.jsonl")),
        &[
            json!(["Bash",{"command":"make build"},"built"]),
            json!(["Bash",{"command":"make test"},"ok  ./... 0.01s"]),
        ],
    )
}

#[test]
fn a_failed_proposer_call_skips_the_tick_instead_of_the_fold() {
    let root = TempDir::new("verbose-proposer-timeout");
    let mut runner = Proposer {
        replies: [Err("side call timed out after 180s".into())].into(),
        calls: 0,
    };
    let mut distiller =
        Distiller::load("acme/tool", &root.path().join("state"), "agent", true).unwrap();
    distiller
        .update(
            &session(root.path(), 0),
            "s0",
            json!(true),
            &mut ProposeContext {
                runner: Some(&mut runner),
                propose_every: 1,
                ..Default::default()
            },
        )
        .unwrap();
    distiller.save(&root.path().join("state")).unwrap();
    assert_eq!(runner.calls, 1);
    assert!(distiller.facts.is_empty());
    assert_eq!(distiller.opt_tokens, 0.0);
    assert_eq!(distiller.last_propose, distiller.now);
}

#[test]
fn the_next_tick_after_a_failure_proposes_as_usual() {
    let root = TempDir::new("verbose-proposer-next");
    let mut runner = Proposer {
        replies: [
            Err("boom".into()),
            Ok((r#"{"extractors":[],"facts":[]}"#.into(), Some(40.0))),
        ]
        .into(),
        calls: 0,
    };
    let mut distiller =
        Distiller::load("acme/tool", &root.path().join("state"), "agent", true).unwrap();
    for n in 0..2 {
        distiller
            .update(
                &session(root.path(), n),
                &format!("s{n}"),
                json!(true),
                &mut ProposeContext {
                    runner: Some(&mut runner),
                    propose_every: 1,
                    ..Default::default()
                },
            )
            .unwrap();
    }
    assert_eq!(distiller.opt_tokens, 40.0);
    assert_eq!(distiller.last_propose, distiller.now);
}

#[test]
fn an_error_result_is_no_reply_and_pauses_a_signed_out_host() {
    for verbose_output in [false, true] {
        let root = TempDir::new("verbose-signed-out");
        let mut error = result();
        error["is_error"] = json!(true);
        error["result"] = json!(if verbose_output {
            "Please run /login"
        } else {
            "Invalid API key · Please run /login"
        });
        let stdout = if verbose_output {
            let mut events = verbose();
            events[4] = error;
            json!(events)
        } else {
            error
        };
        assert_eq!(call(stdout.to_string(), root.path()).0, "");
        assert!(!sidecall::paused("claude", root.path()).is_null());
    }
}

struct Answer(Result<(Value, Option<f64>), Failure>);
impl sidecall::Runtime for Answer {
    fn override_call(
        &mut self,
        _method: &str,
        _args: &Value,
    ) -> Result<Option<(Value, Option<f64>)>, Failure> {
        self.0.clone().map(Some)
    }
}

#[test]
fn a_side_call_that_raises_is_noted_in_errors_log() {
    for (failure, line) in [
        (Failure::Timeout { timeout: 180.0 }, "timed out after 180s"),
        (Failure::Exit { returncode: 2 }, "exit 2"),
        (
            Failure::Exception {
                name: "RuntimeError".into(),
                message: "boom\nmore".into(),
            },
            "RuntimeError: boom more",
        ),
    ] {
        let root = TempDir::new("verbose-side-exception");
        let mut runner = RoutedRunner::new(
            State::default(),
            Context::new(environment(root.path()), root.path().into()),
            Answer(Err(failure.clone())),
        );
        assert!(
            runner
                .select("grok", &LiveConfig::default().as_dict(), None, false)
                .unwrap()
        );
        let actual = runner.run_raw("SECRET prompt", "").unwrap_err();
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::to_value(failure).unwrap()
        );
        let log = fs::read_to_string(root.path().join("errors.log")).unwrap();
        assert!(
            log.starts_with("--- ") && log.contains("sidecall grok:") && log.contains(line),
            "{log}"
        );
        assert_eq!(log.matches('\n').count(), 1);
        assert!(!log.contains("SECRET"));
    }
}

#[test]
fn a_side_call_that_answers_is_passed_through() {
    let root = TempDir::new("verbose-side-answer");
    let mut runner = RoutedRunner::new(
        State::default(),
        Context::new(environment(root.path()), root.path().into()),
        Answer(Ok((json!("hi"), Some(7.0)))),
    );
    assert!(
        runner
            .select("codex", &LiveConfig::default().as_dict(), None, false)
            .unwrap()
    );
    assert_eq!(
        runner.run_raw("q", "agent").unwrap(),
        (json!("hi"), Some(7.0))
    );
    assert!(!root.path().join("errors.log").exists());
}
