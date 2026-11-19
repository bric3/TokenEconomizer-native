// Converted from reference/upstream/tests/test_live_core.py (judge scheduling and control).
pub mod fixtures;

use fixtures::pi_json_run;
use serde_json::{Value, json};
use tokeneconomizer::{
    loopguard::turn::{BackgroundJudge, FoldOptions, Judge, TurnGuard},
    plugin::{config::LiveConfig, hosts, probe, sidecall::NO_SIDE_CALLS},
    rules,
    trace::pi,
};

#[derive(Default)]
struct JudgeFixture {
    started: Vec<String>,
    verdict: Option<Value>,
}

impl BackgroundJudge for JudgeFixture {
    fn start(&mut self, _guard: &mut TurnGuard, digest: &str) -> Result<Value, String> {
        self.started.push(digest.into());
        Ok(json!(format!("job{}", self.started.len())))
    }
    fn poll(&mut self, guard: &mut TurnGuard) -> Result<(bool, Value), String> {
        if self.verdict.is_none()
            || guard.judge_pending["id"] != format!("job{}", self.started.len())
        {
            return Ok((false, Value::Null));
        }
        Ok((true, self.verdict.take().unwrap()))
    }
}

fn spin(
    guard: &mut TurnGuard,
    judge: &mut JudgeFixture,
    n: usize,
) -> Vec<Option<(String, String)>> {
    let mut output = Vec::new();
    for k in 0..n {
        let (name, args, result) = if k % 2 == 0 {
            (
                "Edit",
                json!({"file_path":"/r/a.py","old_string":"x","new_string":"y"}),
                "ok",
            )
        } else {
            (
                "Bash",
                json!({"command":"pytest -q"}),
                "FAILED tests/test_a.py::t - AssertionError",
            )
        };
        output.push(
            guard
                .fold(
                    name,
                    &args,
                    result,
                    None,
                    FoldOptions {
                        envelope: Some(6.0),
                        judge: Some(Judge::Background(judge)),
                        ..FoldOptions::default()
                    },
                )
                .unwrap(),
        );
    }
    output
}

#[test]
fn a_background_judge_starts_once_and_its_verdict_nudges_a_later_call() {
    let mut guard = TurnGuard::new("t", &json!({})).unwrap();
    let mut judge = JudgeFixture::default();
    assert_eq!(spin(&mut guard, &mut judge, 10), vec![None; 10]);
    assert_eq!(judge.started.len(), 1);
    assert!(!judge.started[0].contains("NOT green"));
    judge.verdict = Some(
        json!({"stuck":true,"confidence":0.9,"reason":"loop","_nudge":"Read the failing assertion before editing again."}),
    );
    assert_eq!(
        spin(&mut guard, &mut judge, 1)[0],
        Some(("nudge".into(), "judge-stuck@0.90:loop".into()))
    );
    assert_eq!(
        guard.nudge_body(Some(6.0), None),
        "Read the failing assertion before editing again."
    );
    assert_ne!(
        guard.nudge_body(Some(6.0), None),
        "Read the failing assertion before editing again."
    );
}

#[test]
fn a_productive_verdict_backs_the_judge_off() {
    let mut guard = TurnGuard::new("t", &json!({})).unwrap();
    let mut judge = JudgeFixture::default();
    spin(&mut guard, &mut judge, 7);
    judge.verdict = Some(json!({"stuck":false,"confidence":0.8,"reason":"progress"}));
    assert_eq!(spin(&mut guard, &mut judge, 1), [None]);
    assert_eq!(guard.judge_gap, TurnGuard::JUDGE_BASE_GAP * 2);
    assert!(guard.judge_pending.is_null());
}

#[test]
fn a_job_that_never_answers_stops_blocking_the_next_one() {
    let mut guard = TurnGuard::new("t", &json!({})).unwrap();
    let mut judge = JudgeFixture::default();
    spin(&mut guard, &mut judge, 7);
    assert_eq!(judge.started.len(), 1);
    assert!(!guard.judge_pending.is_null());
    guard.judge_pending["since"] = json!(0);
    let gap = guard.judge_gap as usize;
    spin(&mut guard, &mut judge, gap);
    assert_eq!(judge.started.len(), 2);
}

#[test]
fn a_small_envelopes_backstop_never_preempts_the_nudge() {
    let thresholds = rules::scaled_thresholds("aggressive");
    let failure = "FAIL: test_floats (tests.test_calc.TestAdd)\nTraceback (most recent call last):\nAssertionError: 0.30000000000000004 != 0.3\n\nFAILED (failures=1)\n";
    let mut guard = TurnGuard::new("t", &json!({})).unwrap();
    let mut calls = vec![(
        "Bash",
        json!({"command":"python -m unittest"}),
        failure,
        Some(1),
    )];
    for k in 0..30 {
        calls.push(("Edit",json!({"file_path":"src/calc/__init__.py","old_string":"x","new_string":format!("y{k}")}),"ok",None));
        calls.push((
            "Bash",
            json!({"command":"python -m unittest"}),
            failure,
            Some(1),
        ));
    }
    let mut fired = Vec::new();
    for (i, (name, args, result, code)) in calls.into_iter().enumerate() {
        if let Some((action, _)) = guard
            .fold(
                name,
                &args,
                result,
                code,
                FoldOptions {
                    envelope: Some(4.0),
                    thresholds: Some(&thresholds),
                    nudge_budget: 1,
                    ..FoldOptions::default()
                },
            )
            .unwrap()
        {
            fired.push((i + 1, action.clone()));
            if action == "nudge" {
                guard.after_nudge();
            } else {
                break;
            }
        }
    }
    assert_eq!(fired, [(14, "nudge".into()), (28, "abort".into())]);
}

#[test]
fn the_probe_counts_pytest_attempts_not_exit_codes() {
    let (runs, tried) = probe::test_commands(&[
        r#"find . -name "pytest.ini" -o -name "go.mod" | head -20"#.into(),
        "cd /r && python -m pytest tests/ -v 2>&1 | tail -20".into(),
        "cd /r && python -m unittest discover -s tests -v 2>&1 | tail -20".into(),
    ]);
    assert_eq!(runs.len(), 2);
    assert_eq!(tried, 1);
    let (runs, tried) = probe::test_commands(&[
        "pytest -q".into(),
        "python3 -m unittest".into(),
        "ls tests/".into(),
    ]);
    assert_eq!(runs.len(), 2);
    assert_eq!(tried, 1);
    let command = r#"python -m unittest discover -s tests -p "test_*.py""#;
    assert_eq!(
        probe::test_commands(&[command.into()]),
        (vec![command.into()], 0)
    );
}

#[test]
fn a_pi_reply_is_read_from_its_json_run() {
    let (text, usage) = pi::json_reply(&pi_json_run()).unwrap();
    assert_eq!(text, "- use python -m pytest");
    let expected = json!({"input_tokens":745,"cache_read_input_tokens":0,"cache_creation_input_tokens":0,"output_tokens":298,
        "cache_creation_1h_input_tokens":0,"cost_usd":0.002235,"model":"anthropic/claude-haiku-4.5"});
    let usage = usage.unwrap();
    assert_eq!(
        usage.as_object().unwrap().len(),
        expected.as_object().unwrap().len()
    );
    for (key, value) in expected.as_object().unwrap() {
        if value.is_number() {
            assert_eq!(usage[key].as_f64(), value.as_f64(), "{key}");
        } else {
            assert_eq!(&usage[key], value, "{key}");
        }
    }
    assert_eq!(pi::json_reply("").unwrap(), ("".into(), None));
}

#[test]
fn every_agent_with_side_calls_has_its_own_paid_model_setting() {
    let mut config = LiveConfig::default();
    for host in hosts::HOSTS
        .into_iter()
        .filter(|name| !NO_SIDE_CALLS.contains(name))
    {
        assert!(
            config.set(&format!("{host}_paid_model"), json!("m"), "test"),
            "{host}"
        );
    }
}
