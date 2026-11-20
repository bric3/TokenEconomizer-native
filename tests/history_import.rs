// Converted from reference/upstream/tests/test_history_import.py.
#[path = "support/history_fixture.rs"]
pub mod history_fixture;
#[path = "support/hook_fixture.rs"]
pub mod hook_fixture;
mod support;

use history_fixture::{CLAIM, HistoryFixture, LogOptions};
use serde_json::{Value, json};
use std::{
    fs,
    path::Path,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use support::TempDir;
use tokeneconomizer::{
    accounting,
    plugin::{
        cli,
        config::load_config,
        denylist, fold, history,
        hosts::{self, session},
        paths, repos,
    },
    store::Store,
    trace::claude,
};

#[test]
fn the_first_session_start_reads_the_repositorys_history_once_oldest_first() {
    let root = TempDir::new("history-first");
    let f = HistoryFixture::new(root.path());
    for i in 0..4 {
        f.log(
            &format!("old{i}"),
            &f.trap(i),
            LogOptions {
                age: 3600.0 * f64::from(10 - i),
                ..Default::default()
            },
        );
    }
    let other = root.path().join("other");
    fs::create_dir(&other).unwrap();
    let mut trap = f.trap(0);
    trap[0][1][0][1]["file_path"] = json!(other.join("pkg/mod0.py"));
    f.log(
        "elsewhere",
        &trap,
        LogOptions {
            cwd: Some(other),
            ..Default::default()
        },
    );
    f.log(
        "running",
        &f.trap(9),
        LogOptions {
            age: 5.0,
            ..Default::default()
        },
    );
    f.log(
        "scripted",
        &f.trap(8),
        LogOptions {
            entrypoint: "sdk-cli".into(),
            ..Default::default()
        },
    );
    f.log("empty", &[], LogOptions::default());
    let sub = f
        .claude_home
        .join("projects")
        .join(claude::project_folder(f.ctx.repo.root.to_str().unwrap()))
        .join("old0/subagents");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("agent-x.jsonl"), "{}\n").unwrap();
    let first = f.hook("new1", "SessionStart", None);
    assert_eq!(f.ledger_ids(), ["old0", "old1", "old2", "old3"]);
    let records = f.ctx.store.read_lines("ledger.jsonl", None).unwrap();
    for row in records {
        assert_eq!(row["imported"], true);
        assert_eq!(row["untool_applied"], json!([]));
        assert_eq!(row["overhead"]["optimizer_tokens"].as_f64(), Some(0.0));
    }
    assert!(
        f.ctx
            .store
            .read("folded.json", json!({}))
            .unwrap()
            .as_object()
            .unwrap()
            .values()
            .all(|row| row["imported"] == true)
    );
    let doc = f.ctx.store.read("history.json", json!({})).unwrap();
    assert_eq!(doc["state"], "done");
    assert_eq!(doc["imported"], 4);
    assert!(doc.get("plan").is_none());
    assert_eq!(
        doc["skipped"],
        json!({"recent":1,"headless":1,"no_prompt":1})
    );
    assert!(first.get("systemMessage").is_none());
    let second = f.hook("new2", "SessionStart", None);
    assert!(
        second["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .contains(CLAIM)
    );
    assert!(second["systemMessage"].as_str().unwrap().contains(
        "TokenEconomizer read 4 earlier session(s) of acme/widget from Claude Code's history."
    ));
    assert!(
        !f.hook("new3", "SessionStart", None)["systemMessage"]
            .as_str()
            .unwrap_or("")
            .contains("earlier session")
    );
}

#[test]
fn history_import_off_reads_nothing_and_the_newest_are_kept() {
    let root = TempDir::new("history-capped");
    let mut f = HistoryFixture::new(root.path());
    for i in 0..4 {
        f.log(
            &format!("old{i}"),
            &f.trap(i),
            LogOptions {
                age: 3600.0 * f64::from(10 - i),
                ..Default::default()
            },
        );
    }
    f.log(
        "ancient",
        &f.trap(7),
        LogOptions {
            age: 400.0 * 86400.0,
            ..Default::default()
        },
    );
    f.environment
        .env
        .insert("TE_HISTORY_IMPORT".into(), json!("off"));
    f.hook("a", "SessionStart", None);
    assert!(
        f.ctx
            .store
            .read("history.json", Value::Null)
            .unwrap()
            .is_null()
    );
    assert!(
        f.ctx
            .store
            .read("folded.json", json!({}))
            .unwrap()
            .as_object()
            .unwrap()
            .is_empty()
    );
    f.environment
        .env
        .insert("TE_HISTORY_IMPORT".into(), json!("on"));
    f.environment
        .env
        .insert("TE_HISTORY_MAX_SESSIONS".into(), json!("2"));
    f.hook("b", "SessionStart", None);
    assert_eq!(f.ledger_ids(), ["old2", "old3"]);
    let doc = f.ctx.store.read("history.json", json!({})).unwrap();
    assert_eq!(doc["over_limit"], 2);
    assert_eq!(doc["skipped"]["too_old"], 1);
}

#[test]
fn by_default_a_session_start_reads_no_history_and_an_import_by_hand_does() {
    let root = TempDir::new("history-manual");
    let mut f = HistoryFixture::new(root.path());
    assert_eq!(
        load_config(&root.path().join("empty"), None, &Default::default()).values["history_import"],
        false
    );
    for i in 0..2 {
        f.log(
            &format!("old{i}"),
            &f.trap(i),
            LogOptions {
                age: 3600.0 * f64::from(10 - i),
                ..Default::default()
            },
        );
    }
    f.environment.env.remove("TE_HISTORY_IMPORT");
    f.hook("a", "SessionStart", None);
    assert!(
        f.ctx
            .store
            .read("history.json", Value::Null)
            .unwrap()
            .is_null()
    );
    assert!(
        f.ctx
            .store
            .read("folded.json", json!({}))
            .unwrap()
            .as_object()
            .unwrap()
            .is_empty()
    );
    let output = f.cli(&["history", "import"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("imported 2 earlier session(s)")
    );
    assert_eq!(f.ledger_ids(), ["old0", "old1"]);
}

#[test]
fn a_moved_checkouts_logs_are_matched_by_their_exact_folder() {
    let root = TempDir::new("history-moved");
    let mut f = HistoryFixture::new(root.path());
    let folder = claude::project_folder(f.ctx.repo.root.to_str().unwrap());
    for (id, cwd, folder) in [
        ("moved", "/gone/old/place/repo", folder.clone()),
        (
            "shell",
            "/gone/old/place/repo-shell",
            format!("{folder}-shell"),
        ),
        (
            "prefix",
            "/gone/elsewhere",
            folder[..folder.len() - 2].into(),
        ),
    ] {
        f.log(
            id,
            &f.trap(1),
            LogOptions {
                cwd: Some(cwd.into()),
                folder: Some(folder),
                ..Default::default()
            },
        );
    }
    assert_eq!(ids(&f.plan(&history::Limits::default())), ["moved"]);
}

fn ids(plan: &Value) -> Vec<String> {
    plan["sessions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|session| session["id"].as_str().unwrap().into())
        .collect()
}

#[test]
fn unsafe_session_ids_are_never_used() {
    let root = TempDir::new("history-safe-ids");
    let mut f = HistoryFixture::new(root.path());
    for bad in ["x$(touch y)", "-rf", "a.b"] {
        f.log(bad, &f.trap(1), LogOptions::default());
    }
    let good = "0b6f3c2e-6a4f-4c39-9b55-1f2d9d1c3b7a";
    f.log(good, &f.trap(2), LogOptions::default());
    let plan = f.plan(&history::Limits::default());
    assert_eq!(ids(&plan), [good]);
    assert_eq!(plan["skipped"], json!({"unsafe_id":3}));
    for bad in [
        json!("../x"),
        json!("a/b"),
        json!(""),
        Value::Null,
        json!("x".repeat(200)),
        json!("a\\b"),
    ] {
        assert!(claude::safe_session_id(&bad).is_none());
    }
}

#[test]
fn what_a_plan_leaves_out() {
    let root = TempDir::new("history-plan-skips");
    let mut f = HistoryFixture::new(root.path());
    let small = f.log(
        "small",
        &f.trap(0),
        LogOptions {
            age: 7200.0,
            ..Default::default()
        },
    );
    let mut prompts = f.trap(1);
    prompts.extend(f.looked());
    f.log(
        "large",
        &prompts,
        LogOptions {
            age: 5400.0,
            ..Default::default()
        },
    );
    f.log("seen", &f.trap(2), LogOptions::default());
    f.ctx
        .store
        .write("sessions/seen.json", &json!({"session_id":"seen"}))
        .unwrap();
    let limits = history::Limits {
        max_bytes: fs::metadata(small).unwrap().len(),
        ..Default::default()
    };
    let plan = f.plan(&limits);
    assert_eq!(ids(&plan), ["small"]);
    assert_eq!(plan["skipped"], json!({"too_large":1,"recorded_live":1}));
}

#[test]
fn where_claude_code_keeps_its_logs() {
    let root = TempDir::new("history-layout");
    assert_eq!(
        claude::projects_dir(&json!({"CLAUDE_CONFIG_DIR":"/c"}), root.path()),
        Path::new("/c/projects")
    );
    assert_eq!(
        claude::project_folder("/home/u/my_repo.v2"),
        "-home-u-my-repo-v2"
    );
    let project = root.path().join("projects/-w");
    fs::create_dir_all(project.join("s1/subagents")).unwrap();
    for path in [
        project.join("s1.jsonl"),
        project.join("agent-a.jsonl"),
        project.join("s1/subagents/x.jsonl"),
    ] {
        fs::write(path, "{}\n").unwrap();
    }
    let f = HistoryFixture::new(&root.path().join("fixture"));
    let env = json!({"CLAUDE_CONFIG_DIR":root.path().join("none")});
    let logs = session::history(
        "claude",
        env.as_object().unwrap(),
        &[root.path().to_string_lossy().into_owned()],
        &f.environment,
    )
    .unwrap();
    assert_eq!(
        logs.iter()
            .map(|row| (row["session_id"].clone(), row["folder"].clone()))
            .collect::<Vec<_>>(),
        [(json!("s1"), json!("-w"))]
    );
    assert!(
        session::history("codex", &Default::default(), &[], &f.environment)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn te_history_dry_run_writes_nothing() {
    let root = TempDir::new("history-dry-run");
    let f = HistoryFixture::new(root.path());
    f.log("old0", &f.trap(0), LogOptions::default());
    let out = f.cli(&["history", "import", "--dry-run"]);
    assert!(out.status.success());
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("1 earlier session(s) of acme/widget would be read")
    );
    assert!(
        f.ctx
            .store
            .read("history.json", Value::Null)
            .unwrap()
            .is_null()
    );
    let out = f.cli(&["history", "import"]);
    assert!(out.status.success());
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("imported 1 earlier session(s)")
    );
    let out = f.cli(&["history", "import", "--status"]);
    assert!(out.status.success());
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("read 1 earlier session(s)")
    );
    assert_eq!(f.cli(&["history", "import"]).status.code(), Some(1));
}

#[test]
fn no_hook_reads_history_on_another_agent() {
    let root = TempDir::new("history-other-agent");
    let mut f = HistoryFixture::new(root.path());
    f.log("old0", &f.trap(0), LogOptions::default());
    f.ctx.host = "codex".into();
    f.reload();
    f.hook(
        "c1",
        "SessionStart",
        Some(&root.path().join("rollout-x.jsonl")),
    );
    assert!(
        f.ctx
            .store
            .read("history.json", Value::Null)
            .unwrap()
            .is_null()
    );
    assert!(!hosts::profile("codex").keeps_history);
}

#[test]
fn an_import_that_fails_stops_blocking_live_folds() {
    let root = TempDir::new("history-plan-failure");
    let mut f = HistoryFixture::new(root.path());
    f.log("old0", &f.trap(0), LogOptions::default());
    f.detach();
    f.mark_running();
    f.fail_plan = true;
    assert_eq!(f.run(true, history::BUDGET_S).unwrap()["state"], "error");
    assert_eq!(
        f.ctx.store.read("history.json", json!({})).unwrap()["state"],
        "stopped"
    );
    assert!(!fold::import_blocks(&f.ctx).unwrap());
    assert!(
        fs::read_to_string(f.ctx.root.join("errors.log"))
            .unwrap()
            .contains("unreadable history")
    );
    let path = f.log("live0", &f.trap(5), LogOptions::default());
    f.live("live0", &path);
    assert!(!f.fold_live("live0").is_null());
}

fn old_logs(f: &HistoryFixture, count: i32) {
    for i in 0..count {
        f.log(
            &format!("old{i}"),
            &f.trap(i),
            LogOptions {
                age: 3600.0 * f64::from(10 - i),
                ..Default::default()
            },
        );
    }
}

#[test]
fn a_repository_with_folded_sessions_records_the_skip_and_a_manual_import_refuses() {
    let root = TempDir::new("history-prior-live");
    let mut f = HistoryFixture::new(root.path());
    f.environment
        .env
        .insert("TE_HISTORY_IMPORT".into(), json!("off"));
    for i in 0..2 {
        let path = f.log(&format!("s{i}"), &f.trap(i), LogOptions::default());
        f.hook(&format!("s{i}"), "SessionStart", Some(&path));
        f.hook(&format!("s{i}"), "SessionEnd", Some(&path));
    }
    f.log("old0", &f.trap(0), LogOptions::default());
    f.environment
        .env
        .insert("TE_HISTORY_IMPORT".into(), json!("on"));
    f.detach();
    f.maybe_start();
    let doc = f.ctx.store.read("history.json", json!({})).unwrap();
    assert_eq!(doc["state"], "skipped");
    assert!(doc["reason"].as_str().unwrap().contains("already"));
    assert!(f.spawned.is_empty());
    f.maybe_start();
    assert_eq!(f.ctx.store.read("history.json", json!({})).unwrap(), doc);
    assert!(f.spawned.is_empty());
    let out = f.cli(&["history", "import"]);
    assert_eq!(out.status.code(), Some(1));
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(
        text.trim()
            .starts_with("not imported: acme/widget already has folded sessions")
    );
    assert_eq!(text.trim().lines().count(), 1);
    assert!(
        f.ctx
            .store
            .read("folded.json", json!({}))
            .unwrap()
            .get("old0")
            .is_none()
    );
}

#[test]
fn a_session_that_ends_during_the_import_is_folded_after_it() {
    let root = TempDir::new("history-live-race");
    let mut f = HistoryFixture::new(root.path());
    old_logs(&f, 3);
    f.detach();
    f.maybe_start();
    assert_eq!(f.spawned, vec![vec!["history", "import", "--auto"]]);
    assert_eq!(
        f.ctx.store.read("history.json", json!({})).unwrap()["state"],
        "running"
    );
    let path = f.log("live0", &f.trap(5), LogOptions::default());
    f.live("live0", &path);
    assert!(f.fold_live("live0").is_null());
    let ctx = f.ctx.clone();
    assert!(
        fold::spawn_fold(&ctx, &["live0".into()], fold::FOLD_LOCK_WAIT_S, &mut f)
            .unwrap()
            .is_null()
    );
    assert_eq!(f.spawned.len(), 1);
    f.after_fold = Some(|f, sid| {
        if sid == "old0" {
            let path = f.log("live1", &f.trap(6), LogOptions::default());
            f.live("live1", &path);
            let before = Instant::now();
            assert!(f.fold_live("live1").is_null());
            assert!(before.elapsed() < Duration::from_secs(5));
        }
        Ok(())
    });
    let result = f.run(true, history::BUDGET_S).unwrap();
    assert_eq!(result["state"], "done");
    assert_eq!(result["imported"], 3);
    assert_eq!(f.ledger_ids(), ["old0", "old1", "old2", "live0", "live1"]);
    assert!(
        f.ctx.store.read("folded.json", json!({})).unwrap()["live0"]
            .get("imported")
            .is_none()
    );
}

#[test]
fn a_run_stopped_by_its_budget_resumes_the_same_plan_in_order() {
    let root = TempDir::new("history-budget");
    let mut f = HistoryFixture::new(root.path());
    old_logs(&f, 4);
    f.environment
        .env
        .insert("TE_HISTORY_MAX_SESSIONS".into(), json!("3"));
    f.detach();
    f.clock_tick = Some(0.0);
    f.mark_running();
    let result = f.run(true, 2.5).unwrap();
    assert_eq!(result["state"], "partial");
    assert_eq!(result["imported"], 2);
    let doc = f.ctx.store.read("history.json", json!({})).unwrap();
    assert_eq!(
        doc["plan"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["id"].clone())
            .collect::<Vec<_>>(),
        [json!("old1"), json!("old2"), json!("old3")]
    );
    assert_eq!(doc["next"], 2);
    assert!(fold::import_blocks(&f.ctx).unwrap());
    let path = f.log("live0", &f.trap(5), LogOptions::default());
    f.live("live0", &path);
    assert!(f.fold_live("live0").is_null());
    f.ctx.store.append("ledger.jsonl",&json!({"kind":"overhead","session_id":"live0","host":"claude","at":history::now(),"overhead":{"spiral_judge_tokens":9.0}})).unwrap();
    f.log(
        "old4",
        &f.trap(4),
        LogOptions {
            age: 1800.0,
            ..Default::default()
        },
    );
    f.maybe_start();
    assert_eq!(f.spawned, vec![vec!["history", "import", "--auto"]]);
    let result = f.run(true, 100.0).unwrap();
    assert_eq!(result["state"], "done");
    assert_eq!(result["imported"], 3);
    assert_eq!(f.ledger_ids(), ["old1", "old2", "old3", "live0"]);
}

#[test]
fn a_dead_import_is_restarted_by_the_next_session_start() {
    let root = TempDir::new("history-restart");
    let mut f = HistoryFixture::new(root.path());
    old_logs(&f, 1);
    f.detach();
    f.mark_running();
    f.maybe_start();
    assert!(f.spawned.is_empty());
    f.ctx
        .store
        .write(
            "history.json",
            &json!({"state":"running","heartbeat":history::now()-history::HEARTBEAT_STALE_S-1.0}),
        )
        .unwrap();
    f.maybe_start();
    assert_eq!(f.spawned, vec![vec!["history", "import", "--auto"]]);
    assert!(
        history::now()
            - f.ctx.store.read("history.json", json!({})).unwrap()["heartbeat"]
                .as_f64()
                .unwrap()
            < 60.0
    );
    f.maybe_start();
    assert_eq!(f.spawned.len(), 1);
    assert_eq!(f.run(true, history::BUDGET_S).unwrap()["state"], "done");
    assert!(
        f.ctx
            .store
            .read("folded.json", json!({}))
            .unwrap()
            .get("old0")
            .is_some()
    );
    f.ctx
        .store
        .write(
            "history.json",
            &json!({"state":"partial","plan":[],"next":0}),
        )
        .unwrap();
    f.environment
        .env
        .insert("TE_HISTORY_IMPORT".into(), json!("off"));
    f.reload();
    f.maybe_start();
    assert_eq!(
        f.ctx.store.read("history.json", json!({})).unwrap()["state"],
        "stopped"
    );
    assert!(!fold::import_blocks(&f.ctx).unwrap());
}

#[test]
fn an_import_waits_for_a_live_fold_already_under_way() {
    let root = TempDir::new("history-waits-live");
    let mut f = HistoryFixture::new(root.path());
    old_logs(&f, 1);
    f.detach();
    let path = f.log("live0", &f.trap(5), LogOptions::default());
    f.live("live0", &path);
    f.mark_running();
    let (send, receive) = mpsc::channel();
    let mut other = f.clone();
    let live = thread::spawn(move || {
        let ctx = other.ctx.clone();
        let lock = ctx.store.locked("fold", 10.0).unwrap();
        assert!(lock.acquired);
        send.send(()).unwrap();
        thread::sleep(Duration::from_millis(500));
        fold::fold_locked(&ctx, "live0", None, false, None, &mut other).unwrap();
        lock.release().unwrap();
    });
    receive.recv_timeout(Duration::from_secs(10)).unwrap();
    let before = Instant::now();
    let result = f.run(true, history::BUDGET_S).unwrap();
    live.join().unwrap();
    assert!(before.elapsed() >= Duration::from_millis(300));
    assert_eq!(result["state"], "skipped");
    assert!(
        result["reason"]
            .as_str()
            .unwrap()
            .contains("already has folded sessions")
    );
    assert_eq!(f.ledger_ids(), ["live0"]);
    assert_eq!(
        f.ctx.store.read("history.json", json!({})).unwrap()["state"],
        "skipped"
    );
    assert!(!fold::import_blocks(&f.ctx).unwrap());
}

#[test]
fn a_live_fold_that_waited_on_the_lock_looks_at_the_import_again() {
    let root = TempDir::new("history-recheck");
    let mut f = HistoryFixture::new(root.path());
    f.detach();
    let path = f.log("live0", &f.trap(5), LogOptions::default());
    f.live("live0", &path);
    f.block_after_check = true;
    assert!(f.fold_live("live0").is_null());
    assert_eq!(f.block_checks, 2);
    assert!(
        f.ctx
            .store
            .read("folded.json", json!({}))
            .unwrap()
            .get("live0")
            .is_none()
    );
    let ctx = f.ctx.clone();
    assert_eq!(
        fold::pending_sessions(&ctx, &[], fold::STALE_S, history::now(), &mut f).unwrap(),
        ["live0"]
    );
}

#[test]
fn folded_means_a_session_record_not_a_charge() {
    let root = TempDir::new("history-ledger-folded");
    let mut f = HistoryFixture::new(root.path());
    old_logs(&f, 1);
    f.detach();
    f.ctx.store.append("ledger.jsonl",&json!({"kind":"overhead","session_id":"live0","at":1.0,"overhead":{"spiral_judge_tokens":9.0}})).unwrap();
    f.maybe_start();
    assert_eq!(f.spawned, vec![vec!["history", "import", "--auto"]]);
    assert_eq!(f.run(true, history::BUDGET_S).unwrap()["state"], "done");
    f.environment
        .env
        .insert("TOKENECONOMIZER_HOME".into(), json!(f.root.join("home2")));
    f.reload();
    f.ctx
        .store
        .append(
            "ledger.jsonl",
            &json!({"kind":"session","session_id":"s0","folded_at":1.0}),
        )
        .unwrap();
    f.maybe_start();
    assert_eq!(f.spawned.len(), 1);
    assert_eq!(
        f.ctx.store.read("history.json", json!({})).unwrap()["state"],
        "skipped"
    );
}

#[test]
fn a_session_whose_fold_fails_is_skipped_and_the_rest_imported() {
    let root = TempDir::new("history-bad-fold");
    let mut f = HistoryFixture::new(root.path());
    old_logs(&f, 3);
    f.detach();
    f.mark_running();
    f.before_fold = Some(|_, sid| {
        if sid == "old1" {
            Err(history::Error::Failed("a log it cannot fold".into()))
        } else {
            Ok(())
        }
    });
    let result = f.run(true, history::BUDGET_S).unwrap();
    assert_eq!(result["state"], "done");
    assert_eq!(result["imported"], 2);
    assert_eq!(result["skipped"], json!({"error":1}));
    assert_eq!(f.ledger_ids(), ["old0", "old2"]);
}

#[test]
fn a_session_the_import_dies_on_is_left_out_after_two_tries() {
    let root = TempDir::new("history-cancelled-fold");
    let mut f = HistoryFixture::new(root.path());
    old_logs(&f, 2);
    f.detach();
    f.mark_running();
    f.before_fold = Some(|_, sid| {
        if sid == "old0" {
            Err(history::Error::Cancelled)
        } else {
            Ok(())
        }
    });
    for n in [1, 2] {
        assert!(matches!(
            f.run(true, history::BUDGET_S),
            Err(history::Error::Cancelled)
        ));
        let doc = f.ctx.store.read("history.json", json!({})).unwrap();
        assert_eq!(doc["state"], "running");
        assert_eq!(doc["next"], 0);
        assert_eq!(doc["attempt"], json!({"id":"old0","n":n}));
    }
    let result = f.run(true, history::BUDGET_S).unwrap();
    assert_eq!(result["state"], "done");
    assert_eq!(result["imported"], 1);
    assert_eq!(result["skipped"], json!({"error":1}));
    assert_eq!(f.ledger_ids(), ["old1"]);
    assert!(!fold::import_blocks(&f.ctx).unwrap());
}

#[test]
fn a_session_folded_just_before_the_import_died_counts_as_imported() {
    let root = TempDir::new("history-cancelled-after-fold");
    let mut f = HistoryFixture::new(root.path());
    old_logs(&f, 2);
    f.detach();
    f.mark_running();
    f.after_fold = Some(|_, sid| {
        if sid == "old0" {
            Err(history::Error::Cancelled)
        } else {
            Ok(())
        }
    });
    assert!(matches!(
        f.run(true, history::BUDGET_S),
        Err(history::Error::Cancelled)
    ));
    assert_eq!(
        f.ctx.store.read("history.json", json!({})).unwrap()["next"],
        0
    );
    let result = f.run(true, history::BUDGET_S).unwrap();
    assert_eq!(result["state"], "done");
    assert_eq!(result["imported"], 2);
    assert_eq!(result["skipped"], json!({}));
    assert_eq!(f.ledger_ids(), ["old0", "old1"]);
    let turns = f.ctx.store.read("envelope.json", json!({})).unwrap()["resolved_turns"].clone();
    assert_eq!(
        turns
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_f64().unwrap())
            .collect::<Vec<_>>(),
        [3.0, 3.0]
    );
}

#[test]
fn te_history_import_resumes_a_plan_that_was_stopped() {
    let root = TempDir::new("history-cli-resume");
    let mut f = HistoryFixture::new(root.path());
    old_logs(&f, 2);
    f.detach();
    f.clock_tick = Some(0.0);
    f.mark_running();
    assert_eq!(f.run(true, 1.5).unwrap()["state"], "partial");
    let mut doc = f.ctx.store.read("history.json", json!({})).unwrap();
    doc["state"] = json!("stopped");
    doc["reason"] = json!("history_import is off");
    f.ctx.store.write("history.json", &doc).unwrap();
    let out = f.cli(&["history", "import"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("imported 2 earlier session(s)")
    );
    assert_eq!(f.ledger_ids(), ["old0", "old1"]);
}

const UNUSED: [&str; 3] = ["CronCreate", "ShareOnboardingGuide", "Workflow"];

fn catalog(f: &HistoryFixture, tools: &[&str]) {
    let directory = f.ctx.root.join("hosts/claude");
    fs::create_dir_all(&directory).unwrap();
    fs::write(
        directory.join("catalog.json"),
        json!({"tools":tools}).to_string(),
    )
    .unwrap();
}

fn strings(value: &Value) -> Vec<String> {
    let mut items: Vec<_> = value
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s.as_str().unwrap().to_owned())
        .collect();
    items.sort();
    items
}

#[test]
fn imported_sessions_are_scored_against_their_own_tools() {
    let root = TempDir::new("history-own-tools");
    let mut f = HistoryFixture::new(root.path());
    catalog(&f, &["Bash", "Read", "Edit", "Write", "NewlyInstalled"]);
    for i in 0..3 {
        f.log(
            &format!("old{i}"),
            &f.looked(),
            LogOptions {
                age: 3600.0 * f64::from(10 - i),
                deferred: vec!["CronCreate".into()],
                ..Default::default()
            },
        );
    }
    f.mark_running();
    f.run(true, history::BUDGET_S).unwrap();
    let untool = f.ctx.store.read("untool.json", json!({})).unwrap();
    assert!(untool["streaks"].get("NewlyInstalled").is_none());
    assert!(
        !untool["catalog_tools"]
            .as_array()
            .unwrap()
            .contains(&json!("NewlyInstalled"))
    );
    assert_eq!(untool["streaks"]["CronCreate"], 3);
    assert_eq!(untool["imported_sessions"], 3);
}

#[test]
fn the_next_live_fold_under_auto_holds_nothing_that_rests_on_history() {
    let root = TempDir::new("history-toolprune-provenance");
    let mut f = HistoryFixture::new(root.path());
    catalog(
        &f,
        &[
            "Bash",
            "Read",
            "Edit",
            "Write",
            "AskUserQuestion",
            "CronCreate",
            "ShareOnboardingGuide",
            "Workflow",
        ],
    );
    for i in 0..6 {
        f.log(
            &format!("old{i}"),
            &f.looked(),
            LogOptions {
                age: 3600.0 * f64::from(10 - i),
                deferred: UNUSED.map(String::from).to_vec(),
                ..Default::default()
            },
        );
    }
    f.hook("first", "SessionStart", None);
    assert_eq!(
        f.ctx.store.read("history.json", json!({})).unwrap()["imported"],
        6
    );
    let untool = f.ctx.store.read("untool.json", json!({})).unwrap();
    assert_eq!(strings(&untool["proposal"]), UNUSED);
    assert!(untool.get("auto_pending").is_none());
    assert_eq!(strings(&untool["needs_apply"]), UNUSED);
    f.detach();
    let path = f.log("live0", &f.trap(1), LogOptions::default());
    f.live("live0", &path);
    let record = f.fold_live("live0");
    assert!(!record.is_null());
    assert_eq!(record["untool"]["pending"], false);
    let untool = f.ctx.store.read("untool.json", json!({})).unwrap();
    assert!(untool.get("auto_pending").is_none());
    assert_eq!(strings(&untool["needs_apply"]), UNUSED);
    assert!(!denylist::settings_path(&f.ctx.repo.root, "claude").exists());
    let notice = f.hook("second", "SessionStart", None)["systemMessage"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(notice.contains("3 tool(s) went unused"));
    assert!(notice.contains("counting sessions from before"));
    assert!(notice.contains("nothing changes until you run `/tokeneconomizer:toolprune apply`"));
    let out = f.cli(&["toolprune", "show"]);
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("waiting for `toolprune apply`")
    );
    for i in 1..6 {
        let path = f.log(&format!("live{i}"), &f.trap(i), LogOptions::default());
        f.live(&format!("live{i}"), &path);
        f.fold_live(&format!("live{i}"));
    }
    let doc: Value = serde_json::from_slice(
        &fs::read(denylist::settings_path(&f.ctx.repo.root, "claude")).unwrap(),
    )
    .unwrap();
    assert_eq!(strings(&doc["permissions"]["deny"]), UNUSED);
}

#[test]
fn an_import_calls_no_model_and_keeps_a_waiting_deny_list() {
    let root = TempDir::new("history-no-model");
    let mut f = HistoryFixture::new(root.path());
    old_logs(&f, 3);
    f.environment.env.extend(
        json!({"TE_MICROSKILLS":"on","TE_MICROSKILLS_PROPOSE_EVERY":"1"})
            .as_object()
            .unwrap()
            .clone(),
    );
    f.reload();
    let pending = json!({"items":["Workflow"],"root":f.ctx.repo.root,"since":1.0});
    f.ctx
        .store
        .write("untool.json", &json!({"auto_pending":pending}))
        .unwrap();
    f.mark_running();
    assert_eq!(f.run(true, history::BUDGET_S).unwrap()["imported"], 3);
    assert_eq!(f.proposer_calls, 0);
    assert_eq!(
        f.ctx.store.read("untool.json", json!({})).unwrap()["auto_pending"],
        pending
    );
    assert_eq!(
        f.ctx.store.read("injection.json", json!({})).unwrap()["imported_sessions"],
        3
    );
    let path = f.log("seen", &f.trap(7), LogOptions::default());
    f.live("seen", &path);
    f.fold_live("seen");
    assert!(f.proposer_calls > 0);
}

#[test]
fn an_import_ignores_and_keeps_the_deny_list_in_place() {
    let root = TempDir::new("history-keeps-deny");
    let mut f = HistoryFixture::new(root.path());
    f.environment
        .env
        .insert("TE_TOOLPRUNE_PROTECT".into(), json!("Workflow"));
    f.reload();
    catalog(
        &f,
        &[
            "Bash",
            "Read",
            "Edit",
            "Write",
            "CronCreate",
            "ShareOnboardingGuide",
            "Workflow",
        ],
    );
    assert_eq!(
        denylist::apply(
            &f.ctx.repo.root,
            &["Workflow".into(), "CronCreate".into()],
            &f.ctx.store,
            "claude",
            &mut denylist::NativeRuntime
        )
        .unwrap(),
        ["Workflow", "CronCreate"]
    );
    let path = denylist::settings_path(&f.ctx.repo.root, "claude");
    let before = fs::read(&path).unwrap();
    for i in 0..2 {
        f.log(
            &format!("old{i}"),
            &f.looked(),
            LogOptions {
                age: 3600.0 * f64::from(10 - i),
                deferred: UNUSED.map(String::from).to_vec(),
                ..Default::default()
            },
        );
    }
    f.mark_running();
    assert_eq!(f.run(true, history::BUDGET_S).unwrap()["imported"], 2);
    assert!(
        f.ctx
            .store
            .read_lines("ledger.jsonl", None)
            .unwrap()
            .iter()
            .all(|record| record["untool_applied"] == json!([]))
    );
    assert_eq!(
        f.ctx.store.read("untool.json", json!({})).unwrap()["streaks"]["CronCreate"],
        2
    );
    assert_eq!(fs::read(path).unwrap(), before);
    let mut applied = denylist::applied_for(&f.ctx.store, &f.ctx.repo.root, "claude").unwrap();
    applied.sort();
    assert_eq!(applied, ["CronCreate", "Workflow"]);
}

#[test]
fn the_envelope_is_seeded_from_turns_that_ended_green() {
    let root = TempDir::new("history-envelope");
    let mut f = HistoryFixture::new(root.path());
    let mut prompts = f.trap(0);
    prompts.extend(f.looked());
    f.log(
        "old0",
        &prompts,
        LogOptions {
            age: 7200.0,
            ..Default::default()
        },
    );
    f.log(
        "old1",
        &f.looked()[..1],
        LogOptions {
            age: 3600.0,
            interrupted: vec![0],
            ..Default::default()
        },
    );
    let mut prompts =
        vec![json!(["one",[["Read",{"file_path":f.ctx.repo.root.join("b.py")},"y",false]]])];
    prompts.extend(f.trap(2));
    f.log(
        "old2",
        &prompts,
        LogOptions {
            age: 1800.0,
            interrupted: vec![0],
            ..Default::default()
        },
    );
    f.mark_running();
    f.run(true, history::BUDGET_S).unwrap();
    let doc = f.ctx.store.read("envelope.json", json!({})).unwrap();
    assert_eq!(
        doc["resolved_turns"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_f64().unwrap())
            .collect::<Vec<_>>(),
        [3.0, 2.0, 3.0]
    );
    assert!(!f.ctx.store.base.join("events.jsonl").exists());
}

#[test]
fn imported_sessions_are_counted_apart() {
    let summary=accounting::summarize(&[json!({"kind":"session","effective_tokens":100.0,"requests":2}),json!({"kind":"session","effective_tokens":900.0,"requests":9,"imported":true,"overhead":{"optimizer_tokens":50}})]).unwrap();
    for (key, expected) in [
        ("sessions", 1.0),
        ("imported_sessions", 1.0),
        ("effective_tokens", 100.0),
        ("requests", 2.0),
        ("overhead_tokens", 0.0),
    ] {
        assert_eq!(summary[key].as_f64(), Some(expected), "{key}");
    }
    let root = TempDir::new("history-accounting");
    let f = HistoryFixture::new(root.path());
    old_logs(&f, 2);
    f.hook("new", "SessionStart", None);
    let path = f.log("new", &f.trap(5), LogOptions::default());
    f.hook("new", "SessionEnd", Some(&path));
    let out = f.cli(&["status"]);
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("sessions     1 folded (+2 from earlier history)"));
    assert!(text.contains("(1 sessions learned from, +2 from earlier history)"));
    assert!(text.contains("read 2 earlier session(s) from Claude Code's history"));
    let out = f.cli(&["status", "--json"]);
    let doc: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(doc["toolprune"]["sessions"], 1);
    assert_eq!(doc["toolprune"]["imported_sessions"], 2);
    assert_eq!(doc["ledger"]["sessions"], 1);
    assert_eq!(doc["history"]["imported"], 2);
    let out = f.cli(&["repos"]);
    assert!(
        String::from_utf8(out.stdout)
            .unwrap()
            .contains("claude 1 (+2 from earlier history)")
    );
}

#[test]
fn a_merge_carries_imported_sessions_along() {
    let root = TempDir::new("history-merge");
    let f = HistoryFixture::new(root.path());
    old_logs(&f, 1);
    f.hook("new", "SessionStart", None);
    let other = root.path().join("other");
    fs::create_dir(&other).unwrap();
    let dst = paths::repo_identity(&other, &f.environment.env, &f.environment.home).unwrap();
    fs::create_dir_all(f.ctx.root.join("repos").join(&dst.key).join("claude")).unwrap();
    assert!(
        repos::merge(&f.ctx.root, &f.ctx.repo.key, &json!(dst), 10.0)
            .unwrap()
            .is_some()
    );
    let store = Store::new(
        &paths::repo_state_dir(&dst, "claude", &f.environment.env, &f.environment.home).unwrap(),
    )
    .unwrap();
    assert_eq!(
        store.read("folded.json", json!({})).unwrap()["old0"]["imported"],
        true
    );
    assert!(
        store
            .read_lines("ledger.jsonl", None)
            .unwrap()
            .iter()
            .any(|row| row["imported"] == true)
    );
    assert_eq!(
        store.read("history.json", json!({})).unwrap()["state"],
        "done"
    );
}

#[test]
fn a_merge_into_a_repository_that_learned_more_does_not_fold_imported_sessions_again() {
    let root = TempDir::new("history-merge-live-destination");
    let mut f = HistoryFixture::new(root.path());
    old_logs(&f, 1);
    f.hook("new", "SessionStart", None);
    let source = f.ctx.repo.key.clone();
    let other = root.path().join("other");
    fs::create_dir(&other).unwrap();
    f.ctx.repo = paths::repo_identity(&other, &f.environment.env, &f.environment.home).unwrap();
    f.environment
        .env
        .insert("TE_HISTORY_IMPORT".into(), json!("off"));
    f.reload();
    for i in 0..2 {
        let path = f.log(&format!("d{i}"), &f.trap(i), LogOptions::default());
        f.hook(&format!("d{i}"), "SessionStart", Some(&path));
        f.hook(&format!("d{i}"), "SessionEnd", Some(&path));
    }
    let folded = f.ctx.store.read("folded.json", json!({})).unwrap();
    let mut ids: Vec<_> = folded
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    ids.sort();
    assert_eq!(ids, ["d0", "d1"]);
    let result = repos::merge(&f.ctx.root, &source, &json!(f.ctx.repo), 10.0)
        .unwrap()
        .unwrap();
    assert_eq!(result["claude"]["refold"], 0);
    assert_eq!(
        f.ctx.store.read("folded.json", json!({})).unwrap()["old0"]["imported"],
        true
    );
    assert!(
        f.ctx
            .store
            .read("sessions/old0.json", Value::Null)
            .unwrap()
            .is_null()
    );
}

#[test]
fn a_malformed_history_file_never_empties_the_session_start() {
    let root = TempDir::new("history-malformed-notice");
    let f = HistoryFixture::new(root.path());
    for i in 0..6 {
        let path = f.log(&format!("s{i}"), &f.trap(i), LogOptions::default());
        f.hook(&format!("s{i}"), "SessionStart", Some(&path));
        f.hook(&format!("s{i}"), "SessionEnd", Some(&path));
    }
    let path = f.ctx.store.base.join("history.json");
    for bad in [
        r#"{"state": "done", "noticed": false, "imported": {"x": 1}}"#,
        r#"{"state": "done", "noticed": false, "imported": "many"}"#,
        r#"{"state": ["running"], "heartbeat": "soon"}"#,
        "[1, 2]",
        "{not json",
    ] {
        fs::write(&path, bad).unwrap();
        let out = f.hook("n", "SessionStart", None);
        assert!(
            out["hookSpecificOutput"]["additionalContext"]
                .as_str()
                .unwrap()
                .contains(CLAIM),
            "{bad}"
        );
        assert!(!fold::import_blocks(&f.ctx).unwrap());
    }
    let mut hooks = hook_fixture::HookFixture {
        host: "claude".into(),
        inline_judge: false,
        inline_folds: false,
        fold_wait: 0.0,
        proposer_reply: None,
        proposer_models: Default::default(),
        judge_result: None,
        judge_digests: vec![],
        judge_jobs: vec![],
        defer_judge: false,
        paused_models: false,
        queue_wait: None,
        enqueue_after_post: false,
        fail_replay: false,
        refuse_enqueue: false,
        history_failure: Some("ValueError: a field history.json should not have".into()),
        repo_map: None,
        fold_settings: Box::new(denylist::NativeRuntime),
        session_runtime: None,
        repo: f.ctx.repo.root.clone(),
        environment: f.environment.clone(),
        store: f.ctx.store.clone(),
        folds: vec![],
    };
    let out=hooks.run(json!({"hook_event_name":"SessionStart","session_id":"n2","source":"startup","transcript_path":root.path().join("n2.jsonl"),"cwd":f.ctx.repo.root}));
    assert!(
        out["hookSpecificOutput"]["additionalContext"]
            .as_str()
            .unwrap()
            .contains(CLAIM)
    );
    let log = fs::read_to_string(f.ctx.root.join("errors.log")).unwrap();
    assert_eq!(
        log.matches("ValueError: a field history.json should not have")
            .count(),
        2
    );
}

#[test]
fn the_history_settings() {
    let root = TempDir::new("history-settings");
    let state = root.path().join("root");
    fs::create_dir(&state).unwrap();
    fs::write(
        state.join("config.json"),
        json!({"history_dirs":["/a","/b"],"history_import":"no","history_max_sessions":50})
            .to_string(),
    )
    .unwrap();
    let cfg = load_config(&state, None, &Default::default());
    assert_eq!(cfg.values["history_dirs"], json!(["/a", "/b"]));
    assert_eq!(cfg.values["history_import"], false);
    assert_eq!(cfg.values["history_max_sessions"], 50);
    assert_eq!(cfg.values["history_days"], 365);
    let env =
        json!({"TE_HISTORY_IMPORT":"on","TE_HISTORY_DAYS":"30","TE_HISTORY_MAX_SESSIONS":"0"});
    let cfg = load_config(&state, None, env.as_object().unwrap());
    assert_eq!(cfg.values["history_import"], true);
    assert_eq!(cfg.values["history_days"], 30);
    assert_eq!(cfg.values["history_max_sessions"], 50);
    assert!(
        cfg.warnings
            .iter()
            .any(|warning| warning.contains("history_max_sessions"))
    );
    let cfg = load_config(&state, None, &Default::default());
    let view = cli::config_view(&serde_json::to_value(cfg).unwrap()).unwrap();
    assert_eq!(view["history"]["state"], "off");
    assert_eq!(view["history"]["history_dirs"], json!(["/a", "/b"]));
}
