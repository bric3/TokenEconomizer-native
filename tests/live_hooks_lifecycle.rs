// Converted from reference/upstream/tests/test_live_hooks.py.
pub mod fixtures;
#[path = "support/live_hook_fixture.rs"]
pub mod live_fixture;
mod support;

use live_fixture::{claude_log, fixture, session, transcript};
use serde_json::{Value, json};
use std::{
    fs::{self, File, FileTimes},
    path::Path,
    time::Duration,
};
use support::TempDir;
use tokeneconomizer::plugin::{context::LiveContext, fold, history, hook, paths};

fn start(f: &mut live_fixture::HookFixture, sid: &str, path: &Path) {
    f.main(json!({
        "hook_event_name":"SessionStart", "session_id":sid, "source":"startup",
        "transcript_path":path, "cwd":f.repo
    }));
}

fn end(f: &mut live_fixture::HookFixture, sid: &str, path: &Path) {
    assert_eq!(
        f.main(json!({
            "hook_event_name":"SessionEnd", "session_id":sid, "reason":"other",
            "transcript_path":path, "cwd":f.repo
        })),
        json!({})
    );
}

#[test]
fn each_session_is_folded_once_and_priced() {
    let tmp = TempDir::new("live-fold-price");
    let mut f = fixture(tmp.path(), "claude");
    let path = claude_log(tmp.path(), &f, "s", 0, false, 1);
    session(&mut f, "s", &path, "startup");
    end(&mut f, "s", &path);
    let ledger = f.store.read_lines("ledger.jsonl", None).unwrap();
    assert_eq!(ledger.len(), 1);
    let record = &ledger[0];
    assert_eq!(record["requests"], 3);
    assert!((record["effective_tokens"].as_f64().unwrap() - 3.0 * 435.0).abs() < 0.001);
    assert_eq!(record["priced_by"], "model");
    let overhead = record["overhead"].as_object().unwrap();
    assert_eq!(overhead.len(), 3);
    for key in [
        "optimizer_tokens",
        "spiral_judge_tokens",
        "spiral_nudge_writer_tokens",
    ] {
        assert!(overhead.contains_key(key));
    }
}

#[test]
fn a_fold_that_cannot_take_the_lock_skips_and_the_session_is_still_folded_once() {
    let tmp = TempDir::new("live-fold-lock");
    let mut f = fixture(tmp.path(), "claude");
    f.fold_wait = 0.0;
    let path = claude_log(tmp.path(), &f, "s", 0, false, 1);
    start(&mut f, "s", &path);
    let holder = f.store.locked("fold", 0.0).unwrap();
    assert!(holder.acquired);
    end(&mut f, "s", &path);
    assert!(f.store.read_lines("ledger.jsonl", None).unwrap().is_empty());
    assert!(
        f.store
            .read("folded.json", Value::Null)
            .unwrap()
            .get("s")
            .is_none()
    );
    let ctx = LiveContext::build(
        "claude",
        true,
        &f.repo,
        f.environment.env.clone(),
        &f.environment.home,
        Value::Null,
        false,
    )
    .unwrap();
    assert_eq!(
        fold::pending_sessions(&ctx, &[], fold::STALE_S, history::now(), &mut f).unwrap(),
        ["s"]
    );
    holder.release().unwrap();
    assert!(
        !fold::fold_session(&ctx, "s", None, f.fold_wait, &mut f)
            .unwrap()
            .is_null()
    );
    assert!(
        fold::fold_session(&ctx, "s", None, f.fold_wait, &mut f)
            .unwrap()
            .is_null()
    );
    assert_eq!(f.store.read_lines("ledger.jsonl", None).unwrap().len(), 1);
}

#[test]
fn a_session_whose_end_was_missed_is_folded_by_the_next_start() {
    let tmp = TempDir::new("live-fold-stale");
    let mut f = fixture(tmp.path(), "claude");
    let lost = claude_log(tmp.path(), &f, "lost", 0, false, 1);
    start(&mut f, "lost", &lost);
    assert!(
        f.store
            .read("folded.json", Value::Null)
            .unwrap()
            .get("lost")
            .is_none()
    );
    let file = fs::OpenOptions::new().write(true).open(&lost).unwrap();
    let old = file.metadata().unwrap().modified().unwrap() - Duration::from_secs(3600);
    file.set_times(FileTimes::new().set_accessed(old).set_modified(old))
        .unwrap();
    let next = transcript(tmp.path(), "next");
    File::create(&next).unwrap();
    start(&mut f, "next", &next);
    assert!(
        f.store
            .read("folded.json", Value::Null)
            .unwrap()
            .get("lost")
            .is_some()
    );
}

#[test]
fn garbage_input_and_internal_errors_answer_no_change() {
    struct BrokenController;
    impl hook::Runtime for BrokenController {
        fn handle(&mut self, _ctx: &mut LiveContext, _payload: Value) -> Result<Value, String> {
            Err("controller bug".into())
        }
    }
    let tmp = TempDir::new("live-hook-errors");
    let f = fixture(tmp.path(), "claude");
    let run = |raw: &[u8]| {
        hook::main(
            &[],
            Ok(raw.to_vec()),
            &f.environment,
            0,
            &mut BrokenController,
        )
        .unwrap()
    };
    assert_eq!(run(b"not json"), "{}\n");
    let raw = json!({"hook_event_name":"Stop","session_id":"s","cwd":f.repo});
    assert_eq!(run(raw.to_string().as_bytes()), "{}\n");
    let state = paths::state_root(&f.environment.env, &f.environment.home).unwrap();
    assert!(
        fs::read_to_string(state.join("errors.log"))
            .unwrap()
            .contains("controller bug")
    );
}

#[test]
fn a_non_ascii_checkout_is_found_whatever_the_locale() {
    let tmp = TempDir::new("live-hook-utf8");
    let mut f = fixture(tmp.path(), "claude");
    let repo = tmp.path().join("Программирование").join("agentsync");
    fs::create_dir_all(repo.parent().unwrap()).unwrap();
    fs::rename(&f.repo, &repo).unwrap();
    f.environment.env.insert("LC_ALL".into(), json!("C"));
    let raw = json!({"hook_event_name":"SessionStart","session_id":"s1","source":"startup","transcript_path":"","cwd":repo});
    let output = hook::main(
        &[],
        Ok(raw.to_string().into_bytes()),
        &f.environment.clone(),
        0,
        &mut f,
    )
    .unwrap();
    assert_eq!(output, "{}\n");
    let ctx = LiveContext::build(
        "claude",
        true,
        &repo,
        f.environment.env.clone(),
        &f.environment.home,
        Value::Null,
        false,
    )
    .unwrap();
    assert!(
        ctx.store
            .read("sessions/s1.json", Value::Null)
            .unwrap()
            .is_object()
    );
    assert!(
        !paths::state_root(&f.environment.env, &f.environment.home)
            .unwrap()
            .join("errors.log")
            .exists()
    );
}

#[test]
fn side_calls_a_disabled_plugin_and_a_payload_without_a_session_do_nothing() {
    let tmp = TempDir::new("live-hook-disabled");
    let mut f = fixture(tmp.path(), "claude");
    let state = tmp.path().join("unused-state");
    f.environment
        .env
        .insert("TOKENECONOMIZER_HOME".into(), json!(state));
    let start = json!({"hook_event_name":"SessionStart","session_id":"s","cwd":f.repo});
    for flag in ["TE_SIDECALL", "TE_DISABLE"] {
        f.environment.env.insert(flag.into(), json!("1"));
        assert_eq!(f.main(start.clone()), json!({}));
        f.environment.env.remove(flag);
    }
    assert_eq!(
        f.main(json!({"hook_event_name":"SessionStart","cwd":f.repo})),
        json!({})
    );
    assert!(!state.exists());
}

#[test]
fn mechanism_prints_cannot_corrupt_the_hook_output() {
    let tmp = TempDir::new("live-fold-empty");
    let mut f = fixture(tmp.path(), "claude");
    let path = transcript(tmp.path(), "empty");
    File::create(&path).unwrap();
    end(&mut f, "e", &path);
    assert_eq!(f.folds, ["e"]);
}
