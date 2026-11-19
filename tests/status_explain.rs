// Converted from reference/upstream/tests/test_status_explain.py.
pub mod fixtures;
mod support;

use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use support::TempDir;
use tokeneconomizer::{
    plugin::{
        cli,
        config::LiveConfig,
        context::LiveContext,
        denylist,
        hosts::{self, session::Environment},
        runtime::NativeRuntime,
    },
    store::Store,
};

struct Status {
    ctx: LiveContext,
    native: NativeRuntime,
    output: String,
}
impl Status {
    fn new(root: &Path) -> Self {
        let repo = root.join("repo");
        fs::create_dir(&repo).unwrap();
        let env = json!({"TOKENECONOMIZER_HOME":root.join("state"),"GIT_CEILING_DIRECTORIES":root})
            .as_object()
            .unwrap()
            .clone();
        let environment = Environment {
            env: env.clone(),
            home: root.into(),
            cwd: root.into(),
        };
        let mut ctx = LiveContext::build(
            "codex",
            hosts::profile("codex").keeps_history,
            &repo,
            env,
            root,
            Value::Null,
            false,
        )
        .unwrap();
        let mut config = LiveConfig::default().as_dict();
        config["microskills"] = json!("envfacts");
        config["toolprune"] = json!("propose");
        ctx.config = config;
        Self {
            ctx,
            native: NativeRuntime::new(environment, env!("CARGO_BIN_EXE_te").into()),
            output: String::new(),
        }
    }
    fn write(&self, name: &str, value: Value) {
        self.ctx.store.write(name, &value).unwrap();
    }
    fn run(&mut self, options: &[&str]) -> String {
        self.output.clear();
        let mut args = vec![
            "--host".into(),
            self.ctx.host.clone(),
            "--cwd".into(),
            self.ctx.repo.root.to_string_lossy().into_owned(),
            "status".into(),
        ];
        args.extend(options.iter().map(|s| (*s).into()));
        assert_eq!(
            cli::main(&args, &self.native.environment.clone(), 80, self).unwrap(),
            json!(0)
        );
        self.output.clone()
    }
}
impl cli::Runtime for Status {
    fn context(&mut self, _args: &Value) -> cli::Result<LiveContext> {
        Ok(self.ctx.clone())
    }
    fn pinned_context(&mut self, _args: &Value, _session: &Value) -> cli::Result<LiveContext> {
        panic!("Unexpected pinned context")
    }
    fn global(&mut self, method: &str, args: Value) -> cli::Result<Value> {
        assert_eq!(method, "execute");
        assert_eq!(args[0]["fn"], "cmd_status");
        cli::status(&args[0], self).map(|code| json!(code))
    }
    fn call(&mut self, ctx: &LiveContext, method: &str, args: Value) -> cli::Result<Value> {
        assert!(
            [
                "session_note",
                "applied",
                "history_summary",
                "repomap_state",
                "paused",
                "proposal"
            ]
            .contains(&method),
            "Status attempted an unexpected operation: {method}"
        );
        cli::Runtime::call(&mut self.native, ctx, method, args)
    }
    fn say(&mut self, text: &str) -> cli::Result<()> {
        self.output.push_str(text);
        self.output.push('\n');
        Ok(())
    }
}
fn explanations(output: &str) -> Vec<&str> {
    output
        .lines()
        .filter(|line| line.starts_with("  explain "))
        .collect()
}
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        if path.is_dir() {
            pending.extend(
                fs::read_dir(path)
                    .unwrap()
                    .map(|entry| entry.unwrap().path()),
            );
        } else if path.is_file() {
            files.insert(path.clone(), fs::read(path).unwrap());
        }
    }
    files
}

#[test]
fn explanation_is_opt_in_and_json_remains_machine_readable() {
    let root = TempDir::new("status-opt-in");
    let mut status = Status::new(root.path());
    let ordinary = status.run(&[]);
    let detailed = status.run(&["--explain"]);
    let lines = explanations(&detailed);
    assert_eq!(lines.len(), 3);
    assert!(lines.iter().all(|line| line.chars().count() <= 160));
    assert_eq!(
        detailed
            .lines()
            .filter(|line| !line.starts_with("  explain "))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n",
        ordinary
    );
    let plain: Value = serde_json::from_str(&status.run(&["--json"])).unwrap();
    assert!(plain.get("explanations").is_none());
    let mut extended: Value = serde_json::from_str(&status.run(&["--json", "--explain"])).unwrap();
    let notes = extended
        .as_object_mut()
        .unwrap()
        .shift_remove("explanations")
        .unwrap();
    assert_eq!(
        notes
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>(),
        ["microskills", "toolprune", "loopguard"].into()
    );
    assert_eq!(extended, plain);
}

#[test]
fn cold_start_does_not_blame_a_firing_gate_without_any_facts() {
    let root = TempDir::new("status-cold");
    let output = Status::new(root.path()).run(&["--explain"]);
    for text in [
        "no learning snapshot recorded",
        "no tool-usage snapshot recorded",
        "0/5 ended-green turns",
    ] {
        assert!(output.contains(text), "{output}");
    }
    for text in ["firing gate", "config set microskills=on"] {
        assert!(!output.contains(text));
    }
}

#[test]
fn disabled_plugin_has_one_explanation_even_with_old_state() {
    let root = TempDir::new("status-disabled");
    let mut status = Status::new(root.path());
    status.ctx.config["enabled"] = json!(false);
    status.write(
        "injection.json",
        json!({"rendered":"- old fact","sessions":4}),
    );
    let output = status.run(&["--explain"]);
    let lines = explanations(&output);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("plugin is disabled"));
}

#[test]
fn off_and_shadow_are_not_described_as_live_optimization() {
    let root = TempDir::new("status-shadow");
    let mut status = Status::new(root.path());
    for key in ["microskills", "toolprune", "loopguard"] {
        status.ctx.config[key] = json!("off");
    }
    status.write(
        "injection.json",
        json!({"rendered":"- old fact","sessions":4}),
    );
    assert!(
        explanations(&status.run(&["--explain"]))
            .iter()
            .all(|line| line.contains("off"))
    );
    status.ctx.config["microskills"] = json!("envfacts");
    status.ctx.config["shadow"] = json!(true);
    let output = status.run(&["--explain"]);
    let lines = explanations(&output);
    assert_eq!(lines.len(), 4);
    assert!(lines[0].contains("shadow mode") && lines[0].contains("not applied"));
    assert!(lines.join(" ").contains("cached facts"));
}

#[test]
fn fact_explanations_use_saved_lifecycle_states() {
    for (facts, expected) in [
        (
            json!({"_profile":"envfacts"}),
            "no facts tracked in the last learning snapshot",
        ),
        (
            json!({"_private-id":{"state":"CANDIDATE","fire":"1/1","inject":false},"_task":{"state":"metadata"}}),
            "1 candidate, 0 active, 0 retired",
        ),
        (
            json!({"a":{"state":"CANDIDATE","fire":"1/1","inject":false},"b":{"state":"ACTIVE","fire":"1/4","inject":false},"c":{"state":"RETIRED","fire":"2/5","inject":false}}),
            "1 candidate, 1 active, 1 retired; no facts rendered",
        ),
    ] {
        let root = TempDir::new("status-lifecycle");
        let mut status = Status::new(root.path());
        status.write(
            "injection.json",
            json!({"sessions":6,"rendered":"","facts":facts}),
        );
        let output = status.run(&["--explain"]);
        assert!(output.contains(expected));
        assert!(output.contains("see microskills with the same --host/--cwd"));
    }
}

#[test]
fn unapplied_proposals_report_the_recorded_state() {
    for (mode, extra, expected) in [
        ("propose", json!({}), "proposal awaiting"),
        (
            "auto",
            json!({"auto_pending":{"items":["goals"]}}),
            "proposal held for the idle wait",
        ),
        (
            "auto",
            json!({"auto_apply_refused":"private path or a very long exception"}),
            "automatic apply was refused",
        ),
        ("auto", json!({}), "unapplied proposal recorded"),
    ] {
        let root = TempDir::new("status-proposal");
        let mut status = Status::new(root.path());
        status.ctx.config["toolprune"] = json!(mode);
        let mut state = json!({"sessions":6,"proposal":["goals"]});
        state
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        status.write("untool.json", state);
        let output = status.run(&["--explain"]);
        assert!(output.contains(expected));
        assert!(!output.contains("private path or a very long exception"));
    }
}

#[test]
fn shadow_proposal_does_not_claim_an_automatic_apply_is_pending() {
    let root = TempDir::new("status-shadow-proposal");
    let mut status = Status::new(root.path());
    status.ctx.config["shadow"] = json!(true);
    status.write("untool.json", json!({"sessions":6,"proposal":["goals"]}));
    let output = status.run(&["--explain"]);
    let lines = explanations(&output).join("\n");
    assert!(lines.contains("not applied"));
    assert!(!lines.contains("awaiting") && !lines.contains("idle wait"));
}

#[test]
fn restored_items_are_not_reported_as_waiting_to_be_applied() {
    let root = TempDir::new("status-restored");
    let mut status = Status::new(root.path());
    status.write(
        "untool.json",
        json!({"sessions":6,"proposal":["goals"],"auto_pending":{"items":["goals"]}}),
    );
    denylist::set_kept(
        &status.ctx.store,
        &status.ctx.repo.root,
        &status.ctx.host,
        &["goals".into()],
        &[],
        &mut denylist::NativeRuntime,
    )
    .unwrap();
    let output = status.run(&["--explain"]);
    assert!(!output.contains("awaiting") && !output.contains("held for the idle wait"));
    assert!(output.contains("no eligible proposal recorded"));
}

#[test]
fn applied_rules_belong_to_the_selected_checkout() {
    for same in [true, false] {
        let root = TempDir::new("status-checkout");
        let mut status = Status::new(root.path());
        status.write("untool.json", json!({"sessions":6,"proposal":["goals"]}));
        let checkout = if same {
            status.ctx.repo.root.clone()
        } else {
            status.ctx.repo.root.join("other-clone")
        };
        denylist::record(
            &status.ctx.store,
            &["goals".into()],
            &denylist::settings_path(&checkout, "codex"),
            &mut denylist::NativeRuntime,
        )
        .unwrap();
        let output = status.run(&["--explain"]);
        assert_eq!(
            output.contains("deny rules recorded as applied for this checkout"),
            same
        );
        assert_ne!(output.contains("proposal awaiting approval"), same);
    }
}

#[test]
fn unavailable_pending_details_do_not_claim_an_idle_wait() {
    for pending in [
        Value::Null,
        json!("unexpected"),
        json!({"items":3}),
        json!({"items":"goals"}),
    ] {
        let root = TempDir::new("status-pending-shape");
        let mut status = Status::new(root.path());
        status.ctx.config["toolprune"] = json!("auto");
        status.write(
            "untool.json",
            json!({"sessions":6,"proposal":["goals"],"auto_pending":pending}),
        );
        let output = status.run(&["--explain"]);
        assert!(output.contains("unapplied proposal recorded"));
        assert!(!output.contains("held for the idle wait"));
    }
}

#[test]
fn no_proposal_does_not_automatically_mean_insufficient_evidence() {
    let root = TempDir::new("status-no-proposal");
    let mut status = Status::new(root.path());
    status.write(
        "untool.json",
        json!({"sessions":50,"proposal":[],"capabilities":{"goals":{"used":true}}}),
    );
    let output = status.run(&["--explain"]);
    assert!(
        output.contains("no eligible proposal recorded")
            && output.contains("see toolprune with the same --host/--cwd")
    );
    assert!(!output.contains("insufficient"));
}

#[test]
fn unsupported_host_is_not_told_to_collect_a_catalog() {
    let root = TempDir::new("status-unsupported");
    let mut status = Status::new(root.path());
    status.ctx.host = "pi".into();
    let output = status.run(&["--explain"]);
    assert!(output.contains("tool pruning is not available on pi"));
    assert!(!output.contains("refresh-catalog"));
}

#[test]
fn loopguard_uses_the_configured_warmup_and_does_not_infer_productivity() {
    let root = TempDir::new("status-warmup");
    let mut status = Status::new(root.path());
    status.ctx.config["loopguard_thresholds"]["envelope_min_samples"] = json!(2);
    status.write("envelope.json", json!({"resolved_turns":[10]}));
    assert!(status.run(&["--explain"]).contains("1/2 ended-green turns"));
    status.write("envelope.json", json!({"resolved_turns":[10,12]}));
    let output = status.run(&["--explain"]);
    assert!(
        output.contains("baseline ready; per-turn reasons cannot be inferred from these totals")
    );
    assert!(!output.contains("productive") && !output.contains("no failures"));
}

#[test]
fn paused_model_calls_are_reported_without_exposing_the_error() {
    let root = TempDir::new("status-paused");
    let mut status = Status::new(root.path());
    Store::new(&status.ctx.root)
        .unwrap()
        .write(
            "hosts/codex/sidecall.json",
            &json!({"at":1,"reason":"private error"}),
        )
        .unwrap();
    status.write("envelope.json", json!({"resolved_turns":[10,10,10,10,10]}));
    let output = status.run(&["--explain"]);
    assert!(output.contains("judge calls paused after a sign-in failure; backstop remains active"));
    assert!(!output.contains("private error"));
}

#[test]
fn paused_proposer_does_not_hide_facts_that_can_still_be_injected() {
    let root = TempDir::new("status-paused-proposer");
    let mut status = Status::new(root.path());
    status.ctx.config["microskills"] = json!("envfacts+distiller");
    status.ctx.config["loopguard"] = json!("off");
    Store::new(&status.ctx.root)
        .unwrap()
        .write(
            "hosts/codex/sidecall.json",
            &json!({"at":1,"reason":"not signed in"}),
        )
        .unwrap();
    assert!(
        status
            .run(&["--explain"])
            .contains("model proposer paused after a sign-in failure")
    );
    status.write(
        "injection.json",
        json!({"sessions":4,"rendered":"- a cached fact"}),
    );
    assert!(
        status
            .run(&["--explain"])
            .contains("cached facts are available for session start; model proposer paused")
    );
}

#[test]
fn older_snapshot_without_fact_details_does_not_claim_there_are_no_facts() {
    let root = TempDir::new("status-legacy-facts");
    let mut status = Status::new(root.path());
    status.write("injection.json", json!({"sessions":4,"rendered":""}));
    let output = status.run(&["--explain"]);
    assert!(output.contains("no fact details recorded"));
    assert!(!output.contains("no facts tracked"));
}

#[test]
fn unavailable_fact_details_do_not_crash_or_claim_zero_facts() {
    for facts in [
        Value::Null,
        json!([]),
        json!(["unexpected"]),
        json!("unexpected"),
    ] {
        let root = TempDir::new("status-fact-shape");
        let mut status = Status::new(root.path());
        status.write(
            "injection.json",
            json!({"sessions":4,"rendered":"","facts":facts}),
        );
        let output = status.run(&["--explain"]);
        assert!(output.contains("no fact details recorded"));
        assert!(!output.contains("no facts tracked"));
    }
}

#[test]
fn unknown_lifecycle_states_are_not_counted_as_zero_facts() {
    let root = TempDir::new("status-unknown-lifecycle");
    let mut status = Status::new(root.path());
    status.write(
        "injection.json",
        json!({"sessions":4,"rendered":"","facts":{"legacy":{"state":"UNKNOWN"}}}),
    );
    let output = status.run(&["--explain"]);
    assert!(output.contains("fact lifecycle details unavailable"));
    assert!(!output.contains("0 candidate, 0 active, 0 retired"));
}

#[test]
fn followup_guidance_keeps_the_selected_repository_and_host() {
    let root = TempDir::new("status-followup");
    let mut status = Status::new(root.path());
    status.write(
        "injection.json",
        json!({"sessions":4,"rendered":"","facts":{}}),
    );
    status.write("untool.json", json!({"sessions":4,"proposal":[]}));
    let output = status.run(&["--explain"]);
    let lines = explanations(&output);
    for text in [
        "microskills with the same --host/--cwd",
        "toolprune with the same --host/--cwd",
    ] {
        assert!(lines.iter().any(|line| line.contains(text)));
    }
}

#[test]
fn explanation_does_not_spawn_jobs_call_models_or_change_saved_state() {
    let root = TempDir::new("status-read-only");
    let mut status = Status::new(root.path());
    status.write("injection.json", json!({"sessions":4,"rendered":""}));
    status.write("untool.json", json!({"sessions":4,"proposal":["goals"]}));
    let before = snapshot(&status.ctx.root);
    status.run(&["--explain"]);
    status.run(&["--json", "--explain"]);
    assert_eq!(snapshot(&status.ctx.root), before);
}

#[test]
fn explanations_follow_real_session_learning() {
    let root = TempDir::new("status-learned");
    let mut status = Status::new(root.path());
    status.ctx.host = "claude".into();
    status.ctx.env.extend(json!({"TE_FOLD_SYNC":"1","TE_MICROSKILLS":"envfacts","TE_TOOLPRUNE":"off","TE_LOOPGUARD":"off"}).as_object().unwrap().clone());
    let learned = LiveContext::build(
        "claude",
        hosts::profile("claude").keeps_history,
        &status.ctx.repo.root,
        status.ctx.env.clone(),
        root.path(),
        Value::Null,
        false,
    )
    .unwrap();
    status.ctx.store = learned.store;
    status.ctx.config = learned.config;
    assert!(
        status
            .run(&["--explain"])
            .contains("no learning snapshot recorded")
    );
    let environment = Environment {
        env: status.ctx.env.clone(),
        home: root.path().into(),
        cwd: root.path().into(),
    };
    let mut native = NativeRuntime::new(environment, env!("CARGO_BIN_EXE_te").into());
    for i in 0..4 {
        let transcript = fixtures::write_claude_session(
            &root.path().join(format!("s{i}.jsonl")),
            &[json!([format!("fix bug {i}"),[
                ["Edit",{"file_path":status.ctx.repo.root.join(format!("pkg/mod{i}.py")),"old_string":"a","new_string":"b"},"ok",false],
                ["Bash",{"command":"pytest -q tests/test_a.py"},"bash: pytest: command not found",true],
                ["Bash",{"command":"python -m pytest -q tests/test_a.py"},"1 passed in 0.1s",false]
            ]])],
            "claude-opus-5-5",
        );
        for event in ["SessionStart", "SessionEnd"] {
            let payload = json!({"session_id":format!("s{i}"),"transcript_path":transcript,"cwd":status.ctx.repo.root,"hook_event_name":event,"source":"startup","reason":"other"});
            let output = native
                .run_hook(&[], Ok(payload.to_string().into_bytes()))
                .unwrap();
            assert!(
                serde_json::from_str::<Value>(output.lines().next_back().unwrap())
                    .unwrap()
                    .is_object()
            );
        }
        if i == 0 {
            assert!(
                status
                    .run(&["--explain"])
                    .contains("1 candidate, 0 active, 0 retired")
            );
        }
    }
    assert!(
        status
            .run(&["--explain"])
            .contains("cached facts are available for session start")
    );
}

#[test]
fn real_cli_selects_the_requested_repository_without_applying_a_pending_proposal() {
    let root = TempDir::new("status-real-cli");
    let repo = root.path().join("repo with spaces and 'quotes'");
    let elsewhere = root.path().join("elsewhere");
    fs::create_dir(&repo).unwrap();
    fs::create_dir(&elsewhere).unwrap();
    let env = json!({"TOKENECONOMIZER_HOME":root.path().join("state"),"GIT_CEILING_DIRECTORIES":root.path(),"TE_MICROSKILLS":"envfacts","TE_TOOLPRUNE":"auto","TE_LOOPGUARD":"nudge","TE_SHADOW":"0","TE_DISABLE":"0"}).as_object().unwrap().clone();
    let ctx = LiveContext::build(
        "codex",
        hosts::profile("codex").keeps_history,
        &repo,
        env.clone(),
        root.path(),
        Value::Null,
        false,
    )
    .unwrap();
    ctx.store.write("injection.json",&json!({"sessions":2,"rendered":"","facts":{"test-command":{"state":"CANDIDATE","fire":"2/2","inject":false}}})).unwrap();
    ctx.store
        .write(
            "untool.json",
            &json!({"sessions":5,"proposal":["goals"],"auto_pending":{"items":["goals"]}}),
        )
        .unwrap();
    ctx.store
        .write("envelope.json", &json!({"resolved_turns":[10,12]}))
        .unwrap();
    let saved_json = || -> BTreeMap<_, _> {
        fs::read_dir(&ctx.store.base)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "json"))
            .map(|path| {
                (
                    path.file_name().unwrap().to_owned(),
                    fs::read(path).unwrap(),
                )
            })
            .collect()
    };
    let before = saved_json();
    let run = |options: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_te"))
            .env_clear()
            .env("HOME", root.path())
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .envs(env.iter().map(|(k, v)| (k, v.as_str().unwrap())))
            .current_dir(&elsewhere)
            .args(["--host", "codex", "--cwd", repo.to_str().unwrap(), "status"])
            .args(options)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    let plain: Value = serde_json::from_str(&run(&["--json"])).unwrap();
    let mut explained: Value = serde_json::from_str(&run(&["--json", "--explain"])).unwrap();
    assert_eq!(explained["repo_root"], json!(repo));
    assert!(
        explained["explanations"]["microskills"]
            .as_str()
            .unwrap()
            .contains("1 candidate")
    );
    assert!(
        explained["explanations"]["toolprune"]
            .as_str()
            .unwrap()
            .contains("held for the idle wait")
    );
    assert!(
        explained["explanations"]["microskills"]
            .as_str()
            .unwrap()
            .contains("same --host/--cwd")
    );
    explained
        .as_object_mut()
        .unwrap()
        .shift_remove("explanations");
    assert_eq!(explained, plain);
    let text = run(&["--explain"]);
    let lines = explanations(&text);
    assert_eq!(lines.len(), 3);
    assert!(lines.iter().all(|line| line.chars().count() <= 160));
    assert!(!repo.join(".codex/config.toml").exists());
    assert_eq!(saved_json(), before);
}
