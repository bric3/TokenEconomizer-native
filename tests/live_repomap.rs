// Converted from reference/upstream/tests/test_live_repomap.py.
pub mod fixtures;
#[path = "support/live_hook_fixture.rs"]
pub mod live_fixture;
mod support;

use live_fixture::{HookFixture, fixture};
use serde_json::{Map, Value, json};
use std::{fs, io, path::Path, process::Command};
use support::TempDir;
use tokeneconomizer::plugin::{
    config::LiveConfig,
    context::LiveContext,
    hosts::session::Environment,
    process,
    repomap::{self, RepoMap},
    sidecall,
};

struct Generator {
    named_python: String,
    available: bool,
    slow: bool,
}

impl sidecall::Runtime for Generator {
    fn which(&mut self, program: &str) -> Result<Option<String>, String> {
        panic!("Unexpected fallback interpreter lookup: {program}");
    }

    fn run(
        &mut self,
        argv: &[String],
        _: &Path,
        env: &Map<String, Value>,
        input: Option<&str>,
        _: f64,
    ) -> io::Result<process::Output> {
        assert!(input.is_none());
        let (code, stdout) = if argv == ["git", "rev-parse", "HEAD"] {
            (0, "abc123\n")
        } else {
            assert_eq!(argv, [&self.named_python, "-c", repomap::PROBE]);
            assert_eq!(env["TE_SIDECALL"], "1");
            assert!(!env.contains_key("PYTHONPATH"));
            (i32::from(!self.available), "")
        };
        Ok(process::Output {
            code,
            stdout: stdout.into(),
            stderr: String::new(),
        })
    }
}

impl repomap::Runtime for Generator {
    fn spawn(&mut self, args: &[&str], _: &str) -> Result<Value, String> {
        panic!("Expected inline refresh, got {args:?}");
    }

    fn run_generator(
        &mut self,
        argv: &[String],
        cwd: &Path,
        env: &Map<String, Value>,
        timeout: f64,
    ) -> io::Result<process::Output> {
        assert_eq!(
            &argv[1..5],
            [
                "--python",
                &self.named_python,
                cwd.to_str().unwrap(),
                "2048"
            ]
        );
        assert_eq!(env["TE_SIDECALL"], "1");
        let idents = &argv[5];
        if self.slow && !idents.is_empty() {
            assert_eq!(timeout, 1.0);
            return Err(io::ErrorKind::TimedOut.into());
        }
        Ok(process::Output {
            code: 0,
            stdout: format!("pkg/core.py:\n| def run(): ranked toward [{idents}]\n"),
            stderr: String::new(),
        })
    }
}

fn map(
    ctx: &LiveContext,
    environment: &Environment,
    method: &str,
    args: Value,
) -> Result<Value, String> {
    let map = RepoMap {
        ctx,
        environment,
        generator: Path::new(env!("CARGO_BIN_EXE_te-repomap-gen")),
        interpreter: None,
    };
    let mut io = Generator {
        named_python: ctx.config["repomap_python"].as_str().unwrap().into(),
        available: ctx.env["TEST_AIDER_AVAILABLE"] == "1",
        slow: ctx.env.contains_key("TE_REPOMAP_TIMEOUT"),
    };
    Ok(match method {
        "stale" => json!(map.stale(&mut io)),
        "spawn_refresh" => map.spawn_refresh(&mut io)?,
        "cached_block" => json!(map.cached_block()?),
        "for_prompt" => json!(map.for_prompt(args[0].as_str().unwrap(), &mut io)?),
        _ => panic!("Unexpected repository map operation: {method}"),
    })
}

fn map_fixture(root: &Path, host: &str, mode: &str) -> HookFixture {
    let mut f = fixture(root, host);
    f.environment.env.insert("TE_REPOMAP".into(), json!(mode));
    f.environment
        .env
        .insert("TE_REPOMAP_PYTHON".into(), json!(root.join("named-python")));
    f.environment
        .env
        .insert("TEST_AIDER_AVAILABLE".into(), json!("1"));
    f.repo_map = Some(map);
    f
}

fn prompt_session(f: &mut HookFixture, sid: &str, prompts: &[&str]) -> (Value, Vec<Value>) {
    let start = f.main(
        json!({"hook_event_name":"SessionStart","session_id":sid,"cwd":f.repo,
        "transcript_path":"","prompt_id":"p","source":"startup"}),
    );
    let prompts = prompts
        .iter()
        .enumerate()
        .map(|(i, prompt)| {
            f.main(json!({
        "hook_event_name":"UserPromptSubmit","session_id":sid,"cwd":f.repo,"transcript_path":"",
        "prompt":prompt,"prompt_id":format!("p{i}")
    }))
        })
        .collect();
    (start, prompts)
}

fn context(output: &Value) -> &str {
    output["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .or_else(|| output["additionalContext"].as_str())
        .unwrap_or_default()
}

#[test]
fn repomap_takes_on_off_and_start() {
    let mut config = LiveConfig::default();
    assert_eq!(config.values["repomap"], "off");
    for (value, expected) in [("on", "on"), ("start", "start"), ("false", "off")] {
        assert!(config.set("repomap", json!(value), "test"));
        assert_eq!(config.values["repomap"], expected);
    }
    assert!(!config.set("repomap", json!("sometimes"), "test"));
}

#[test]
fn the_first_prompt_gets_a_map_ranked_toward_what_it_names() {
    let tmp = TempDir::new("repomap-ranked");
    let mut f = map_fixture(tmp.path(), "claude", "on");
    let (start, prompts) = prompt_session(
        &mut f,
        "s1",
        &["Fix `parse_config` in ConfigLoader", "and then?"],
    );
    assert!(!context(&start).contains("Repository map"));
    let first = context(&prompts[0]);
    assert!(
        first.starts_with("# Repository map")
            && first.contains("parse_config")
            && first.contains("ConfigLoader")
    );
    assert!(
        prompts[0]["systemMessage"]
            .as_str()
            .unwrap()
            .contains("added a map of acme/widget")
    );
    assert!(context(&prompts[1]).is_empty());
    let (_, again) = prompt_session(&mut f, "s2", &["Fix `other_thing`"]);
    assert!(context(&again[0]).contains("other_thing"));
    assert!(again[0].get("systemMessage").is_none());
}

#[test]
fn a_slow_ranked_map_falls_back_to_the_cached_one() {
    let tmp = TempDir::new("repomap-timeout");
    let mut f = map_fixture(tmp.path(), "claude", "on");
    f.environment
        .env
        .insert("TE_REPOMAP_TIMEOUT".into(), json!("1"));
    let (_, prompts) = prompt_session(&mut f, "s1", &["Fix `parse_config`"]);
    let text = context(&prompts[0]);
    assert!(text.starts_with("# Repository map") && text.contains("ranked toward []"));
}

#[test]
fn codex_and_start_mode_get_the_cached_map_at_session_start() {
    for (host, mode) in [("codex", "on"), ("claude", "start")] {
        let tmp = TempDir::new(host);
        let mut f = map_fixture(tmp.path(), host, mode);
        let (start, prompts) = prompt_session(&mut f, "s1", &["Fix `x_y`"]);
        assert!(context(&start).starts_with("# Repository map"));
        assert!(context(&prompts[0]).is_empty());
    }
}

#[test]
fn shadow_and_off_inject_nothing() {
    let tmp = TempDir::new("repomap-disabled");
    let mut f = map_fixture(tmp.path(), "claude", "on");
    f.environment.env.insert("TE_SHADOW".into(), json!("1"));
    let (_, prompts) = prompt_session(&mut f, "s1", &["Fix `x_y`"]);
    assert!(context(&prompts[0]).is_empty());
    f.environment.env.remove("TE_SHADOW");
    f.environment.env.insert("TE_REPOMAP".into(), json!("off"));
    let (_, prompts) = prompt_session(&mut f, "s2", &["Fix `x_y`"]);
    assert_eq!(prompts[0], json!({}));
}

#[test]
fn a_named_python_without_aider_is_reported_not_replaced() {
    let tmp = TempDir::new("repomap-no-aider");
    let mut f = map_fixture(tmp.path(), "claude", "on");
    f.environment
        .env
        .insert("TEST_AIDER_AVAILABLE".into(), json!("0"));
    let (start, prompts) = prompt_session(&mut f, "s1", &["Fix `x_y`"]);
    assert!(context(&start).is_empty() && context(&prompts[0]).is_empty());
    let path = f.store.base.parent().unwrap().join("repomap.json");
    let state: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    assert!(
        state["error"]
            .as_str()
            .unwrap()
            .contains("cannot import aider")
    );
    assert!(state["map"].is_null());
}

#[test]
fn prompt_identifiers() {
    let ids = repomap::prompt_idents("Fix `Model.save` so get_readonly_fields works on ModelAdmin");
    for id in ["save", "get_readonly_fields", "ModelAdmin"] {
        assert!(ids.contains(&id.into()));
    }
}

#[ignore = "requires Git and a Python interpreter with aider installed"]
#[test]
fn a_real_map_comes_from_tracked_files_and_leaves_the_repository_clean() {
    let tmp = TempDir::new("repomap-aider");
    let repo = tmp.path().join("proj");
    fs::create_dir_all(repo.join("pkg")).unwrap();
    fs::create_dir(repo.join("static")).unwrap();
    fs::write(
        repo.join("pkg/core.py"),
        "class Engine:\n    def start(self):\n        pass\n",
    )
    .unwrap();
    fs::write(
        repo.join("pkg/util.py"),
        "from pkg.core import Engine\n\ndef boot():\n    return Engine().start()\n",
    )
    .unwrap();
    fs::write(repo.join("static/big.min.js"), "function vendored(){}\n").unwrap();
    for args in [["init", "-q"], ["add", "-A"]] {
        assert!(
            Command::new("git")
                .args(args)
                .current_dir(&repo)
                .status()
                .unwrap()
                .success()
        );
    }
    fs::write(repo.join("untracked.py"), "class Untracked: pass\n").unwrap();
    let cache = tmp.path().join("tags");
    let python = std::env::var_os("TE_REPOMAP_PYTHON").unwrap_or_else(|| "python3".into());
    let output = Command::new(env!("CARGO_BIN_EXE_te-repomap-gen"))
        .arg("--python")
        .arg(python)
        .arg(&repo)
        .args(["512", "Engine", "--git", "--cache"])
        .arg(&cache)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("pkg/core.py") && text.contains("Engine"));
    assert!(!text.contains("big.min.js") && !text.contains("untracked.py"));
    assert!(!fs::read_dir(repo).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".aider")
    }));
    assert!(cache.exists());
}
