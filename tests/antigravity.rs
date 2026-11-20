// Converted from reference/upstream/tests/test_antigravity.py.
#[path = "support/hook_fixture.rs"]
pub mod hook_fixture;
mod support;

use hook_fixture::HookFixture;
use serde_json::{Value, json};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(windows)]
use std::process::{Command, Output, Stdio};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use support::TempDir;
use tokeneconomizer::{
    accounting, package,
    plugin::{
        context::LiveContext,
        controller::{self, JudgeMode},
        hook::{self, Host},
        hosts::{
            self,
            session::{self, Environment},
            state,
        },
        sidecall,
    },
    trace::{antigravity, calls},
};

const CID: &str = "4c748b89-0000-4000-8000-000000000001";
const NOTE: &str = "Created At: 2026-11-05T10:00:00Z\nCompleted At: 2026-11-05T10:00:01Z\nA pre-tool hook changed the arguments of this tool call before it ran. Changed: CommandLine.\n\n";

struct Transcript {
    path: PathBuf,
    next: usize,
}
impl Transcript {
    fn new(root: &Path) -> Self {
        let directory = root.join("brain").join(CID).join(".system_generated/logs");
        fs::create_dir_all(&directory).unwrap();
        Self {
            path: directory.join("transcript_full.jsonl"),
            next: 0,
        }
    }
    fn add(&mut self, mut row: Value) {
        row["step_index"] = json!(self.next);
        self.next += 1;
        row.as_object_mut()
            .unwrap()
            .entry("status")
            .or_insert(json!("DONE"));
        row["created_at"] = json!("2026-11-05T10:00:00Z");
        writeln!(
            OpenOptions::new()
                .append(true)
                .create(true)
                .open(&self.path)
                .unwrap(),
            "{row}"
        )
        .unwrap();
    }
    fn prompt(&mut self, text: &str) {
        self.add(json!({"source":"USER_EXPLICIT","type":"USER_INPUT","content":format!("<USER_REQUEST>\n{text}\n</USER_REQUEST>\n<ADDITIONAL_METADATA>\nnow\n</ADDITIONAL_METADATA>")}));
    }
    fn call(&mut self, calls: &[Value], results: &[Value], tokens: bool) {
        let mut row = json!({"source":"MODEL","type":"PLANNER_RESPONSE","tool_calls":calls.iter().map(|call|json!({"name":call[0],"args":call[1]})).collect::<Vec<_>>()});
        if tokens {
            row.as_object_mut().unwrap().extend(
                json!({"input_tokens":100,"cache_read_tokens":1000,"output_tokens":20})
                    .as_object()
                    .unwrap()
                    .clone(),
            );
        }
        self.add(row);
        for result in results {
            let mut row = json!({"source":"MODEL","type":"GENERIC","content":result[0]});
            row.as_object_mut()
                .unwrap()
                .extend(result[1].as_object().unwrap().clone());
            self.add(row);
        }
    }
    fn system(&mut self) {
        self.add(json!({"source":"SYSTEM","type":"CHECKPOINT","content":"checkpoint"}));
    }
}

fn sh(command: &str) -> Value {
    json!(["run_command",{"CommandLine":command,"Cwd":"/work/repo","toolAction":"x","toolSummary":"y","WaitMsBeforeAsync":500}])
}
fn out(code: i32, text: &str) -> Value {
    json!([
        format!("{NOTE}Command exited with code {code}.\n\nCommand output:\n{text}\r\n"),
        {}
    ])
}
fn environment(root: &Path) -> Environment {
    Environment{env:json!({"TOKENECONOMIZER_HOME":root.join("home"),"TE_FOLD_SYNC":"1","TE_MICROSKILLS":"envfacts","GIT_CEILING_DIRECTORIES":root,"PATH":""}).as_object().unwrap().clone(),home:root.into(),cwd:root.into()}
}
fn payload(path: &Path, event: &str, extra: Value) -> Value {
    let mut p = json!({"hook_event_name":event,"conversationId":CID,"workspacePaths":["/work/repo"],"transcriptPath":path,"modelName":"m","lastUserInput":"fix it"});
    p.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    p
}
fn expand(t: &Transcript, event: &str, extra: Value, env: &Environment) -> Vec<Value> {
    state::expand(
        &payload(&t.path, event, extra),
        &json!(env.env),
        state::STATE_DIR,
        state::STATE_KEEP_S,
        env,
    )
    .unwrap()
}
fn kinds(events: &[Value]) -> Value {
    json!(
        events
            .iter()
            .map(|e| json!([
                e["hook_event_name"],
                e.get("tool_name")
                    .filter(|v| !v.is_null() && *v != "")
                    .unwrap_or(&e["prompt"]),
                e["prompt_id"]
            ]))
            .collect::<Vec<_>>()
    )
}
fn read(t: &Transcript) -> Vec<Value> {
    antigravity::read(&t.path, 0, -1, true).0
}

#[test]
fn expand_puts_a_turns_results_before_the_next_prompt_and_counts_turns() {
    let root = TempDir::new("agy-order");
    let env = environment(root.path());
    let mut t = Transcript::new(root.path());
    t.prompt("fix it");
    assert_eq!(
        kinds(&expand(&t, "SessionStart", json!({}), &env)),
        json!([["SessionStart", null, null]])
    );
    assert_eq!(
        kinds(&expand(&t, "PreInvocation", json!({}), &env)),
        json!([["UserPromptSubmit", "fix it", format!("{CID}:1")]])
    );
    t.call(&[sh("ls")], &[out(0, "a")], true);
    t.call(
        &[json!(["view_file",{"AbsolutePath":"/work/repo/a.py"}])],
        &[json!(["print(1)\n", {}])],
        true,
    );
    t.call(&[json!(["replace_file_content",{"TargetFile":"/work/repo/a.py","ReplacementContent":"x","TargetContent":"y"}])],&[json!(["edited",{"type":"CODE_ACTION"}])],true);
    assert_eq!(
        kinds(&expand(
            &t,
            "PreInvocation",
            json!({"invocationNum":3}),
            &env
        )),
        json!([
            ["PostToolUse", "Bash", format!("{CID}:1")],
            ["PostToolUse", "Read", format!("{CID}:1")],
            ["PostToolUse", "Edit", format!("{CID}:1")]
        ])
    );
    t.call(&[sh("pytest")], &[out(1, "FAILED")], true);
    t.prompt("again");
    assert_eq!(
        kinds(&expand(&t, "PreInvocation", json!({}), &env)),
        json!([
            ["PostToolUse", "Bash", format!("{CID}:1")],
            ["UserPromptSubmit", "fix it", format!("{CID}:2")]
        ])
    );
    t.call(&[sh("ls")], &[out(0, "b")], true);
    let stop = expand(
        &t,
        "Stop",
        json!({"terminationReason":"NO_TOOL_CALL"}),
        &env,
    );
    assert_eq!(
        kinds(&stop),
        json!([
            ["PostToolUse", "Bash", format!("{CID}:2")],
            ["Stop", null, null]
        ])
    );
    assert_eq!(stop.last().unwrap()["terminationReason"], "NO_TOOL_CALL");
}

#[test]
fn expand_does_not_emit_a_step_twice() {
    let root = TempDir::new("agy-once");
    let env = environment(root.path());
    let mut t = Transcript::new(root.path());
    expand(&t, "SessionStart", json!({}), &env);
    expand(&t, "PreInvocation", json!({}), &env);
    t.call(&[sh("ls")], &[out(0, "")], true);
    assert_eq!(
        expand(&t, "PreInvocation", json!({"invocationNum":1}), &env).len(),
        1
    );
    assert!(expand(&t, "PreInvocation", json!({"invocationNum":2}), &env).is_empty());
    t.call(&[sh("pwd")], &[out(0, "")], true);
    let again = expand(&t, "PreInvocation", json!({"invocationNum":3}), &env);
    assert_eq!(
        again
            .iter()
            .map(|e| e["tool_input"].clone())
            .collect::<Vec<_>>(),
        [json!({"command":"pwd"})]
    );
}

#[test]
fn a_call_whose_results_are_not_all_in_waits_for_them() {
    let root = TempDir::new("agy-incomplete");
    let env = environment(root.path());
    let mut t = Transcript::new(root.path());
    expand(&t, "SessionStart", json!({}), &env);
    expand(&t, "PreInvocation", json!({}), &env);
    t.call(&[sh("a"), sh("b")], &[out(0, "")], true);
    assert!(expand(&t, "PreInvocation", json!({"invocationNum":1}), &env).is_empty());
    t.add(json!({"source":"MODEL","type":"GENERIC","content":out(0,"")[0]}));
    assert_eq!(
        expand(&t, "PreInvocation", json!({"invocationNum":2}), &env)
            .iter()
            .map(|e| e["tool_input"]["command"].clone())
            .collect::<Vec<_>>(),
        [json!("a"), json!("b")]
    );
}

#[test]
fn a_conversation_this_plugin_did_not_start_is_not_replayed() {
    let root = TempDir::new("agy-unseen");
    let env = environment(root.path());
    let mut t = Transcript::new(root.path());
    t.prompt("old");
    t.call(&[sh("ls")], &[out(0, "")], true);
    assert_eq!(
        kinds(&expand(&t, "PreInvocation", json!({}), &env)),
        json!([["UserPromptSubmit", "fix it", format!("{CID}:1")]])
    );
}

#[test]
fn a_resumed_conversation_does_not_replay_its_old_calls_at_session_start() {
    let root = TempDir::new("agy-resumed");
    let env = environment(root.path());
    let mut t = Transcript::new(root.path());
    t.prompt("old");
    t.call(&[sh("ls")], &[out(0, "")], true);
    expand(&t, "SessionStart", json!({}), &env);
    assert_eq!(
        kinds(&expand(&t, "PreInvocation", json!({}), &env)),
        json!([["UserPromptSubmit", "fix it", format!("{CID}:1")]])
    );
    t.call(&[sh("pwd")], &[out(0, "")], true);
    assert_eq!(
        expand(&t, "PreInvocation", json!({"invocationNum":1}), &env)
            .iter()
            .map(|e| e["tool_input"]["command"].clone())
            .collect::<Vec<_>>(),
        [json!("pwd")]
    );
}

#[cfg(unix)]
#[test]
fn the_cursor_file_is_private() {
    let root = TempDir::new("agy-private");
    let env = environment(root.path());
    let t = Transcript::new(root.path());
    expand(&t, "SessionStart", json!({}), &env);
    let state = root.path().join("home/antigravity-sessions");
    assert_eq!(
        fs::metadata(&state).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(state.join(format!("{CID}.json")))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
}

#[test]
fn a_missing_or_unreadable_transcript_gives_no_tool_events() {
    let root = TempDir::new("agy-missing");
    let env = environment(root.path());
    let t = Transcript {
        path: root.path().join("nope.jsonl"),
        next: 0,
    };
    expand(&t, "SessionStart", json!({}), &env);
    assert_eq!(
        kinds(&expand(&t, "PreInvocation", json!({}), &env)),
        json!([["UserPromptSubmit", "fix it", format!("{CID}:1")]])
    );
    assert_eq!(
        kinds(&expand(&t, "Stop", json!({}), &env)),
        json!([["Stop", null, null]])
    );
    assert!(expand(&t, "PreInvocation", json!({"conversationId":"../x"}), &env).is_empty());
    assert!(
        state::expand(
            &json!({"hook_event_name":"PostToolUse","conversationId":CID}),
            &json!(env.env),
            state::STATE_DIR,
            state::STATE_KEEP_S,
            &env
        )
        .unwrap()
        .is_empty()
    );
}

#[test]
fn generic_and_typed_steps_become_claude_shaped_calls() {
    let root = TempDir::new("agy-tools");
    let mut t = Transcript::new(root.path());
    t.prompt("go");
    t.call(
        &[
            sh("pytest -q"),
            json!(["grep_search",{"Query":"foo","SearchPath":"/work/repo","toolAction":"z"}]),
            json!(["list_dir",{"DirectoryPath":"/work/repo"}]),
            json!(["call_mcp_tool",{"ServerName":"s","ToolName":"t","toolSummary":"x"}]),
        ],
        &[
            out(1, "ls: nope"),
            json!(["hit\n",{"type":"GREP_SEARCH"}]),
            json!(["a\nb\n",{"type":"LIST_DIRECTORY"}]),
            json!(["ok",{"type":"MCP_TOOL"}]),
        ],
        true,
    );
    t.call(
        &[json!(["run_command",{"CommandLine":"make"}])],
        &[json!(["done",{"type":"RUN_COMMAND","exit_code":2}])],
        true,
    );
    let items: Vec<_> = read(&t)
        .into_iter()
        .filter(|i| i["kind"] == "tool")
        .collect();
    assert_eq!(
        items
            .iter()
            .map(|i| (i["name"].clone(), i["args"].clone()))
            .collect::<Vec<_>>(),
        [
            (json!("Bash"), json!({"command":"pytest -q"})),
            (json!("Grep"), json!({"pattern":"foo","path":"/work/repo"})),
            (json!("Glob"), json!({"path":"/work/repo"})),
            (
                json!("call_mcp_tool"),
                json!({"ServerName":"s","ToolName":"t"})
            ),
            (json!("Bash"), json!({"command":"make"}))
        ]
    );
    assert_eq!(items[0]["cwd"], "/work/repo");
    assert!(
        items[0]["content"]
            .as_str()
            .unwrap()
            .starts_with("Command exited with code 1.")
    );
    assert_eq!(
        items
            .iter()
            .map(|i| i["exit_code"].clone())
            .collect::<Vec<_>>(),
        [json!(1), Value::Null, Value::Null, Value::Null, json!(2)]
    );
    assert_eq!(
        items
            .iter()
            .map(|i| i["is_error"].clone())
            .collect::<Vec<_>>(),
        [
            json!(true),
            json!(false),
            json!(false),
            json!(false),
            json!(true)
        ]
    );
}

#[test]
fn a_step_with_status_error_is_a_failure_with_its_error_text() {
    let root = TempDir::new("agy-error-step");
    let mut t = Transcript::new(root.path());
    t.call(
        &[json!(["view_file",{"AbsolutePath":"/work/repo/x"}])],
        &[json!(["",{"status":"ERROR","error":"tool call denied by pre-tool hook: "}])],
        true,
    );
    let items = read(&t);
    assert_eq!(items.len(), 1);
    let item = &items[0];
    assert_eq!(item["is_error"], true);
    assert!(item["exit_code"].is_null());
    assert!(
        item["content"]
            .as_str()
            .unwrap()
            .contains("denied by pre-tool hook")
    );
    let call=hosts::tool_call("antigravity",&json!({"tool_name":"Read","tool_input":{},"tool_response":{"content":item["content"],"exit_code":null,"isError":true}})).unwrap();
    assert_eq!(call.0, "Read");
    assert_eq!(call.3, 1);
    assert!(call.2.contains("denied"));
    assert!(hosts::tool_call("antigravity",&json!({"tool_name":"Bash","tool_input":{},"tool_response":{"content":"ok","exit_code":null,"isError":false}})).unwrap().3.is_null());
}

#[test]
fn running_steps_system_rows_and_finish_do_not_shift_the_pairing() {
    let root = TempDir::new("agy-running");
    let mut t = Transcript::new(root.path());
    t.call(
        &[sh("sleep 9"), sh("ls")],
        &[json!(["",{"status":"RUNNING"}]), out(0, "x")],
        true,
    );
    t.system();
    t.call(
        &[json!(["finish",{"summary":"s"}])],
        &[json!(["",{"type":"FINISH"}])],
        true,
    );
    assert_eq!(
        read(&t)
            .iter()
            .map(|i| (i["args"]["command"].clone(), i["exit_code"].clone()))
            .collect::<Vec<_>>(),
        [(json!("ls"), json!(0))]
    );
}

#[test]
fn write_file_and_edit_file_read_as_write_and_edit() {
    let root = TempDir::new("agy-desktop-edits");
    let mut t = Transcript::new(root.path());
    t.prompt("add a helper");
    for name in ["write_file", "edit_file"] {
        t.call(
            &[json!([name,{"Path":"/work/repo/src/helper.py"}])],
            &[json!(["ok",{"type":"CODE_ACTION"}])],
            true,
        );
    }
    let items: Vec<_> = read(&t)
        .into_iter()
        .filter(|i| i["kind"] == "tool")
        .collect();
    assert_eq!(
        items
            .iter()
            .map(|i| (i["name"].clone(), i["args"].clone(), i["is_error"].clone()))
            .collect::<Vec<_>>(),
        [
            (
                json!("Write"),
                json!({"file_path":"/work/repo/src/helper.py"}),
                json!(false)
            ),
            (
                json!("Edit"),
                json!({"file_path":"/work/repo/src/helper.py"}),
                json!(false)
            )
        ]
    );
    let work = root.path().join("w");
    fs::create_dir(&work).unwrap();
    let path = session::mechanism_trace(
        "antigravity",
        t.path.to_str(),
        &work,
        &environment(root.path()),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        calls::load_trajectory(&path)
            .iter()
            .map(|c| (c.0.clone(), c.1.clone()))
            .collect::<Vec<_>>(),
        [
            (
                json!("Write"),
                json!({"file_path":"/work/repo/src/helper.py"})
            ),
            (
                json!("Edit"),
                json!({"file_path":"/work/repo/src/helper.py"})
            )
        ]
    );
}

#[test]
fn render_answers_each_event_in_agys_terms() {
    let context = hosts::merge_outputs(&[
        hosts::context("antigravity", "SessionStart", "facts"),
        hosts::notice("antigravity", "heads up"),
    ])
    .unwrap();
    let start =
        state::render_antigravity(&json!({"hook_event_name":"SessionStart"}), &context).unwrap();
    let steps = start["injectSteps"].as_array().unwrap();
    assert_eq!(steps.len(), 1);
    let text = steps[0]["systemMessage"]["systemMessage"].as_str().unwrap();
    assert!(text.starts_with("facts\n\n"));
    assert!(text.contains("heads up"));
    assert_eq!(state::render_antigravity(&json!({"hook_event_name":"PreInvocation"}),&json!({"hookSpecificOutput":{"hookEventName":"PostToolUse","additionalContext":"redirect"},"systemMessage":"n"})).unwrap(),json!({"injectSteps":[{"ephemeralMessage":"redirect\n\nn"}]}));
    assert_eq!(
        state::render_antigravity(&json!({"hook_event_name":"Stop"}), &context).unwrap(),
        json!({})
    );
    for event in ["PreInvocation", "SessionStart"] {
        assert_eq!(
            state::render_antigravity(&json!({"hook_event_name":event}), &json!({})).unwrap(),
            json!({})
        );
    }
}

#[test]
fn only_a_turn_that_ran_to_its_end_stopped_normally() {
    for (reason, normal) in [
        (json!("NO_TOOL_CALL"), true),
        (json!("USER_CANCELED"), false),
        (json!("ERROR"), false),
        (Value::Null, false),
    ] {
        assert_eq!(
            hosts::stopped_normally("antigravity", &json!({"terminationReason":reason})),
            normal
        );
    }
}

#[test]
fn session_id_cwd_and_transcript_come_from_the_payload() {
    let root = TempDir::new("agy-payload");
    let env = environment(root.path());
    let path = Path::new("/h/.gemini/antigravity-cli/brain/x/t.jsonl");
    let p = payload(path, "Stop", json!({}));
    assert_eq!(hosts::session_id("antigravity", &p), CID);
    assert_eq!(
        session::cwd("antigravity", &p, &json!({}), &env.cwd),
        "/work/repo"
    );
    assert_eq!(
        session::transcript("antigravity", &p, &env).unwrap(),
        json!(path)
    );
    assert_eq!(
        session::cwd(
            "antigravity",
            &json!({"workspacePaths":[]}),
            &json!({}),
            &env.cwd
        ),
        json!(env.cwd)
    );
}

#[test]
fn detect_finds_agy_by_its_payload_and_leaves_the_others_alone() {
    let mut p =
        json!({"conversationId":CID,"transcriptPath":"/home/u/.gemini/antigravity-cli/brain/x/t"});
    let registered = hosts::HOSTS.map(String::from);
    assert_eq!(
        hosts::detect(&p, &json!({}), None, &registered),
        "antigravity"
    );
    assert_eq!(
        hosts::detect(&p, &json!({}), Some("antigravity"), &registered),
        "antigravity"
    );
    p["transcriptPath"] = json!("/x/other.jsonl");
    assert_eq!(hosts::detect(&p, &json!({}), None, &registered), "claude");
    assert_eq!(
        hosts::detect(
            &json!({"transcript_path":"/x/y"}),
            &json!({}),
            None,
            &registered
        ),
        "claude"
    );
    assert!(hosts::HOSTS.contains(&"antigravity"));
}

#[test]
fn usage_sums_the_rows_that_report_it_and_skips_the_rest() {
    let root = TempDir::new("agy-usage");
    let mut t = Transcript::new(root.path());
    t.prompt("go");
    t.call(&[sh("a")], &[out(0, "")], true);
    t.call(&[sh("b")], &[out(0, "")], false);
    t.call(&[], &[], true);
    t.add(json!({"source":"MODEL","type":"PLANNER_RESPONSE","input_tokens":5,"output_tokens":7}));
    let usage = antigravity::session_usage(&t.path);
    assert_eq!(usage.as_object().unwrap().len(), 6);
    for (key, expected) in [
        ("input_tokens", 205.0),
        ("cache_read_input_tokens", 2000.0),
        ("cache_creation_input_tokens", 0.0),
        ("output_tokens", 47.0),
        ("requests", 3.0),
    ] {
        assert_eq!(usage[key].as_f64(), Some(expected), "{key}");
    }
    assert_eq!(usage["model"], "antigravity");
    let missing = t.path.with_extension("jsonl.none");
    assert_eq!(antigravity::session_usage(&missing), json!({}));
    assert_eq!(
        accounting::effective(&session::session_usage("antigravity", &t.path).unwrap())
            .unwrap()
            .1,
        "fallback"
    );
    assert_eq!(
        accounting::effective(&session::session_usage("antigravity", &missing).unwrap()).unwrap(),
        (None, "no_usage")
    );
}

#[test]
fn the_trace_conversion_feeds_the_mechanisms_and_prompts_are_counted() {
    let root = TempDir::new("agy-trajectory");
    let env = environment(root.path());
    let mut t = Transcript::new(root.path());
    t.prompt("first");
    t.call(&[sh("pytest")], &[out(1, "FAILED x")], true);
    t.call(
        &[json!(["replace_file_content",{"TargetFile":"/work/repo/a.py"}])],
        &[json!(["ok",{"type":"CODE_ACTION"}])],
        true,
    );
    t.prompt("second");
    t.call(
        &[json!(["view_file",{"AbsolutePath":"/work/repo/a.py"}])],
        &[json!(["body",{"type":"VIEW_FILE"}])],
        true,
    );
    let work = root.path().join("w");
    fs::create_dir(&work).unwrap();
    let path = session::mechanism_trace("antigravity", t.path.to_str(), &work, &env)
        .unwrap()
        .unwrap();
    assert_eq!(
        calls::load_trajectory(&path),
        vec![
            (
                json!("Bash"),
                json!({"command":"pytest"}),
                "Command exited with code 1.\n\nCommand output:\nFAILED x".into(),
                json!(false)
            ),
            (
                json!("Edit"),
                json!({"file_path":"/work/repo/a.py"}),
                "ok".into(),
                json!(true)
            ),
            (
                json!("Read"),
                json!({"file_path":"/work/repo/a.py"}),
                "body".into(),
                json!(true)
            )
        ]
    );
    assert_eq!(calls::call_cwds(&path)[0], Some("/work/repo".into()));
    assert_eq!(session::prompt_count("antigravity", &t.path), Some(2));
    assert!(
        session::mechanism_trace(
            "antigravity",
            t.path.with_extension("jsonl.none").to_str(),
            &work,
            &env
        )
        .unwrap()
        .is_none()
    );
}

fn main(f: &mut HookFixture, args: &[&str], p: Value) -> Value {
    let env = f.environment.clone();
    let args: Vec<_> = args.iter().map(|arg| arg.to_string()).collect();
    let out = hook::main(
        &args,
        Ok(p.to_string().into_bytes()),
        &env,
        std::process::id(),
        f,
    )
    .unwrap();
    serde_json::from_str(out.trim()).unwrap()
}

#[test]
fn main_reads_the_event_from_the_command_line() {
    let root = TempDir::new("agy-event-arg");
    let mut f = HookFixture::new(root.path(), "antigravity");
    f.environment
        .env
        .insert("TE_MICROSKILLS".into(), json!("off"));
    let mut t = Transcript::new(root.path());
    t.prompt("x");
    let mut p = payload(&t.path, "ignored", json!({"workspacePaths":[f.repo]}));
    p.as_object_mut().unwrap().remove("hook_event_name");
    assert_eq!(
        main(
            &mut f,
            &["--host", "antigravity", "--event", "SessionStart"],
            p
        ),
        json!({})
    );
    assert!(
        root.path()
            .join("state/antigravity-sessions")
            .join(format!("{CID}.json"))
            .is_file()
    );
}

#[test]
fn a_sidecall_marker_makes_the_hook_do_nothing() {
    let root = TempDir::new("agy-sidecall-marker");
    let mut f = HookFixture::new(root.path(), "antigravity");
    f.environment.env.insert("TE_SIDECALL".into(), json!("1"));
    let t = Transcript::new(root.path());
    let p = payload(&t.path, "SessionStart", json!({"workspacePaths":[f.repo]}));
    assert_eq!(
        main(
            &mut f,
            &["--host", "antigravity", "--event", "SessionStart"],
            p
        ),
        json!({})
    );
    assert!(!root.path().join("state/antigravity-sessions").exists());
}

#[test]
fn the_expand_render_seam_leaves_other_hosts_as_they_were() {
    let root = TempDir::new("agy-other-hosts");
    let mut f = HookFixture::new(root.path(), "claude");
    assert_eq!(f.run(json!({"session_id":"s","cwd":f.repo,"transcript_path":"","hook_event_name":"SessionStart","source":"startup"})),json!({}));
    for name in ["claude", "cursor"] {
        let host = Host::builtin(name.into());
        assert!(!host.expand);
        assert!(!host.render);
    }
}

#[test]
fn the_seam_runs_each_expanded_event_in_order_and_renders_the_merge() {
    struct Fake {
        seen: Vec<Value>,
    }
    impl hook::Runtime for Fake {
        fn detect(&mut self, _: &Value, _: &Environment, _: Option<&str>) -> Result<Host, String> {
            let mut host = Host::builtin("claude".into());
            host.expand = true;
            host.render = true;
            Ok(host)
        }
        fn host(
            &mut self,
            host: &Host,
            method: &str,
            p: &Value,
            env: &Environment,
            render: Option<(&LiveContext, &Value)>,
        ) -> Result<Value, String> {
            match method {
                "expand" => {
                    let mut events = vec![];
                    for n in [1, 2] {
                        let mut event = p.clone();
                        event["hook_event_name"] = json!("SessionStart");
                        event["n"] = json!(n);
                        events.push(event);
                    }
                    Ok(json!(events))
                }
                "render" => Ok(json!({"event":p["hook_event_name"],"merged":render.unwrap().1})),
                _ => hook::host_method(host, method, p, env, env, render),
            }
        }
        fn handle(&mut self, _: &mut LiveContext, p: Value) -> Result<Value, String> {
            self.seen.push(p["n"].clone());
            Ok(json!({"additionalContext":format!("c{}",p["n"])}))
        }
    }
    let root = TempDir::new("agy-expanded-seam");
    let env = environment(root.path());
    let mut fake = Fake { seen: vec![] };
    let got = hook::run(
        json!({"session_id":"s","cwd":root.path(),"hook_event_name":"PreInvocation"}),
        &env,
        Some("fake"),
        &mut fake,
    )
    .unwrap();
    assert_eq!(fake.seen, [json!(1), json!(2)]);
    assert_eq!(
        got,
        json!({"event":"PreInvocation","merged":{"additionalContext":"c1\n\nc2"}})
    );
}

fn run_turn(
    f: &mut HookFixture,
    t: &mut Transcript,
    prompt: &str,
    batches: &[(Vec<Value>, Vec<Value>)],
) -> Vec<Value> {
    let call = |f: &mut HookFixture, t: &Transcript, event: &str, fields: Value| {
        let mut fields = fields;
        fields["workspacePaths"] = json!([f.repo]);
        fields["lastUserInput"] = json!(prompt);
        main(
            f,
            &["--host", "antigravity", "--event", event],
            payload(&t.path, event, fields),
        )
    };
    t.prompt(prompt);
    let mut outs = vec![call(f, t, "PreInvocation", json!({}))];
    for (index, (calls, results)) in batches.iter().enumerate() {
        t.call(calls, results, true);
        outs.push(call(
            f,
            t,
            "PreInvocation",
            json!({"invocationNum":index+1}),
        ));
    }
    call(
        f,
        t,
        "Stop",
        json!({"terminationReason":"NO_TOOL_CALL","executionNum":1}),
    );
    outs
}

#[test]
fn a_stuck_turn_is_redirected_through_an_ephemeral_message() {
    let root = TempDir::new("agy-stuck");
    let mut f = HookFixture::new(root.path(), "antigravity");
    f.environment
        .env
        .insert("TE_MICROSKILLS".into(), json!("envfacts"));
    f.guard_on();
    let mut t = Transcript::new(root.path());
    let p = payload(&t.path, "SessionStart", json!({"workspacePaths":[f.repo]}));
    main(
        &mut f,
        &["--host", "antigravity", "--event", "SessionStart"],
        p,
    );
    let green = vec![(
        vec![json!(["view_file",{"AbsolutePath":f.repo.join("a.py")}]); 8],
        vec![json!(["ok", {}]); 8],
    )];
    for i in 0..5 {
        run_turn(&mut f, &mut t, &format!("green {i}"), &green);
    }
    let fail = "FAILED tests/test_a.py::test_x - AssertionError: assert 1 == 2";
    let stuck = vec![
        (
            vec![
                json!(["replace_file_content",{"TargetFile":f.repo.join("pkg/a.py")}]),
                sh("pytest -q")
            ],
            vec![json!(["edited",{"type":"CODE_ACTION"}]), out(1, fail)]
        );
        20
    ];
    let outs = run_turn(&mut f, &mut t, "stuck", &stuck);
    let steps: Vec<_> = outs
        .iter()
        .filter_map(|out| out["injectSteps"].as_array())
        .flatten()
        .collect();
    assert!(!steps.is_empty());
    assert!(
        steps
            .iter()
            .all(|step| step.as_object().unwrap().len() == 1
                && step.get("ephemeralMessage").is_some())
    );
    assert!(steps.iter().any(|step| {
        step["ephemeralMessage"]
            .as_str()
            .unwrap()
            .contains("Process check (automated)")
    }));
}

#[test]
fn antigravity_makes_no_side_call() {
    let root = TempDir::new("agy-no-sidecall");
    assert!(
        sidecall::runner::host_call(
            "antigravity",
            &json!({"paid_model":"m"}),
            &environment(root.path())
        )
        .unwrap()
        .is_none()
    );
    assert!(sidecall::NO_SIDE_CALLS.contains(&"antigravity"));
}

#[test]
fn a_stuck_looking_agy_turn_never_starts_a_judge() {
    let root = TempDir::new("agy-no-judge");
    let mut f = HookFixture::new(root.path(), "antigravity");
    assert!(matches!(
        controller::judge_mode("antigravity", &json!({}), &mut f).unwrap(),
        JudgeMode::NoVerdict
    ));
}

fn bundle(root: &Path) -> PathBuf {
    package::build(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("reference/upstream"),
        Some(Path::new(env!("CARGO_BIN_EXE_te")).parent().unwrap()),
        &root.join("marketplace"),
        false,
    )
    .unwrap()
}
fn document(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn the_plugin_declares_the_three_events_and_never_pretooluse() {
    let root = TempDir::new("agy-package");
    let plugin = bundle(root.path());
    let manifest = document(&plugin.join(".antigravity-plugin/plugin.json"));
    let claude = document(&plugin.join(".claude-plugin/plugin.json"));
    assert_eq!(manifest["name"], "tokeneconomizer");
    assert_eq!(manifest["version"], claude["version"]);
    let hooks = document(&plugin.join("hooks.json"));
    assert_eq!(hooks.as_object().unwrap().len(), 1);
    let events = hooks["tokeneconomizer"].as_object().unwrap();
    assert_eq!(events.len(), 3);
    for (event, timeout) in [("SessionStart", 15), ("PreInvocation", 10), ("Stop", 10)] {
        let handlers = events[event].as_array().unwrap();
        assert_eq!(handlers.len(), 1);
        assert_eq!(handlers[0]["timeout"], timeout);
        assert_eq!(
            handlers[0]["command"],
            format!(
                "./scripts/te-hook --host antigravity --event {event} || scripts\\te-hook.cmd --host antigravity --event {event}"
            )
        );
    }
    assert!(plugin.join("scripts/te-hook").is_file());
    #[cfg(unix)]
    assert_ne!(
        fs::metadata(plugin.join("scripts/te-hook"))
            .unwrap()
            .permissions()
            .mode()
            & 0o111,
        0
    );
    for name in ["te-hook", "te"] {
        let script = fs::read_to_string(plugin.join(format!("scripts/{name}.cmd"))).unwrap();
        assert!(script.contains(&format!("if not exist \"%~dp0{name}.exe\"")));
        assert!(script.contains(&format!("\"%~dp0{name}.exe\" %*")));
        assert!(!script.contains("python"));
        if name == "te-hook" {
            assert!(script.contains("echo {}\r\n  exit /b 0"));
        } else {
            assert!(script.contains(">&2 echo TokenEconomizer: te.exe is missing"));
            assert!(script.contains("exit /b 1"));
        }
    }
    assert!(claude.get("hooks").is_none());
}

#[cfg(windows)]
fn windows_run(
    root: &Path,
    plugin: &Path,
    name: &str,
    input: &str,
    python: Option<&Path>,
) -> Output {
    let mut command = Command::new(plugin.join(format!("scripts/{name}.cmd")));
    command
        .env_clear()
        .env("PATH", "")
        .env("HOME", root)
        .env("USERPROFILE", root)
        .env("TOKENECONOMIZER_HOME", root.join("home"))
        .env("TE_MICROSKILLS", "off")
        .env("GIT_CEILING_DIRECTORIES", root)
        .current_dir(root);
    for key in ["SYSTEMROOT", "COMSPEC", "PATHEXT", "TEMP", "TMP"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    if let Some(python) = python {
        command.env("TE_PYTHON", python);
    }
    command.args(["--host", "antigravity"]);
    if name == "te-hook" {
        command.args(["--event", "SessionStart"]);
    } else {
        command.arg("status");
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        // A missing executable makes the wrapper exit before reading stdin.
        .unwrap_or_else(|error| assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe));
    child.wait_with_output().unwrap()
}

#[cfg(windows)]
#[test]
fn windows_hook_launcher_fails_open_with_no_python() {
    let root = TempDir::new("agy-win-missing-hook");
    let plugin = bundle(root.path());
    fs::remove_file(plugin.join("scripts/te-hook.exe")).unwrap();
    let out = windows_run(root.path(), &plugin, "te-hook", "{}", None);
    assert!(out.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap(),
        json!({})
    );
}

#[cfg(windows)]
#[test]
fn windows_te_launcher_reports_missing_python() {
    let root = TempDir::new("agy-win-missing-cli");
    let plugin = bundle(root.path());
    fs::remove_file(plugin.join("scripts/te.exe")).unwrap();
    let out = windows_run(root.path(), &plugin, "te", "", None);
    assert_eq!(out.status.code(), Some(1));
    assert!(
        String::from_utf8(out.stderr)
            .unwrap()
            .contains("te.exe is missing")
    );
}

#[cfg(windows)]
#[test]
fn windows_hook_launcher_executes_with_valid_python() {
    let root = TempDir::new("agy-win-native-hook");
    let plugin = bundle(root.path());
    let p = payload(
        &root.path().join("transcript.jsonl"),
        "SessionStart",
        json!({"workspacePaths":[root.path()],"lastUserInput":"test"}),
    );
    let out = windows_run(
        root.path(),
        &plugin,
        "te-hook",
        &p.to_string(),
        Some(Path::new("python")),
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap(),
        json!({})
    );
    assert!(
        root.path()
            .join("home/antigravity-sessions")
            .join(format!("{CID}.json"))
            .is_file()
    );
}

#[cfg(windows)]
#[test]
fn windows_hook_launcher_passes_over_a_te_python_that_is_not_there() {
    let root = TempDir::new("agy-win-stale-python");
    let plugin = bundle(root.path());
    let out = windows_run(
        root.path(),
        &plugin,
        "te-hook",
        "{}",
        Some(Path::new(r"C:\no such folder (x86)\python.exe")),
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap(),
        json!({})
    );
}

#[cfg(windows)]
#[test]
fn windows_hook_launcher_runs_a_te_python_under_a_path_with_spaces() {
    let root = TempDir::new("agy-win-space-path");
    let folder = root.path().join("Program Files (x86)");
    fs::create_dir(&folder).unwrap();
    let plugin = bundle(&folder);
    let p = payload(
        &root.path().join("transcript.jsonl"),
        "SessionStart",
        json!({"workspacePaths":[root.path()],"lastUserInput":"test"}),
    );
    let out = windows_run(
        root.path(),
        &plugin,
        "te-hook",
        &p.to_string(),
        Some(&folder.join("Python/python.cmd")),
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&out.stdout).unwrap(),
        json!({})
    );
    assert!(
        root.path()
            .join("home/antigravity-sessions")
            .join(format!("{CID}.json"))
            .is_file()
    );
}

#[cfg(windows)]
#[test]
fn windows_te_launcher_passes_over_a_te_python_that_is_not_there() {
    let root = TempDir::new("agy-win-stale-python-cli");
    let plugin = bundle(root.path());
    let out = windows_run(
        root.path(),
        &plugin,
        "te",
        "",
        Some(Path::new(r"C:\no such folder (x86)\python.exe")),
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}
