// Converted from reference/upstream/tests/test_live_hooks.py.
pub mod fixtures;
#[path = "support/live_hook_fixture.rs"]
pub mod live_fixture;
mod support;

use live_fixture::{fixture, green, stuck, turn, warm};
use serde_json::{Value, json};
use support::TempDir;
use tokeneconomizer::{accounting, plugin::hosts};

const NUDGE: &str = "Read the failing assertion before editing again.";

fn first_redirect(outputs: &[Value]) -> Option<usize> {
    outputs
        .iter()
        .position(|output| output.get("hookSpecificOutput").is_some())
}

#[test]
fn cold_start_never_intervenes() {
    let tmp = TempDir::new("live-guard-cold");
    let mut f = fixture(tmp.path(), "claude");
    assert!(
        turn(&mut f, "s", "p0", &stuck("claude", 40))
            .iter()
            .all(|output| *output == json!({}))
    );
}

#[test]
fn warm_guard_nudges_then_only_notifies_in_an_interactive_session() {
    let tmp = TempDir::new("live-guard-warm");
    let mut f = fixture(tmp.path(), "claude");
    warm(&mut f);
    let outputs = turn(&mut f, "s", "stuck", &stuck("claude", 40));
    let nudge = first_redirect(&outputs).unwrap();
    assert!(
        outputs[nudge]["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .contains("Process check (automated)")
    );
    assert!(
        outputs[nudge]["systemMessage"]
            .as_str()
            .unwrap()
            .contains("nudged")
    );
    let mut later = outputs[nudge + 1..]
        .iter()
        .filter(|output| **output != json!({}));
    let notice = later.next().unwrap();
    assert!(notice.get("continue").is_none());
    assert!(
        notice["systemMessage"]
            .as_str()
            .unwrap()
            .contains("still looks stuck")
    );
    assert!(later.next().is_none());
}

#[test]
fn the_guard_redirects_then_tells_the_person_how_to_stop_on_every_agent() {
    for host in ["claude", "codex", "copilot", "pi", "droid"] {
        for mode in ["interactive", "headless"] {
            for setting in ["nudge", "stop"] {
                let case = format!("{host}/{mode}/{setting}");
                let tmp = TempDir::new("live-guard-ladder");
                let mut f = fixture(tmp.path(), host);
                f.environment.env.insert("TE_MODE".into(), json!(mode));
                f.environment
                    .env
                    .insert("TE_LOOPGUARD".into(), json!(setting));
                for k in 0..5 {
                    turn(&mut f, "s", &format!("g{k}"), &green());
                }
                let outputs = turn(&mut f, "s", "stuck", &stuck(host, 40));
                let to_model: Vec<_> = outputs
                    .iter()
                    .filter_map(|output| {
                        if host == "copilot" {
                            output["additionalContext"].as_str()
                        } else {
                            output["hookSpecificOutput"]["additionalContext"].as_str()
                        }
                        .filter(|text| !text.is_empty())
                    })
                    .collect();
                let notices: Vec<_> = outputs
                    .iter()
                    .map(|output| {
                        if host == "copilot" {
                            output["progress"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .map(|v| v.as_str().unwrap())
                                .collect::<Vec<_>>()
                                .join(" ")
                        } else {
                            output["systemMessage"]
                                .as_str()
                                .unwrap_or_default()
                                .to_owned()
                        }
                    })
                    .filter(|text| text.contains("still looks stuck"))
                    .collect();
                assert!(
                    outputs.iter().all(|output| output.get("continue").is_none()
                        && output.get("stopReason").is_none()),
                    "{case}"
                );
                if host == "droid" {
                    assert!(notices.is_empty(), "{case}");
                    let relay: Vec<_> = to_model
                        .iter()
                        .filter(|text| text.contains("still looks stuck"))
                        .collect();
                    assert_eq!(relay.len(), 1, "{case}");
                    assert!(relay[0].contains("it does not change your task"), "{case}");
                    assert!(
                        to_model
                            .iter()
                            .any(|text| text.contains("Process check (automated)")),
                        "{case}"
                    );
                    continue;
                }
                let profile = hosts::profile(host);
                let hint = hosts::stop_hint(
                    &json!(mode == "headless"),
                    profile.interrupt,
                    profile.end_run,
                );
                assert_eq!(notices.len(), 1, "{case}");
                assert!(notices[0].ends_with(hint), "{case}");
                if host == "claude" {
                    let [redirect, relay] = to_model.as_slice() else {
                        panic!("{case}: {to_model:?}")
                    };
                    assert!(redirect.contains("Process check (automated)"), "{case}");
                    assert!(
                        relay.contains(&notices[0])
                            && relay.contains("it does not change your task"),
                        "{case}"
                    );
                } else {
                    assert_eq!(to_model.len(), 1, "{case}");
                }
            }
        }
    }
}

#[test]
fn the_judge_answers_from_a_job_and_its_spend_is_charged_as_it_happens() {
    let tmp = TempDir::new("live-guard-judge-charge");
    let mut f = fixture(tmp.path(), "claude");
    f.judge_result = Some(true);
    warm(&mut f);
    let outputs = turn(&mut f, "s", "stuck", &stuck("claude", 40));
    let nudge = first_redirect(&outputs).unwrap();
    assert_eq!(
        outputs[nudge]["hookSpecificOutput"]["additionalContext"],
        NUDGE
    );
    let started_at: usize = f.judge_digests[0]
        .split_whitespace()
        .next()
        .unwrap()
        .strip_prefix("turns=")
        .unwrap()
        .parse()
        .unwrap();
    assert!(nudge >= started_at);
    assert!(
        f.judge_digests
            .iter()
            .all(|digest| !digest.contains("NOT green"))
    );

    let ledger = f.store.read_lines("ledger.jsonl", None).unwrap();
    let charged: Vec<_> = ledger
        .iter()
        .filter(|row| row["kind"] == "overhead")
        .collect();
    assert_eq!(charged[0]["session_id"], "s");
    assert_eq!(
        charged[0]["overhead"],
        json!({"spiral_judge_tokens":1000.0,"spiral_nudge_writer_tokens":300.0})
    );
    let overhead = accounting::summarize(&ledger).unwrap()["overhead_tokens"]
        .as_f64()
        .unwrap();
    let expected = 1300.0 * charged.len() as f64;
    assert!((overhead - expected).abs() < expected * 1e-6);
}

#[test]
fn a_hook_only_starts_the_judge_and_never_waits_for_it() {
    let tmp = TempDir::new("live-guard-judge-request");
    let mut f = fixture(tmp.path(), "claude");
    f.judge_result = Some(true);
    f.defer_judge = true;
    warm(&mut f);
    turn(&mut f, "s", "stuck", &stuck("claude", 12));
    assert_eq!(f.judge_jobs.len(), 1);
    assert!(f.judge_digests.is_empty());
    let request = f.store.read("judge/s.request.json", Value::Null).unwrap();
    assert_eq!(request["id"], f.judge_jobs[0]);
    assert!(!request["digest"].as_str().unwrap().contains("NOT green"));
}

#[test]
fn the_judge_decides_every_redirect_but_the_backstops() {
    let tmp = TempDir::new("live-guard-backstop");
    let mut f = fixture(tmp.path(), "claude");
    warm(&mut f);
    f.judge_result = Some(false);
    let calls = stuck("claude", 40);
    let spared = turn(&mut f, "s", "spared", &calls);
    assert!(!f.judge_digests.is_empty());
    assert!(first_redirect(&spared).unwrap() >= 24);

    f.judge_result = Some(true);
    let judged = turn(&mut f, "s", "judged", &calls);
    let first = first_redirect(&judged).unwrap();
    assert!(first < 24);
    assert_eq!(
        judged[first]["hookSpecificOutput"]["additionalContext"],
        NUDGE
    );
}

#[test]
fn the_judge_is_not_asked_before_the_repository_is_learned() {
    let tmp = TempDir::new("live-guard-cold-judge");
    let mut f = fixture(tmp.path(), "claude");
    f.judge_result = Some(true);
    assert!(
        turn(&mut f, "s", "cold", &stuck("claude", 40))
            .iter()
            .all(|output| *output == json!({}))
    );
    assert!(f.judge_digests.is_empty());
}

#[test]
fn a_long_turn_with_no_sign_of_trouble_never_wakes_the_judge() {
    let tmp = TempDir::new("live-guard-long-clean");
    let mut f = fixture(tmp.path(), "claude");
    warm(&mut f);
    f.judge_result = Some(true);
    let calls: Vec<_> = (0..60)
        .map(|k| json!(["Read", {"file_path":format!("/r/m{k}.py")}, "ok"]))
        .collect();
    assert!(
        turn(&mut f, "s", "long", &calls)
            .iter()
            .all(|output| *output == json!({}))
    );
    assert!(f.judge_digests.is_empty());
}

#[test]
fn while_model_calls_are_paused_only_the_backstop_redirects() {
    let tmp = TempDir::new("live-guard-paused");
    let mut f = fixture(tmp.path(), "claude");
    warm(&mut f);
    f.judge_result = Some(true);
    f.paused_models = true;
    let outputs = turn(&mut f, "s", "stuck", &stuck("claude", 40));
    assert!(f.judge_digests.is_empty());
    assert!(first_redirect(&outputs).unwrap() >= 24);
}

#[test]
fn only_turns_that_end_green_feed_the_envelope() {
    let tmp = TempDir::new("live-guard-envelope");
    let mut f = fixture(tmp.path(), "claude");
    let edit = json!(["Edit", {"file_path":"/r/a.py"}, "ok"]);
    turn(
        &mut f,
        "s",
        "red",
        &[
            edit.clone(),
            json!(["Bash", {"command":"pytest -q"}, {"error":"Exit code 1\nFAILED tests/test_a.py::t - AssertionError"}]),
        ],
    );
    turn(
        &mut f,
        "s",
        "green",
        &[
            edit,
            json!(["Bash", {"command":"pytest -q"}, {"stdout":"3 passed"}]),
        ],
    );
    assert_eq!(f.resolved_turns(), [2.0]);
}

#[test]
fn subagent_tool_calls_do_not_reach_the_guard() {
    let tmp = TempDir::new("live-guard-subagent");
    let mut f = fixture(tmp.path(), "claude");
    f.main(json!({
        "hook_event_name":"PostToolUse", "session_id":"s", "cwd":f.repo, "prompt_id":"p",
        "agent_id":"a1", "tool_name":"Bash", "tool_input":{"command":"ls"},
        "tool_response":{"stdout":""}
    }));
    assert_eq!(
        f.store.read("sessions/s.json", Value::Null).unwrap(),
        Value::Null
    );
}
