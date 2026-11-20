// Converted from reference/upstream/tests/test_live_hooks.py.
pub mod fixtures;
#[path = "support/live_hook_fixture.rs"]
pub mod live_fixture;
mod support;

use live_fixture::{HookFixture, cli, fixture, session, transcript};
use serde_json::{Value, json};
use std::{
    fs::{self, File, FileTimes, OpenOptions},
    io,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};
use support::TempDir;
use tokeneconomizer::{
    plugin::{adapter, cli as cli_handler, context::LiveContext, denylist, fold, history, hosts},
    runner::Runner,
    store::Store,
};

const UNUSED: [&str; 3] = ["CronCreate", "ShareOnboardingGuide", "Workflow"];

fn context(f: &HookFixture) -> LiveContext {
    LiveContext::build(
        &f.host,
        hosts::profile(&f.host).keeps_history,
        &f.repo,
        f.environment.env.clone(),
        &f.environment.home,
        Value::Null,
        false,
    )
    .unwrap()
}

fn pending_repo(root: &Path) -> HookFixture {
    let mut f = fixture(root, "claude");
    f.environment.env.remove("TE_TOOLPRUNE_QUIET_MINUTES");
    observe_unused_tools(&mut f);
    assert_eq!(untool(&f)["auto_pending"]["items"], json!(UNUSED));
    f
}

fn catalog(f: &HookFixture, tools: &[&str]) {
    let path = adapter::catalog_path(&f.environment.home.join("state"), &f.host);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, json!({"tools":tools}).to_string()).unwrap();
}

fn observe_unused_tools(f: &mut HookFixture) {
    catalog(
        f,
        &[
            "Bash",
            "Read",
            "Edit",
            "Write",
            "AskUserQuestion",
            "Workflow",
            "CronCreate",
            "ShareOnboardingGuide",
            "WebFetch",
        ],
    );
    for i in 0..5 {
        let sid = format!("u{i}");
        let prompts = [
            json!(["look", [["Read",{"file_path":f.repo.join("a.py")},"x",false],
                ["Bash",{"command":"ls"},"a.py",false]]]),
            json!(["and again", []]),
        ];
        fold_transcript(f, &sid, &prompts);
    }
}

fn fold_transcript(f: &mut HookFixture, sid: &str, prompts: &[Value]) {
    let log = transcript(&f.environment.home, sid);
    fixtures::write_claude_session(&log, prompts, "claude-opus-5-5");
    session(f, sid, &log, "startup");
}

fn fold_prompts(f: &mut HookFixture, sid: &str, count: usize) {
    let prompts: Vec<_> = (0..count)
        .map(|index| {
            let calls = if index == 0 {
                json!([["Bash", {"command":"ls"}, "a.py", false]])
            } else {
                json!([])
            };
            json!([format!("prompt {index}"), calls])
        })
        .collect();
    fold_transcript(f, sid, &prompts);
}

fn untool(f: &HookFixture) -> Value {
    f.store.read("untool.json", Value::Null).unwrap()
}

fn deny(f: &HookFixture) -> Vec<String> {
    let path = denylist::settings_path(&f.repo, &f.host);
    let doc = denylist::read_settings(&path).unwrap();
    let mut items: Vec<_> = doc["permissions"]["deny"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|item| item.as_str().unwrap().into())
        .collect();
    items.sort();
    items
}

fn apply(f: &HookFixture, items: &[&str]) -> Vec<String> {
    let items: Vec<_> = items.iter().map(|item| (*item).into()).collect();
    denylist::apply(
        &f.repo,
        &items,
        &f.store,
        &f.host,
        &mut denylist::NativeRuntime,
    )
    .unwrap()
}

fn start(f: &mut HookFixture, sid: &str, path: &Path) -> Value {
    f.main(json!({
        "hook_event_name":"SessionStart", "session_id":sid, "source":"startup",
        "transcript_path":path, "cwd":f.repo
    }))
}

fn propose(f: &mut HookFixture) {
    f.environment
        .env
        .insert("TE_TOOLPRUNE".into(), json!("propose"));
}

#[test]
fn with_propose_unused_tools_are_proposed_but_nothing_changes_until_applied() {
    let tmp = TempDir::new("toolprune-proposal");
    let mut f = fixture(tmp.path(), "claude");
    propose(&mut f);
    observe_unused_tools(&mut f);
    assert_eq!(untool(&f)["proposal"], json!(UNUSED));
    assert!(!denylist::settings_path(&f.repo, "claude").exists());
    let log = transcript(tmp.path(), "next");
    File::create(&log).unwrap();
    let value = start(&mut f, "n", &log);
    let notice = value["systemMessage"].as_str().unwrap();
    assert!(
        notice.contains("3 tool(s) went unused") && notice.contains("/tokeneconomizer:toolprune")
    );
    assert!(notice.contains("nothing changes until you run `/tokeneconomizer:toolprune apply`"));
}

#[test]
fn by_default_unused_tools_are_removed_from_the_next_session_and_it_says_so() {
    let tmp = TempDir::new("toolprune-auto");
    let mut f = fixture(tmp.path(), "claude");
    observe_unused_tools(&mut f);
    assert_eq!(deny(&f), UNUSED);
    assert_eq!(untool(&f)["proposal"], json!([]));
    let log = transcript(tmp.path(), "next");
    File::create(&log).unwrap();
    let output = start(&mut f, "n1", &log);
    let notice = output["systemMessage"].as_str().unwrap();
    assert!(notice.contains("removed 3 tool(s) this repository's sessions never used"));
    assert!(notice.contains("`/tokeneconomizer:toolprune restore` gives them back"));
    assert!(start(&mut f, "n2", &log).get("systemMessage").is_none());
}

#[test]
fn tools_given_back_stay_back_until_named_again() {
    let tmp = TempDir::new("toolprune-kept");
    let mut f = fixture(tmp.path(), "claude");
    catalog(&f, &["Bash", "Read", "Workflow", "CronCreate"]);
    for i in 0..5 {
        fold_prompts(&mut f, &format!("a{i}"), 2);
    }
    assert_eq!(deny(&f), ["CronCreate", "Workflow"]);
    let restored = cli(&f, &["toolprune", "restore"]);
    let output = String::from_utf8(restored.stdout).unwrap();
    assert!(restored.status.success(), "{output}");
    assert!(output.contains("kept from now on"));
    assert!(deny(&f).is_empty());
    for i in 0..6 {
        fold_prompts(&mut f, &format!("b{i}"), 2);
    }
    assert!(deny(&f).is_empty());
    let show = cli(&f, &["toolprune", "show"]);
    assert!(show.status.success());
    let text = String::from_utf8(show.stdout).unwrap();
    assert!(text.contains("proposed: -") && text.contains("kept, as you gave them back"));
    assert!(
        cli(&f, &["toolprune", "apply", "Workflow"])
            .status
            .success()
    );
    assert_eq!(deny(&f), ["Workflow"]);
    assert_eq!(
        denylist::kept_for(&f.store, &f.repo, "claude").unwrap(),
        ["CronCreate"]
    );
}

#[test]
fn ten_prompts_over_two_sessions_are_enough_and_one_sessions_are_not() {
    let tmp = TempDir::new("toolprune-two-sessions");
    let mut f = fixture(tmp.path(), "claude");
    propose(&mut f);
    let tools = ["Bash", "Read", "Workflow", "CronCreate"];
    catalog(&f, &tools);
    fold_prompts(&mut f, "long", 10);
    let data = untool(&f);
    assert_eq!(data["proposal"], json!([]));
    assert_eq!(data["streaks"]["Workflow"], 1);
    assert_eq!(data["prompt_streaks"]["Workflow"], 10);
    fold_prompts(&mut f, "short", 1);
    assert_eq!(untool(&f)["proposal"], json!(["CronCreate", "Workflow"]));

    for (count, expected) in [(9, json!([])), (10, json!(["CronCreate", "Workflow"]))] {
        let tmp = TempDir::new("toolprune-one-session");
        let mut solo = fixture(tmp.path(), "claude");
        propose(&mut solo);
        solo.environment
            .env
            .insert("TE_TOOLPRUNE_MIN_SESSIONS".into(), json!("1"));
        catalog(&solo, &tools);
        fold_prompts(&mut solo, "s", count);
        assert_eq!(untool(&solo)["proposal"], expected);
    }
}

#[test]
fn five_sessions_are_enough_however_short() {
    let tmp = TempDir::new("toolprune-five-sessions");
    let mut f = fixture(tmp.path(), "claude");
    propose(&mut f);
    catalog(&f, &["Bash", "Workflow"]);
    for i in 0..4 {
        fold_prompts(&mut f, &format!("s{i}"), 1);
    }
    assert_eq!(untool(&f)["proposal"], json!([]));
    fold_prompts(&mut f, "s4", 1);
    assert_eq!(untool(&f)["proposal"], json!(["Workflow"]));
}

#[test]
fn a_session_without_prompts_is_no_evidence() {
    let tmp = TempDir::new("toolprune-empty-session");
    let mut f = fixture(tmp.path(), "claude");
    propose(&mut f);
    catalog(&f, &["Bash", "Workflow"]);
    fold_prompts(&mut f, "some", 4);
    fold_prompts(&mut f, "none", 0);
    let data = untool(&f);
    assert_eq!(data["sessions"], 1);
    assert_eq!(data["streaks"]["Workflow"], 1);
    assert_eq!(data["prompt_streaks"]["Workflow"], 4);
}

#[test]
fn a_high_risk_tool_needs_three_times_the_prompts() {
    let tmp = TempDir::new("toolprune-high-risk");
    let mut f = fixture(tmp.path(), "claude");
    propose(&mut f);
    catalog(&f, &["Bash", "Workflow", "NotebookEdit"]);
    fold_prompts(&mut f, "a", 20);
    fold_prompts(&mut f, "b", 9);
    assert_eq!(untool(&f)["proposal"], json!(["Workflow"]));
    fold_prompts(&mut f, "c", 1);
    assert_eq!(untool(&f)["proposal"], json!(["NotebookEdit", "Workflow"]));
}

#[test]
fn a_protected_tool_is_never_left_out_and_a_rule_already_written_comes_back() {
    let tmp = TempDir::new("toolprune-protect-applied");
    let mut f = fixture(tmp.path(), "claude");
    let candidate = "mcp__headroom__headroom_retrieve";
    catalog(&f, &["Bash", candidate, "Workflow"]);
    let path = denylist::settings_path(&f.repo, "claude");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, json!({"permissions":{"deny":["Mine"]}}).to_string()).unwrap();
    for i in 0..5 {
        fold_prompts(&mut f, &format!("a{i}"), 3);
    }
    assert_eq!(deny(&f), ["Mine", "Workflow", candidate]);
    f.environment.env.insert(
        "TE_TOOLPRUNE_PROTECT".into(),
        json!("mcp__headroom__*, Nothing"),
    );
    fold_prompts(&mut f, "b", 3);
    assert_eq!(deny(&f), ["Mine", "Workflow"]);
    assert!(
        !untool(&f)["proposal"]
            .as_array()
            .unwrap()
            .contains(&json!(candidate))
    );
}

#[test]
fn the_web_is_never_proposed_and_an_old_deny_rule_for_it_is_given_back() {
    let tmp = TempDir::new("toolprune-web-protected");
    let mut f = fixture(tmp.path(), "claude");
    catalog(&f, &["Bash", "Read", "CronCreate", "WebFetch", "WebSearch"]);
    apply(&f, &["CronCreate", "WebFetch", "WebSearch"]);
    for i in 0..20 {
        fold_prompts(&mut f, &format!("w{i}"), 1);
    }
    assert_eq!(
        denylist::applied_for(&f.store, &f.repo, "claude").unwrap(),
        ["CronCreate"]
    );
    assert_eq!(deny(&f), ["CronCreate"]);
    let data = untool(&f);
    for tool in ["WebFetch", "WebSearch"] {
        assert!(!data["proposal"].as_array().unwrap().contains(&json!(tool)));
    }
}

#[test]
fn by_default_a_deny_list_waits_until_the_repositorys_sessions_have_been_idle() {
    let tmp = TempDir::new("toolprune-idle");
    let mut f = pending_repo(tmp.path());
    assert!(deny(&f).is_empty());
    let path = transcript(tmp.path(), "next");
    File::create(&path).unwrap();
    assert!(
        !start(&mut f, "n1", &path)["systemMessage"]
            .as_str()
            .unwrap_or_default()
            .contains("went unused")
    );
    let show = cli(&f, &["toolprune", "show"]);
    assert!(show.status.success());
    assert!(
        String::from_utf8(show.stdout)
            .unwrap()
            .contains("pending: written once this repository's sessions have been idle 60 minutes")
    );
    let ctx = context(&f);
    let now = history::now();
    let wait = fold::apply_pending(&ctx, now + 600.0, &mut f)
        .unwrap()
        .unwrap();
    assert!(wait > 2900.0 && wait <= 3000.0);
    assert!(deny(&f).is_empty());
    assert_eq!(
        fold::apply_pending(&ctx, now + 3700.0, &mut f).unwrap(),
        None
    );
    assert_eq!(deny(&f), UNUSED);
    assert!(untool(&f).get("auto_pending").is_none());
    assert!(
        start(&mut f, "n2", &path)["systemMessage"]
            .as_str()
            .unwrap()
            .contains("removed 3 tool(s)")
    );
}

#[test]
fn copilot_server_lists_are_not_filtered_as_tool_names() {
    let tmp = TempDir::new("toolprune-sql-server");
    let f = fixture(tmp.path(), "copilot");
    let servers = ["sql".into(), "github-mcp-server".into()];
    assert_eq!(fold::unprotected(&context(&f), &servers), servers);
}

#[test]
fn a_tool_protected_after_the_list_formed_is_not_written() {
    let tmp = TempDir::new("toolprune-protect-pending");
    let mut f = pending_repo(tmp.path());
    f.environment
        .env
        .insert("TE_TOOLPRUNE_PROTECT".into(), json!("Workflow,Share*"));
    let ctx = context(&f);
    assert_eq!(
        cli_handler::proposal(&ctx, &json!({"proposal":["CronCreate","Workflow"]})).unwrap(),
        ["CronCreate"]
    );
    assert_eq!(
        fold::apply_pending(&ctx, history::now() + 3700.0, &mut f).unwrap(),
        None
    );
    assert_eq!(deny(&f), ["CronCreate"]);
    assert_eq!(untool(&f)["proposal"], json!([]));
}

fn set_modified(path: &Path, at: SystemTime) {
    OpenOptions::new()
        .write(true)
        .open(path)
        .unwrap()
        .set_times(FileTimes::new().set_accessed(at).set_modified(at))
        .unwrap();
}

#[test]
fn a_session_still_writing_its_log_keeps_the_deny_list_waiting() {
    let tmp = TempDir::new("toolprune-log-active");
    let mut f = pending_repo(tmp.path());
    let now = SystemTime::now();
    let old = now - Duration::from_secs(7200);
    for i in 0..5 {
        set_modified(&transcript(tmp.path(), &format!("u{i}")), old);
        set_modified(&f.store.base.join(format!("sessions/u{i}.json")), old);
    }
    let active = transcript(tmp.path(), "u4");
    set_modified(&active, now);
    let ctx = context(&f);
    assert!(
        fold::apply_pending(&ctx, history::now(), &mut f)
            .unwrap()
            .unwrap()
            > 3500.0
    );
    assert!(deny(&f).is_empty());
    set_modified(&active, old);
    assert_eq!(
        fold::apply_pending(&ctx, history::now(), &mut f).unwrap(),
        None
    );
    assert_eq!(deny(&f), UNUSED);
}

#[test]
fn a_deny_list_that_waited_a_day_is_written_even_if_sessions_never_pause() {
    let tmp = TempDir::new("toolprune-timeout");
    let mut f = pending_repo(tmp.path());
    let mut data = untool(&f);
    let now = history::now();
    data["auto_pending"]["since"] = json!(now - fold::PENDING_MAX_S - 1.0);
    f.store.write("untool.json", &data).unwrap();
    assert_eq!(
        fold::apply_pending(&context(&f), now, &mut f).unwrap(),
        None
    );
    assert_eq!(deny(&f), UNUSED);
}

#[test]
fn a_pending_deny_list_leaves_out_what_was_given_back_meanwhile() {
    let tmp = TempDir::new("toolprune-restore-pending");
    let mut f = pending_repo(tmp.path());
    denylist::set_kept(
        &f.store,
        &f.repo,
        &f.host,
        &["Workflow".into()],
        &[],
        &mut denylist::NativeRuntime,
    )
    .unwrap();
    assert_eq!(
        fold::apply_pending(&context(&f), history::now() + 3700.0, &mut f).unwrap(),
        None
    );
    assert_eq!(deny(&f), ["CronCreate", "ShareOnboardingGuide"]);
}

#[test]
fn a_pending_deny_list_is_dropped_once_toolprune_is_no_longer_auto() {
    let tmp = TempDir::new("toolprune-mode-pending");
    let mut f = pending_repo(tmp.path());
    f.environment
        .env
        .insert("TE_TOOLPRUNE".into(), json!("propose"));
    assert_eq!(
        fold::apply_pending(&context(&f), history::now() + 3700.0, &mut f).unwrap(),
        None
    );
    assert!(deny(&f).is_empty());
    let data = untool(&f);
    assert!(data.get("auto_pending").is_none());
    assert_eq!(data["proposal"], json!(UNUSED));
}

#[test]
fn the_fold_leaves_one_detached_waiter_and_a_second_one_leaves() {
    #[derive(Default)]
    struct Spawn {
        argv: Option<Vec<String>>,
    }
    impl fold::Runtime for Spawn {
        fn host(&mut self, method: &str, _: Value) -> fold::Result<Value> {
            panic!("Unexpected host operation: {method}")
        }
        fn proposer(&mut self, _: &LiveContext) -> fold::Result<Option<Box<dyn Runner>>> {
            panic!("Unexpected proposer")
        }
        fn spawn(&mut self, argv: &[String], _: &str) -> fold::Result<Value> {
            assert!(self.argv.replace(argv.to_vec()).is_none());
            Ok(Value::Null)
        }
        fn fold(&mut self, _: &LiveContext, _: &str, _: Option<&str>) -> fold::Result<Value> {
            panic!("Unexpected session fold")
        }
    }
    let tmp = TempDir::new("toolprune-waiter");
    let mut f = pending_repo(tmp.path());
    let mut detached = context(&f);
    detached.env.remove("TE_FOLD_SYNC");
    let mut spawn = Spawn::default();
    assert!(
        fold::schedule_pending(&detached, &mut spawn)
            .unwrap()
            .unwrap()
            > 0.0
    );
    assert_eq!(spawn.argv.unwrap(), ["toolprune", "apply-pending"]);

    let heartbeat = f.store.base.join("toolprune-waiter");
    File::create(&heartbeat).unwrap();
    let args = ["toolprune", "apply-pending"];
    assert!(cli(&f, &args).status.success());
    assert!(deny(&f).is_empty() && heartbeat.exists());
    fs::remove_file(&heartbeat).unwrap();
    f.environment
        .env
        .insert("TE_TOOLPRUNE_QUIET_MINUTES".into(), json!("0"));
    assert!(cli(&f, &args).status.success());
    assert_eq!(deny(&f), UNUSED);
    assert!(!heartbeat.exists());
}

#[test]
fn apply_and_restore_touch_only_the_plugins_own_rules() {
    let tmp = TempDir::new("toolprune-ownership");
    let f = fixture(tmp.path(), "claude");
    let path = denylist::settings_path(&f.repo, "claude");
    let user_rule = "Bash(rm -rf:*)";
    let original = json!({"permissions":{"deny":[user_rule],"allow":["Read"]},"model":"opus"});
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, original.to_string()).unwrap();
    assert_eq!(
        apply(&f, &["Workflow", "CronCreate"]),
        ["Workflow", "CronCreate"]
    );
    let doc = denylist::read_settings(&path).unwrap();
    assert_eq!(
        doc["permissions"]["deny"],
        json!([user_rule, "Workflow", "CronCreate"])
    );
    assert_eq!(doc["model"], "opus");
    assert_eq!(doc["permissions"]["allow"], json!(["Read"]));
    let mut runtime = denylist::NativeRuntime;
    assert_eq!(
        denylist::restore(
            &f.repo,
            &f.store,
            &["CronCreate".into()],
            "claude",
            &mut runtime
        )
        .unwrap(),
        ["CronCreate"]
    );
    assert_eq!(
        denylist::restore(&f.repo, &f.store, &[], "claude", &mut runtime).unwrap(),
        ["Workflow"]
    );
    assert_eq!(denylist::read_settings(&path).unwrap(), original);
    let excluded = fs::read_to_string(f.repo.join(".git/info/exclude")).unwrap_or_default();
    assert!(!excluded.contains("settings.local.json"));
}

#[test]
fn restore_takes_back_a_rule_whichever_write_of_apply_failed() {
    struct FailWrite {
        record: bool,
    }
    impl denylist::Runtime for FailWrite {
        fn write_record(&mut self, store: &Store, doc: &Value) -> Result<(), denylist::Error> {
            if self.record {
                return Err(io::Error::from(io::ErrorKind::PermissionDenied).into());
            }
            denylist::Runtime::write_record(&mut denylist::NativeRuntime, store, doc)
        }
        fn write_json(&mut self, path: &Path, doc: &Value) -> Result<(), denylist::Error> {
            if !self.record {
                return Err(io::Error::from(io::ErrorKind::PermissionDenied).into());
            }
            denylist::Runtime::write_json(&mut denylist::NativeRuntime, path, doc)
        }
    }
    let tmp = TempDir::new("toolprune-failed-write");
    let f = fixture(tmp.path(), "claude");
    for record in [true, false] {
        let result = denylist::apply(
            &f.repo,
            &["Workflow".into()],
            &f.store,
            "claude",
            &mut FailWrite { record },
        );
        assert!(
            matches!(result, Err(denylist::Error::Io(e)) if e.kind() == io::ErrorKind::PermissionDenied)
        );
        denylist::restore(
            &f.repo,
            &f.store,
            &[],
            "claude",
            &mut denylist::NativeRuntime,
        )
        .unwrap();
        assert!(!deny(&f).contains(&"Workflow".into()));
        assert!(
            denylist::applied_for(&f.store, &f.repo, "claude")
                .unwrap()
                .is_empty()
        );
    }
}

#[test]
fn a_settings_file_the_plugin_creates_is_kept_out_of_git() {
    let tmp = TempDir::new("toolprune-exclude");
    let f = fixture(tmp.path(), "claude");
    let exclude = f.repo.join(".git/info/exclude");
    apply(&f, &["Workflow"]);
    let rules = fs::read_to_string(&exclude).unwrap();
    assert!(
        rules
            .split_whitespace()
            .any(|rule| rule == "/.claude/settings.local.json")
    );
    apply(&f, &["CronCreate"]);
    assert_eq!(
        fs::read_to_string(exclude)
            .unwrap()
            .matches("/.claude/settings.local.json")
            .count(),
        1
    );
}

fn trust_config(f: &mut HookFixture) -> PathBuf {
    let (variable, name) = if f.host == "codex" {
        ("CODEX_HOME", "config.toml")
    } else {
        ("COPILOT_HOME", "config.json")
    };
    let home = f.environment.home.join(format!("{}-home", f.host));
    fs::create_dir(&home).unwrap();
    f.environment.env.insert(variable.into(), json!(home));
    home.join(name)
}

fn codex_trust(f: &HookFixture, repo: &Path) -> Option<bool> {
    denylist::codex_project_trusted(repo, &f.environment.env, &f.environment.home).unwrap()
}

fn copilot_trust(f: &HookFixture, repo: &Path) -> Option<bool> {
    denylist::copilot_folder_trusted(repo, &f.environment.env, &f.environment.home).unwrap()
}

#[test]
fn codex_trust_is_read_from_the_users_config() {
    let tmp = TempDir::new("codex-trust-user");
    let mut f = fixture(tmp.path(), "codex");
    let path = trust_config(&mut f);
    let repo = f.repo.display();
    assert_eq!(codex_trust(&f, &f.repo), None);
    fs::write(
        &path,
        format!("[projects.\"{repo}\"]\ntrust_level = \"trusted\"\n"),
    )
    .unwrap();
    assert_eq!(codex_trust(&f, &f.repo), Some(true));
    fs::write(&path, format!("[projects.\"{repo}/sub\"]\ntrust_level = \"trusted\"\n[projects.\"{repo}\"]\ntrust_level = \"untrusted\"\n")).unwrap();
    assert_eq!(codex_trust(&f, &f.repo), Some(false));
}

#[test]
fn codex_trust_of_a_linked_worktree_is_its_clones() {
    let tmp = TempDir::new("codex-trust-worktree");
    let mut f = fixture(tmp.path(), "codex");
    let path = trust_config(&mut f);
    let git = f.repo.join(".git/worktrees/feature");
    fs::create_dir_all(&git).unwrap();
    fs::write(git.join("commondir"), "../..").unwrap();
    let worktree = tmp.path().join("feature");
    fs::create_dir(&worktree).unwrap();
    fs::write(
        worktree.join(".git"),
        format!("gitdir: {}\n", git.display()),
    )
    .unwrap();
    let trust = format!(
        "[projects.'{}']\ntrust_level = \"trusted\"\n",
        f.repo.display()
    );
    fs::write(&path, &trust).unwrap();
    assert_eq!(codex_trust(&f, &worktree), Some(true));
    fs::write(
        &path,
        format!(
            "{trust}[projects.'{}']\ntrust_level = \"untrusted\"\n",
            worktree.display()
        ),
    )
    .unwrap();
    assert_eq!(codex_trust(&f, &worktree), Some(false));
    fs::write(
        &path,
        format!(
            "[projects.'{}']\ntrust_level = \"trusted\"\n",
            tmp.path().display()
        ),
    )
    .unwrap();
    assert_eq!(codex_trust(&f, &worktree), Some(false));
}

#[cfg(windows)]
#[test]
fn codex_trust_matches_the_lowercased_path_codex_writes_on_windows() {
    let tmp = TempDir::new("Codex-Trust-Case");
    let mut f = fixture(tmp.path(), "codex");
    let path = trust_config(&mut f);
    let lowercase = f.repo.to_str().unwrap().to_lowercase();
    assert_ne!(f.repo.to_str().unwrap(), lowercase);
    fs::write(
        path,
        format!("[projects.'{lowercase}']\ntrust_level = \"trusted\"\n"),
    )
    .unwrap();
    assert_eq!(codex_trust(&f, &f.repo), Some(true));
}

#[test]
fn copilot_trust_is_read_from_its_config() {
    let tmp = TempDir::new("copilot-trust-user");
    let mut f = fixture(tmp.path(), "copilot");
    let path = trust_config(&mut f);
    assert_eq!(copilot_trust(&f, &f.repo), None);
    let data = json!({"trustedFolders":[tmp.path()]});
    fs::write(&path, format!("// User settings belong in settings.json.\n// This file is managed automatically.\n{data}")).unwrap();
    assert_eq!(copilot_trust(&f, &f.repo), Some(true));
    fs::write(
        &path,
        json!({"trustedFolders":[f.repo.with_file_name("repo-other")]}).to_string(),
    )
    .unwrap();
    assert_eq!(copilot_trust(&f, &f.repo), Some(false));
}

#[cfg(windows)]
#[test]
fn copilot_trust_matches_paths_with_different_case_on_windows() {
    let tmp = TempDir::new("copilot-trust-case");
    let mut f = fixture(tmp.path(), "copilot");
    let path = trust_config(&mut f);
    let repo = tmp.path().join("MixedCaseRepo");
    fs::create_dir(&repo).unwrap();
    let lowercase = repo.to_str().unwrap().to_lowercase();
    assert_ne!(repo.to_str().unwrap(), lowercase);
    fs::write(path, json!({"trustedFolders":[lowercase]}).to_string()).unwrap();
    assert_eq!(copilot_trust(&f, &repo), Some(true));
}
