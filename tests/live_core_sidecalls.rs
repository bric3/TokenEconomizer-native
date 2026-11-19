// Converted from reference/upstream/tests/test_live_core.py (side-call processes).
pub mod fixtures;
mod support;

use fixtures::{opencode_run_events, pi_json_run, write_copilot_events};
use serde_json::{Map, Value, json};
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use support::TempDir;
use tokeneconomizer::plugin::{
    config::{self, LiveConfig},
    hosts::session::Environment,
    process,
    sidecall::{
        self, Call,
        runner::{self, Context, RoutedRunner, State},
    },
};

struct Invocation {
    argv: Vec<String>,
    cwd: PathBuf,
    env: Map<String, Value>,
    input: Option<String>,
    mcp_config: Option<Value>,
}
struct Fixture<F> {
    respond: F,
    calls: Vec<Invocation>,
    wrapped: bool,
    cleanup: Vec<bool>,
}
fn fixture<F: FnMut(&Invocation) -> process::Output>(respond: F) -> Fixture<F> {
    Fixture {
        respond,
        calls: vec![],
        wrapped: false,
        cleanup: vec![],
    }
}
impl<F: FnMut(&Invocation) -> process::Output> sidecall::Runtime for Fixture<F> {
    fn program(
        &mut self,
        cli: &str,
        _windows: bool,
        _environment: &Environment,
    ) -> Result<Vec<String>, String> {
        Ok(if self.wrapped {
            vec!["/n/node".into(), format!("/n/{cli}.js")]
        } else {
            vec![cli.into()]
        })
    }
    fn claude_cli(&mut self, _environment: &Environment, _windows: bool) -> Result<String, String> {
        Ok("new".into())
    }
    fn temporary_directory(
        &mut self,
        _prefix: &str,
        ignore_cleanup_errors: bool,
    ) -> Result<Option<PathBuf>, String> {
        self.cleanup.push(ignore_cleanup_errors);
        Ok(None)
    }
    fn run(
        &mut self,
        argv: &[String],
        cwd: &Path,
        env: &Map<String, Value>,
        input: Option<&str>,
        _timeout: f64,
    ) -> io::Result<process::Output> {
        let mcp_config = argv
            .iter()
            .position(|a| a == "--mcp-config")
            .map(|i| serde_json::from_slice(&fs::read(&argv[i + 1]).unwrap()).unwrap());
        self.calls.push(Invocation {
            argv: argv.to_vec(),
            cwd: cwd.into(),
            env: env.clone(),
            input: input.map(str::to_owned),
            mcp_config,
        });
        Ok((self.respond)(self.calls.last().unwrap()))
    }
}
fn output(stdout: impl Into<String>) -> process::Output {
    process::Output {
        code: 0,
        stdout: stdout.into(),
        stderr: String::new(),
    }
}
fn environment(root: &Path) -> Environment {
    Environment {
        env: json!({"TOKENECONOMIZER_HOME":root.join("te"),"COPILOT_HOME":root.join("cp")})
            .as_object()
            .unwrap()
            .clone(),
        home: root.into(),
        cwd: root.into(),
    }
}
fn request<'a>(cli: &'a str, prompt: &'a str) -> Call<'a> {
    Call {
        cli,
        prompt,
        model: "",
        effort: "",
        timeout: 120.0,
        windows: cfg!(windows),
    }
}
fn after<'a>(argv: &'a [String], flag: &str) -> &'a str {
    &argv[argv.iter().position(|a| a == flag).unwrap() + 1]
}

#[test]
fn a_copilot_side_call_is_a_bare_completion_priced_from_its_own_log() {
    let root = TempDir::new("core-copilot-side");
    let env = environment(root.path());
    write_copilot_events(
        &root.path().join("cp/session-state/side1/events.jsonl"),
        &[],
        true,
    );
    let mut runtime = fixture(|_| {
        output(format!(
            "{}\n{}",
            json!({"type":"assistant.message","data":{"content":"- use python -m pytest"}}),
            json!({"type":"result","sessionId":"side1","exitCode":0})
        ))
    });
    let (text, eff) = sidecall::copilot_call(
        &request("copilot", "propose facts"),
        &env,
        &root.path().join("te"),
        &mut runtime,
    )
    .unwrap();
    assert_eq!(text, "- use python -m pytest");
    assert!(eff.is_some_and(|e| e > 0.0));
    let call = runtime.calls.last().unwrap();
    assert_eq!(call.argv[..3], ["copilot", "-p", "propose facts"]);
    assert!(call.argv.iter().any(|a| a == "--disable-builtin-mcps"));
    assert_eq!(
        call.argv
            .iter()
            .filter(|a| *a == "--excluded-tools")
            .count(),
        sidecall::COPILOT_TOOLS.len()
    );
    assert_eq!(call.env["TE_SIDECALL"], "1");
    for host in ["copilot", "claude", "pi"] {
        assert_eq!(
            runner::host_call(host, &LiveConfig::default().as_dict(), &env)
                .unwrap()
                .unwrap()
                .method,
            format!("{host}_call")
        );
    }
}

#[test]
fn a_side_call_with_no_reply_is_noted_and_keeps_to_a_bare_completion() {
    let root = TempDir::new("core-side-no-reply");
    let env = environment(root.path());
    fs::create_dir(root.path().join("cp")).unwrap();
    fs::write(
        root.path().join("cp/mcp-config.json"),
        json!({"mcpServers":{"acme-db":{}}}).to_string(),
    )
    .unwrap();
    let mut runtime = fixture(|_| process::Output {
        code: 1,
        stdout: String::new(),
        stderr: "Error: No authentication information found.".into(),
    });
    assert_eq!(
        sidecall::copilot_call(
            &request("copilot", "propose facts"),
            &env,
            &root.path().join("te"),
            &mut runtime
        )
        .unwrap(),
        (json!(""), None)
    );
    assert_eq!(
        after(&runtime.calls[0].argv, "--disable-mcp-server"),
        "acme-db"
    );
    let log = fs::read_to_string(root.path().join("te/errors.log")).unwrap();
    assert!(
        log.contains("sidecall copilot: no reply (exit 1)") && log.contains("No authentication")
    );
}

#[test]
fn a_pi_side_call_is_a_bare_run_on_the_persons_own_pi() {
    let root = TempDir::new("core-pi-side");
    let env = environment(root.path());
    let mut runtime = fixture(|call| {
        if call.argv.iter().any(|a| a == "--no-mcp") {
            process::Output {
                code: 1,
                stdout: String::new(),
                stderr: "Error: Unknown option: --no-mcp\n".into(),
            }
        } else {
            output(pi_json_run())
        }
    });
    let mut call = request("pi", "propose facts");
    let (text, eff) = sidecall::pi_call(&call, &[], &env, root.path(), &mut runtime).unwrap();
    assert_eq!(text, "- use python -m pytest");
    assert!(eff.is_some_and(|e| e > 0.0));
    assert_eq!(runtime.calls.len(), 2);
    let first = &runtime.calls[0];
    assert_eq!(first.argv[..4], ["pi", "--print", "--mode", "json"]);
    assert_eq!(first.argv[first.argv.len() - 2..], ["--", "propose facts"]);
    for flag in [
        "--no-session",
        "--no-tools",
        "--no-extensions",
        "--no-context-files",
        "--no-mcp",
    ] {
        assert!(first.argv.iter().any(|a| a == flag), "{flag}");
    }
    assert!(!first.argv.iter().any(|a| a == "--model"));
    assert!(!runtime.calls[1].argv.iter().any(|a| a == "--no-mcp"));
    assert_eq!(first.env["TE_SIDECALL"], "1");
    assert!(first.input.is_none());
    runtime.calls.clear();
    call.model = "openrouter/anthropic/claude-haiku-4.5";
    sidecall::pi_call(&call, &[], &env, root.path(), &mut runtime).unwrap();
    let last = &runtime.calls.last().unwrap().argv;
    assert_eq!(
        last[last.len() - 4..last.len() - 2],
        ["--model", call.model]
    );
}

#[test]
fn a_pi_side_call_loads_the_provider_extensions_and_the_pi_the_person_names() {
    let root = TempDir::new("core-pi-extensions");
    let env = environment(root.path());
    let cfg = config::load_config(&root.path().join("te"), None, json!({"TE_PI_EXTENSIONS":"ext/gateway.ts, builtin:web","TE_PI_CLI":"/opt/pi/bin/pi","TE_PAID_MODEL":"gateway/claude-haiku"}).as_object().unwrap());
    assert!(cfg.warnings.is_empty());
    let runtime = fixture(|_| output(pi_json_run()));
    let mut runner = RoutedRunner::new(
        State::default(),
        Context::new(env, root.path().join("te")),
        runtime,
    );
    assert!(runner.select("pi", &cfg.as_dict(), None, true).unwrap());
    assert_eq!(
        runner.run_raw("propose facts", "").unwrap().0,
        "- use python -m pytest"
    );
    let cmd = &runner.runtime.calls.last().unwrap().argv;
    assert_eq!(cmd[0], "/opt/pi/bin/pi");
    for flag in ["--no-extensions", "--no-tools"] {
        assert!(cmd.iter().any(|a| a == flag));
    }
    let extensions: Vec<_> = cmd
        .windows(2)
        .filter(|pair| pair[0] == "--extension")
        .map(|pair| pair[1].as_str())
        .collect();
    assert_eq!(
        extensions,
        [
            root.path().join("ext/gateway.ts").to_str().unwrap(),
            "builtin:web"
        ]
    );
    assert_eq!(
        cmd[cmd.len() - 4..],
        ["--model", "gateway/claude-haiku", "--", "propose facts"]
    );
    assert!(
        runner
            .select("pi", &LiveConfig::default().as_dict(), None, true)
            .unwrap()
    );
    runner.run_raw("propose facts", "").unwrap();
    let cmd = &runner.runtime.calls.last().unwrap().argv;
    assert_eq!(cmd[0], "pi");
    assert!(!cmd.iter().any(|a| a == "--extension"));
}

#[test]
fn a_claude_side_call_attaches_none_of_the_persons_mcp_servers() {
    let root = TempDir::new("core-claude-mcp");
    let env = environment(root.path());
    let help = "  --no-session-persistence\n  --strict-mcp-config   Only use MCP servers from --mcp-config\n  --mcp-config <configs...>";
    let mut runtime = fixture(|call| {
        if call.argv[1..] == ["--help"] {
            output(if call.argv[0] == "new" { help } else { "  -p" })
        } else {
            output(json!({"result":"OK","usage":{"input_tokens":10,"output_tokens":1},"modelUsage":{"claude-haiku-4-5":{}}}).to_string())
        }
    });
    let mut cache = Map::new();
    assert_eq!(
        sidecall::claude_call(
            &request("new", "Reply with exactly: OK"),
            &env,
            root.path(),
            &mut cache,
            &mut runtime
        )
        .unwrap()
        .0,
        "OK"
    );
    let call = runtime.calls.last().unwrap();
    assert!(call.argv.iter().any(|a| a == "--strict-mcp-config"));
    assert_eq!(call.mcp_config, Some(json!({"mcpServers":{}})));
    assert_eq!(
        Path::new(after(&call.argv, "--mcp-config")).parent(),
        Some(call.cwd.as_path())
    );
    let i = call.argv.iter().position(|a| a == "--mcp-config").unwrap();
    assert!(call.argv[i + 2].starts_with("--"));
    assert_eq!(call.env["TE_SIDECALL"], "1");
    assert!(
        !sidecall::claude_flags(help)
            .iter()
            .any(|a| a == "--strict-mcp-config")
    );
    sidecall::claude_call(
        &request("old", "Reply with exactly: OK"),
        &env,
        root.path(),
        &mut cache,
        &mut runtime,
    )
    .unwrap();
    let cmd = &runtime.calls.last().unwrap().argv;
    assert!(
        !cmd.iter()
            .any(|a| a == "--strict-mcp-config" || a == "--mcp-config")
    );
}

#[test]
fn side_calls_take_a_model_per_agent_and_an_effort() {
    let root = TempDir::new("core-side-models");
    let env = environment(root.path());
    let mut cfg = config::load_config(root.path(), None, json!({"TE_PAID_MODEL":"shared","TE_CODEX_PAID_MODEL":"small","TE_CLAUDE_PAID_EFFORT":"max","TE_CODEX_PAID_EFFORT":"minimal"}).as_object().unwrap());
    assert!(cfg.warnings.is_empty());
    assert!(!cfg.set("claude_paid_effort", json!("minimal"), "test"));
    assert!(!cfg.set("codex_paid_effort", json!("x"), "test"));
    let runtime = fixture(|call| {
        if call.argv[1..] == ["--help"] {
            output(if call.argv[0] == "new" {
                "  --effort <level>"
            } else {
                "  -p"
            })
        } else {
            output(r#"{"result":"OK"}"#)
        }
    });
    let mut runner = RoutedRunner::new(
        State::default(),
        Context::new(env.clone(), root.path().join("te")),
        runtime,
    );
    for host in ["claude", "codex"] {
        assert!(runner.select(host, &cfg.as_dict(), None, true).unwrap());
        runner.run_raw("hi", "").unwrap();
    }
    let calls: Vec<_> = runner
        .runtime
        .calls
        .iter()
        .filter(|c| c.argv[1..] != ["--help"])
        .collect();
    assert_eq!(calls.len(), 2);
    assert_eq!(after(&calls[0].argv, "--model"), "shared");
    assert_eq!(after(&calls[0].argv, "--effort"), "max");
    assert_eq!(after(&calls[1].argv, "-m"), "small");
    assert_eq!(
        after(&calls[1].argv, "-c"),
        "model_reasoning_effort=\"minimal\""
    );
    let mut call = request("old", "hi");
    call.effort = "high";
    sidecall::claude_call(
        &call,
        &env,
        root.path(),
        &mut Map::new(),
        &mut runner.runtime,
    )
    .unwrap();
    assert!(
        !runner
            .runtime
            .calls
            .last()
            .unwrap()
            .argv
            .iter()
            .any(|a| a == "--effort")
    );
    assert!(
        runner
            .select("codex", &LiveConfig::default().as_dict(), None, true)
            .unwrap()
    );
    runner.run_raw("hi", "").unwrap();
    assert!(
        !runner
            .runtime
            .calls
            .last()
            .unwrap()
            .argv
            .iter()
            .any(|a| a == "-m" || a == "-c")
    );
}

#[test]
fn the_opencode_side_call_denies_tools_and_deletes_its_session() {
    let root = TempDir::new("core-opencode-side");
    let env = environment(root.path());
    let mut runtime = fixture(|call| {
        output(if call.argv.iter().any(|a| a == "--format") {
            opencode_run_events()
        } else {
            "--pure --standalone".into()
        })
    });
    let mut call = request("oc", "is it stuck?");
    call.model = "p/m";
    let (text, eff) =
        sidecall::opencode_call(&call, &mut Map::new(), &env, root.path(), &mut runtime).unwrap();
    assert_eq!(text, "pong");
    assert!(eff.is_some_and(|e| e != 0.0));
    let run = runtime
        .calls
        .iter()
        .find(|call| call.argv.iter().any(|a| a == "--format"))
        .unwrap();
    assert_eq!(run.argv[..2], ["oc", "run"]);
    for flag in ["--pure", "--standalone"] {
        assert!(run.argv.iter().any(|a| a == flag));
    }
    assert_eq!(
        run.argv[run.argv.len() - 3..],
        ["--model", "p/m", "is it stuck?"]
    );
    assert_eq!(run.env["TE_SIDECALL"], "1");
    let config: Value =
        serde_json::from_str(run.env["OPENCODE_CONFIG_CONTENT"].as_str().unwrap()).unwrap();
    assert_eq!(config["permission"]["bash"], "deny");
    assert_eq!(config["permission"]["edit"], "deny");
    assert!(
        runtime
            .calls
            .iter()
            .any(|c| c.argv == ["oc", "session", "delete", "ses_x"])
    );
}

#[test]
fn the_opencode_side_call_adds_only_the_flags_its_cli_lists() {
    let root = TempDir::new("core-opencode-flags");
    let env = environment(root.path());
    let mut runtime = fixture(|_| output("  --format  json\n"));
    assert!(
        sidecall::opencode_flags("oc", cfg!(windows), &mut Map::new(), &env, &mut runtime)
            .unwrap()
            .is_empty()
    );
    assert!(
        runner::host_call("opencode", &LiveConfig::default().as_dict(), &env)
            .unwrap()
            .is_some()
    );
}

#[test]
fn every_side_call_starts_its_cli_through_program() {
    let root = TempDir::new("core-side-program");
    let env = environment(root.path());
    let mut runtime = fixture(|_| output(""));
    runtime.wrapped = true;
    let mut runner = RoutedRunner::new(
        State::default(),
        Context::new(env, root.path().join("te")),
        runtime,
    );
    let mut config = LiveConfig::default().as_dict();
    config["droid_sidecall"] = json!(true);
    let prompt = "Say \"hi & exit\n100% of %PATH%";
    for (host, cli) in [
        ("codex", "codex"),
        ("copilot", "copilot"),
        ("opencode", "opencode"),
        ("pi", "pi"),
        ("cursor", "cursor-agent"),
        ("droid", "droid"),
    ] {
        runner.runtime.calls.clear();
        assert!(runner.select(host, &config, None, true).unwrap());
        runner.run_raw(prompt, "").unwrap();
        let calls = &runner.runtime.calls;
        assert!(!calls.is_empty(), "{host}");
        assert!(
            calls
                .iter()
                .all(|c| c.argv[..2] == ["/n/node", &format!("/n/{cli}.js")]),
            "{host}"
        );
        assert!(
            calls.iter().any(|c| c.argv.iter().any(|a| a == prompt)),
            "{host}"
        );
    }
}

#[test]
fn sidecalls_ignore_cleanup_errors() {
    let root = TempDir::new("core-side-cleanup");
    let env = environment(root.path());
    let runtime = fixture(|call| {
        output(if call.argv[1..] == ["--help"] {
            "  -p".into()
        } else {
            match call.argv[0].as_str() {
            "codex" => json!({"type":"item.completed","item":{"type":"agent_message","text":"OK"}}),
            "copilot" => json!({"type":"assistant.message","data":{"content":"OK"}}),
            "opencode" => json!({"type":"text","part":{"text":"OK"}}),
            "cursor-agent" => json!({"result":"OK"}),
            "pi" => json!({"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"OK"}]}}),
            _ => json!({"result":"OK","usage":{"input_tokens":10,"output_tokens":1},"modelUsage":{"claude-haiku-4-5":{}}}),
        }.to_string()
        })
    });
    let mut runner = RoutedRunner::new(
        State::default(),
        Context::new(env, root.path().join("te")),
        runtime,
    );
    for host in ["claude", "codex", "copilot", "opencode", "cursor", "pi"] {
        runner.runtime.cleanup.clear();
        assert!(
            runner
                .select(host, &LiveConfig::default().as_dict(), None, true)
                .unwrap()
        );
        runner.run_raw("prompt", "").unwrap();
        assert!(!runner.runtime.cleanup.is_empty(), "{host}");
        assert!(
            runner.runtime.cleanup.iter().all(|ignore| *ignore),
            "{host}"
        );
    }
}

#[test]
fn the_side_calls_skip_json_lines_that_are_not_objects() {
    use tokeneconomizer::plugin::{cli, probe};
    let root = TempDir::new("core-side-nonobjects");
    let env = environment(root.path());
    write_copilot_events(
        &root.path().join("cp/session-state/side1/events.jsonl"),
        &[],
        true,
    );
    let noisy = |codex| {
        let mut events = vec![Value::Null, json!([1, 2]), json!("text"), json!(42)];
        events.extend(if codex {
            vec![
                json!({"type":"item.completed","item":"x"}),
                json!({"type":"item.completed","item":{"type":"agent_message","text":"hi"}}),
                json!({"type":"turn.completed","usage":[1]}),
            ]
        } else {
            vec![
                json!({"type":"assistant.message","data":[1,2]}),
                json!({"type":"assistant.message","data":{"content":"- use python -m pytest"}}),
                json!({"type":"result","sessionId":"side1","exitCode":0}),
            ]
        });
        events
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    };
    let mut runtime = fixture(|call| output(noisy(call.argv[0].contains("codex"))));
    assert_eq!(
        sidecall::codex_call(&request("codex", "say hi"), &env, root.path(), &mut runtime)
            .unwrap()
            .0,
        "hi"
    );
    let (text, eff) = sidecall::copilot_call(
        &request("copilot", "propose facts"),
        &env,
        root.path(),
        &mut runtime,
    )
    .unwrap();
    assert_eq!(text, "- use python -m pytest");
    assert!(eff.is_some_and(|e| e > 0.0));
    struct Probe(String);
    impl probe::Runtime for Probe {
        fn say(&mut self, _text: &str) -> cli::Result<()> {
            panic!("Unexpected probe output")
        }
        fn call(&mut self, method: &str, _args: Value) -> cli::Result<Value> {
            Ok(match method {
                "program" => json!(["copilot"]),
                "process" => json!({"stdout":self.0,"stderr":""}),
                _ => panic!("Unexpected probe call: {method}"),
            })
        }
    }
    let report = probe::run_session(
        root.path().to_str().unwrap(),
        &json!({}),
        root.path().to_str().unwrap(),
        &json!(""),
        "copilot",
        &json!(600),
        &mut Probe(noisy(false)),
    )
    .unwrap();
    assert_eq!(report[0], "side1");
    assert_eq!(report[1], "- use python -m pytest");
}
