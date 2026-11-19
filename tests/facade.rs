// Converted from reference/upstream/experiments/tests/test_facade.py.
pub mod fixtures;
#[path = "support/recording_runner.rs"]
mod recording_runner;
mod support;

use fixtures::{write_claude_transcript, write_codex_transcript};
use recording_runner::RecordingRunner;
use serde_json::{Map, json};
use std::fs;
use support::TempDir;
use tokeneconomizer::{
    config::{EconomizerConfig, InjectionConfig, SpiralConfig, ToolingConfig},
    experiments::{
        adapters::Adapter,
        arms,
        facade::{Economizer, StreamOptions},
    },
    microskills::distiller::ProposeContext,
    runner::Runner,
};

#[test]
fn generic_trimech_end_to_end() {
    let root = TempDir::new("facade-generic");
    let config = EconomizerConfig {
        injection: InjectionConfig {
            enabled: true,
            propose_every: 5,
            ..InjectionConfig::default()
        },
        tooling: ToolingConfig {
            evidence: 3,
            judge: false,
            ..ToolingConfig::default()
        },
        spiral: SpiralConfig {
            guard: true,
            nudge: true,
            ..SpiralConfig::default()
        },
    };
    let options = StreamOptions {
        agent: "claude".into(),
        repo: "acme/widget".into(),
        results_root: root.path().into(),
        fresh: true,
        ..StreamOptions::default()
    };
    let mut resolve = |_: &str| {
        Ok(Some(
            Box::new(RecordingRunner::new(r#"{"extractors":[],"facts":[]}"#, 5.0))
                as Box<dyn Runner>,
        ))
    };
    let mut propose = ProposeContext::default();
    let mut messages = Vec::new();
    let mut economizer = Economizer::new(
        config,
        &options,
        Some(Box::new(Adapter::swebench(None, &Map::new()))),
        &mut resolve,
        &mut propose,
        &mut messages,
    )
    .unwrap();
    let task = json!({"instance_id":"w-1","problem_statement":"fix"});
    assert_eq!(economizer.injection_prefix(), "");
    assert_eq!(
        economizer.build_tools(&task, 0, &mut messages).unwrap(),
        Some(vec![])
    );
    assert_eq!(economizer.run_kwargs().unwrap()["spiral_guard"], true);
    let run = root.path().join("run");
    fs::create_dir(&run).unwrap();
    write_claude_transcript(
        &run.join("transcript.jsonl"),
        &[json!(["bash",{"command":"pytest"},"ok"])],
    );
    let row = json!({"resolved":true,"num_turns":10,"tokens_total":1000,"input_tokens":100,
        "cache_read_input_tokens":800,"cache_creation_input_tokens":50,"output_tokens":50});
    let mut summary = json!({"resolved":true,"num_turns":10});
    let (_, reason) = economizer
        .after_task(&task, &row, &run, &mut summary, &propose, &mut messages)
        .unwrap();
    assert_eq!(reason.as_deref(), Some("manual-distill"));
    assert!(summary.get("manual_facts").is_some() && summary.get("aborted").is_some());
}

#[test]
fn codex_arm_end_to_end() {
    let root = TempDir::new("facade-codex");
    let config = EconomizerConfig {
        injection: InjectionConfig {
            enabled: false,
            ..InjectionConfig::default()
        },
        tooling: ToolingConfig {
            codex_untool: true,
            ..ToolingConfig::default()
        },
        spiral: SpiralConfig {
            codex_spiral: true,
            ..SpiralConfig::default()
        },
    };
    let options = StreamOptions {
        agent: "codex".into(),
        repo: "pydata/xarray".into(),
        results_root: root.path().into(),
        fresh: true,
        ..StreamOptions::default()
    };
    let mut resolve = |_: &str| Ok(None);
    let mut propose = ProposeContext::default();
    let mut messages = Vec::new();
    let mut economizer = Economizer::new(
        config,
        &options,
        Some(Box::new(Adapter::swebench(None, &Map::new()))),
        &mut resolve,
        &mut propose,
        &mut messages,
    )
    .unwrap();
    let task = json!({"instance_id":"x-1","problem_statement":"fix concat"});
    assert_eq!(
        economizer.build_tools(&task, 0, &mut messages).unwrap(),
        Some(vec![])
    );
    assert!(
        economizer
            .run_kwargs()
            .unwrap()
            .get("codex_spiral_policy")
            .is_some()
    );
    let run = root.path().join("run");
    fs::create_dir(&run).unwrap();
    write_codex_transcript(
        &run.join("transcript.jsonl"),
        &[json!({"type":"command_execution","command":"pytest"})],
    );
    let row = json!({"resolved":true,"num_turns":30,"tokens_total":5000,"input_tokens":500,
        "cache_read_input_tokens":4000,"cache_creation_input_tokens":200,"output_tokens":300,"codex_spiral_nudges":0});
    let mut summary = json!({"resolved":true,"num_turns":30});
    let (_, reason) = economizer
        .after_task(&task, &row, &run, &mut summary, &propose, &mut messages)
        .unwrap();
    assert_eq!(reason, None);
    assert_eq!(summary["codex_untool_active"], true);
    assert_eq!(summary["codex_spiral_active"], true);
    assert!(
        economizer
            .build_tools(&task, 1, &mut messages)
            .unwrap()
            .unwrap()
            .contains(&"web".into())
    );
}

#[test]
fn codex_trimech_end_to_end() {
    let root = TempDir::new("facade-codex-trimech");
    let runner = RecordingRunner::new(r#"{"extractors":[],"facts":[]}"#, 3.0);
    let calls = runner.prompts.clone();
    let mut runner = Some(runner);
    let mut resolve = |_: &str| {
        Ok(runner
            .take()
            .map(|runner| Box::new(runner) as Box<dyn Runner>))
    };
    let options = StreamOptions {
        agent: "codex".into(),
        repo: "pydata/xarray".into(),
        results_root: root.path().into(),
        fresh: true,
        ..StreamOptions::default()
    };
    let mut propose = ProposeContext::default();
    let mut messages = Vec::new();
    let mut economizer = Economizer::new(
        arms::get("codex-trimech").unwrap().config,
        &options,
        Some(Box::new(Adapter::swebench(None, &Map::new()))),
        &mut resolve,
        &mut propose,
        &mut messages,
    )
    .unwrap();
    let mut task = json!({"instance_id":"x-1","problem_statement":"fix concat"});
    let kwargs = economizer.run_kwargs().unwrap();
    assert!(
        kwargs.get("codex_config_capabilities").is_some()
            && kwargs.get("codex_spiral_policy").is_some()
    );
    assert_eq!(
        economizer.build_tools(&task, 0, &mut messages).unwrap(),
        Some(vec![])
    );
    let mut total = 0.0;
    for i in 0..10 {
        task = json!({"instance_id":format!("x-{i}"),"problem_statement":"fix concat"});
        economizer.injection_prefix();
        economizer.build_tools(&task, i, &mut messages).unwrap();
        economizer.run_kwargs().unwrap();
        let run = root.path().join(format!("run{i}"));
        fs::create_dir(&run).unwrap();
        write_codex_transcript(
            &run.join("transcript.jsonl"),
            &[
                json!({"type":"file_change","changes":[{"path":"xarray/core/concat.py","kind":"update"}]}),
                json!({"type":"command_execution","command":"/bin/bash -lc \"python -m pytest xarray/tests/test_concat.py -q\"",
                "aggregated_output":"1 passed","exit_code":0}),
            ],
        );
        let row = json!({"resolved":true,"num_turns":30,"tokens_total":5000,"input_tokens":500,
            "cache_read_input_tokens":4000,"cache_creation_input_tokens":200,"output_tokens":300,
            "codex_spiral_nudges":0,"codex_spiral_tool_turns":12});
        let mut summary = json!({"resolved":true,"num_turns":30});
        let (tokens, reason) = economizer
            .after_task(&task, &row, &run, &mut summary, &propose, &mut messages)
            .unwrap();
        total += tokens;
        assert_eq!(summary["codex_untool_active"], true);
        assert_eq!(summary["codex_spiral_active"], true);
        assert!(summary.get("manual_facts").is_some());
        assert!(reason.unwrap().starts_with("envfacts"));
    }
    assert!(!calls.borrow().is_empty());
    assert_eq!(total, 3.0 * calls.borrow().len() as f64);
    assert!(
        economizer
            .build_tools(&task, 10, &mut messages)
            .unwrap()
            .unwrap()
            .contains(&"web".into())
    );
}
