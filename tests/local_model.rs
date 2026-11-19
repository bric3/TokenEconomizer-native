// Converted from reference/upstream/tests/test_local_model.py.
mod support;

use serde_json::{Map, Value, json};
use std::{
    cell::{Cell, RefCell},
    io,
    path::{Path, PathBuf},
    rc::Rc,
};
use support::TempDir;
use tokeneconomizer::{
    loopguard::judge as loop_judge,
    plugin::{
        config::{LiveConfig, load_config},
        context::LiveContext,
        controller::{self, JudgeMode, LiveController},
        hosts::session::{self, Environment},
        judge, process,
        sidecall::{
            self, Call, Failure,
            runner::{self, Context, RoutedRunner, State},
        },
    },
    runner::Runner,
    store::Store,
};

fn cfg(settings: Value) -> Value {
    let mut config = LiveConfig::default();
    for (name, value) in settings.as_object().unwrap() {
        assert!(
            config.set(name, value.clone(), "test"),
            "{:?}",
            config.warnings
        );
    }
    config.as_dict()
}
fn environment(root: &Path) -> Environment {
    Environment {
        env: Map::new(),
        home: root.into(),
        cwd: root.into(),
    }
}
#[derive(Clone)]
struct Fake {
    available: bool,
    probes: Rc<Cell<usize>>,
    local: Value,
    nudge: Value,
    host: Value,
    tokens: f64,
    calls: Rc<RefCell<Vec<(String, Value)>>>,
}
impl Default for Fake {
    fn default() -> Self {
        let verdict = json!(r#"{"stuck": true, "confidence": 0.9, "reason": "same failure"}"#);
        Self {
            available: true,
            probes: Rc::default(),
            local: verdict.clone(),
            nudge: verdict,
            host: json!("h"),
            tokens: 9.0,
            calls: Rc::default(),
        }
    }
}
impl sidecall::Runtime for Fake {
    fn override_available(&mut self) -> Result<Option<bool>, String> {
        self.probes.set(self.probes.get() + 1);
        Ok(Some(self.available))
    }
    fn override_call(
        &mut self,
        method: &str,
        args: &Value,
    ) -> Result<Option<(Value, Option<f64>)>, Failure> {
        self.calls.borrow_mut().push((method.into(), args.clone()));
        Ok(Some(if method == "fm_call" {
            (
                if args["kwargs"]["schema"].is_null() {
                    self.nudge.clone()
                } else {
                    self.local.clone()
                },
                Some(0.0),
            )
        } else {
            assert_eq!(method, "claude_call");
            (self.host.clone(), Some(self.tokens))
        }))
    }
    fn run(
        &mut self,
        _argv: &[String],
        _cwd: &Path,
        _env: &Map<String, Value>,
        _input: Option<&str>,
        _timeout: f64,
    ) -> io::Result<process::Output> {
        panic!("The routing fixture must never start a process")
    }
}
fn routed(root: &Path, fake: Fake) -> RoutedRunner<Fake> {
    let mut context = Context::new(environment(root), root.into());
    context.macos = true;
    RoutedRunner::new(State::default(), context, fake)
}

#[test]
fn local_model_is_off_by_default_and_names_the_judge() {
    let config = LiveConfig::default();
    assert_eq!(config.values["local_model"], "off");
    assert_eq!(config.values["local_roles"], json!(["judge"]));
}

#[test]
fn local_settings_parse_and_reject_unknown_values() {
    let mut c = LiveConfig::default();
    assert!(c.set("local_model", json!("fm"), "test"));
    assert_eq!(c.values["local_model"], "fm");
    assert!(c.set("local_roles", json!("judge, nudge"), "test"));
    assert_eq!(c.values["local_roles"], json!(["judge", "nudge"]));
    assert!(!c.set("local_model", json!("ollama"), "test"));
    assert_eq!(c.values["local_model"], "fm");
    assert!(!c.set("local_roles", json!("judge,summarizer"), "test"));
    assert_eq!(c.values["local_roles"], json!(["judge", "nudge"]));
    assert_eq!(c.warnings.len(), 2);
}

#[test]
fn local_settings_come_from_the_environment() {
    let root = TempDir::new("local-env");
    let c = load_config(
        root.path(),
        None,
        json!({"TE_LOCAL_MODEL":"fm","TE_LOCAL_ROLES":"judge,proposer"})
            .as_object()
            .unwrap(),
    );
    assert_eq!(c.values["local_model"], "fm");
    assert_eq!(c.values["local_roles"], json!(["judge", "proposer"]));
}

#[test]
fn no_local_runner_unless_turned_on_and_the_role_listed() {
    let root = TempDir::new("local-roles");
    let mut runner = routed(root.path(), Fake::default());
    assert!(!runner.select_local("judge", &cfg(json!({}))).unwrap());
    assert!(
        !runner
            .select_local("nudge", &cfg(json!({"local_model":"fm"})))
            .unwrap()
    );
    assert!(
        runner
            .select_local("judge", &cfg(json!({"local_model":"fm"})))
            .unwrap()
    );
}

#[test]
fn local_routes_reads_only_the_settings_and_the_platform() {
    assert!(sidecall::local_routes(
        &json!("judge"),
        &cfg(json!({"local_model":"fm"})),
        true
    ));
    assert!(!sidecall::local_routes(
        &json!("nudge"),
        &cfg(json!({"local_model":"fm"})),
        true
    ));
    assert!(!sidecall::local_routes(
        &json!("judge"),
        &cfg(json!({})),
        true
    ));
    assert!(!sidecall::local_routes(
        &json!("judge"),
        &cfg(json!({"local_model":"fm"})),
        false
    ));
}

struct Job {
    config: Value,
    root: PathBuf,
    fake: Fake,
    paused: bool,
    appended: Vec<Value>,
}
impl judge::Runtime for Job {
    fn paused(&mut self) -> Result<bool, String> {
        Ok(self.paused)
    }
    fn runner(&mut self, role: &str, local_only: bool) -> Result<Option<Box<dyn Runner>>, String> {
        let mut runner = routed(&self.root, self.fake.clone());
        let selected = if local_only {
            runner.select_local(role, &self.config)?
        } else {
            runner.select("claude", &self.config, Some(role), false)?
        };
        Ok(selected.then(|| Box::new(runner) as Box<dyn Runner>))
    }
    fn append(&mut self, store: &Store, name: &str, value: &Value) -> Result<(), String> {
        self.appended.push(value.clone());
        store.append(name, value).map_err(|e| e.to_string())
    }
    fn spawn_job(&mut self, _sid: &str, _id: &str) -> Result<(), String> {
        panic!("Unexpected judge spawn")
    }
    fn spawn(&mut self, _argv: &[String], _log: &str) -> Result<Value, String> {
        panic!("Unexpected process spawn")
    }
}
impl session::Runtime for Job {
    fn program(&mut self, _cli: &str) -> session::Result<Vec<String>> {
        panic!("Unexpected program lookup")
    }
    fn ancestors(
        &mut self,
        _limit: i64,
        _pid: u32,
        _windows: bool,
        _cwd: &Path,
    ) -> session::Result<Vec<String>> {
        panic!("Unexpected ancestor lookup")
    }
}
impl controller::Runtime for Job {
    fn local_routes(&mut self) -> Result<bool, String> {
        Ok(sidecall::local_routes(&json!("judge"), &self.config, true))
    }
    fn dispatch(&mut self, _ctl: &mut LiveController, _method: &str) -> Result<Value, String> {
        panic!("Unexpected dispatch")
    }
    fn launcher(&mut self, _ctx: &LiveContext) -> io::Result<()> {
        panic!("Unexpected launcher")
    }
    fn paused_detail(&mut self) -> Result<Value, String> {
        panic!("Unexpected pause detail")
    }
    fn call_name(&mut self) -> Result<String, String> {
        panic!("Unexpected call name")
    }
    fn map(&mut self, _ctx: &LiveContext, _method: &str, _args: Value) -> Result<Value, String> {
        panic!("Unexpected map")
    }
    fn history_notice(&mut self, _ctx: &LiveContext) -> Result<Value, String> {
        panic!("Unexpected history notice")
    }
    fn history_start(&mut self, _ctx: &LiveContext) -> Result<(), String> {
        panic!("Unexpected history start")
    }
    fn migration_start(&mut self, _ctx: &LiveContext) -> Result<(), String> {
        panic!("Unexpected migration")
    }
    fn pending_sessions(
        &mut self,
        _ctx: &LiveContext,
        _exclude: &[String],
    ) -> Result<Vec<String>, String> {
        panic!("Unexpected pending lookup")
    }
    fn spawn_folds(&mut self, _ctx: &LiveContext, _sessions: &[String]) -> Result<(), String> {
        panic!("Unexpected fold spawn")
    }
}

#[test]
fn a_paused_hosts_hook_picks_its_judge_from_the_settings_without_probing_fm() {
    let root = TempDir::new("local-paused-judge");
    for local in [true, false] {
        let fake = Fake::default();
        let mut job = Job {
            config: cfg(json!({"local_model":if local {"fm"} else {"off"}})),
            root: root.path().into(),
            fake: fake.clone(),
            paused: true,
            appended: vec![],
        };
        let mode = controller::judge_mode("claude", &json!({}), &mut job).unwrap();
        assert!(if local {
            matches!(mode, JudgeMode::Async)
        } else {
            matches!(mode, JudgeMode::NoVerdict)
        });
        assert_eq!(fake.probes.get(), 0);
        assert!(fake.calls.borrow().is_empty());
    }
}

#[test]
fn no_local_runner_off_macos() {
    let root = TempDir::new("local-not-macos");
    let mut runner = routed(root.path(), Fake::default());
    runner.context.macos = false;
    assert!(
        !runner
            .select_local("judge", &cfg(json!({"local_model":"fm"})))
            .unwrap()
    );
}

#[test]
fn no_local_runner_without_fm() {
    let root = TempDir::new("local-missing");
    let mut runner = routed(
        root.path(),
        Fake {
            available: false,
            ..Default::default()
        },
    );
    assert!(
        !runner
            .select_local("judge", &cfg(json!({"local_model":"fm"})))
            .unwrap()
    );
}

#[test]
fn runner_for_a_local_role_uses_fm_with_its_schema_and_charges_nothing() {
    let root = TempDir::new("local-schema-route");
    let fake = Fake::default();
    let mut runner = routed(root.path(), fake.clone());
    assert!(
        runner
            .select(
                "claude",
                &cfg(json!({"local_model":"fm"})),
                Some("judge"),
                false
            )
            .unwrap()
    );
    let (text, eff) = runner.run_raw("digest", "agent").unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(text.as_str().unwrap()).unwrap()["stuck"],
        true
    );
    assert_eq!(eff, Some(0.0));
    let calls = fake.calls.borrow();
    assert_eq!(calls.len(), 1);
    assert_eq!(
        calls[0],
        (
            "fm_call".into(),
            json!({"prompt":"digest","kwargs":{"schema":runner::judge_schema()}})
        )
    );
}

#[test]
fn runner_for_without_a_role_or_for_another_role_stays_on_the_host() {
    let root = TempDir::new("local-host-roles");
    let fake = Fake::default();
    let mut runner = routed(root.path(), fake.clone());
    for role in [None, Some("proposer")] {
        assert!(
            runner
                .select("claude", &cfg(json!({"local_model":"fm"})), role, false)
                .unwrap()
        );
        assert_eq!(runner.run_raw("q", "").unwrap(), (json!("h"), Some(9.0)));
    }
    assert!(
        fake.calls
            .borrow()
            .iter()
            .all(|(method, _)| method == "claude_call")
    );
}

#[test]
fn a_failed_local_call_falls_back_to_the_agents_cli() {
    let root = TempDir::new("local-host-fallback");
    let mut runner = routed(
        root.path(),
        Fake {
            local: Value::Null,
            host: json!("host verdict"),
            ..Default::default()
        },
    );
    assert!(
        runner
            .select(
                "claude",
                &cfg(json!({"local_model":"fm"})),
                Some("judge"),
                false
            )
            .unwrap()
    );
    assert_eq!(
        runner.run_raw("digest", "").unwrap(),
        (json!("host verdict"), Some(9.0))
    );
}

#[test]
fn a_host_with_no_side_calls_gets_none_from_runner_for_and_an_empty_local_fallback() {
    let root = TempDir::new("local-no-host");
    let mut runner = routed(
        root.path(),
        Fake {
            local: Value::Null,
            nudge: Value::Null,
            ..Default::default()
        },
    );
    assert!(
        !runner
            .select("antigravity", &cfg(json!({})), None, false)
            .unwrap()
    );
    assert!(
        runner
            .select(
                "antigravity",
                &cfg(json!({"local_model":"fm","local_roles":"proposer"})),
                Some("proposer"),
                false
            )
            .unwrap()
    );
    assert_eq!(runner.run_raw("q", "").unwrap(), (json!(""), Some(0.0)));
}

struct Process {
    reply: io::Result<process::Output>,
    installed: bool,
    calls: Vec<Vec<String>>,
    input: Option<String>,
    schema: Value,
}
impl sidecall::Runtime for Process {
    fn executable(&mut self, path: &Path) -> Result<bool, String> {
        Ok(self.installed && path == Path::new("/usr/bin/fm"))
    }
    fn which(&mut self, _cli: &str) -> Result<Option<String>, String> {
        panic!("fm must not use PATH")
    }
    fn run(
        &mut self,
        argv: &[String],
        _cwd: &Path,
        _env: &Map<String, Value>,
        input: Option<&str>,
        _timeout: f64,
    ) -> io::Result<process::Output> {
        self.calls.push(argv.to_vec());
        self.input = input.map(str::to_owned);
        if let Some(i) = argv.iter().position(|a| a == "--schema") {
            self.schema = serde_json::from_slice(&std::fs::read(&argv[i + 1]).unwrap()).unwrap();
        }
        std::mem::replace(
            &mut self.reply,
            Err(io::Error::other("Unexpected repeated process")),
        )
    }
}
fn process(code: i32, stdout: &str) -> Process {
    Process {
        reply: Ok(process::Output {
            code,
            stdout: stdout.into(),
            stderr: String::new(),
        }),
        installed: true,
        calls: vec![],
        input: None,
        schema: Value::Null,
    }
}
fn fm(root: &Path, prompt: &str, schema: &Value, runtime: &mut Process) -> (Value, u64) {
    sidecall::fm_call(
        &Call {
            prompt,
            model: "",
            cli: "/usr/bin/fm",
            effort: "",
            timeout: 60.0,
            windows: false,
        },
        schema,
        &environment(root),
        root,
        runtime,
    )
    .unwrap()
}

#[test]
fn fm_call_runs_greedy_with_the_schema_file_and_the_prompt_on_stdin() {
    let root = TempDir::new("local-fm-process");
    let mut runtime = process(0, "\x1b[1m{\"stuck\": false}\x1b[0m\n");
    assert_eq!(
        fm(root.path(), "PROMPT", &runner::judge_schema(), &mut runtime),
        (json!("{\"stuck\": false}"), 0)
    );
    assert_eq!(
        runtime.calls[0][..4],
        ["/usr/bin/fm", "respond", "--no-stream", "--greedy"]
    );
    assert_eq!(runtime.input.as_deref(), Some("PROMPT"));
    assert!(!runtime.calls[0].iter().any(|a| a == "PROMPT"));
    assert_eq!(
        runtime.schema["required"],
        json!(["stuck", "confidence", "reason"])
    );
}

#[test]
fn fm_call_without_a_usable_reply_returns_none_and_logs() {
    for (code, out) in [
        (1, ""),
        (0, ""),
        (
            0,
            "Error: The session's transcript exceeded the model's context size.",
        ),
    ] {
        let root = TempDir::new("local-fm-no-reply");
        assert_eq!(
            fm(
                root.path(),
                "SECRET prompt",
                &Value::Null,
                &mut process(code, out)
            ),
            (Value::Null, 0)
        );
        let log = std::fs::read_to_string(root.path().join("errors.log")).unwrap();
        assert!(log.contains("sidecall fm: no reply") && !log.contains("SECRET"));
    }
}

#[test]
fn fm_call_that_cannot_start_returns_none() {
    let root = TempDir::new("local-fm-timeout");
    let mut runtime = process(0, "");
    runtime.reply = Err(io::Error::new(io::ErrorKind::TimedOut, "timeout"));
    assert_eq!(
        fm(root.path(), "q", &Value::Null, &mut runtime),
        (Value::Null, 0)
    );
}

#[test]
fn fm_available_runs_usr_bin_fm_on_macos_only() {
    let root = TempDir::new("local-fm-available");
    for (macos, installed, code, out, ok) in [
        (true, false, 0, "System model available", false),
        (false, true, 0, "System model available", false),
        (true, true, 0, "System model available", true),
        (
            true,
            true,
            0,
            "System model unavailable: modelNotReady",
            false,
        ),
        (true, true, 1, "", false),
    ] {
        let mut runtime = process(code, out);
        runtime.installed = installed;
        assert_eq!(
            sidecall::fm_available(
                "/usr/bin/fm",
                macos,
                &mut Map::new(),
                &environment(root.path()),
                &mut runtime
            )
            .unwrap(),
            ok
        );
        assert_eq!(
            runtime.calls,
            if macos && installed {
                vec![vec!["/usr/bin/fm", "available"]]
            } else {
                vec![]
            }
        );
    }
}

#[test]
fn the_stuck_judge_parses_a_schema_ordered_local_verdict_at_zero_cost() {
    let root = TempDir::new("local-judge-parse");
    let mut runner = routed(root.path(), Fake::default());
    assert!(
        runner
            .select(
                "claude",
                &cfg(json!({"local_model":"fm"})),
                Some("judge"),
                false
            )
            .unwrap()
    );
    let verdict = loop_judge::llm_stuck_judge("turns=20 ...", Some(&mut runner), "agent");
    assert_eq!(verdict["stuck"], true);
    assert_eq!(verdict["confidence"].as_f64(), Some(0.9));
    assert_eq!(verdict["_tokens"].as_f64(), Some(0.0));
}

fn job(root: &Path, confidence: f64, floor: Option<f64>, host_answer: bool) -> (Value, Job) {
    let mut settings = json!({"local_model":"fm","local_roles":"judge,nudge"});
    if let Some(floor) = floor {
        settings["local_judge_confidence"] = json!(floor);
    }
    if host_answer {
        settings["local_roles"] = json!("judge");
    }
    let config = cfg(settings);
    let fake = if host_answer {
        Fake {
            local: Value::Null,
            host: json!(r#"{"stuck": true, "confidence": 0.75, "reason": "loop"}"#),
            tokens: 5.0,
            ..Default::default()
        }
    } else {
        Fake {
            local: json!(
                json!({"stuck":true,"confidence":confidence,"reason":"same failure"}).to_string()
            ),
            nudge: json!("Re-read the failing test before editing again."),
            ..Default::default()
        }
    };
    let mut runtime = Job {
        root: root.into(),
        config,
        fake,
        paused: false,
        appended: vec![],
    };
    let store = Store::new(root).unwrap();
    store
        .write(
            "judge/S.request.json",
            &json!({"id":"J","digest":"turns=20 ...","threshold":0.7}),
        )
        .unwrap();
    let verdict = judge::run_job(
        &store,
        "claude",
        "S",
        "J",
        runtime.config["local_judge_confidence"].as_f64().unwrap(),
        &mut runtime,
    )
    .unwrap();
    (verdict, runtime)
}

#[test]
fn a_local_stuck_verdict_below_the_floor_spares_the_turn() {
    let root = TempDir::new("local-below-floor");
    let (verdict, runtime) = job(root.path(), 0.85, None, false);
    assert_eq!(verdict["stuck"], false);
    assert_eq!(verdict["local"], "fm");
    assert!(verdict.get("_nudge").is_none());
    assert!(
        verdict["reason"]
            .as_str()
            .unwrap()
            .starts_with("below local_judge_confidence")
    );
    assert!(runtime.appended.is_empty());
}

#[test]
fn a_local_stuck_verdict_at_the_floor_redirects_with_a_local_nudge() {
    let root = TempDir::new("local-above-floor");
    let (verdict, _) = job(root.path(), 0.92, None, false);
    assert_eq!(verdict["stuck"], true);
    assert!(verdict["_nudge"].as_str().unwrap().starts_with("Re-read"));
}

#[test]
fn the_floor_is_a_setting() {
    let root = TempDir::new("local-configured-floor");
    assert_eq!(job(root.path(), 0.85, Some(0.8), false).0["stuck"], true);
}

#[test]
fn the_floor_does_not_apply_when_the_host_answered() {
    let root = TempDir::new("local-host-floor");
    let (verdict, runtime) = job(root.path(), 0.75, None, true);
    assert_eq!(verdict["stuck"], true);
    assert!(verdict.get("local").is_none());
    assert!(!runtime.appended.is_empty());
}

#[test]
fn local_judge_confidence_must_be_a_probability() {
    let mut c = LiveConfig::default();
    assert!(c.set("local_judge_confidence", json!("0.8"), "test"));
    assert_eq!(c.values["local_judge_confidence"].as_f64(), Some(0.8));
    assert!(!c.set("local_judge_confidence", json!(1.5), "test"));
    assert!(!c.set("local_judge_confidence", json!(true), "test"));
}
