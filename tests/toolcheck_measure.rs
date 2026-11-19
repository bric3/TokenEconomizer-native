// Converted from reference/upstream/tests/test_toolcheck_measure.py.
mod support;

use serde_json::{Map, Value, json};
use std::{
    collections::VecDeque,
    fs, io,
    path::{Path, PathBuf},
    time::Instant,
};
use support::TempDir;
use tokeneconomizer::plugin::{
    hosts::session::Environment,
    process, sidecall,
    toolcheck::{self, Checks},
};

fn checks(root: &Path) -> Checks {
    Checks {
        environment: Environment {
            env: Map::new(),
            home: root.into(),
            cwd: root.into(),
        },
        windows: cfg!(windows),
        help: Map::new(),
    }
}
struct Recorded {
    argv: Vec<String>,
    settings: Option<Value>,
}
struct Runtime {
    replies: VecDeque<process::Output>,
    seen: Vec<Recorded>,
}
impl sidecall::Runtime for Runtime {
    fn program(
        &mut self,
        cli: &str,
        _windows: bool,
        _environment: &Environment,
    ) -> Result<Vec<String>, String> {
        Ok(vec![cli.into()])
    }
    fn run(
        &mut self,
        argv: &[String],
        _cwd: &Path,
        _env: &Map<String, Value>,
        _input: Option<&str>,
        _timeout: f64,
    ) -> io::Result<process::Output> {
        let settings = argv
            .iter()
            .position(|s| s == "--settings")
            .map(|i| serde_json::from_slice(&fs::read(&argv[i + 1]).unwrap()).unwrap());
        self.seen.push(Recorded {
            argv: argv.to_vec(),
            settings,
        });
        Ok(self
            .replies
            .pop_front()
            .expect("Unexpected measurement probe"))
    }
}
impl toolcheck::Runtime for Runtime {
    fn override_probe(&mut self, method: &str, _args: Value) -> Result<Option<Value>, String> {
        Ok((method == "claude_flags").then(|| json!([])))
    }
}
fn output(code: i32, stdout: impl Into<String>, stderr: &str) -> process::Output {
    process::Output {
        code,
        stdout: stdout.into(),
        stderr: stderr.into(),
    }
}
fn runtime(replies: Vec<process::Output>) -> Runtime {
    Runtime {
        replies: replies.into(),
        seen: vec![],
    }
}

#[test]
fn the_lean_probe_takes_the_deny_list_from_a_settings_file() {
    let root = TempDir::new("measure-lean");
    let tools: Vec<_> = (0..400)
        .map(|i| format!("mcp__server_{i}__some_long_tool_name_{i}"))
        .collect();
    let mut runtime = runtime(vec![
        output(0, r#"{"usage":{"input_tokens":100}}"#, ""),
        output(0, r#"{"usage":{"input_tokens":100}}"#, ""),
    ]);
    checks(root.path())
        .measure(&tools, "claude", "claude", &mut runtime)
        .unwrap();
    let lean = &runtime.seen[1];
    assert!(lean.argv.iter().map(|a| a.chars().count()).sum::<usize>() < 2000);
    assert_eq!(lean.settings, Some(json!({"permissions":{"deny":tools}})));
}

#[test]
fn a_failed_probe_raises_instead_of_counting_zero() {
    for stdout in [
        "",
        "The command line is too long.",
        r#"{"usage":{}}"#,
        r#"{"is_error":true,"usage":{"input_tokens":0}}"#,
    ] {
        let root = TempDir::new("measure-failed");
        assert!(
            checks(root.path())
                .measure(
                    &["mcp__x__y".into()],
                    "claude",
                    "claude",
                    &mut runtime(vec![output(0, stdout, "")])
                )
                .is_err()
        );
    }
}

#[test]
fn a_probe_that_exits_non_zero_raises_even_with_usage() {
    let root = TempDir::new("measure-crash");
    let mut runtime = runtime(vec![
        output(0, r#"{"is_error":false,"usage":{"input_tokens":100}}"#, ""),
        output(
            1,
            r#"{"is_error":false,"usage":{"input_tokens":20}}"#,
            "boom",
        ),
    ]);
    let error = checks(root.path())
        .measure(&["mcp__x__y".into()], "claude", "claude", &mut runtime)
        .unwrap_err();
    assert!(error.contains("exited 1: boom"), "{error}");
}

#[test]
fn a_codex_probe_that_prints_nothing_raises() {
    for lean in [Some(""), None] {
        let root = TempDir::new("measure-codex-empty");
        // Native pipe output is a String; missing Python stdout has the same empty boundary value.
        let mut runtime = runtime(vec![
            output(0, "x".repeat(1000), ""),
            output(0, lean.unwrap_or_default(), ""),
        ]);
        assert!(
            checks(root.path())
                .measure(&["web_search".into()], "codex", "codex", &mut runtime)
                .is_err()
        );
    }
}

struct Stream(Option<PathBuf>);
impl sidecall::Runtime for Stream {}
impl toolcheck::Runtime for Stream {
    fn override_probe(&mut self, method: &str, _args: Value) -> Result<Option<Value>, String> {
        Ok((method == "claude_flags").then(|| json!([])))
    }
    fn start_probe(
        &mut self,
        argv: &[String],
        cwd: &Path,
        env: &Map<String, Value>,
        new_session: bool,
    ) -> Result<Box<dyn process::Probe>, String> {
        assert_eq!(argv[0], "claude");
        let mut child = vec![env!("CARGO_BIN_EXE_te-stream-probe").into()];
        if let Some(path) = &self.0 {
            child.push(path.to_str().unwrap().into());
        }
        process::start_probe(&child, cwd, env, new_session).map_err(|e| e.to_string())
    }
}

#[test]
fn the_init_probe_gives_up_on_a_cli_that_prints_nothing() {
    let root = TempDir::new("measure-init-silent");
    let started = Instant::now();
    assert_eq!(
        checks(root.path())
            .init_tools(&[], "claude", 1.0, &mut Stream(None))
            .unwrap(),
        (Value::Null, Value::Null)
    );
    assert!(started.elapsed().as_secs_f64() < 4.0);
}

#[test]
fn the_init_probe_returns_the_tools_when_the_init_event_arrives() {
    let root = TempDir::new("measure-init-ready");
    let path = root.path().join("output.jsonl");
    fs::write(&path,format!("{}\n",json!({"type":"system","subtype":"init","tools":["Bash","Read"],"claude_code_version":"2.1"}))).unwrap();
    let started = Instant::now();
    assert_eq!(
        checks(root.path())
            .init_tools(&[], "claude", 30.0, &mut Stream(Some(path)))
            .unwrap(),
        (json!(["Bash", "Read"]), json!("2.1"))
    );
    assert!(started.elapsed().as_secs_f64() < 4.0);
}

#[test]
fn the_init_probe_skips_json_lines_that_are_not_objects() {
    let root = TempDir::new("measure-init-nonobjects");
    let path = root.path().join("output.jsonl");
    fs::write(
        &path,
        format!(
            "null\n[1, 2]\n\"text\"\n42\n{}\n",
            json!({"type":"system","subtype":"init","tools":["Bash"],"claude_code_version":"2.1"})
        ),
    )
    .unwrap();
    assert_eq!(
        checks(root.path())
            .init_tools(&[], "claude", 30.0, &mut Stream(Some(path)))
            .unwrap(),
        (json!(["Bash"]), json!("2.1"))
    );
}
