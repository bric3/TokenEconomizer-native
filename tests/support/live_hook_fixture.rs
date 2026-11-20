#[path = "hook_fixture.rs"]
pub mod hook_fixture;
pub use hook_fixture::HookFixture;

use crate::fixtures;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

pub const CLAIM: &str = "`pytest …` fails here with command not found; `python -m pytest …` works.";
pub const FID: &str = "form_repair:run_tests:pytest";

pub fn fixture(root: &Path, host: &str) -> HookFixture {
    let mut f = HookFixture::new(root, host);
    f.inline_folds = true;
    f.inline_judge = true;
    fs::write(f.repo.join("pyproject.toml"), "[project]\nname='w'\n").unwrap();
    fs::create_dir_all(root.join("claude/projects")).unwrap();
    f.environment.env.remove("TE_LOOPGUARD");
    f.environment.env.extend(json!({"TE_FOLD_SYNC":"1","TE_TOOLPRUNE_QUIET_MINUTES":"0","TE_MICROSKILLS":"envfacts","CLAUDE_CONFIG_DIR":root.join("claude"),"COPILOT_HOME":root.join("copilot-home")}).as_object().unwrap().clone());
    f
}

pub fn session(f: &mut HookFixture, sid: &str, transcript: &Path, source: &str) -> Value {
    let start=f.main(json!({"hook_event_name":"SessionStart","session_id":sid,"source":source,"transcript_path":transcript,"cwd":f.repo}));
    assert_eq!(f.main(json!({"hook_event_name":"SessionEnd","session_id":sid,"reason":"other","transcript_path":transcript,"cwd":f.repo})),json!({}));
    start
}

pub fn env_state(f: &HookFixture) -> Value {
    let paths: Vec<_> = fs::read_dir(f.store.base.join("mechanisms/envfacts_state"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    assert_eq!(paths.len(), 1);
    serde_json::from_slice(&fs::read(&paths[0]).unwrap()).unwrap()
}

pub fn cli(f: &HookFixture, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_te"))
        .args(["--host", &f.host, "--cwd", f.repo.to_str().unwrap()])
        .args(args)
        .env_clear()
        .env("HOME", &f.environment.home)
        .env("USERPROFILE", &f.environment.home)
        .envs(
            f.environment
                .env
                .iter()
                .map(|(key, value)| (key, value.as_str().unwrap())),
        )
        .current_dir(&f.repo)
        .output()
        .unwrap()
}

pub fn trap_calls(repo: &Path, index: usize, prompt: Option<usize>, trap: bool) -> Vec<Value> {
    let file = match prompt {
        Some(prompt) => format!("pkg/m{index}_{prompt}.py"),
        None => format!("pkg/mod{index}.py"),
    };
    let mut calls = vec![
        json!(["Edit",{"file_path":repo.join(file),"old_string":"a","new_string":"b"},"ok",false]),
    ];
    if trap {
        calls.push(json!(["Bash",{"command":"pytest -q tests/test_a.py"},"bash: pytest: command not found",true]));
    }
    calls.push(
        json!(["Bash",{"command":"python -m pytest -q tests/test_a.py"},"1 passed in 0.1s",false]),
    );
    calls
}

pub fn transcript(root: &Path, sid: &str) -> PathBuf {
    root.join(format!("{sid}.jsonl"))
}

pub fn claude_log(
    root: &Path,
    f: &HookFixture,
    sid: &str,
    index: usize,
    prevented: bool,
    prompts: usize,
) -> PathBuf {
    let prompts: Vec<_> = (0..prompts)
        .map(|k| {
            json!([
                if prompts == 1 {
                    format!("fix bug {index}")
                } else {
                    format!("fix bug {index}.{k}")
                },
                trap_calls(
                    &f.repo,
                    index,
                    (prompts > 1).then_some(k),
                    !prevented && k == 0
                )
            ])
        })
        .collect();
    fixtures::write_claude_session(&transcript(root, sid), &prompts, "claude-opus-5-5")
}

pub fn green() -> Vec<Value> {
    vec![json!(["Read",{"file_path":"/r/a.py"},"ok"]); 8]
}

pub fn stuck(host: &str, n: usize) -> Vec<Value> {
    let fail = "FAILED tests/test_a.py::test_x - AssertionError: assert 1 == 2";
    (0..n).map(|k|match (host,k%2==0) {
        ("codex",true)=>json!(["apply_patch",{"input":"*** Begin Patch\n*** Update File: pkg/a.py\n"},"Success."]),
        ("codex",false)=>json!(["exec_command",{"command":"pytest -q"},{"output":fail,"exit_code":1}]),
        ("droid",true)=>json!(["Edit",{"file_path":"/r/pkg/a.py","old_string":"x","new_string":"y"},{"output":"ok","exitCode":0}]),
        ("droid",false)=>json!(["Execute",{"command":"pytest -q"},{"output":fail,"exitCode":1}]),
        ("pi",true)=>json!(["edit",{"path":"/r/pkg/a.py","oldText":"x","newText":"y"},{"content":"ok"}]),
        ("pi",false)=>json!(["bash",{"command":"pytest -q"},{"content":fail,"isError":true,"exit_code":1}]),
        ("copilot",true)=>json!(["Edit",{"path":"/r/pkg/a.py"},{"result_type":"success","text_result_for_llm":"ok"}]),
        ("copilot",false)=>json!(["Bash",{"command":"pytest -q"},{"result_type":"success","text_result_for_llm":format!("{fail}\n<shellId: 0 completed with exit code 1>")}]),
        (_,true)=>json!(["Edit",{"file_path":"/r/pkg/a.py","old_string":"x","new_string":"y"},"ok"]),
        (_,false)=>json!(["Bash",{"command":"pytest -q"},{"error":format!("Exit code 1\n{fail}")}]),
    }).collect()
}

fn post(base: &Value, name: &Value, args: &Value, result: &Value, rkey: &str) -> Value {
    let mut payload = base.clone();
    payload["tool_name"] = name.clone();
    payload["tool_input"] = args.clone();
    if result
        .as_object()
        .is_some_and(|object| object.len() == 1 && object.contains_key("error"))
    {
        payload["hook_event_name"] = json!("PostToolUseFailure");
        payload["error"] = result["error"].clone();
    } else {
        payload["hook_event_name"] = json!("PostToolUse");
        payload[rkey] = result.clone();
    }
    payload
}

pub fn turn(f: &mut HookFixture, sid: &str, key: &str, calls: &[Value]) -> Vec<Value> {
    let mut base = json!({"session_id":sid,"transcript_path":"","cwd":f.repo});
    if f.host != "copilot" {
        base[if f.host == "codex" {
            "turn_id"
        } else {
            "prompt_id"
        }] = json!(key);
    }
    let mut prompt = base.clone();
    prompt["hook_event_name"] = json!("UserPromptSubmit");
    prompt["prompt"] = json!("go");
    f.main(prompt);
    let rkey = if f.host == "copilot" {
        "tool_result"
    } else {
        "tool_response"
    };
    let outputs = calls
        .iter()
        .map(|call| f.main(post(&base, &call[0], &call[1], &call[2], rkey)))
        .collect();
    base["hook_event_name"] = json!("Stop");
    base["stop_hook_active"] = json!(false);
    f.main(base);
    outputs
}
pub fn warm(f: &mut HookFixture) {
    for k in 0..5 {
        turn(f, "s", &format!("g{k}"), &green());
    }
}
