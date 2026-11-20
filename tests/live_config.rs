// Converted from reference/upstream/tests/test_live_config.py.
pub mod fixtures;
#[path = "support/hook_fixture.rs"]
pub mod hook_fixture;
mod support;

use hook_fixture::HookFixture;
use serde_json::{Map, Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    slice,
};
use support::TempDir;
use tokeneconomizer::{
    envelope,
    microskills::envfacts,
    plugin::{
        config::{LiveConfig, load_config, unset_config, write_config},
        hosts::session::Environment,
        paths,
        runtime::NativeRuntime,
    },
    store::Store,
};

fn read(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn write(path: &Path, updates: Value) {
    write_config(path, updates.as_object().unwrap()).unwrap();
}

#[test]
fn each_mechanism_takes_a_plain_on_and_off() {
    for (key, on) in [
        ("microskills", "envfacts+distiller"),
        ("toolprune", "auto"),
        ("loopguard", "nudge"),
    ] {
        let mut cfg = LiveConfig::default();
        assert!(cfg.set(key, json!("off"), "test"));
        assert_eq!(cfg.values[key], "off");
        assert!(cfg.set(key, json!("on"), "test"));
        assert_eq!(cfg.values[key], on);
        assert!(cfg.set(key, json!(false), "test"));
        assert_eq!(cfg.values[key], "off");
        assert!(!cfg.set(key, json!("sometimes"), "test"));
        assert_eq!(cfg.values[key], "off");
    }
}

#[test]
fn a_guard_threshold_is_typed_and_range_checked() {
    let mut cfg = LiveConfig::default();
    assert!(cfg.set("loopguard.min_turns", json!("12"), "test"));
    assert_eq!(cfg.values["loopguard_thresholds"]["min_turns"], 12);
    assert!(cfg.set("loopguard.backstop_mult", json!("2"), "test"));
    assert_eq!(cfg.values["loopguard_thresholds"]["backstop_mult"], 2.0);
    for (key, value) in [
        ("loopguard.min_turns", "12.5"),
        ("loopguard.stagnation", "1.5"),
        ("loopguard.min_turns", "0"),
        ("loopguard.no_such_threshold", "3"),
    ] {
        assert!(!cfg.set(key, json!(value), "test"));
    }
    assert_eq!(
        cfg.values["loopguard_thresholds"],
        json!({"min_turns":12,"backstop_mult":2.0})
    );
    assert_eq!(cfg.warnings.len(), 4);
}

#[test]
fn set_and_unset_keep_thresholds_in_their_own_object() {
    let root = TempDir::new("live-config-write");
    let path = root.path().join("config.json");
    write(
        &path,
        json!({"loopguard":"off","loopguard.min_turns":"12","loopguard.edit_churn":"6"}),
    );
    assert_eq!(
        read(&path),
        json!({"loopguard":"off","loopguard_thresholds":{"edit_churn":6,"min_turns":12}})
    );
    let before = fs::read(&path).unwrap();
    assert!(
        write_config(
            &path,
            json!({"loopguard.min_turns":"-1"}).as_object().unwrap()
        )
        .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), before);
    unset_config(&path, &["loopguard.min_turns".into(), "loopguard".into()]).unwrap();
    assert_eq!(
        read(&path),
        json!({"loopguard_thresholds":{"edit_churn":6}})
    );
    unset_config(&path, &["loopguard.edit_churn".into()]).unwrap();
    assert_eq!(read(&path), json!({}));
    assert!(unset_config(&path, &["loopguard.nope".into()]).is_err());
}

#[test]
fn the_repository_file_beats_the_global_one_and_the_environment_beats_both() {
    let temp = TempDir::new("live-config-precedence");
    let root = temp.path().join("root");
    let repo = temp.path().join("repo");
    write(
        &root.join("config.json"),
        json!({"loopguard_nudges":"3","loopguard.min_turns":"30"}),
    );
    write(
        &repo.join("config.json"),
        json!({"loopguard.min_turns":"10"}),
    );
    let cfg = load_config(&root, Some(&repo), &Map::new());
    assert_eq!(cfg.values["loopguard_nudges"], 3);
    assert_eq!(cfg.values["loopguard_thresholds"], json!({"min_turns":10}));
    let env = json!({"TE_LOOPGUARD_NUDGES":"2","TE_LOOPGUARD_THRESHOLDS":"{\"min_turns\": 8, \"stagnation\": 0.5}","TE_MICROSKILLS_MAX_FACTS":"2","TE_LOOPGUARD":"off"});
    let cfg = load_config(&root, Some(&repo), env.as_object().unwrap());
    assert_eq!(cfg.values["loopguard_nudges"], 2);
    assert_eq!(cfg.values["microskills_max_facts"], 2);
    assert_eq!(cfg.values["loopguard"], "off");
    assert_eq!(
        cfg.values["loopguard_thresholds"],
        json!({"min_turns":8,"stagnation":0.5})
    );
    assert!(cfg.warnings.is_empty());
}

#[test]
fn the_names_before_the_rename_still_work() {
    let temp = TempDir::new("live-config-aliases");
    let root = temp.path().join("root");
    let repo = temp.path().join("repo");
    fs::create_dir(&root).unwrap();
    let path = root.join("config.json");
    fs::write(&path, json!({"loopguard":"off","spiral":"guard","inject_max_facts":3,"untool":"auto","propose_every":7,"spiral_aggr":"aggressive","spiral_thresholds":{"min_turns":9}}).to_string()).unwrap();
    let env = json!({"TE_SPIRAL_NUDGES":"2","TE_UNTOOL_EVIDENCE":"4","TE_INJECT":"off"});
    let cfg = load_config(&root, Some(&repo), env.as_object().unwrap());
    for (key, value) in [
        ("loopguard", json!("off")),
        ("microskills_max_facts", json!(3)),
        ("toolprune", json!("auto")),
        ("microskills_propose_every", json!(7)),
        ("loopguard_aggr", json!("aggressive")),
        ("loopguard_thresholds", json!({"min_turns":9})),
        ("loopguard_nudges", json!(2)),
        ("toolprune_evidence", json!(4)),
        ("microskills", json!("off")),
    ] {
        assert_eq!(cfg.values[key], value, "{key}");
    }
    assert!(cfg.warnings.is_empty());
    let env = json!({"TE_SPIRAL":"nudge","TE_LOOPGUARD":"stop"});
    assert_eq!(
        load_config(&root, None, env.as_object().unwrap()).values["loopguard"],
        "stop"
    );
    assert!(LiveConfig::default().set("spiral", json!("guard"), "test"));
    assert!(LiveConfig::default().set("loopguard", json!("guard"), "test"));
    write(&path, json!({"spiral":"nudge","spiral.edit_churn":"5"}));
    let written = read(&path);
    assert_eq!(written["loopguard"], "nudge");
    assert!(written.get("spiral").is_none());
    assert_eq!(
        written["loopguard_thresholds"],
        json!({"min_turns":9,"edit_churn":5})
    );
    assert!(written.get("spiral_thresholds").is_none());
    unset_config(
        &path,
        &[
            "untool".into(),
            "inject_max_facts".into(),
            "spiral.min_turns".into(),
        ],
    )
    .unwrap();
    let written = read(&path);
    for key in ["untool", "toolprune", "inject_max_facts"] {
        assert!(written.get(key).is_none());
    }
    assert_eq!(written["loopguard_thresholds"], json!({"edit_churn":5}));
}

#[test]
fn the_fact_caps_default_to_the_cores_own() {
    let cfg = LiveConfig::default();
    assert_eq!(cfg.values["microskills_max_facts"], envfacts::MAX_FACTS);
    assert_eq!(cfg.values["microskills_max_tokens"], envfacts::MAX_TOKENS);
}

#[test]
fn a_pinned_threshold_wins_over_the_preset_and_over_what_was_learned() {
    let root = TempDir::new("live-config-pinned");
    let store = Store::new(root.path()).unwrap();
    store
        .write("envelope.json", &json!({"resolved_turns":[6,7,8,8,9]}))
        .unwrap();
    let tracker = envelope::load_envelope(&store, "default", &json!({})).unwrap();
    assert_eq!(
        envelope::thresholds(&tracker, &json!({}))["min_turns"].as_f64(),
        Some(8.0)
    );
    let overrides = json!({"min_turns":30,"backstop_mult":4.0});
    let tracker = envelope::load_envelope(&store, "default", &overrides).unwrap();
    let thresholds = envelope::thresholds(&tracker, &overrides);
    assert_eq!(thresholds["min_turns"], 30);
    assert!(
        (thresholds["backstop_turns"].as_f64().unwrap() - tracker.envelope().unwrap() * 4.0).abs()
            <= 1e-6
    );
}

#[test]
fn the_envelope_settings_reach_the_tracker() {
    let root = TempDir::new("live-config-envelope");
    let store = Store::new(root.path()).unwrap();
    store
        .write("envelope.json", &json!({"resolved_turns":[6,8]}))
        .unwrap();
    assert!(
        envelope::load_envelope(&store, "default", &json!({}))
            .unwrap()
            .envelope()
            .is_none()
    );
    let tracker = envelope::load_envelope(
        &store,
        "default",
        &json!({"envelope_min_samples":2,"envelope_margin":2.0,"envelope_pct":50}),
    )
    .unwrap();
    assert!((tracker.envelope().unwrap() - 14.0).abs() <= 1e-6);
}

#[test]
fn base_thresholds_show_the_preset_with_the_pinned_values_on_top() {
    let thresholds = envelope::base_thresholds("aggressive", &json!({"min_turns":9}));
    assert_eq!(thresholds["min_turns"], 9);
    assert_eq!(thresholds["backstop_mult"], 1.8);
    assert!(thresholds.get("restart_budget").is_none());
}

fn fixture(root: &Path) -> HookFixture {
    let mut fixture = HookFixture::new(root, "claude");
    fs::write(fixture.repo.join("pyproject.toml"), "[project]\nname='w'\n").unwrap();
    fixture.environment.env.remove("TE_LOOPGUARD");
    fixture.environment.env.extend(
        json!({"TE_MICROSKILLS":"envfacts","TE_TOOLPRUNE_QUIET_MINUTES":"0"})
            .as_object()
            .unwrap()
            .clone(),
    );
    fixture.inline_judge = true;
    fixture
}

fn turn(f: &mut HookFixture, key: &str, calls: &[Value]) -> Vec<Value> {
    let base = json!({"session_id":"s","transcript_path":"","cwd":f.repo,"prompt_id":key});
    f.tool(
        &base,
        json!({"hook_event_name":"UserPromptSubmit","prompt":"go"}),
    );
    let out = calls.iter().map(|call| {
        let failed = call[2].as_object().is_some_and(|obj| obj.len() == 1 && obj.contains_key("error"));
        let mut fields = json!({"hook_event_name":if failed {"PostToolUseFailure"} else {"PostToolUse"},"tool_name":call[0],"tool_input":call[1]});
        fields[if failed { "error" } else { "tool_response" }] = if failed { call[2]["error"].clone() } else { call[2].clone() };
        f.tool(&base, fields)
    }).collect();
    f.tool(
        &base,
        json!({"hook_event_name":"Stop","stop_hook_active":false}),
    );
    out
}

fn warm(f: &mut HookFixture) {
    let calls = vec![json!(["Read",{"file_path":"/r/a.py"},"ok"]); 8];
    for i in 0..5 {
        turn(f, &format!("g{i}"), &calls);
    }
}

fn stuck_calls(count: usize) -> Vec<Value> {
    (0..count).map(|i| if i % 2 == 0 {
        json!(["Edit",{"file_path":"/r/pkg/a.py","old_string":"x","new_string":"y"},"ok"])
    } else {
        json!(["Bash",{"command":"pytest -q"},{"error":"Exit code 1\nFAILED tests/test_a.py::test_x - AssertionError: assert 1 == 2"}])
    }).collect()
}

#[test]
fn a_guard_turned_off_in_the_repository_file_never_intervenes() {
    let root = TempDir::new("live-config-guard-off");
    let mut f = fixture(root.path());
    warm(&mut f);
    write(
        &f.store.base.parent().unwrap().join("config.json"),
        json!({"spiral":"off"}),
    );
    assert!(
        turn(&mut f, "stuck", &stuck_calls(40))
            .iter()
            .all(|out| out == &json!({}))
    );
}

#[test]
fn more_redirects_are_given_before_the_notice_when_asked_for() {
    let root = TempDir::new("live-config-redirects");
    let mut f = fixture(root.path());
    warm(&mut f);
    f.environment
        .env
        .insert("TE_SPIRAL_NUDGES".into(), json!("2"));
    let out = turn(&mut f, "stuck", &stuck_calls(60));
    let messages: Vec<_> = out
        .iter()
        .map(|out| out["systemMessage"].as_str().unwrap_or(""))
        .collect();
    let nudges: Vec<_> = messages
        .iter()
        .enumerate()
        .filter_map(|(i, message)| message.contains("nudged").then_some(i))
        .collect();
    let notice = messages
        .iter()
        .position(|message| message.contains("still looks stuck"))
        .unwrap();
    assert_eq!(nudges.len(), 2);
    assert!(notice > *nudges.last().unwrap());
}

#[test]
fn a_higher_min_turns_holds_the_guard_back() {
    let root = TempDir::new("live-config-min-turns");
    let mut f = fixture(root.path());
    warm(&mut f);
    f.environment
        .env
        .insert("TE_SPIRAL_THRESHOLDS".into(), json!("{\"min_turns\": 80}"));
    assert!(
        turn(&mut f, "stuck", &stuck_calls(40))
            .iter()
            .all(|out| out == &json!({}))
    );
}

fn native_environment(f: &HookFixture) -> Environment {
    let mut environment = f.environment.clone();
    environment.env.extend(
        json!({"TE_FOLD_SYNC":"1","TE_LOOPGUARD":"off"})
            .as_object()
            .unwrap()
            .clone(),
    );
    environment
}

fn run_session(environment: Environment, repo: &Path, sid: &str, transcript: &Path) -> Value {
    let mut runtime = NativeRuntime::new(environment, env!("CARGO_BIN_EXE_te").into());
    let mut start = Value::Null;
    for event in ["SessionStart", "SessionEnd"] {
        let payload = json!({"session_id":sid,"transcript_path":transcript,"cwd":repo,"hook_event_name":event,"source":"startup","reason":"other"});
        let output = runtime
            .run_hook(&[], Ok(payload.to_string().into_bytes()))
            .unwrap();
        let value: Value = serde_json::from_str(output.lines().next_back().unwrap()).unwrap();
        if event == "SessionStart" {
            start = value;
        }
    }
    start
}

fn trap_session(root: &Path, repo: &Path, i: i32) -> PathBuf {
    fixtures::write_claude_session(
        &root.join(format!("s{i}.jsonl")),
        &[json!([format!("fix bug {i}"), [
            ["Edit",{"file_path":repo.join(format!("pkg/mod{i}.py")),"old_string":"a","new_string":"b"},"ok",false],
            ["Bash",{"command":"pytest -q tests/test_a.py"},"bash: pytest: command not found",true],
            ["Bash",{"command":"python -m pytest -q tests/test_a.py"},"1 passed in 0.1s",false]
        ]])],
        "claude-opus-5-5",
    )
}

#[test]
fn injection_off_and_a_tight_fact_cap_keep_the_context_clean() {
    let root = TempDir::new("live-config-injection");
    let f = fixture(root.path());
    let environment = native_environment(&f);
    for i in 0..4 {
        run_session(
            environment.clone(),
            &f.repo,
            &format!("s{i}"),
            &trap_session(root.path(), &f.repo, i),
        );
    }
    assert!(
        !f.store.read("injection.json", json!({})).unwrap()["rendered"]
            .as_str()
            .unwrap()
            .is_empty()
    );
    let mut off = environment.clone();
    off.env.insert("TE_MICROSKILLS".into(), json!("off"));
    assert_eq!(
        run_session(off, &f.repo, "s4", &trap_session(root.path(), &f.repo, 4)),
        json!({})
    );
    let mut tight = environment;
    tight.env.insert("TE_INJECT_MAX_TOKENS".into(), json!("5"));
    run_session(
        tight.clone(),
        &f.repo,
        "s5",
        &trap_session(root.path(), &f.repo, 5),
    );
    let after = run_session(tight, &f.repo, "s6", &trap_session(root.path(), &f.repo, 6));
    assert!(after.get("hookSpecificOutput").is_none());
}

#[test]
fn loopguard_stop_is_still_read_and_acts_as_nudge_with_a_warning() {
    let root = TempDir::new("live-config-stop");
    for env in [json!({"TE_LOOPGUARD":"stop"}), json!({"TE_SPIRAL":"guard"})] {
        let cfg = load_config(root.path(), None, env.as_object().unwrap());
        assert_eq!(cfg.values["loopguard"], "stop");
        assert!(
            cfg.warnings
                .iter()
                .any(|warning| warning.contains("loopguard=stop acts as nudge"))
        );
    }
    assert!(
        load_config(
            root.path(),
            None,
            json!({"TE_LOOPGUARD":"nudge"}).as_object().unwrap()
        )
        .warnings
        .is_empty()
    );
}

#[test]
fn the_quiet_window_before_a_deny_list_is_written_may_be_zero() {
    let mut cfg = LiveConfig::default();
    assert_eq!(cfg.values["toolprune_quiet_minutes"], 60);
    assert!(cfg.set("toolprune_quiet_minutes", json!("0"), "test"));
    assert_eq!(cfg.values["toolprune_quiet_minutes"], 0);
    assert!(!cfg.set("toolprune_quiet_minutes", json!("-5"), "test"));
    assert!(!cfg.set("toolprune_evidence", json!("0"), "test"));
    assert_eq!(
        load_config(
            Path::new("/nonexistent"),
            None,
            json!({"TE_TOOLPRUNE_QUIET_MINUTES":"15"})
                .as_object()
                .unwrap()
        )
        .values["toolprune_quiet_minutes"],
        15
    );
}

#[test]
fn off_everywhere_and_on_in_one_repository_runs_only_there() {
    let root = TempDir::new("live-config-opt-in");
    let f = fixture(root.path());
    let other = root.path().join("other");
    fs::create_dir_all(other.join(".git/info")).unwrap();
    fs::write(other.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
    fs::write(
        other.join(".git/config"),
        "[remote \"origin\"]\n\turl = https://github.com/acme/other.git\n",
    )
    .unwrap();
    let environment = native_environment(&f);
    let home = paths::state_root(&environment.env, &environment.home).unwrap();
    let on = paths::repo_identity(&f.repo, &environment.env, &environment.home)
        .unwrap()
        .key;
    write(&home.join("config.json"), json!({"enabled":false}));
    write(
        &home.join("repos").join(&on).join("config.json"),
        json!({"enabled":true}),
    );
    for (repo, sid) in [(&f.repo, "s-on"), (&other, "s-off")] {
        run_session(
            environment.clone(),
            repo,
            sid,
            &root.path().join(format!("{sid}.jsonl")),
        );
    }
    let entries: Vec<_> = fs::read_dir(home.join("repos"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_str().unwrap().to_owned())
        .collect();
    assert_eq!(entries, slice::from_ref(&on));
    let entries: Vec<_> = fs::read_dir(home.join("repos").join(on))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_str().unwrap().to_owned())
        .collect();
    assert_ne!(entries, ["config.json"]);
}
