// Converted from reference/upstream/tests/test_live_core.py (identity, storage and usage).
mod support;

use serde_json::{Map, Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};
use support::TempDir;
use tokeneconomizer::{
    plugin::{config, paths},
    store::Store,
    toolprune,
    trace::{calls, claude, codex},
};

fn git_repo(root: &Path, remote: Option<&str>) -> PathBuf {
    fs::create_dir_all(root.join(".git")).unwrap();
    fs::write(root.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
    let remote = remote.map_or_else(String::new, |remote| {
        format!(
            "[remote \"origin\"]\n\turl = {remote}\n\tfetch = +refs/heads/*:refs/remotes/origin/*\n"
        )
    });
    fs::write(
        root.join(".git/config"),
        format!("[core]\n\trepositoryformatversion = 0\n{remote}"),
    )
    .unwrap();
    root.into()
}

#[test]
fn repo_identity_follows_the_remote_not_the_spelling() {
    let root = TempDir::new("core-remote-spelling");
    let a = git_repo(
        &root.path().join("a"),
        Some("git@github.com:Owner/Repo.git"),
    );
    let b = git_repo(
        &root.path().join("b/repo"),
        Some("https://github.com/owner/repo"),
    );
    let a = paths::repo_identity(&a, &Map::new(), root.path()).unwrap();
    let b = paths::repo_identity(&b, &Map::new(), root.path()).unwrap();
    assert_eq!(a.remote, "github.com/owner/repo");
    assert_eq!(a.remote, b.remote);
    assert_eq!(a.label, "owner/repo");
    assert!(a.key.ends_with(b.key.rsplit('-').next().unwrap()));
}

#[test]
fn a_subdirectory_and_a_worktree_share_their_repository() {
    let root = TempDir::new("core-worktree");
    let main = git_repo(
        &root.path().join("main"),
        Some("https://github.com/o/r.git"),
    );
    fs::create_dir_all(main.join("src/pkg")).unwrap();
    let wt_git = main.join(".git/worktrees/feature");
    fs::create_dir_all(&wt_git).unwrap();
    fs::write(wt_git.join("commondir"), "../..").unwrap();
    let worktree = root.path().join("feature");
    fs::create_dir(&worktree).unwrap();
    fs::write(
        worktree.join(".git"),
        format!("gitdir: {}\n", wt_git.display()),
    )
    .unwrap();
    let sub = paths::repo_identity(&main.join("src/pkg"), &Map::new(), root.path()).unwrap();
    assert_eq!(sub.root, main);
    assert_eq!(sub.remote, "github.com/o/r");
    assert_eq!(
        paths::repo_identity(&worktree, &Map::new(), root.path())
            .unwrap()
            .remote,
        sub.remote
    );
}

#[test]
fn a_directory_without_git_is_keyed_by_its_path() {
    let root = TempDir::new("core-without-git");
    let directory = root.path().join("scratch");
    fs::create_dir(&directory).unwrap();
    let environment = json!({"GIT_CEILING_DIRECTORIES":root.path()});
    let repo =
        paths::repo_identity(&directory, environment.as_object().unwrap(), root.path()).unwrap();
    assert_eq!(repo.remote, "");
    assert_eq!(repo.label, "scratch");
    assert_eq!(repo.root, directory);
}

#[test]
fn state_root_honours_the_environment() {
    let root = TempDir::new("core-state-root");
    assert_eq!(
        paths::state_root(
            json!({"TOKENECONOMIZER_HOME":root.path()})
                .as_object()
                .unwrap(),
            root.path()
        )
        .unwrap(),
        root.path()
    );
}

#[test]
fn store_writes_atomically_and_logs() {
    let root = TempDir::new("core-store");
    let store = Store::new(&root.path().join("st")).unwrap();
    store.write("a/b.json", &json!({"x":1})).unwrap();
    assert_eq!(store.read("a/b.json", Value::Null).unwrap(), json!({"x":1}));
    assert_eq!(store.read("missing.json", json!({})).unwrap(), json!({}));
    store.append("log.jsonl", &json!({"n":1})).unwrap();
    store.append("log.jsonl", &json!({"n":2})).unwrap();
    assert_eq!(
        store
            .read_lines("log.jsonl", None)
            .unwrap()
            .iter()
            .map(|row| row["n"].clone())
            .collect::<Vec<_>>(),
        [json!(1), json!(2)]
    );
    assert_eq!(
        store
            .read_lines("log.jsonl", Some(1))
            .unwrap()
            .iter()
            .map(|row| row["n"].clone())
            .collect::<Vec<_>>(),
        [json!(2)]
    );
    let x = store.locked("x", 2.0).unwrap();
    let got_x = x.acquired;
    let got_y;
    {
        let y = store.locked("y", 2.0).unwrap();
        got_y = y.acquired;
        store.write("c.json", &json!([1])).unwrap();
    }
    {
        let again = store.locked("x", 0.1).unwrap();
        assert!(!again.acquired);
    }
    drop(x);
    assert!(got_x && got_y);
    assert!(fs::read_dir(&store.base).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")
    }));
}

#[test]
fn config_layers_and_never_raises_on_bad_values() {
    let root = TempDir::new("core-config-layers");
    let repo = root.path().join("repos/r");
    fs::create_dir_all(&repo).unwrap();
    fs::write(
        root.path().join("config.json"),
        json!({"spiral":"guard","untool_evidence":7,"bogus":1}).to_string(),
    )
    .unwrap();
    fs::write(
        repo.join("config.json"),
        json!({"spiral":"nope","inject":"envfacts+distiller"}).to_string(),
    )
    .unwrap();
    let config = config::load_config(
        root.path(),
        Some(&repo),
        json!({"TE_UNTOOL":"auto"}).as_object().unwrap(),
    );
    assert_eq!(config.values["loopguard"], "stop");
    assert_eq!(config.values["toolprune_evidence"], 7);
    assert_eq!(config.values["toolprune"], "auto");
    assert_eq!(config.values["microskills"], "envfacts+distiller");
    let warnings = config.warnings.join(" ");
    assert!(warnings.contains("bogus") && warnings.contains("nope") && !warnings.contains("paid"));
    assert_eq!(
        config::load_config(
            root.path(),
            None,
            json!({"TE_DISABLE":"1"}).as_object().unwrap()
        )
        .values["enabled"],
        false
    );
}

#[test]
fn write_config_validates_before_writing() {
    let root = TempDir::new("core-config-write");
    let path = root.path().join("config.json");
    config::write_config(
        &path,
        json!({"loopguard":"off","paid_model":"haiku"})
            .as_object()
            .unwrap(),
    )
    .unwrap();
    let saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved, json!({"loopguard":"off","paid_model":"haiku"}));
    let error =
        config::write_config(&path, json!({"paid":"true"}).as_object().unwrap()).unwrap_err();
    assert!(error.contains("always asks its judge"));
    let saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert!(saved.get("paid").is_none());
    fs::write(&path, json!({"paid":true,"loopguard":"off"}).to_string()).unwrap();
    assert!(
        config::load_config(root.path(), None, &Map::new())
            .warnings
            .is_empty()
    );
    assert!(
        config::write_config(&path, json!({"loopguard":"sometimes"}).as_object().unwrap()).is_err()
    );
    let saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["loopguard"], "off");
}

fn claude_lines(path: &Path, blocks: usize, messages: usize, model: &str) -> PathBuf {
    let tag = path.file_stem().unwrap().to_str().unwrap();
    let mut lines = Vec::new();
    for message in 0..messages {
        for block in 0..blocks {
            lines.push(json!({"type":"assistant","message":{"id":format!("msg_{tag}_{message}"),"model":model,"role":"assistant",
            "usage":{"input_tokens":10,"cache_read_input_tokens":1000,"cache_creation_input_tokens":100,"output_tokens":50},
            "content":[{"type":"text","text":format!("block {block}")}]}}).to_string());
        }
    }
    fs::write(path, format!("{}\n", lines.join("\n"))).unwrap();
    path.into()
}

#[test]
fn claude_usage_counts_each_message_once_and_includes_subagents() {
    let root = TempDir::new("core-claude-subagents");
    let main = claude_lines(&root.path().join("sess.jsonl"), 3, 2, "claude-opus-5-5");
    let sub = root.path().join("sess/subagents");
    fs::create_dir_all(&sub).unwrap();
    claude_lines(&sub.join("agent-1.jsonl"), 2, 1, "claude-opus-5-5");
    let usage = claude::session_usage(&main, true).unwrap();
    assert_eq!(usage["requests"], 3);
    assert_eq!(usage["cache_read_input_tokens"].as_f64(), Some(3000.0));
    assert_eq!(usage["model"], "claude-opus-5-5");
    assert_eq!(usage["files"], 2);
    assert_eq!(claude::session_usage(&main, false).unwrap()["requests"], 2);
}

#[test]
fn claude_usage_leaves_out_the_notices_claude_code_writes() {
    let root = TempDir::new("core-claude-notices");
    let path = claude_lines(&root.path().join("sess.jsonl"), 3, 1, "claude-sonnet-4-5");
    {
        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        for (i, text) in ["API Error: Connection error.", "No response requested."]
            .into_iter()
            .enumerate()
        {
            writeln!(file,"{}",json!({"type":"assistant","isApiErrorMessage":i==0,"message":{"id":format!("synthetic-{i}"),"model":"<synthetic>","role":"assistant",
            "usage":{"input_tokens":0,"cache_read_input_tokens":0,"cache_creation_input_tokens":0,"output_tokens":0},"content":[{"type":"text","text":text}]}})).unwrap();
        }
    }
    let usage = claude::session_usage(&path, true).unwrap();
    assert_eq!(usage["requests"], 1);
    assert_eq!(usage["model"], "claude-sonnet-4-5");
    assert_eq!(usage["cache_read_input_tokens"].as_f64(), Some(1000.0));
}

#[test]
fn deferred_tools_are_read_from_attachments() {
    let root = TempDir::new("core-deferred-tools");
    let path = root.path().join("t.jsonl");
    let lines = [
        json!({"type":"attachment","attachment":{"type":"deferred_tools_delta","addedNames":["CronCreate","WebSearch"],"removedNames":[]}}),
        json!({"type":"attachment","attachment":{"type":"deferred_tools_delta","addedNames":[],"removedNames":["WebSearch"]}}),
    ];
    fs::write(
        &path,
        format!(
            "{}\n",
            lines
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        ),
    )
    .unwrap();
    assert_eq!(claude::deferred_tools(&path), ["CronCreate".into()].into());
}

fn rollout(path: &Path) -> PathBuf {
    let records = [
        json!({"type":"session_meta","payload":{"id":"thread-1","cwd":"/work/r","cli_version":"0.153.1"}}),
        json!({"type":"turn_context","payload":{"turn_id":"t1","model":"gpt-6-astra"}}),
        json!({"type":"event_msg","payload":{"type":"item_completed","turn_id":"t1","item":{"type":"CommandExecution","id":"c1",
            "command":["/bin/bash","-lc","pytest -q"],"aggregated_output":"1 failed","exit_code":1,"status":"completed"}}}),
        json!({"type":"event_msg","payload":{"type":"item_completed","turn_id":"t1","item":{"type":"FileChange","id":"f1",
            "changes":{"/work/r/pkg/a.py":{"type":"update"}},"status":"completed"}}}),
        json!({"type":"event_msg","payload":{"type":"item_completed","turn_id":"t1","item":{"type":"Reasoning","id":"r1"}}}),
        json!({"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1000,"cached_input_tokens":600,
            "cache_write_input_tokens":100,"output_tokens":40,"reasoning_output_tokens":10}}}}),
        json!({"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":3000,"cached_input_tokens":2000,
            "cache_write_input_tokens":200,"output_tokens":90,"reasoning_output_tokens":30}}}}),
    ];
    fs::write(
        path,
        format!(
            "{}\n",
            records
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        ),
    )
    .unwrap();
    path.into()
}

#[test]
fn codex_rollout_reads_through_the_existing_trace_seam() {
    let root = TempDir::new("core-codex-seam");
    let source = rollout(&root.path().join("rollout-x.jsonl"));
    let out = root.path().join("exec.jsonl");
    assert_eq!(codex::rollout_to_exec(&source, &out).unwrap(), Some(2));
    let trajectory = calls::load_trajectory(&out);
    assert_eq!(trajectory[0].0, "shell");
    assert_eq!(trajectory[0].1["command"], "/bin/bash -lc 'pytest -q'");
    assert_eq!(trajectory[0].2, "1 failed");
    assert_eq!(trajectory[0].3, false);
    assert_eq!(trajectory[1].0, "apply_patch");
    assert_eq!(trajectory[1].1["path"], "/work/r/pkg/a.py");
}

#[test]
fn codex_usage_is_converted_as_the_harness_converts_it() {
    let root = TempDir::new("core-codex-usage");
    let usage = codex::session_usage(&rollout(&root.path().join("rollout-x.jsonl"))).unwrap();
    assert_eq!(usage["input_tokens"].as_f64(), Some(800.0));
    assert_eq!(usage["cache_read_input_tokens"].as_f64(), Some(2000.0));
    assert_eq!(usage["cache_creation_input_tokens"].as_f64(), Some(200.0));
    assert_eq!(usage["output_tokens"].as_f64(), Some(120.0));
    assert_eq!(usage["model"], "gpt-6-astra");
}

#[test]
fn codex_capability_evidence_includes_the_models_own_exec_calls() {
    let root = TempDir::new("core-codex-capability");
    let source = rollout(&root.path().join("rollout-x.jsonl"));
    {
        let mut file = fs::OpenOptions::new().append(true).open(&source).unwrap();
        writeln!(file,"{}",json!({"type":"response_item","payload":{"type":"custom_tool_call","name":"exec","call_id":"call_9","status":"completed",
        "input":"const r = await tools.view_image({path: 'a.png'});"}})).unwrap();
    }
    let out = root.path().join("cap.jsonl");
    assert_eq!(codex::capability_trace(&source, &out).unwrap(), Some(3));
    let usage = toolprune::codex::inspect_usage(&out);
    assert_eq!(usage["completed"]["image_media"], 1);
    assert_eq!(usage["completed"]["collaboration"], 0);
    let plain = root.path().join("exec.jsonl");
    codex::rollout_to_exec(&source, &plain).unwrap();
    assert_eq!(
        toolprune::codex::inspect_usage(&plain)["completed"]["image_media"],
        0
    );
}

#[test]
fn a_compressed_rollout_is_reported_unreadable() {
    let root = TempDir::new("core-compressed-rollout");
    let source = root.path().join("rollout-x.jsonl.zst");
    fs::write(&source, b"\x28\xb5\x2f\xfd").unwrap();
    assert_eq!(
        codex::rollout_to_exec(&source, &root.path().join("o.jsonl")).unwrap(),
        None
    );
}
