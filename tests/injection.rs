// Converted from reference/upstream/tests/test_injection.py.
pub mod fixtures;
#[path = "support/recording_runner.rs"]
mod recording_runner;
mod support;

use fixtures::write_claude_transcript;
use recording_runner::RecordingRunner;
use serde_json::json;
use std::fs;
use support::TempDir;
use tokeneconomizer::{
    config::InjectionConfig,
    microskills::{
        distiller::{Distiller, ProposeContext},
        observe::Vocabulary,
        stage::PrefixStage,
    },
};

const EMPTY_PAYLOAD: &str = r#"{"extractors":[],"facts":[]}"#;

#[test]
fn distiller_empty_render_before_formation() {
    let root = TempDir::new("distiller-empty");
    let mut runner = RecordingRunner::new(EMPTY_PAYLOAD, 5.0);
    let mut context = ProposeContext {
        runner: Some(&mut runner),
        ..ProposeContext::default()
    };
    let mut distiller = Distiller::new("acme/widget", "agent", false);
    let transcript = write_claude_transcript(
        &root.path().join("t.jsonl"),
        &[json!(["bash",{"command":"pytest"},"ok"])],
    );
    distiller
        .update(&transcript, "widget-1", json!(true), &mut context)
        .unwrap();
    assert_eq!(distiller.render(), "");
    assert_eq!(distiller.now, 1);
}

#[test]
fn distiller_persist_roundtrip() {
    let root = TempDir::new("distiller-roundtrip");
    let mut runner = RecordingRunner::new(EMPTY_PAYLOAD, 5.0);
    let mut context = ProposeContext {
        runner: Some(&mut runner),
        ..ProposeContext::default()
    };
    let mut distiller = Distiller::new("acme/widget", "agent", false);
    let transcript = write_claude_transcript(
        &root.path().join("t.jsonl"),
        &[json!(["bash",{"command":"pytest"},"ok"])],
    );
    distiller
        .update(&transcript, "w-1", json!(true), &mut context)
        .unwrap();
    distiller.save(root.path()).unwrap();
    let restored = Distiller::load("acme/widget", root.path(), "agent", false).unwrap();
    assert_eq!(restored.now, distiller.now);
}

#[test]
fn distiller_grounds_and_fires_a_fact() {
    let root = TempDir::new("distiller-grounding");
    let payload=json!({"extractors":[],"facts":[{"id":"use-pytest","claim":"pytest is canonical",
        "injection":"run tests with pytest","high_stakes":true,
        "detector":{"scope":"ran_tests","confirm":"passed","fired":"ran_tests"},"cite":["w-0","w-1"]}]}).to_string();
    let mut runner = RecordingRunner::new(&payload, 5.0);
    let mut context = ProposeContext {
        runner: Some(&mut runner),
        propose_every: 1,
        ..ProposeContext::default()
    };
    let mut distiller = Distiller::new("acme/widget", "agent", false);
    distiller.last_propose = -1;
    for i in 0..8 {
        let transcript = write_claude_transcript(
            &root.path().join(format!("t{i}.jsonl")),
            &[
                json!(["edit",{"file_path":format!("pkg/mod{i}.py")},"ok"]),
                json!(["bash",{"command":"python -m pytest tests/test_x.py"},"1 passed"]),
            ],
        );
        distiller
            .update(&transcript, &format!("w-{i}"), json!(true), &mut context)
            .unwrap();
    }
    assert!(distiller.facts.contains_key("use-pytest"));
    assert!(distiller.facts["use-pytest"].life.fire_den >= 1);
}

#[test]
fn prefix_stage_after_task_reports_tokens() {
    let root = TempDir::new("prefix-after-task");
    let mut runner = RecordingRunner::new(EMPTY_PAYLOAD, 5.0);
    let mut context = ProposeContext {
        runner: Some(&mut runner),
        ..ProposeContext::default()
    };
    let config = InjectionConfig {
        enabled: true,
        propose_every: 5,
        ..InjectionConfig::default()
    };
    let mut stage = PrefixStage::new(
        &config,
        "acme/widget",
        root.path(),
        true,
        "agent",
        Vocabulary::default(),
        &mut context,
    )
    .unwrap();
    let run = root.path().join("run");
    fs::create_dir(&run).unwrap();
    write_claude_transcript(
        &run.join("transcript.jsonl"),
        &[json!(["bash",{"command":"pytest"},"ok"])],
    );
    let mut summary = json!({"resolved":true});
    let (_, reason) = stage
        .after_task(
            &json!({"instance_id":"w-1"}),
            &run,
            &mut summary,
            &mut context,
        )
        .unwrap();
    assert_eq!(reason, "manual-distill");
    assert!(summary.get("manual_facts").is_some());
}
