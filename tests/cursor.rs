// Converted from reference/upstream/tests/test_cursor.py.
mod support;

use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    fs, io,
    path::{Path, PathBuf},
};
use support::TempDir;
use tokeneconomizer::{
    plugin::{
        context::LiveContext,
        controller::{self, LiveController},
        history, hook,
        hosts::{
            self,
            session::{self, Environment},
            state,
        },
        judge, paths, sidecall,
    },
    runner::Runner,
    store::Store,
    trace::{calls, cursor},
};

fn write_lines(path: &Path, rows: &[Value]) {
    fs::write(
        path,
        rows.iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
            + "\n",
    )
    .unwrap();
}
struct Cursor {
    repo: PathBuf,
    environment: Environment,
    store: Store,
    folds: Vec<String>,
}
impl Cursor {
    fn new(root: &Path) -> Self {
        let repo = root.join("repo");
        fs::create_dir_all(repo.join(".git/info")).unwrap();
        fs::write(repo.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(
            repo.join(".git/config"),
            "[remote \"origin\"]\n\turl = https://github.com/acme/widget.git\n",
        )
        .unwrap();
        let state = root.join("state");
        fs::create_dir(&state).unwrap();
        let env = json!({"TOKENECONOMIZER_HOME":state,"CURSOR_PROJECT_DIR":repo,"TE_LOOPGUARD":"off","GIT_CEILING_DIRECTORIES":root,"PATH":""}).as_object().unwrap().clone();
        let id = paths::repo_identity(&repo, &env, root).unwrap();
        let store = Store::new(&paths::repo_state_dir(&id, "cursor", &env, root).unwrap()).unwrap();
        Self {
            repo,
            store,
            environment: Environment {
                env,
                home: root.into(),
                cwd: root.into(),
            },
            folds: vec![],
        }
    }
    fn run(&mut self, payload: Value) -> Value {
        hook::run(payload, &self.environment.clone(), Some("cursor"), self).unwrap()
    }
    fn resolved_turns(&self) -> Vec<f64> {
        let envelope = self.store.read("envelope.json", json!({})).unwrap();
        let turns = envelope.get("resolved_turns").cloned().unwrap_or(json!([]));
        turns
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_f64().unwrap())
            .collect()
    }
    fn prepare(&self, payload: &Value) -> Value {
        state::prepare(
            "cursor",
            payload,
            &json!(self.environment.env),
            &self.environment,
        )
        .unwrap()
    }
    fn tool(&mut self, base: &Value, fields: Value) -> Value {
        let mut payload = base.clone();
        payload
            .as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        self.run(payload)
    }
    fn guard_on(&mut self) {
        self.environment
            .env
            .insert("TE_LOOPGUARD".into(), json!("on"));
    }
}
impl hook::Runtime for Cursor {
    fn handle(&mut self, ctx: &mut LiveContext, payload: Value) -> Result<Value, String> {
        let mut controller = LiveController::new(
            ctx.clone(),
            payload,
            self.environment.clone(),
            0,
            cfg!(windows),
        );
        let output = controller.handle(self);
        *ctx = controller.ctx;
        output
    }
}
impl session::Runtime for Cursor {
    fn program(&mut self, _cli: &str) -> session::Result<Vec<String>> {
        panic!("Unexpected program lookup")
    }
    fn ancestors(
        &mut self,
        _limit: i64,
        _pid: u32,
        _windows: bool,
        _cwd: &Path,
    ) -> session::Result<Vec<String>> {
        panic!("Unexpected process lookup")
    }
}
impl judge::Runtime for Cursor {
    fn paused(&mut self) -> Result<bool, String> {
        panic!("These cold turns must not select a model")
    }
    fn runner(
        &mut self,
        _role: &str,
        _local_only: bool,
    ) -> Result<Option<Box<dyn Runner>>, String> {
        panic!("Unexpected model selection")
    }
    fn spawn_job(&mut self, _sid: &str, _id: &str) -> Result<(), String> {
        panic!("Unexpected judge job")
    }
    fn spawn(&mut self, _argv: &[String], _log: &str) -> Result<Value, String> {
        panic!("Unexpected process spawn")
    }
}
impl controller::Runtime for Cursor {
    fn dispatch(&mut self, controller: &mut LiveController, method: &str) -> Result<Value, String> {
        controller.invoke(method, self)
    }
    fn launcher(&mut self, ctx: &LiveContext) -> io::Result<()> {
        paths::ensure_native_launcher(
            &ctx.env,
            &ctx.home,
            Path::new(env!("CARGO_BIN_EXE_te")),
            cfg!(windows),
        )
        .map(|_| ())
        .map_err(io::Error::other)
    }
    fn paused_detail(&mut self) -> Result<Value, String> {
        Ok(sidecall::paused(
            "cursor",
            &paths::state_root(&self.environment.env, &self.environment.home)?,
        ))
    }
    fn call_name(&mut self) -> Result<String, String> {
        Ok(sidecall::call_name("cursor"))
    }
    fn local_routes(&mut self) -> Result<bool, String> {
        panic!("Unexpected local model routing")
    }
    fn map(&mut self, _ctx: &LiveContext, _method: &str, _args: Value) -> Result<Value, String> {
        panic!("Unexpected repository map")
    }
    fn history_notice(&mut self, ctx: &LiveContext) -> Result<Value, String> {
        history::notice(ctx, &history::Limits::default()).map_err(|e| e.to_string())
    }
    fn history_start(&mut self, ctx: &LiveContext) -> Result<(), String> {
        assert_eq!(ctx.config["history_import"], false);
        Ok(())
    }
    fn migration_start(&mut self, _ctx: &LiveContext) -> Result<(), String> {
        Ok(())
    }
    fn pending_sessions(
        &mut self,
        _ctx: &LiveContext,
        _exclude: &[String],
    ) -> Result<Vec<String>, String> {
        Ok(vec![])
    }
    fn spawn_folds(&mut self, _ctx: &LiveContext, sessions: &[String]) -> Result<(), String> {
        self.folds.extend_from_slice(sessions);
        Ok(())
    }
}

#[test]
fn cursor_translates_prompt_and_tool_hook_schemas() {
    let root = TempDir::new("cursor-prepare");
    let fixture = Cursor::new(root.path());
    let prompt = fixture.prepare(&json!({"hook_event_name":"beforeSubmitPrompt","conversation_id":"conv-1","generation_id":"gen-1","prompt":"Fix the bug","workspace_roots":[fixture.repo]}));
    assert_eq!(prompt["hook_event_name"], "UserPromptSubmit");
    assert_eq!(prompt["session_id"], "conv-1");
    assert_eq!(prompt["prompt_id"], "gen-1");
    assert_eq!(prompt["cwd"], json!(fixture.repo));
    assert_eq!(
        prompt["transcript_path"],
        json!(root.path().join("state/cursor-sessions/conv-1.jsonl"))
    );
    let success = fixture.prepare(&json!({"hook_event_name":"postToolUse","conversation_id":"conv-1","generation_id":"gen-1","tool_name":"Shell","tool_input":{"command":"pytest"},"tool_output":"{\"exitCode\":1,\"stdout\":\"failed\",\"stderr\":\"details\"}"}));
    assert_eq!(success["hook_event_name"], "PostToolUse");
    assert_eq!(success["tool_use_id"], "gen-1");
    assert_eq!(
        success["tool_response"],
        json!({"content":"failed\ndetails","exit_code":1,"isError":false})
    );
    let failure = fixture.prepare(&json!({"hook_event_name":"postToolUseFailure","conversation_id":"conv-1","generation_id":"gen-1","tool_name":"Shell","tool_input":{"command":"rm -rf /"},"error_message":"Denied","failure_type":"permission_denied"}));
    assert_eq!(failure["hook_event_name"], "PostToolUseFailure");
    assert_eq!(failure["error"], "Permission denied: Denied");
}

#[test]
fn cursor_plugin_declares_all_supported_hook_events() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("reference/upstream/plugin");
    let read = |path: &Path| serde_json::from_slice::<Value>(&fs::read(path).unwrap()).unwrap();
    let manifest = read(&root.join(".cursor-plugin/plugin.json"));
    let hooks = read(&root.join("hooks/cursor-hooks.json"));
    let hooks = hooks["hooks"].as_object().unwrap();
    assert_eq!(manifest["name"], "tokeneconomizer");
    assert_eq!(manifest["hooks"], "./hooks/cursor-hooks.json");
    assert_eq!(
        hooks.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        [
            "sessionStart",
            "beforeSubmitPrompt",
            "postToolUse",
            "postToolUseFailure",
            "stop",
            "sessionEnd"
        ]
        .into()
    );
    assert!(
        hooks
            .values()
            .flat_map(|entries| entries.as_array().unwrap())
            .all(|hook| {
                let command = hook["command"].as_str().unwrap();
                command.contains("${CURSOR_PLUGIN_ROOT}/scripts/te-hook")
                    && command.contains("--host cursor")
            })
    );
}

#[test]
fn cursor_events_reconstruct_tool_results_and_count_prompts() {
    let root = TempDir::new("cursor-events");
    let source = root.path().join("events.jsonl");
    let target = root.path().join("trace.jsonl");
    write_lines(
        &source,
        &[
            json!({"hook_event_name":"UserPromptSubmit","prompt":"Fix the bug"}),
            json!({"hook_event_name":"PostToolUse","tool_use_id":"t1","tool_name":"Shell","tool_input":{"command":"pytest"},"tool_response":{"content":"3 passed","exit_code":0}}),
            json!({"hook_event_name":"PostToolUseFailure","tool_use_id":"t2","tool_name":"Read","tool_input":{"file_path":"missing"},"error":"not found"}),
        ],
    );
    assert_eq!(cursor::prompt_count(&source), 1);
    assert_eq!(cursor::events_to_claude(&source, &target), Some(5));
    assert_eq!(
        calls::load_trajectory(&target)
            .into_iter()
            .map(|(name, args, _, ok)| (name, args, ok))
            .collect::<Vec<_>>(),
        [
            (json!("Bash"), json!({"command":"pytest"}), json!(true)),
            (json!("Read"), json!({"file_path":"missing"}), json!(false))
        ]
    );
}

#[test]
fn cursor_readers_skip_json_lines_that_are_not_objects() {
    let root = TempDir::new("cursor-nonobjects");
    let source = root.path().join("events.jsonl");
    let target = root.path().join("trace.jsonl");
    write_lines(
        &source,
        &[
            Value::Null,
            json!([1, 2]),
            json!("str"),
            json!(42),
            json!({"hook_event_name":"UserPromptSubmit","prompt":"Fix the bug"}),
            json!({"hook_event_name":"PostToolUse","tool_use_id":"t1","tool_name":"Shell","tool_input":{"command":"pytest"},"tool_response":{"content":"3 passed","exit_code":0}}),
        ],
    );
    assert_eq!(cursor::prompt_count(&source), 1);
    assert_eq!(cursor::events_to_claude(&source, &target), Some(3));
    assert_eq!(
        calls::load_trajectory(&target)
            .into_iter()
            .map(|(name, args, _, _)| (name, args))
            .collect::<Vec<_>>(),
        [(json!("Bash"), json!({"command":"pytest"}))]
    );
}

#[test]
fn host_stopped_normally_default() {
    for host in ["claude", "codex", "copilot", "opencode", "pi"] {
        assert!(hosts::stopped_normally(host, &json!({})), "{host}");
    }
}

#[test]
fn cursor_stop_lifecycle_status_handling() {
    for (status, normal) in [
        (Some("completed"), true),
        (Some("aborted"), false),
        (Some("error"), false),
        (None, false),
        (Some("unknown"), false),
    ] {
        let root = TempDir::new("cursor-stop");
        let mut fixture = Cursor::new(root.path());
        fixture.guard_on();
        let base =
            json!({"conversation_id":"c1","generation_id":"g1","workspace_roots":[fixture.repo]});
        fixture.tool(
            &base,
            json!({"hook_event_name":"beforeSubmitPrompt","prompt":"check"}),
        );
        fixture.tool(&base,json!({"hook_event_name":"postToolUse","tool_name":"Shell","tool_use_id":"t1","tool_input":{"command":"pytest"},"tool_output":"{\"output\":\"1 passed\",\"exitCode\":0}"}));
        let mut stop = json!({"hook_event_name":"stop"});
        if let Some(status) = status {
            stop["status"] = json!(status);
        }
        assert_eq!(fixture.tool(&base, stop), json!({}));
        let session = fixture.store.read("sessions/c1.json", Value::Null).unwrap();
        assert!(!session.is_null() && session["turn"].is_null());
        assert_eq!(session["turns"].as_array().unwrap().len(), 1);
        assert_eq!(session["turns"][0]["resolved_proxy"], normal);
        assert_eq!(
            fixture.resolved_turns(),
            if normal { vec![1.0] } else { vec![] }
        );
    }
}

#[test]
fn cursor_native_shell_json_output_positive_and_negative() {
    let root = TempDir::new("cursor-shell-json");
    let mut fixture = Cursor::new(root.path());
    let base = json!({"conversation_id":"c-shell","generation_id":"g-shell","workspace_roots":[fixture.repo]});
    fixture.tool(
        &base,
        json!({"hook_event_name":"beforeSubmitPrompt","prompt":"build & test"}),
    );
    for (id, command, output) in [
        ("t1", "make build", json!({"output":"ok\n","exitCode":0})),
        ("t2", "pytest -q", json!({"output":"failed\n","exitCode":1})),
    ] {
        fixture.tool(&base,json!({"hook_event_name":"postToolUse","tool_name":"Shell","tool_use_id":id,"tool_input":{"command":command},"tool_output":output.to_string()}));
    }
    let mut payload = base;
    payload["session_id"] = json!("c-shell");
    let prepared = fixture.prepare(&payload);
    let path = session::transcript("cursor", &prepared, &fixture.environment).unwrap();
    let trace =
        session::mechanism_trace("cursor", path.as_str(), root.path(), &fixture.environment)
            .unwrap()
            .unwrap();
    assert_eq!(
        calls::load_trajectory(&trace),
        [
            (
                json!("Bash"),
                json!({"command":"make build"}),
                "ok\n".into(),
                json!(true)
            ),
            (
                json!("Bash"),
                json!({"command":"pytest -q"}),
                "failed\n".into(),
                json!(false)
            )
        ]
    );
}

#[test]
fn notices_name_each_agents_side_call_command() {
    assert_eq!(
        [
            "claude", "codex", "copilot", "pi", "opencode", "cursor", "hermes"
        ]
        .map(sidecall::call_name),
        [
            "claude -p",
            "codex exec",
            "copilot -p",
            "pi -p",
            "opencode run",
            "cursor-agent -p",
            "hermes chat"
        ]
    );
}

fn agent_log(root: &Path, users: usize) -> PathBuf {
    let path = root.join("agent-transcripts/h1/h1.jsonl");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut rows = vec![];
    for _ in 0..users {
        rows.extend([
            json!({"role":"user","message":{"content":[{"type":"text","text":"q"}]}}),
            json!({"role":"assistant","message":{"content":[{"type":"text","text":"a"}]}}),
            json!({"type":"turn_ended","status":"success"}),
        ]);
    }
    write_lines(&path, &rows);
    path
}
fn headless_run(fixture: &mut Cursor, log: &Path, end: Value) {
    let base = json!({"session_id":"h1","conversation_id":"h1","generation_id":"h1","workspace_roots":[fixture.repo]});
    fixture.tool(
        &base,
        json!({"hook_event_name":"sessionStart","transcript_path":null}),
    );
    for (id, name, input, output, transcript) in [
        (
            "call_a\nfc_1",
            "Shell",
            json!({"command":"ls","cwd":fixture.repo,"timeout":30000}),
            "{\"output\":\"\",\"exitCode\":0}",
            Value::Null,
        ),
        (
            "call_b\nfc_2",
            "Read",
            json!({"file_path":fixture.repo.join("a.py")}),
            "x = 1",
            json!(log),
        ),
    ] {
        fixture.tool(&base,json!({"hook_event_name":"postToolUse","tool_use_id":id,"tool_name":name,"tool_input":input,"tool_output":output,"transcript_path":transcript}));
    }
    let mut fields = json!({"hook_event_name":"sessionEnd","transcript_path":log});
    fields
        .as_object_mut()
        .unwrap()
        .extend(end.as_object().unwrap().clone());
    fixture.tool(&base, fields);
}
fn completed() -> Value {
    json!({"reason":"completed","final_status":"completed","duration_ms":5})
}

#[test]
fn cursor_headless_run_yields_one_turn_closed_normally_at_session_end() {
    let root = TempDir::new("cursor-headless");
    let mut fixture = Cursor::new(root.path());
    fixture.guard_on();
    headless_run(&mut fixture, &agent_log(root.path(), 1), completed());
    let session = fixture.store.read("sessions/h1.json", Value::Null).unwrap();
    assert!(session["turn"].is_null());
    assert_eq!(session["turns"].as_array().unwrap().len(), 1);
    assert_eq!(session["turns"][0]["resolved_proxy"], true);
    assert_eq!(session["turns"][0]["tool_calls"], 2);
    assert_eq!(fixture.resolved_turns(), [2.0]);
    assert!(fixture.folds.iter().any(|id| id == "h1"));
}

#[test]
fn cursor_session_end_closes_open_turn_as_not_normal_unless_completed() {
    for end in [
        json!({"reason":"aborted","final_status":"aborted"}),
        json!({"reason":"user_close"}),
        json!({"duration_ms":1}),
    ] {
        let root = TempDir::new("cursor-aborted");
        let mut fixture = Cursor::new(root.path());
        fixture.guard_on();
        headless_run(&mut fixture, &agent_log(root.path(), 1), end);
        let session = fixture.store.read("sessions/h1.json", Value::Null).unwrap();
        assert_eq!(session["turns"].as_array().unwrap().len(), 1);
        assert_eq!(session["turns"][0]["resolved_proxy"], false);
    }
}

#[test]
fn cursor_completed_session_end_does_not_close_a_turn_twice() {
    let root = TempDir::new("cursor-close-once");
    let mut fixture = Cursor::new(root.path());
    fixture.guard_on();
    let base =
        json!({"conversation_id":"c2","generation_id":"g2","workspace_roots":[fixture.repo]});
    fixture.tool(
        &base,
        json!({"hook_event_name":"beforeSubmitPrompt","prompt":"go"}),
    );
    fixture.tool(&base,json!({"hook_event_name":"postToolUse","tool_name":"Shell","tool_use_id":"t","tool_input":{"command":"ls"},"tool_output":"{\"output\":\"\",\"exitCode\":0}"}));
    fixture.tool(
        &base,
        json!({"hook_event_name":"stop","status":"completed"}),
    );
    fixture.tool(
        &base,
        json!({"hook_event_name":"sessionEnd","reason":"completed","final_status":"completed"}),
    );
    let session = fixture.store.read("sessions/c2.json", Value::Null).unwrap();
    assert_eq!(session["turns"].as_array().unwrap().len(), 1);
    assert_eq!(fixture.resolved_turns(), [1.0]);
}

#[test]
fn cursor_prompt_count_falls_back_to_agent_transcript() {
    let root = TempDir::new("cursor-prompt-fallback");
    let mut fixture = Cursor::new(root.path());
    headless_run(&mut fixture, &agent_log(root.path(), 2), completed());
    let prepared = fixture.prepare(&json!({"conversation_id":"h1"}));
    let spool = session::transcript("cursor", &prepared, &fixture.environment).unwrap();
    let spool = Path::new(spool.as_str().unwrap());
    assert_eq!(session::prompt_count("cursor", spool), Some(2));
    let output =
        session::mechanism_trace("cursor", spool.to_str(), root.path(), &fixture.environment)
            .unwrap()
            .unwrap();
    assert_eq!(
        calls::load_trajectory(&output)
            .into_iter()
            .map(|(name, args, _, _)| (name, args))
            .collect::<Vec<_>>(),
        [
            (
                json!("Bash"),
                json!({"command":"ls","cwd":fixture.repo,"timeout":30000})
            ),
            (
                json!("Read"),
                json!({"file_path":fixture.repo.join("a.py")})
            )
        ]
    );
}

#[test]
fn cursor_prompt_count_prefers_recorded_prompts_and_tolerates_gaps() {
    let root = TempDir::new("cursor-prompt-gaps");
    let spool = root.path().join("s.jsonl");
    let log = root.path().join("agent.jsonl");
    write_lines(&log, &[json!({"role":"user"})]);
    let row = json!({"hook_event_name":"PostToolUse","agent_transcript":log});
    write_lines(&spool, std::slice::from_ref(&row));
    assert_eq!(cursor::prompt_count(&spool), 1);
    write_lines(
        &spool,
        &[
            row.clone(),
            json!({"hook_event_name":"UserPromptSubmit","prompt":"p"}),
        ],
    );
    assert_eq!(cursor::prompt_count(&spool), 1);
    fs::remove_file(log).unwrap();
    write_lines(&spool, &[row]);
    assert_eq!(cursor::prompt_count(&spool), 0);
}

#[test]
fn cursor_prepare_keeps_agent_transcript_path_only_when_set() {
    let root = TempDir::new("cursor-transcript-path");
    let env = Environment {
        env: json!({"TOKENECONOMIZER_HOME":root.path()})
            .as_object()
            .unwrap()
            .clone(),
        home: root.path().into(),
        cwd: root.path().into(),
    };
    let null = state::prepare("cursor",&json!({"hook_event_name":"sessionStart","session_id":"s","conversation_id":"s","transcript_path":null}),&json!(env.env),&env).unwrap();
    assert!(null.get("agent_transcript").is_none());
    let real = state::prepare("cursor",&json!({"hook_event_name":"sessionEnd","session_id":"s","conversation_id":"s","transcript_path":"/home/u/.cursor/projects/x/agent-transcripts/s/s.jsonl"}),&json!(env.env),&env).unwrap();
    assert!(
        real["agent_transcript"]
            .as_str()
            .unwrap()
            .ends_with("/s/s.jsonl")
    );
    assert!(
        Path::new(real["transcript_path"].as_str().unwrap()).ends_with("cursor-sessions/s.jsonl")
    );
}
