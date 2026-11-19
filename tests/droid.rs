// Converted from reference/upstream/tests/test_droid.py.
mod support;

use serde_json::{Map, Value, json};
use std::{
    collections::BTreeSet,
    fs::{self, File, FileTimes},
    io::{self, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};
use support::TempDir;
use tokeneconomizer::{
    accounting, package,
    plugin::{
        cli::{self, Runtime as _},
        config::{LiveConfig, load_config, write_config},
        context::LiveContext,
        denylist, history,
        hosts::{
            self,
            session::{self, Environment},
        },
        process,
        runtime::NativeRuntime,
        sidecall::{self, Call, runner},
    },
    trace::{calls, droid},
};

const FAIL: &str =
    "python: can't open file 'tests/runtests.py': [Errno 2] No such file or directory";
const OK: &str = "Ran 3 tests in 0.01s\nOK";
fn test_calls() -> Vec<Value> {
    vec![
        json!(["Execute",{"command":"python tests/runtests.py"},FAIL,true]),
        json!(["Execute",{"command":"python runtests.py"},OK,false]),
    ]
}
fn usage() -> Value {
    json!({"inputTokens":1200,"outputTokens":400,"cacheCreationTokens":3000,"cacheReadTokens":9000,"thinkingTokens":20,"factoryCredits":700})
}
struct Options<'a> {
    parent: Option<&'a str>,
    age: f64,
    usage: Option<Value>,
    model: &'a str,
}
impl Default for Options<'_> {
    fn default() -> Self {
        Self {
            parent: None,
            age: 7200.0,
            usage: None,
            model: "claude-sonnet-5",
        }
    }
}
fn write_session(
    root: &Path,
    sid: &str,
    cwd: &Path,
    calls: &[Value],
    options: Options<'_>,
) -> PathBuf {
    let folder = root.join("ws");
    fs::create_dir_all(&folder).unwrap();
    let path = folder.join(format!("{sid}.jsonl"));
    let mut header = json!({"type":"session_start","id":sid,"cwd":cwd,"version":2});
    if let Some(parent) = options.parent {
        header["callingSessionId"] = json!(parent);
    }
    let mut events = vec![
        header,
        json!({"type":"message","timestamp":"2026-01-01T00:00:00.000Z","message":{"role":"user","content":[{"type":"text","text":"run the tests"}]}}),
    ];
    for (i, call) in calls.iter().enumerate() {
        events.push(json!({"type":"message","message":{"role":"assistant","modelId":options.model,"content":[{"type":"tool_use","id":format!("t{i}"),"name":call[0],"input":call[1]}]}}));
        events.push(json!({"type":"message","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":format!("t{i}"),"is_error":call[3],"content":call[2]}]}}));
    }
    fs::write(
        &path,
        format!(
            "{}\n",
            events
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        ),
    )
    .unwrap();
    let old = SystemTime::now() - Duration::from_secs_f64(options.age);
    File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(FileTimes::new().set_accessed(old).set_modified(old))
        .unwrap();
    if let Some(usage) = options.usage {
        let settings = path.with_extension("settings.json");
        fs::write(
            &settings,
            json!({"model":options.model,"tokenUsage":usage}).to_string(),
        )
        .unwrap();
        File::options()
            .write(true)
            .open(settings)
            .unwrap()
            .set_times(FileTimes::new().set_accessed(old).set_modified(old))
            .unwrap();
    }
    path
}
fn environment(root: &Path) -> Environment {
    Environment {env:json!({"TOKENECONOMIZER_HOME":root.join("state"),"TE_FOLD_SYNC":"1","TE_MICROSKILLS":"envfacts","TE_TOOLPRUNE":"off","DROID_SESSIONS_DIR":root.join("sessions")}).as_object().unwrap().clone(),home:root.into(),cwd:root.into()}
}
fn context(root: &Path, repo: &Path) -> (LiveContext, NativeRuntime) {
    let environment = environment(root);
    let ctx = LiveContext::build(
        "droid",
        hosts::profile("droid").keeps_history,
        repo,
        environment.env.clone(),
        root,
        Value::Null,
        true,
    )
    .unwrap();
    (
        ctx,
        NativeRuntime::new(environment, env!("CARGO_BIN_EXE_te").into()),
    )
}

#[test]
fn reader_maps_droid_names_and_skips_reminders() {
    let root = TempDir::new("droid-reader");
    let path = write_session(
        root.path(),
        "s1",
        root.path(),
        &test_calls(),
        Options::default(),
    );
    assert_eq!(droid::prompt_count(&path), 1);
    let output = root.path().join("claude.jsonl");
    assert_eq!(droid::session_to_claude(&path, &output).unwrap(), Some(2));
    let calls = calls::load_trajectory(&output);
    assert_eq!(
        calls
            .iter()
            .map(|c| c.0.as_str().unwrap())
            .collect::<Vec<_>>(),
        ["Bash", "Bash"]
    );
    assert_eq!(calls[0].1["command"], "python tests/runtests.py");
    assert_eq!(calls[0].3, false);
    assert_eq!(calls[1].3, true);
    assert_eq!(
        calls::call_cwds(&output),
        vec![Some(root.path().to_string_lossy().into_owned()); 2]
    );
}

#[test]
fn started_is_epoch_seconds_in_the_head_and_in_the_row() {
    let root = TempDir::new("droid-started");
    let sessions = root.path().join("sessions");
    let path = write_session(
        &sessions,
        "s1",
        root.path(),
        &test_calls(),
        Options::default(),
    );
    let head = droid::log_head(&path);
    assert_eq!(head["started"].as_f64(), Some(1767225600.0));
    assert_eq!(droid::sessions(&sessions)[0]["started"], head["started"]);
}

#[test]
fn prompt_count_ignores_hidden_and_reminder_messages() {
    let root = TempDir::new("droid-prompts");
    let path = write_session(root.path(), "s2", root.path(), &[], Options::default());
    let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
    for message in [
        json!({"role":"user","visibility":"llm_only","content":[{"type":"text","text":"a reminder"}]}),
        json!({"role":"user","content":[{"type":"text","text":"<system-reminder>meta"}]}),
        json!({"role":"user","content":[{"type":"text","text":"a real prompt"}]}),
    ] {
        writeln!(file, "{}", json!({"type":"message","message":message})).unwrap();
    }
    drop(file);
    assert_eq!(droid::prompt_count(&path), 2);
}

#[test]
fn host_tool_mapping_and_usage() {
    assert!(hosts::HOSTS.contains(&"droid"));
    let (name,args,_,code) = hosts::tool_call("droid",&json!({"tool_name":"Execute","tool_input":{"command":"pytest -q"},"tool_response":{"output":"ok","exitCode":0}})).unwrap();
    assert_eq!(name, "bash");
    assert_eq!(args["command"], "pytest -q");
    assert_eq!(code, 0);
    let (name,args,_,_) = hosts::tool_call("droid",&json!({"tool_name":"Create","tool_input":{"file_path":"/tmp/x"},"tool_response":"written"})).unwrap();
    assert_eq!(name, "create");
    assert_eq!(args["path"], "/tmp/x");
    assert_eq!(hosts::tool_call("droid",&json!({"tool_name":"Edit","tool_input":{"file_path":"/tmp/x"},"tool_response":"edited"})).unwrap().0,"edit");
    let usage = session::session_usage("droid", Path::new("/nonexistent")).unwrap();
    assert!(usage["input_tokens"].is_null());
    assert_eq!(accounting::effective(&usage).unwrap(), (None, "no_usage"));
}

#[test]
fn usage_comes_from_the_settings_file_beside_the_log() {
    let root = TempDir::new("droid-usage");
    let path = write_session(
        root.path(),
        "s1",
        root.path(),
        &test_calls(),
        Options {
            usage: Some(usage()),
            ..Default::default()
        },
    );
    let usage = droid::session_usage(&path);
    for (key, expected) in [
        ("input_tokens", 1200.0),
        ("cache_read_input_tokens", 9000.0),
        ("cache_creation_input_tokens", 3000.0),
        ("output_tokens", 400.0),
        ("requests", 2.0),
        ("factory_credits", 700.0),
    ] {
        assert_eq!(usage[key].as_f64(), Some(expected), "{key}");
    }
    assert_eq!(usage["model"], "claude-sonnet-5");
    let (effective, priced_by) = accounting::effective(&usage).unwrap();
    assert_eq!(priced_by, "model");
    assert!(effective.is_some_and(|n| n > 0.0));
}

#[test]
fn an_unpriced_model_is_labelled_fallback() {
    let root = TempDir::new("droid-fallback");
    let path = write_session(
        root.path(),
        "s1",
        root.path(),
        &test_calls(),
        Options {
            usage: Some(usage()),
            model: "custom:cline-sub:glm-5-3-flash",
            ..Default::default()
        },
    );
    let (effective, priced_by) = accounting::effective(&droid::session_usage(&path)).unwrap();
    assert_eq!(priced_by, "fallback");
    assert!(effective.is_some_and(|n| n > 0.0));
}

#[test]
fn detect_finds_droid_from_the_session_path() {
    let root = TempDir::new("droid-detect");
    let registered = hosts::HOSTS.map(str::to_owned);
    assert_eq!(
        hosts::detect(
            &json!({"transcript_path":root.path().join(".factory/sessions/ws/s.jsonl")}),
            &json!({}),
            None,
            &registered
        ),
        "droid"
    );
    assert_eq!(
        hosts::detect(
            &json!({}),
            &json!({"DROID_PLUGIN_ROOT":"/x"}),
            None,
            &registered
        ),
        "droid"
    );
}

#[test]
fn a_droid_shell_is_about_droid() {
    assert_eq!(
        hosts::default_host(
            &json!({"FACTORY_SESSION_ID":"s"}),
            None,
            &hosts::HOSTS.map(str::to_owned)
        ),
        "droid"
    );
}

#[test]
fn tool_pruning_is_not_supported_for_droid() {
    assert!(!denylist::supports("droid"));
}

#[test]
fn side_calls_are_off_unless_the_person_enables_them() {
    let root = TempDir::new("droid-opt-in");
    let mut env = environment(root.path());
    assert!(!sidecall::droid_enabled(&Map::new()));
    assert!(sidecall::droid_enabled(
        json!({"TE_DROID_SIDECALL":"1"}).as_object().unwrap()
    ));
    let config = load_config(root.path(), None, &Map::new()).as_dict();
    assert!(runner::host_call("droid", &config, &env).unwrap().is_none());
    env.env.insert("TE_DROID_SIDECALL".into(), json!("1"));
    assert!(runner::host_call("droid", &config, &env).unwrap().is_some());
}

#[test]
fn the_setting_turns_side_calls_on_without_a_variable() {
    let root = TempDir::new("droid-setting");
    write_config(
        &root.path().join("config.json"),
        json!({"droid_sidecall":"on"}).as_object().unwrap(),
    )
    .unwrap();
    let config = load_config(root.path(), None, &Map::new());
    assert_eq!(config.values["droid_sidecall"], true);
    assert!(
        runner::host_call("droid", &config.as_dict(), &environment(root.path()))
            .unwrap()
            .is_some()
    );
}

#[test]
fn the_setting_is_off_by_default_and_shown_by_config_show() {
    let config = LiveConfig::default().as_dict();
    assert_eq!(config["droid_sidecall"], false);
    assert_eq!(
        cli::config_view(&config).unwrap()["droid"],
        json!({"state":"off","droid_sidecall":false})
    );
}

#[derive(Default)]
struct SideCall {
    argv: Vec<String>,
    env: Map<String, Value>,
    input: Option<String>,
    cwd: PathBuf,
}
impl sidecall::Runtime for SideCall {
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
        cwd: &Path,
        env: &Map<String, Value>,
        input: Option<&str>,
        _timeout: f64,
    ) -> io::Result<process::Output> {
        self.argv = argv.to_vec();
        self.cwd = cwd.into();
        self.env = env.clone();
        self.input = input.map(str::to_owned);
        Ok(process::Output {code:0,stdout:json!({"type":"result","result":"- use python -m pytest","session_id":"s1","usage":{"input_tokens":900,"output_tokens":40}}).to_string(),stderr:String::new()})
    }
}

#[test]
fn a_droid_side_call_allows_only_the_read_tool() {
    let root = TempDir::new("droid-sidecall");
    let mut runtime = SideCall::default();
    let (text, eff) = sidecall::droid_call(
        &Call {
            prompt: "propose facts",
            model: "claude-sonnet-5",
            cli: "droid",
            timeout: 180.0,
            effort: "",
            windows: cfg!(windows),
        },
        &environment(root.path()),
        root.path(),
        &mut runtime,
    )
    .unwrap();
    assert_eq!(text, "- use python -m pytest");
    assert!(eff.is_some_and(|n| n > 0.0));
    assert_eq!(runtime.argv[..2], ["droid", "exec"]);
    assert_eq!(runtime.argv.last().unwrap(), "propose facts");
    let after =
        |flag| runtime.argv[runtime.argv.iter().position(|a| a == flag).unwrap() + 1].as_str();
    assert_eq!(after("--only-tools"), sidecall::DROID_TOOLS);
    assert_eq!(sidecall::DROID_TOOLS, "Read");
    assert!(
        !runtime
            .argv
            .iter()
            .any(|a| a == "--auto" || a == "--skip-permissions-unsafe")
    );
    assert_eq!(after("--model"), "claude-sonnet-5");
    assert_eq!(runtime.env["TE_SIDECALL"], "1");
    assert!(runtime.input.is_none());
    assert_ne!(runtime.cwd, std::env::current_dir().unwrap());
}

#[test]
fn the_settings_usage_is_normalized_for_pricing() {
    let usage = sidecall::droid_usage(
        &json!({"inputTokens":5,"outputTokens":7,"cacheReadTokens":11,"cacheCreationTokens":3}),
    );
    for (key, n) in [
        ("input_tokens", 5),
        ("output_tokens", 7),
        ("cache_read_input_tokens", 11),
        ("cache_creation_input_tokens", 3),
    ] {
        assert_eq!(usage[key], n);
    }
    assert!(sidecall::droid_usage(&json!({"outputTokens":7})).is_null());
}

#[test]
fn history_lists_droid_sessions_and_leaves_subagents_out() {
    let root = TempDir::new("droid-history-list");
    let repo = root.path().join("repo");
    fs::create_dir(&repo).unwrap();
    let sessions = root.path().join("sessions");
    write_session(&sessions, "old", &repo, &test_calls(), Options::default());
    write_session(
        &sessions,
        "sub",
        &repo,
        &test_calls(),
        Options {
            parent: Some("old"),
            ..Default::default()
        },
    );
    let env = environment(root.path());
    let rows = session::history("droid", &env.env, &[], &env).unwrap();
    assert_eq!(
        rows.iter()
            .map(|r| r["session_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["old"]
    );
    assert_eq!(rows[0]["folder"], "ws");
}

#[test]
fn history_import_learns_facts_and_prices_the_sessions() {
    let root = TempDir::new("droid-history-import");
    let repo = root.path().join("repo");
    fs::create_dir(&repo).unwrap();
    fs::write(repo.join("runtests.py"), "print('OK')\n").unwrap();
    let sessions = root.path().join("sessions");
    for i in 0..5 {
        write_session(
            &sessions,
            &format!("s{i}"),
            &repo,
            &test_calls(),
            Options {
                age: 7200.0 + i as f64 * 60.0,
                usage: Some(usage()),
                ..Default::default()
            },
        );
    }
    let (ctx, mut runtime) = context(root.path(), &repo);
    assert_eq!(
        runtime.call(&ctx, "history_plan", json!([])).unwrap()["sessions"]
            .as_array()
            .unwrap()
            .len(),
        5
    );
    let result = runtime.call(&ctx, "history_run", json!([false])).unwrap();
    assert_eq!(result["state"], "done");
    assert_eq!(result["imported"], 5);
    assert!(
        history::describe(&history::summary(&ctx).unwrap(), "droid")
            .as_str()
            .unwrap()
            .starts_with("read 5 earlier session(s) from Droid's history")
    );
    let injected = ctx.store.read("injection.json", Value::Null).unwrap();
    assert_eq!(injected["profile"], "envfacts");
    assert_eq!(injected["sessions"], 5);
    assert!(
        injected["rendered"]
            .as_str()
            .unwrap()
            .contains("`python runtests.py")
    );
    let rows: Vec<_> = ctx
        .store
        .read_lines("ledger.jsonl", None)
        .unwrap()
        .into_iter()
        .filter(|r| r["kind"] == "session")
        .collect();
    assert_eq!(rows.len(), 5);
    assert!(
        rows.iter()
            .all(|r| r["host"] == "droid" && r["prompts"].as_f64() == Some(1.0))
    );
    assert!(
        rows.iter()
            .all(|r| r["effective_tokens"].as_f64().is_some_and(|n| n != 0.0)
                && r["priced_by"] == "model")
    );
}

#[test]
fn history_plan_skips_subagents_and_sessions_still_being_written() {
    let root = TempDir::new("droid-history-plan");
    let repo = root.path().join("repo");
    let other = root.path().join("other");
    fs::create_dir(&repo).unwrap();
    fs::create_dir(&other).unwrap();
    let sessions = root.path().join("sessions");
    write_session(&sessions, "mine", &repo, &test_calls(), Options::default());
    write_session(
        &sessions,
        "sub",
        &repo,
        &test_calls(),
        Options {
            parent: Some("mine"),
            ..Default::default()
        },
    );
    write_session(
        &sessions,
        "live",
        &repo,
        &test_calls(),
        Options {
            age: 0.0,
            ..Default::default()
        },
    );
    write_session(
        &sessions,
        "elsewhere",
        &other,
        &test_calls(),
        Options::default(),
    );
    let (ctx, mut runtime) = context(root.path(), &repo);
    let plan = runtime.call(&ctx, "history_plan", json!([])).unwrap();
    assert_eq!(
        plan["sessions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["mine"]
    );
    assert_eq!(plan["skipped"], json!({"recent":1}));
}

#[test]
fn the_session_start_hook_returns_the_learned_fact() {
    let root = TempDir::new("droid-hook-history");
    let repo = root.path().join("repo");
    fs::create_dir(&repo).unwrap();
    fs::write(repo.join("runtests.py"), "print('OK')\n").unwrap();
    let sessions = root.path().join("sessions");
    for i in 0..5 {
        write_session(
            &sessions,
            &format!("s{i}"),
            &repo,
            &test_calls(),
            Options {
                age: 7200.0 + i as f64 * 60.0,
                ..Default::default()
            },
        );
    }
    let (ctx, mut runtime) = context(root.path(), &repo);
    assert_eq!(
        runtime.call(&ctx, "history_run", json!([false])).unwrap()["imported"],
        5
    );
    let payload = json!({"session_id":"s5","transcript_path":sessions.join("ws/s5.jsonl"),"cwd":repo,"hook_event_name":"SessionStart","source":"startup"});
    let output = runtime
        .run_hook(
            &["--host".into(), "droid".into()],
            Ok(payload.to_string().into_bytes()),
        )
        .unwrap();
    let out: Value = serde_json::from_str(output.lines().next_back().unwrap()).unwrap();
    assert!(
        out["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .contains("`python runtests.py")
    );
}

#[test]
fn droid_plugin_packaging() {
    let root = TempDir::new("droid-package");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("reference/upstream");
    let read = |path: &Path| serde_json::from_slice::<Value>(&fs::read(path).unwrap()).unwrap();
    assert_eq!(
        read(&source.join("plugin/.factory-plugin/plugin.json"))["name"],
        "tokeneconomizer"
    );
    let hooks = read(&source.join("plugin/hooks/droid-hooks.json"));
    assert!(
        hooks["SessionStart"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .ends_with("--host droid")
    );
    assert!(hooks.get("PostToolUseFailure").is_none());
    let output = root.path().join("marketplace");
    let binaries = Path::new(env!("CARGO_BIN_EXE_te")).parent().unwrap();
    let plugin = package::build(&source, Some(binaries), &output, true).unwrap();
    let hooks = read(&plugin.join("hooks/hooks.json"));
    assert_eq!(
        hooks
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        [
            "SessionStart",
            "UserPromptSubmit",
            "PostToolUse",
            "Stop",
            "SessionEnd"
        ]
        .into()
    );
    for name in [
        "config",
        "facts",
        "microskills",
        "repos",
        "status",
        "toolprune",
        "untool",
    ] {
        let command =
            fs::read_to_string(plugin.join(format!("commands/tokeneconomizer-{name}.md"))).unwrap();
        assert!(command.contains("${DROID_PLUGIN_ROOT}") && command.contains("Execute tool"));
        assert!(!command.contains("Bash tool"));
    }
    assert!(
        plugin
            .join(format!("scripts/te-hook{}", std::env::consts::EXE_SUFFIX))
            .is_file()
    );
    assert!(!plugin.join("hooks.json").exists());
    assert!(!plugin.join(".antigravity-plugin").exists());
    assert_eq!(
        read(&output.join(".factory-plugin/marketplace.json"))["plugins"][0]["name"],
        "tokeneconomizer"
    );
}
