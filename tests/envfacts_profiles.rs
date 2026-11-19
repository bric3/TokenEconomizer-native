// Converted from reference/upstream/experiments/tests/test_envfacts_profiles.py.
pub mod fixtures;
#[path = "support/recording_runner.rs"]
mod recording_runner;
mod support;

use fixtures::write_codex_transcript;
use recording_runner::RecordingRunner;
use serde_json::{Map, Value, json};
use std::{fs, path::PathBuf};
use support::TempDir;
use tokeneconomizer::{
    config::InjectionConfig,
    experiments::{
        adapters::Adapter,
        arms::{self, ARM_ORDER},
    },
    microskills::{
        distiller::ProposeContext,
        envfacts::{self, EnvFactsDistiller},
        observe::{self, Vocabulary},
        stage::PrefixStage,
    },
};

const BASE: &str = "/opt/env/base/bin/python";
const PROJ: &str = "/opt/env/proj/bin/python";

fn broken(root: &TempDir, i: i32) -> PathBuf {
    write_codex_transcript(
        &root.path().join(format!("t{i}.jsonl")),
        &[
            json!({"type":"file_change","changes":[{"path":format!("pkg/mod{i}.py"),"kind":"update"}]}),
            json!({"type":"command_execution","command":"/bin/bash -lc \"python -m pytest tests/test_a.py -q\"",
            "aggregated_output":format!("{BASE}: No module named pytest"),"exit_code":1}),
            json!({"type":"command_execution","command":format!("/bin/bash -lc \"{PROJ} -m pytest tests/test_a.py -q\""),
            "aggregated_output":"1 passed","exit_code":0}),
        ],
    )
}

#[test]
fn codex_injection_profiles() {
    assert_eq!(
        arms::get("codex-inject").unwrap().config.injection.profile,
        "envfacts+distiller"
    );
    assert_eq!(
        arms::get("codex-inject-envfacts")
            .unwrap()
            .config
            .injection
            .profile,
        "envfacts"
    );
}

#[test]
fn node_vocabulary_reads_a_node_repository() {
    let classify = |command: &str, vocabulary: &Vocabulary| {
        observe::classify_command(command, 0, vocabulary, "", None, None).unwrap()
    };
    let node = Vocabulary::node_web();
    let observations = classify("npx jest tests/ --ci", &node);
    assert_eq!(
        observations
            .into_iter()
            .map(|o| (o.intent, o.form))
            .collect::<Vec<_>>(),
        [("run_tests".into(), "npx jest".into())]
    );
    let observations = classify("node scripts/build.js && npm test", &node);
    assert_eq!(
        observations
            .into_iter()
            .map(|o| (o.intent, o.form))
            .collect::<Vec<_>>(),
        [
            ("run_interpreter".into(), "node".into()),
            ("run_tests".into(), "npm test".into())
        ]
    );
    assert_eq!(
        classify("npm install left-pad", &node)[0].intent,
        "fetch_remote"
    );
    assert!(
        classify("npx jest", &Vocabulary::default())
            .first()
            .is_none_or(|o| o.intent != "run_tests")
    );
    assert!(classify("pytest -q", &node).is_empty());
    assert_eq!(
        observe::error_class("Error: Cannot find module 'left-pad'"),
        Some("module_missing")
    );
    assert_eq!(
        observe::error_class("connect ECONNREFUSED 127.0.0.1:443"),
        Some("connection_refused")
    );
}

#[test]
fn adapters_supply_a_vocabulary() {
    assert_eq!(
        Adapter::swebench(None, &Map::new()).env_vocabulary().name,
        "python-git"
    );
    assert_eq!(
        Adapter::webbench("".into(), None, &Map::new())
            .env_vocabulary()
            .name,
        "node-web"
    );
}

#[test]
fn distiller_records_its_vocabulary() {
    let root = TempDir::new("envfacts-vocabulary");
    let mut distiller = EnvFactsDistiller::new("acme/widget", Vocabulary::node_web(), None, None);
    distiller
        .update(&broken(&root, 0), "w-0", Value::Null, None)
        .unwrap();
    distiller.save(root.path()).unwrap();
    let saved: Value = serde_json::from_slice(
        &fs::read(
            root.path()
                .join(envfacts::STATE_SUBDIR)
                .join("acme__widget.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(saved["vocab"], "node-web");
    assert_eq!(distiller.debug_row()["_vocab"], "node-web");
}

#[test]
fn composed_profile_renders_both_layers_and_briefs_the_proposer() {
    let root = TempDir::new("envfacts-composed");
    let mut runner = RecordingRunner::new(r#"{"extractors":[],"facts":[]}"#, 7.0);
    let prompts = runner.prompts.clone();
    let config = InjectionConfig {
        enabled: true,
        profile: "envfacts+distiller".into(),
        propose_every: 1,
        ..InjectionConfig::default()
    };
    let mut context = ProposeContext {
        runner: Some(&mut runner),
        ..ProposeContext::default()
    };
    let mut stage = PrefixStage::new(
        &config,
        "acme/widget",
        root.path(),
        true,
        "agent",
        Adapter::swebench(None, &Map::new()).env_vocabulary(),
        &mut context,
    )
    .unwrap();
    assert!(stage.distiller.env.is_some() && stage.distiller.dist.is_some());
    let mut total = 0.0;
    let mut block = String::new();
    for i in 0..6 {
        block = stage.render();
        let run = root.path().join(format!("run{i}"));
        fs::create_dir(&run).unwrap();
        fs::rename(broken(&root, i), run.join("transcript.jsonl")).unwrap();
        let (tokens, reason) = stage
            .after_task(
                &json!({"instance_id":format!("w-{i}")}),
                &run,
                &mut json!({"resolved":true}),
                &mut context,
            )
            .unwrap();
        total += tokens;
        assert_eq!(reason, "envfacts+manual-distill");
    }
    assert!(block.starts_with(envfacts::HEADER) && block.contains(PROJ));
    assert!(prompts.borrow().len() >= 5);
    assert_eq!(total, 7.0 * prompts.borrow().len() as f64);
    assert!(
        prompts.borrow()[3..]
            .iter()
            .any(|prompt| prompt.contains("ALREADY injected") && prompt.contains("fails here"))
    );
    let mut summary = json!({"resolved":true});
    let run = root.path().join("run_last");
    fs::create_dir(&run).unwrap();
    fs::rename(broken(&root, 9), run.join("transcript.jsonl")).unwrap();
    stage
        .after_task(
            &json!({"instance_id":"w-9"}),
            &run,
            &mut summary,
            &mut context,
        )
        .unwrap();
    assert_eq!(summary["manual_facts"]["_profile"], "envfacts+distiller");
    assert!(
        summary["manual_facts"]
            .as_object()
            .unwrap()
            .keys()
            .any(|key| key.starts_with("form_repair:"))
    );
}

#[test]
fn envfacts_alone_still_needs_no_runner() {
    let root = TempDir::new("envfacts-free");
    let config = InjectionConfig {
        enabled: true,
        profile: "envfacts".into(),
        ..InjectionConfig::default()
    };
    let mut stage = PrefixStage::new(
        &config,
        "acme/widget",
        root.path(),
        true,
        "agent",
        Adapter::swebench(None, &Map::new()).env_vocabulary(),
        &mut ProposeContext::default(),
    )
    .unwrap();
    assert_eq!(stage.render(), "");
    assert_eq!(stage.reason, "envfacts");
}

#[test]
fn codex_arms_carry_the_intended_profiles() {
    let default = arms::get("codex-inject").unwrap().config;
    assert_eq!(default.injection.profile, "envfacts+distiller");
    assert!(default.injection.paid());
    assert_eq!(default.injection.propose_every, 10);
    assert!(default.needs_optimizer());
    let combined = arms::get("codex-inject-untool").unwrap().config;
    assert_eq!(combined.injection.profile, "envfacts+distiller");
    assert!(combined.tooling.codex_untool);
    let free = arms::get("codex-inject-envfacts").unwrap().config;
    assert_eq!(free.injection.profile, "envfacts");
    assert!(!free.injection.paid());
    assert!(!free.needs_optimizer());
    assert_eq!(
        serde_json::to_value(
            arms::get("codex-inject-distiller")
                .unwrap()
                .config
                .injection
        )
        .unwrap(),
        serde_json::to_value(default.injection).unwrap()
    );
    for name in [
        "codex-inject",
        "codex-inject-untool",
        "codex-inject-envfacts",
        "codex-inject-distiller",
    ] {
        assert_eq!(arms::get(name).unwrap().mode, "manual4", "{name}");
        assert!(ARM_ORDER.contains(&name), "{name}");
    }
}
