// Converted from reference/upstream/tests/test_live_core.py (CLI discovery and session I/O).
pub mod fixtures;
mod support;

use fixtures::opencode_session_export;
use serde_json::{Map, Value, json};
use std::{
    collections::VecDeque,
    fs, io,
    path::{Path, PathBuf},
};
use support::TempDir;
use tokeneconomizer::{
    plugin::{
        cli,
        hosts::{
            self,
            session::{self, Environment, Limits},
        },
        paths, probe, process, sidecall,
    },
    trace::opencode,
};

fn environment(root: &Path) -> Environment {
    Environment {
        env: Map::new(),
        home: root.into(),
        cwd: root.into(),
    }
}

#[test]
fn from_a_plain_terminal_te_is_about_the_agent_that_last_worked_here() {
    let root = TempDir::new("core-recent-host");
    let mut env = environment(root.path());
    env.env
        .insert("TOKENECONOMIZER_HOME".into(), json!(root.path().join("te")));
    let work = root.path().join("work");
    fs::create_dir(&work).unwrap();
    assert!(cli::recent_host(&work, &env).unwrap().is_null());
    let repo = paths::repo_identity(&work, &env.env, &env.home).unwrap();
    let state = paths::repo_state_dir(&repo, "codex", &env.env, &env.home).unwrap();
    fs::create_dir_all(&state).unwrap();
    fs::write(state.join("ledger.jsonl"), "").unwrap();
    assert_eq!(cli::recent_host(&work, &env).unwrap(), "codex");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_te"))
        .env_clear()
        .env("HOME", root.path())
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("TOKENECONOMIZER_HOME", root.path().join("te"))
        .args(["--cwd", work.to_str().unwrap(), "status"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .unwrap()
            .contains("(codex)")
    );
}

#[derive(Default)]
struct Locate(Map<String, Value>);
impl sidecall::Runtime for Locate {
    fn which(&mut self, name: &str) -> Result<Option<String>, String> {
        Ok(self.0.get(name).and_then(Value::as_str).map(str::to_owned))
    }
}

#[test]
fn claude_cli_falls_back_to_the_native_install_when_not_on_path() {
    let root = TempDir::new("core-native-cli");
    let env = environment(root.path());
    let mut runtime = Locate::default();
    assert_eq!(
        sidecall::installed_cli("claude", &env, cfg!(windows), &mut runtime).unwrap(),
        "claude"
    );
    let exe = root.path().join(".local/bin").join(if cfg!(windows) {
        "claude.exe"
    } else {
        "claude"
    });
    fs::create_dir_all(exe.parent().unwrap()).unwrap();
    fs::write(&exe, "").unwrap();
    assert_eq!(
        sidecall::installed_cli("claude", &env, cfg!(windows), &mut runtime).unwrap(),
        exe.to_str().unwrap()
    );
    runtime.0.insert("claude".into(), json!("/opt/bin/claude"));
    assert_eq!(
        sidecall::installed_cli("claude", &env, cfg!(windows), &mut runtime).unwrap(),
        "/opt/bin/claude"
    );
}

const SHIM_HEAD: &str = "@ECHO off\r\nGOTO start\r\n:find_dp0\r\nSET dp0=%~dp0\r\nEXIT /b\r\n:start\r\nSETLOCAL\r\nCALL :find_dp0\r\n";
fn script_shim() -> String {
    format!(
        "{SHIM_HEAD}\r\nIF EXIST \"%dp0%\\node.exe\" (\r\n  SET \"_prog=%dp0%\\node.exe\"\r\n) ELSE (\r\n  SET \"_prog=node\"\r\n  SET PATHEXT=%PATHEXT:;.JS;=;%\r\n)\r\n\r\nendLocal & goto #_undefined_# 2>NUL || title %COMSPEC% & \"%_prog%\"  \"%dp0%\\node_modules\\@openai\\codex\\bin\\codex.js\" %*\r\n"
    )
}
fn shim(folder: &Path, text: &str, files: &[&str]) -> PathBuf {
    fs::create_dir_all(folder).unwrap();
    for file in files {
        let path = folder.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "").unwrap();
    }
    let path = folder.join("cli.cmd");
    fs::write(&path, text).unwrap();
    path
}

#[test]
fn an_npm_shim_is_read_for_what_it_runs() {
    let root = TempDir::new("core-npm-shim");
    let env = environment(root.path());
    let mut runtime = Locate(json!({"node":"/usr/bin/node"}).as_object().unwrap().clone());
    let script = "node_modules/@openai/codex/bin/codex.js";
    let a = shim(
        &root.path().join("a"),
        &script_shim(),
        &[script, "node.exe"],
    );
    assert_eq!(
        sidecall::shim_argv(&a, &env, &mut runtime).unwrap(),
        Some(vec![
            root.path()
                .join("a/node.exe")
                .to_string_lossy()
                .into_owned(),
            root.path()
                .join("a")
                .join(script)
                .to_string_lossy()
                .into_owned()
        ])
    );
    let b = shim(&root.path().join("b"), &script_shim(), &[script]);
    assert_eq!(
        sidecall::shim_argv(&b, &env, &mut runtime).unwrap(),
        Some(vec![
            "/usr/bin/node".into(),
            root.path()
                .join("b")
                .join(script)
                .to_string_lossy()
                .into_owned()
        ])
    );
    let exe = "node_modules/@opencode/cli/bin/opencode.exe";
    let c = shim(
        &root.path().join("c"),
        &format!("{SHIM_HEAD}\"%dp0%\\node_modules\\@opencode\\cli\\bin\\opencode.exe\"   %*\r\n"),
        &[exe],
    );
    assert_eq!(
        sidecall::shim_argv(&c, &env, &mut runtime).unwrap(),
        Some(vec![
            root.path()
                .join("c")
                .join(exe)
                .to_string_lossy()
                .into_owned()
        ])
    );
    let npm6 = "@IF EXIST \"%~dp0\\node.exe\" (\r\n  \"%~dp0\\node.exe\"  \"%~dp0\\..\\lib\\pi\\cli.js\" %*\r\n) ELSE (\r\n  @SETLOCAL\r\n  @SET PATHEXT=%PATHEXT:;.JS;=;%\r\n  node  \"%~dp0\\..\\lib\\pi\\cli.js\" %*\r\n)\r\n";
    let d = shim(&root.path().join("d/bin"), npm6, &["../lib/pi/cli.js"]);
    assert_eq!(
        sidecall::shim_argv(&d, &env, &mut runtime).unwrap(),
        Some(vec![
            "/usr/bin/node".into(),
            root.path()
                .join("d/lib/pi/cli.js")
                .to_string_lossy()
                .into_owned()
        ])
    );
    for (folder, text) in [
        ("e", script_shim()),
        ("f", "@echo off\r\ncodex-real.exe %*\r\n".into()),
    ] {
        assert_eq!(
            sidecall::shim_argv(
                &shim(&root.path().join(folder), &text, &[]),
                &env,
                &mut runtime
            )
            .unwrap(),
            None
        );
    }
}

#[test]
fn a_cli_on_windows_is_started_without_cmd_exe() {
    let root = TempDir::new("core-windows-cli");
    let env = environment(root.path());
    let script = "node_modules/@openai/codex/bin/codex.js";
    let npm = shim(
        &root.path().join("npm"),
        &script_shim(),
        &[script, "node.exe"],
    );
    let other = shim(
        &root.path().join("other"),
        "@echo off\r\ntool.exe %*\r\n",
        &[],
    );
    let pi = root.path().join("pi.exe");
    let mut runtime = Locate(
        json!({"codex":npm,"tool":other,"pi":pi})
            .as_object()
            .unwrap()
            .clone(),
    );
    assert_eq!(
        sidecall::program("codex", true, &env, &mut runtime).unwrap(),
        [
            root.path()
                .join("npm/node.exe")
                .to_string_lossy()
                .into_owned(),
            root.path()
                .join("npm")
                .join(script)
                .to_string_lossy()
                .into_owned()
        ]
    );
    assert_eq!(
        sidecall::program("pi", true, &env, &mut runtime).unwrap(),
        [pi.to_string_lossy()]
    );
    assert_eq!(
        sidecall::program("opencode", true, &env, &mut runtime).unwrap(),
        ["opencode"]
    );
    assert!(
        sidecall::program("tool", true, &env, &mut runtime)
            .unwrap_err()
            .contains("not an npm shim")
    );
}

#[derive(Default)]
struct SessionFixture {
    ancestors: Vec<String>,
    answers: VecDeque<io::Result<String>>,
    commands: Vec<Vec<String>>,
    cwd: PathBuf,
}
impl session::Runtime for SessionFixture {
    fn program(&mut self, cli: &str) -> session::Result<Vec<String>> {
        Ok(vec![cli.into()])
    }
    fn ancestors(
        &mut self,
        _limit: i64,
        _pid: u32,
        _windows: bool,
        _cwd: &Path,
    ) -> session::Result<Vec<String>> {
        Ok(self.ancestors.clone())
    }
    fn sleep(&mut self, seconds: f64) -> session::Result<()> {
        assert_eq!(seconds, 0.0);
        Ok(())
    }
    fn run(
        &mut self,
        argv: &[String],
        cwd: &Path,
        env: Option<&Map<String, Value>>,
        _timeout: f64,
    ) -> io::Result<process::Output> {
        self.commands.push(argv.to_vec());
        assert_eq!(cwd, self.cwd);
        assert_eq!(env.unwrap()["TE_SIDECALL"], "1");
        Ok(process::Output {
            code: 0,
            stdout: self
                .answers
                .pop_front()
                .expect("Unexpected export attempt")?,
            stderr: String::new(),
        })
    }
}

#[test]
fn codex_exec_and_copilot_p_are_told_from_their_terminal_uis() {
    let root = TempDir::new("core-headless-cli");
    let env = environment(root.path());
    let mut runtime = SessionFixture::default();
    let exec = root.path().join("rollout-exec.jsonl");
    let tui = root.path().join("rollout-tui.jsonl");
    fs::write(
        &exec,
        json!({"type":"session_meta","payload":{"originator":"codex_exec","source":"exec"}})
            .to_string()
            + "\n",
    )
    .unwrap();
    fs::write(
        &tui,
        json!({"type":"session_meta","payload":{"originator":"codex-tui","source":"vscode"}})
            .to_string()
            + "\n",
    )
    .unwrap();
    for (path, expected) in [
        (Some(exec.as_path()), true),
        (Some(tui.as_path()), false),
        (None, false),
        (Some(root.path().join("missing").as_path()), false),
    ] {
        assert_eq!(
            session::headless(
                "codex",
                &json!({}),
                path.map(|p| p.to_str().unwrap()),
                &env,
                0,
                false,
                &mut runtime
            )
            .unwrap(),
            expected
        );
    }
    for (args, expected) in [
        (
            vec![
                "/bin/sh /p/scripts/te-hook --host copilot",
                "copilot -p Reply with the word ready. --model claude-haiku-4.5",
            ],
            true,
        ),
        (vec!["copilot", "sh -c cd ~/proj && copilot"], false),
        (vec!["copilot --prompt=fix it"], true),
    ] {
        runtime.ancestors = args.into_iter().map(str::to_owned).collect();
        assert_eq!(
            session::headless("copilot", &json!({}), None, &env, 0, false, &mut runtime).unwrap(),
            expected
        );
    }
    assert_eq!(
        hosts::stop_hint(
            &json!(true),
            "Press Esc twice to interrupt it.",
            "Press Ctrl+C to end the run."
        ),
        "Press Ctrl+C to end the run."
    );
    assert_eq!(
        hosts::stop_hint(
            &json!(false),
            "Press Esc twice to interrupt it.",
            "Press Ctrl+C to end the run."
        ),
        "Press Esc twice to interrupt it."
    );
}

#[test]
fn the_opencode_host_fetches_opencodes_export_when_its_copy_is_empty() {
    let root = TempDir::new("core-opencode-export");
    let env = environment(root.path());
    let copy = root.path().join("ses_f.json");
    let empty = json!({"info":{"id":"ses_f"},"messages":[]}).to_string();
    fs::write(&copy, &empty).unwrap();
    let mut runtime = SessionFixture {
        cwd: root.path().into(),
        answers: [Ok(empty), Ok(opencode_session_export("ses_f").to_string())].into(),
        ..Default::default()
    };
    let limits = Limits {
        export_wait: 0.0,
        ..Default::default()
    };
    assert_eq!(
        session::complete(
            "opencode",
            copy.to_str(),
            "ses_f",
            Some(root.path()),
            &json!({}),
            &env,
            &limits,
            &mut runtime
        )
        .unwrap(),
        json!(copy)
    );
    assert_eq!(
        runtime.commands,
        vec![vec!["opencode", "session", "export", "ses_f"]; 2]
    );
    assert_eq!(opencode::prompt_count(&copy), 1);
    runtime.commands.clear();
    assert_eq!(
        session::complete(
            "opencode",
            copy.to_str(),
            "ses_f",
            Some(root.path()),
            &json!({}),
            &env,
            &limits,
            &mut runtime
        )
        .unwrap(),
        json!(copy)
    );
    assert_eq!(
        session::complete(
            "opencode",
            Some(""),
            "ses_x; rm -rf ~",
            None,
            &json!({}),
            &env,
            &limits,
            &mut runtime
        )
        .unwrap(),
        ""
    );
    assert!(runtime.commands.is_empty());
}

#[test]
fn the_opencode_host_keeps_its_copy_when_opencode_has_nothing() {
    let root = TempDir::new("core-opencode-empty");
    let env = environment(root.path());
    let copy = root.path().join("ses_f.json");
    let empty = json!({"info":{"id":"ses_f"},"messages":[]}).to_string();
    fs::write(&copy, &empty).unwrap();
    let mut runtime = SessionFixture {
        cwd: root.path().into(),
        answers: (0..3).map(|_| Ok(empty.clone())).collect(),
        ..Default::default()
    };
    let limits = Limits {
        export_wait: 0.0,
        ..Default::default()
    };
    assert_eq!(
        session::complete(
            "opencode",
            copy.to_str(),
            "ses_f",
            None,
            &json!({}),
            &env,
            &limits,
            &mut runtime
        )
        .unwrap(),
        json!(copy)
    );
    assert_eq!(fs::read_to_string(&copy).unwrap(), empty);
    assert!(
        fs::read_dir(root.path()).unwrap().all(|e| e
            .unwrap()
            .path()
            .extension()
            .is_none_or(|ext| ext != "tmp"))
    );
    runtime
        .answers
        .push_back(Err(io::Error::new(io::ErrorKind::NotFound, "opencode")));
    assert_eq!(
        session::complete(
            "opencode",
            copy.to_str(),
            "ses_f",
            None,
            &json!({}),
            &env,
            &limits,
            &mut runtime
        )
        .unwrap(),
        json!(copy)
    );
}

struct InspectFixture(Environment);
impl probe::Runtime for InspectFixture {
    fn say(&mut self, _text: &str) -> cli::Result<()> {
        panic!("Unexpected probe output")
    }
    fn call(&mut self, method: &str, args: Value) -> cli::Result<Value> {
        assert_eq!(method, "settle");
        session::settle(
            "copilot",
            Some(Path::new(args[0].as_str().unwrap())),
            15.0,
            60.0,
            &mut session::NativeRuntime {
                environment: &self.0,
            },
        )
        .map_err(|e| e.to_string())?;
        Ok(Value::Null)
    }
}
fn inspect(root: &Path, log: &str) -> Value {
    let mut env = environment(root);
    env.env
        .insert("COPILOT_HOME".into(), json!(root.join("cp")));
    let path = root.join("cp/session-state/s1/events.jsonl");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, log).unwrap();
    probe::inspect_session("s1", &env, &mut InspectFixture(env.clone())).unwrap()
}

#[test]
fn inspect_session_skips_hook_lines_that_do_not_parse() {
    let root = TempDir::new("core-inspect-corrupt");
    let report = inspect(
        root.path(),
        concat!(
            "{\"type\": \"session.start\", \"data\": {\"context\": {\"cwd\": \"/r\"}}}\n",
            "\"hook.end sessionStart\"\n",
            "{\"type\": \"hook.end\", \"data\": [\"x\"], \"hookType\": \"sessionStart\"}\n",
            "{broken \"hook.end\" \"sessionStart\"\n",
            "{\"type\": \"session.shutdown\", \"data\": {}}\n"
        ),
    );
    assert_eq!(report["session"], "s1");
    assert_eq!(report["injected"], false);
}

#[test]
fn inspect_session_reads_the_context_its_session_start_hook_returned() {
    let root = TempDir::new("core-inspect-context");
    let hook = json!({"type":"hook.end","data":{"hookType":"sessionStart","output":{"additionalContext":"In this repository, pytest fails here: use unittest."}}});
    assert_eq!(
        inspect(
            root.path(),
            &format!(
                "\"hook.end sessionStart\"\n{hook}\n{{\"type\":\"session.shutdown\",\"data\":{{}}}}\n"
            )
        )["injected"],
        true
    );
}
