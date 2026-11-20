// Converted from reference/upstream/tests/test_live_hooks.py.
pub mod fixtures;
#[path = "support/live_hook_fixture.rs"]
pub mod live_fixture;
mod support;

use live_fixture::{CLAIM, FID, HookFixture, claude_log, cli, env_state, fixture, session};
use serde_json::{Value, json};
use std::fs;
use support::TempDir;

#[test]
fn an_injected_fact_that_prevents_its_failure_stays_injected() {
    let root = TempDir::new("live-prevented");
    let mut f = fixture(root.path(), "claude");
    for i in 0..5 {
        let sid = format!("s{i}");
        let path = claude_log(root.path(), &f, &sid, i, false, 1);
        session(&mut f, &sid, &path, "startup");
    }
    for i in 0..8 {
        let sid = format!("p{i}");
        let path = claude_log(root.path(), &f, &sid, i + 5, true, 1);
        let start = session(&mut f, &sid, &path, "startup");
        assert!(
            start["hookSpecificOutput"]["additionalContext"]
                .as_str()
                .unwrap()
                .contains(CLAIM)
        );
    }
    let state = env_state(&f);
    assert_eq!(
        state["facts"][FID]["life"]["fire_num"],
        state["facts"][FID]["life"]["fire_den"]
    );
    assert_eq!(
        f.store.read("sessions/p7.json", Value::Null).unwrap()["injected_ids"],
        json!([FID])
    );
}

#[test]
fn a_learned_fact_reaches_the_next_session_and_is_announced_once() {
    let root = TempDir::new("live-injection-notice");
    let mut f = fixture(root.path(), "claude");
    let mut starts = Vec::new();
    for i in 0..6 {
        let sid = format!("s{i}");
        let path = claude_log(root.path(), &f, &sid, i, false, 1);
        starts.push(session(&mut f, &sid, &path, "startup"));
    }
    assert!(starts[..4].iter().all(|start| start == &json!({})));
    let context = starts[4]["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(context.contains(CLAIM));
    assert!(
        starts[4]["systemMessage"]
            .as_str()
            .unwrap()
            .contains("learned fact")
    );
    assert!(starts[5].get("systemMessage").is_none());
    assert_eq!(
        starts[5]["hookSpecificOutput"]["additionalContext"],
        context
    );
}

#[test]
fn micro_skills_is_off_by_default_and_learns_nothing() {
    let root = TempDir::new("live-default-off");
    let mut f = fixture(root.path(), "claude");
    f.environment.env.remove("TE_MICROSKILLS");
    for i in 0..6 {
        let sid = format!("s{i}");
        let path = claude_log(root.path(), &f, &sid, i, false, 1);
        assert_eq!(session(&mut f, &sid, &path, "startup"), json!({}));
    }
    assert!(
        f.store
            .read("injection.json", Value::Null)
            .unwrap()
            .is_null()
    );
    let output = cli(&f, &["microskills"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("`config set microskills=on` turns it on")
    );
}

#[test]
fn micro_skills_on_runs_the_proposer_without_paid_and_charges_it() {
    let root = TempDir::new("live-proposer-on");
    let mut f = fixture(root.path(), "claude");
    f.environment
        .env
        .insert("TE_MICROSKILLS".into(), json!("on"));
    f.proposer_reply = Some(("{}".into(), 120.0));
    for i in 0..5 {
        let sid = format!("s{i}");
        let path = claude_log(root.path(), &f, &sid, i, false, 1);
        session(&mut f, &sid, &path, "startup");
    }
    assert_eq!(&*f.proposer_models.borrow(), &["haiku", "haiku"]);
    let ledger = f.store.read_lines("ledger.jsonl", None).unwrap();
    let tokens: Vec<_> = ledger
        .iter()
        .map(|row| row["overhead"]["optimizer_tokens"].as_f64().unwrap())
        .collect();
    assert_eq!(tokens, [0.0, 120.0, 0.0, 0.0, 120.0]);
    assert_eq!(
        f.store.read("injection.json", Value::Null).unwrap()["profile"],
        "envfacts+distiller"
    );
}

#[test]
fn facts_learn_per_session_however_many_prompts_it_ran() {
    let root = TempDir::new("live-learning-sessions");
    let mut f = fixture(root.path(), "claude");
    let mut starts = Vec::new();
    for i in 0..6 {
        let sid = format!("s{i}");
        let path = claude_log(root.path(), &f, &sid, i, false, 4);
        starts.push(session(&mut f, &sid, &path, "startup"));
    }
    assert!(starts[..4].iter().all(|start| start == &json!({})));
    assert!(
        starts[4]["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .contains(CLAIM)
    );
}

#[test]
fn resume_does_not_inject_again_and_shadow_never_injects() {
    let root = TempDir::new("live-resume-shadow");
    let mut f = fixture(root.path(), "claude");
    for i in 0..5 {
        let sid = format!("s{i}");
        let path = claude_log(root.path(), &f, &sid, i, false, 1);
        session(&mut f, &sid, &path, "startup");
    }
    let later = root.path().join("later.jsonl");
    fs::write(&later, "").unwrap();
    assert_eq!(f.main(json!({"hook_event_name":"SessionStart","session_id":"s4","source":"resume","transcript_path":later,"cwd":f.repo})),json!({}));
    f.environment.env.insert("TE_SHADOW".into(), json!("1"));
    assert_eq!(f.main(json!({"hook_event_name":"SessionStart","session_id":"z","source":"startup","transcript_path":later,"cwd":f.repo})),json!({}));
    assert!(
        f.store
            .read_lines("events.jsonl", None)
            .unwrap()
            .iter()
            .any(|event| event["kind"] == "inject" && event["shadow"] == true)
    );
}

#[test]
fn a_resumed_copilot_session_gets_back_the_facts_it_started_with() {
    let root = TempDir::new("live-copilot-resume");
    let mut f = fixture(root.path(), "copilot");
    f.store
        .write(
            "injection.json",
            &json!({"rendered":"Facts:\n- one\n- two","rendered_ids":["a","b"]}),
        )
        .unwrap();
    let start = |f: &mut HookFixture, sid: &str, source: &str| {
        f.main(
            json!({"hook_event_name":"SessionStart","session_id":sid,"source":source,"cwd":f.repo}),
        )
        .to_string()
    };
    assert!(start(&mut f, "c1", "new").contains("- two"));
    f.store
        .write(
            "injection.json",
            &json!({"rendered":"Facts:\n- one","rendered_ids":["a"]}),
        )
        .unwrap();
    assert!(start(&mut f, "c1", "resume").contains("- two"));
    assert!(!start(&mut f, "c2", "new").contains("- two"));
    assert!(start(&mut f, "c3", "new").contains("- one"));
    f.store
        .write("injection.json", &json!({"rendered":"","rendered_ids":[]}))
        .unwrap();
    start(&mut f, "c4", "new");
    f.store
        .write(
            "injection.json",
            &json!({"rendered":"Facts:\n- one","rendered_ids":["a"]}),
        )
        .unwrap();
    assert!(!start(&mut f, "c4", "resume").contains("- one"));
}
