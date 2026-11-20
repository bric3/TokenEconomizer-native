// Converted from reference/upstream/tests/test_hook_queue.py.
#[path = "support/hook_fixture.rs"]
pub mod hook_fixture;
mod support;

use hook_fixture::HookFixture;
use serde_json::{Value, json};
#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;
use std::{
    fs::{self, File, FileTimes, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant, SystemTime},
};
use support::TempDir;
use tokeneconomizer::{
    plugin::{
        hookqueue::{self, Hook, Limits, Replay},
        paths,
    },
    store::Store,
};

fn fixture(root: &Path) -> HookFixture {
    let mut f = HookFixture::new(root, "claude");
    fs::write(f.repo.join("pyproject.toml"), "[project]\nname='w'\n").unwrap();
    f.environment.env.remove("TE_LOOPGUARD");
    f.environment.env.extend(
        json!({"TE_MICROSKILLS":"envfacts","TE_TOOLPRUNE_QUIET_MINUTES":"0"})
            .as_object()
            .unwrap()
            .clone(),
    );
    f.inline_judge = true;
    f.queue_wait = Some(0.01);
    f
}

fn base(f: &HookFixture, sid: &str, key: &str) -> Value {
    json!({"session_id":sid,"transcript_path":"","cwd":f.repo,"prompt_id":key})
}

fn post(mut base: Value, call: &Value) -> Value {
    let failed = call[2]
        .as_object()
        .is_some_and(|obj| obj.len() == 1 && obj.contains_key("error"));
    base["hook_event_name"] = json!(if failed {
        "PostToolUseFailure"
    } else {
        "PostToolUse"
    });
    base["tool_name"] = call[0].clone();
    base["tool_input"] = call[1].clone();
    base[if failed { "error" } else { "tool_response" }] = if failed {
        call[2]["error"].clone()
    } else {
        call[2].clone()
    };
    base
}

fn read_call() -> Value {
    json!(["Read",{"file_path":"/r/a.py"},"ok"])
}

fn prompt(f: &HookFixture, sid: &str, key: &str) -> Value {
    let mut p = base(f, sid, key);
    p["hook_event_name"] = json!("UserPromptSubmit");
    p["prompt"] = json!("go");
    p
}

fn stop(f: &HookFixture, sid: &str, key: &str) -> Value {
    let mut p = base(f, sid, key);
    p["hook_event_name"] = json!("Stop");
    p["stop_hook_active"] = json!(false);
    p
}

fn locks(f: &HookFixture) -> PathBuf {
    Path::new(f.environment.env["TOKENECONOMIZER_HOME"].as_str().unwrap()).join("locks/claude")
}

fn queue(f: &HookFixture, sid: &str) -> PathBuf {
    locks(f).join(format!("{sid}.queue"))
}

fn waiting(f: &HookFixture) -> usize {
    fs::read_dir(queue(f, "s"))
        .into_iter()
        .flatten()
        .filter(|entry| {
            entry
                .as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|ext| ext == "json")
        })
        .count()
}

fn behind_lock(f: &mut HookFixture, payload: Value) -> Value {
    let lock = Store::new(&locks(f)).unwrap().locked("s", 2.0).unwrap();
    assert!(lock.acquired);
    let output = f.run(payload);
    lock.release().unwrap();
    output
}

fn document(f: &HookFixture) -> Value {
    f.store.read("sessions/s.json", json!({})).unwrap()
}

fn events(f: &HookFixture, kind: &str) -> Vec<Value> {
    f.store
        .read_lines("events.jsonl", None)
        .unwrap()
        .into_iter()
        .filter(|event| event["kind"] == kind && event["session_id"] == "s")
        .collect()
}

fn wait(mut child: Child) -> Output {
    let started = Instant::now();
    loop {
        if child.try_wait().unwrap().is_some() {
            return child.wait_with_output().unwrap();
        }
        if started.elapsed() > Duration::from_secs(60) {
            child.kill().unwrap();
            panic!(
                "native hook exceeded 60 seconds: {:?}",
                child.wait_with_output().unwrap()
            );
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn tool_calls_made_in_parallel_are_all_recorded() {
    let root = TempDir::new("queue-parallel");
    let home = root.path().join("home");
    let state = root.path().join("state");
    let cwd = root.path().join("work");
    for dir in [&home, &state, &cwd] {
        fs::create_dir(dir).unwrap();
    }
    let command = || {
        let mut command = Command::new(env!("CARGO_BIN_EXE_te-hook"));
        command
            .env_clear()
            .env("TOKENECONOMIZER_HOME", &state)
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("CLAUDE_CONFIG_DIR", home.join(".claude"))
            .env("GIT_CEILING_DIRECTORIES", root.path())
            .env("PATH", "")
            .current_dir(&cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    };
    let payload = |i| json!({"hook_event_name":"PostToolUse","session_id":"race","cwd":cwd,"transcript_path":cwd.join("t.jsonl"),"tool_name":"Read","tool_input":{"file_path":cwd.join(format!("f{i}"))},"tool_response":{"type":"text"}});
    let mut first = command().spawn().unwrap();
    writeln!(first.stdin.take().unwrap(), "{}", payload(0)).unwrap();
    assert!(wait(first).status.success());
    let mut hooks: Vec<_> = (0..8).map(|_| command().spawn().unwrap()).collect();
    for (i, child) in hooks.iter_mut().enumerate() {
        writeln!(child.stdin.take().unwrap(), "{}", payload(i + 1)).unwrap();
    }
    for child in hooks {
        let out = wait(child);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(
            String::from_utf8(out.stdout).unwrap().trim(),
            "{}",
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let env = json!({"TOKENECONOMIZER_HOME":state,"GIT_CEILING_DIRECTORIES":root.path()});
    let id = paths::repo_identity(&cwd, env.as_object().unwrap(), &home).unwrap();
    let store =
        Store::new(&paths::repo_state_dir(&id, "claude", env.as_object().unwrap(), &home).unwrap())
            .unwrap();
    assert_eq!(
        store.read("sessions/race.json", json!({})).unwrap()["turn"]["cum_turns"],
        9
    );
    let mut documents = Vec::new();
    for repo in fs::read_dir(state.join("repos")).unwrap() {
        let repo = repo.unwrap().path();
        if !repo.is_dir() {
            continue;
        }
        for host in fs::read_dir(repo).unwrap() {
            let path = host.unwrap().path().join("sessions/race.json");
            if path.is_file() {
                documents.push(path);
            }
        }
    }
    assert_eq!(documents, [store.base.join("sessions/race.json")]);
    assert!(!state.join("errors.log").exists());
}

#[test]
fn a_hook_without_the_lock_queues_and_the_next_one_applies_it() {
    let root = TempDir::new("queue-next");
    let mut f = fixture(root.path());
    f.run(prompt(&f, "s", "k"));
    let p = post(base(&f, "s", "k"), &read_call());
    assert_eq!(behind_lock(&mut f, p.clone()), json!({}));
    assert_eq!(waiting(&f), 1);
    assert_eq!(document(&f)["turn"]["cum_turns"], 0);
    f.run(p);
    assert_eq!(waiting(&f), 0);
    assert_eq!(document(&f)["turn"]["cum_turns"], 2);
}

#[test]
fn the_holder_applies_what_queued_behind_it() {
    let root = TempDir::new("queue-holder");
    let mut f = fixture(root.path());
    f.run(prompt(&f, "s", "k"));
    f.enqueue_after_post = true;
    f.run(post(base(&f, "s", "k"), &read_call()));
    assert_eq!(waiting(&f), 0);
    assert_eq!(document(&f)["turn"]["cum_turns"], 2);
}

#[test]
fn queued_events_apply_oldest_first() {
    let root = TempDir::new("queue-order");
    let mut f = fixture(root.path());
    let p = prompt(&f, "s", "k");
    assert_eq!(behind_lock(&mut f, p), json!({}));
    let p = post(base(&f, "s", "k"), &read_call());
    assert_eq!(behind_lock(&mut f, p.clone()), json!({}));
    f.run(p);
    assert_eq!(document(&f)["turn"]["turn_key"], "k");
    assert_eq!(document(&f)["turn"]["cum_turns"], 2);
}

#[test]
fn session_start_answers_itself() {
    let root = TempDir::new("queue-start");
    let mut f = fixture(root.path());
    let mut p = base(&f, "s", "k");
    p["hook_event_name"] = json!("SessionStart");
    p["source"] = json!("startup");
    behind_lock(&mut f, p);
    assert_eq!(waiting(&f), 0);
    assert_eq!(document(&f)["source"], "startup");
}

#[test]
fn a_prompt_that_may_carry_the_map_is_not_queued() {
    assert!(hookqueue::queues("off", "UserPromptSubmit"));
    assert!(hookqueue::queues("start", "UserPromptSubmit"));
    assert!(!hookqueue::queues("on", "UserPromptSubmit"));
    assert!(hookqueue::queues("on", "PostToolUse"));
    assert!(!hookqueue::queues("off", "SessionStart"));
}

#[test]
fn a_hook_that_cannot_queue_runs() {
    let root = TempDir::new("queue-refused");
    let mut f = fixture(root.path());
    f.run(prompt(&f, "s", "k"));
    f.refuse_enqueue = true;
    let p = post(base(&f, "s", "k"), &read_call());
    behind_lock(&mut f, p);
    assert_eq!(document(&f)["turn"]["cum_turns"], 1);
}

#[test]
fn a_replay_that_fails_does_not_fail_the_hook_replaying_it() {
    let root = TempDir::new("queue-replay-failure");
    let mut f = fixture(root.path());
    f.run(prompt(&f, "s", "k"));
    let p = post(base(&f, "s", "k"), &read_call());
    behind_lock(&mut f, p.clone());
    f.fail_replay = true;
    assert_eq!(f.run(p), json!({}));
    assert_eq!(waiting(&f), 0);
    assert_eq!(document(&f)["turn"]["cum_turns"], 1);
    let home = Path::new(f.environment.env["TOKENECONOMIZER_HOME"].as_str().unwrap());
    assert!(
        fs::read_to_string(home.join("errors.log"))
            .unwrap()
            .contains("RuntimeError: replayed")
    );
}

#[test]
fn an_unreadable_queue_entry_is_dropped() {
    let root = TempDir::new("queue-unreadable");
    let mut f = fixture(root.path());
    f.run(prompt(&f, "s", "k"));
    fs::create_dir_all(queue(&f, "s")).unwrap();
    fs::write(queue(&f, "s").join("0.json"), "{not json").unwrap();
    f.run(post(base(&f, "s", "k"), &read_call()));
    assert_eq!(waiting(&f), 0);
    assert_eq!(document(&f)["turn"]["cum_turns"], 1);
}

#[test]
fn every_wait_is_short_of_every_agents_hook_timeout() {
    let root = TempDir::new("queue-package-timeouts");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("reference/upstream");
    let binaries = Path::new(env!("CARGO_BIN_EXE_te")).parent().unwrap();
    let plugin = tokeneconomizer::package::build(
        &source,
        Some(binaries),
        &root.path().join("marketplace"),
        false,
    )
    .unwrap();
    let mut seen = 0;
    for entry in fs::read_dir(plugin.join("hooks")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        let doc: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        let Some(hooks) = doc["hooks"].as_object() else {
            continue;
        };
        for (event, groups) in hooks {
            let Some((_, wait)) = hookqueue::WAITS.iter().find(|(name, _)| name == event) else {
                continue;
            };
            for group in groups.as_array().unwrap() {
                for hook in group["hooks"].as_array().unwrap() {
                    if let Some(timeout) =
                        hook["timeout"].as_f64().filter(|timeout| *timeout != 0.0)
                    {
                        seen += 1;
                        assert!(*wait + 1.0 < timeout, "{event}: {wait} >= {timeout} - 1");
                    }
                }
            }
        }
    }
    assert!(seen >= 15);
}

fn set_modified(path: &Path, time: SystemTime) {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    // Windows needs FILE_WRITE_ATTRIBUTES and FILE_FLAG_BACKUP_SEMANTICS for directories.
    options.access_mode(0x0100).custom_flags(0x02000000);
    options
        .open(path)
        .unwrap()
        .set_times(FileTimes::new().set_accessed(time).set_modified(time))
        .unwrap();
}

#[test]
fn idle_locks_go_and_a_queue_with_events_stays() {
    let root = TempDir::new("queue-prune");
    let old = SystemTime::now() - Duration::from_secs_f64(hookqueue::IDLE_S + 60.0);
    for name in [".gone.lock", ".kept.lock"] {
        File::create(root.path().join(name)).unwrap();
    }
    for name in ["gone.queue", "kept.queue"] {
        fs::create_dir(root.path().join(name)).unwrap();
    }
    File::create(root.path().join("kept.queue/1.json")).unwrap();
    for name in [".gone.lock", "gone.queue", "kept.queue"] {
        set_modified(&root.path().join(name), old);
    }
    hookqueue::prune(root.path(), hookqueue::IDLE_S);
    let mut names: Vec<_> = fs::read_dir(root.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name != ".pruned")
        .collect();
    names.sort();
    assert_eq!(names, [".kept.lock", "kept.queue"]);
}

#[test]
fn the_prune_leaves_the_lock_of_a_session_older_than_idle() {
    let root = TempDir::new("queue-active-prune");
    let base = root.path().join("locks/claude");
    fs::create_dir_all(&base).unwrap();
    let lock = base.join(".s.lock");
    File::create(&lock).unwrap();
    let old = SystemTime::now() - Duration::from_secs_f64(hookqueue::IDLE_S + 60.0);
    set_modified(&lock, old);
    struct Probe {
        base: PathBuf,
        beside: Vec<bool>,
    }
    impl hookqueue::Runtime for Probe {
        fn prepare_replay(&mut self, _: &Value, _: &str) -> Result<Replay, String> {
            panic!("Unexpected replay")
        }
        fn replay_handler(&mut self, _: bool) -> Result<(), String> {
            panic!("Unexpected replay")
        }
        fn record_error(&mut self, error: &str) -> Result<(), String> {
            panic!("Unexpected replay error: {error}")
        }
        fn handler(&mut self) -> Result<Value, String> {
            let lock = Store::new(&self.base).unwrap().locked("s", 0.1).unwrap();
            self.beside.push(lock.acquired);
            lock.release().unwrap();
            Ok(json!({}))
        }
    }
    let mut probe = Probe {
        base,
        beside: vec![],
    };
    assert_eq!(
        hookqueue::run(
            Hook {
                root: root.path(),
                host: "claude",
                session_id: "s",
                repomap: "off",
                event: "PostToolUse",
                payload: &json!({})
            },
            &Limits::for_event("PostToolUse"),
            &mut probe
        )
        .unwrap(),
        json!({})
    );
    assert_eq!(probe.beside, [false]);
    assert!(
        fs::metadata(lock).unwrap().modified().unwrap()
            > old + Duration::from_secs_f64(hookqueue::IDLE_S)
    );
}

fn turn(f: &mut HookFixture, sid: &str, key: &str, calls: &[Value]) -> Vec<Value> {
    f.run(prompt(f, sid, key));
    let outputs = calls
        .iter()
        .map(|call| f.run(post(base(f, sid, key), call)))
        .collect();
    f.run(stop(f, sid, key));
    outputs
}

fn stuck_turn(f: &mut HookFixture) -> (Vec<Value>, usize, usize) {
    for i in 0..5 {
        turn(f, "warm", &format!("g{i}"), &vec![read_call(); 8]);
    }
    let calls: Vec<_> = (0..40).map(|i| if i % 2 == 0 { json!(["Edit",{"file_path":"/r/pkg/a.py","old_string":"x","new_string":"y"},"ok"]) }
        else { json!(["Bash",{"command":"pytest -q"},{"error":"Exit code 1\nFAILED tests/test_a.py::test_x - AssertionError: assert 1 == 2"}]) }).collect();
    let out = turn(f, "probe", "p", &calls);
    let nudge = out
        .iter()
        .position(|out| out.get("hookSpecificOutput").is_some())
        .unwrap();
    let notice = nudge
        + 1
        + out[nudge + 1..]
            .iter()
            .position(|out| out != &json!({}))
            .unwrap();
    (calls, nudge, notice)
}

fn run_until(f: &mut HookFixture, calls: &[Value], end: usize) -> Vec<Value> {
    f.run(prompt(f, "s", "k"));
    calls[..end]
        .iter()
        .map(|call| f.run(post(base(f, "s", "k"), call)))
        .collect()
}

#[test]
fn a_redirect_reached_on_a_queued_call_goes_out_with_the_next_one() {
    for ahead in [1, 2] {
        let root = TempDir::new("queue-redirect");
        let mut f = fixture(root.path());
        let (calls, nudge, _) = stuck_turn(&mut f);
        assert!(
            run_until(&mut f, &calls, nudge)
                .iter()
                .all(|out| out == &json!({}))
        );
        let p = post(base(&f, "s", "k"), &calls[nudge]);
        assert_eq!(behind_lock(&mut f, p), json!({}));
        assert!(events(&f, "nudge").is_empty());
        let p = post(base(&f, "s", "k"), &calls[nudge + ahead]);
        let event = p["hook_event_name"].clone();
        let out = f.run(p);
        assert_eq!(out["hookSpecificOutput"]["hookEventName"], event);
        assert!(
            out["hookSpecificOutput"]["additionalContext"]
                .as_str()
                .unwrap()
                .contains("Process check (automated)")
        );
        assert!(out["systemMessage"].as_str().unwrap().contains("nudged"));
        assert_eq!(events(&f, "nudge").len(), 1);
        assert_eq!(document(&f)["nudges"], 1);
    }
}

#[test]
fn a_redirect_held_for_the_next_call_is_said_once() {
    let root = TempDir::new("queue-redirect-once");
    let mut f = fixture(root.path());
    let (calls, nudge, _) = stuck_turn(&mut f);
    run_until(&mut f, &calls, nudge);
    for i in [nudge, nudge + 1] {
        let p = post(base(&f, "s", "k"), &calls[i]);
        behind_lock(&mut f, p);
    }
    assert_eq!(waiting(&f), 2);
    let outs: Vec<_> = calls[nudge + 2..nudge + 4]
        .iter()
        .map(|call| f.run(post(base(&f, "s", "k"), call)))
        .collect();
    assert!(outs[0].get("hookSpecificOutput").is_some());
    assert_eq!(outs[1], json!({}));
    assert_eq!(events(&f, "nudge").len(), 1);
}

#[test]
fn a_turn_that_ends_first_drops_the_redirect_uncounted() {
    let root = TempDir::new("queue-redirect-dropped");
    let mut f = fixture(root.path());
    let (calls, nudge, _) = stuck_turn(&mut f);
    run_until(&mut f, &calls, nudge);
    let p = post(base(&f, "s", "k"), &calls[nudge]);
    behind_lock(&mut f, p);
    f.run(stop(&f, "s", "k"));
    let session = document(&f);
    assert_eq!(session["nudges"], 0);
    assert_eq!(session["turns"][0]["nudges"], 0);
    assert!(events(&f, "nudge").is_empty());
    assert_eq!(
        events(&f, "undelivered")
            .iter()
            .map(|event| event["action"].clone())
            .collect::<Vec<_>>(),
        [json!("nudge")]
    );
    f.run(prompt(&f, "s", "k2"));
    assert_eq!(f.run(post(base(&f, "s", "k2"), &read_call())), json!({}));
}

#[test]
fn the_stop_notice_reached_on_a_queued_call_goes_out_too() {
    let root = TempDir::new("queue-stop-notice");
    let mut f = fixture(root.path());
    let (calls, _, notice) = stuck_turn(&mut f);
    run_until(&mut f, &calls, notice);
    let p = post(base(&f, "s", "k"), &calls[notice]);
    assert_eq!(behind_lock(&mut f, p), json!({}));
    let out = f.run(post(base(&f, "s", "k"), &calls[notice + 1]));
    assert!(
        out["systemMessage"]
            .as_str()
            .unwrap()
            .contains("still looks stuck")
    );
    assert_eq!(document(&f)["aborts"], 1);
    assert_eq!(
        f.run(post(base(&f, "s", "k"), &calls[notice + 2])),
        json!({})
    );
}
