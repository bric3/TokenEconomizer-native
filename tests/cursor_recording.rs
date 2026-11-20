// Converted from reference/upstream/tests/test_cursor_recording.py.
mod support;

use serde_json::{Map, Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use support::TempDir;
use tokeneconomizer::plugin::{
    hosts::{
        RESULT_CAP,
        session::{self, Environment},
    },
    paths,
};

struct Fixture {
    repo: PathBuf,
    state: PathBuf,
    env: Map<String, Value>,
    home: PathBuf,
}
impl Fixture {
    fn new(root: &Path) -> Self {
        let repo = root.join("repo");
        fs::create_dir_all(repo.join(".git/info")).unwrap();
        fs::write(repo.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(
            repo.join(".git/config"),
            "[remote \"origin\"]\n\turl = https://github.com/acme/project.git\n",
        )
        .unwrap();
        let state = root.join("state");
        fs::create_dir(&state).unwrap();
        let env = json!({"TOKENECONOMIZER_HOME":state,"GIT_CEILING_DIRECTORIES":root,"TE_LOOPGUARD":"off","TE_MICROSKILLS":"off","TE_REPOMAP":"off"}).as_object().unwrap().clone();
        Self {
            repo,
            state,
            env,
            home: root.into(),
        }
    }
    fn call(&self, payload: Value) -> (i32, Value) {
        let mut child = Command::new(env!("CARGO_BIN_EXE_te-hook"))
            .args(["--host", "cursor"])
            .env_clear()
            .env("HOME", &self.home)
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .envs(self.env.iter().map(|(k, v)| (k, v.as_str().unwrap())))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        writeln!(child.stdin.take().unwrap(), "{payload}").unwrap();
        let output = child.wait_with_output().unwrap();
        let value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "{error}: {} / {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        });
        (output.status.code().unwrap(), value)
    }
    fn prompt(&self, id: &str) -> Value {
        json!({"hook_event_name":"beforeSubmitPrompt","conversation_id":id,"generation_id":"gen-p-1","prompt":"Fix bug in payment handler","workspace_roots":[self.repo],"cwd":self.repo})
    }
    fn tool(&self, id: &str) -> Value {
        json!({"hook_event_name":"postToolUse","conversation_id":id,"generation_id":"gen-p-1","tool_use_id":"tool-rec-1","tool_name":"Shell","tool_input":{"command":"npm test"},"tool_output":"{\"exitCode\":0,\"output\":\"PASS test/pay.test.js\"}","workspace_roots":[self.repo],"cwd":self.repo})
    }
    fn spool(&self, id: &str) -> PathBuf {
        self.state
            .join("cursor-sessions")
            .join(format!("{id}.jsonl"))
    }
    fn rows(&self, id: &str) -> Vec<Value> {
        fs::read_to_string(self.spool(id))
            .unwrap()
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
}

#[test]
fn cursor_records_prompt_and_tool_when_enabled() {
    let root = TempDir::new("cursor-record-enabled");
    let fixture = Fixture::new(root.path());
    assert_eq!(fixture.call(fixture.prompt("conv-rec-1")), (0, json!({})));
    assert_eq!(fixture.call(fixture.tool("conv-rec-1")), (0, json!({})));
    assert!(fixture.spool("conv-rec-1").exists());
    let rows = fixture.rows("conv-rec-1");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["hook_event_name"], "UserPromptSubmit");
    assert_eq!(rows[0]["prompt"], "Fix bug in payment handler");
    assert_eq!(rows[1]["hook_event_name"], "PostToolUse");
    assert_eq!(
        rows[1]["tool_response"],
        json!({"content":"PASS test/pay.test.js","exit_code":0,"isError":false})
    );
}

#[test]
fn global_config_disabled_creates_no_new_spool_and_does_not_append() {
    let root = TempDir::new("cursor-record-global-off");
    let fixture = Fixture::new(root.path());
    fs::write(
        fixture.state.join("config.json"),
        json!({"enabled":false}).to_string(),
    )
    .unwrap();
    let id = "conv-disabled-global";
    assert_eq!(fixture.call(fixture.prompt(id)), (0, json!({})));
    let spool = fixture.spool(id);
    assert!(!spool.exists());
    fs::create_dir_all(spool.parent().unwrap()).unwrap();
    fs::write(&spool, "{\"seed\":true}\n").unwrap();
    assert_eq!(fixture.call(fixture.tool(id)), (0, json!({})));
    assert_eq!(fs::read_to_string(spool).unwrap(), "{\"seed\":true}\n");
}

#[test]
fn repo_config_disabled_creates_no_new_spool_and_does_not_append() {
    let root = TempDir::new("cursor-record-repo-off");
    let fixture = Fixture::new(root.path());
    let repo = paths::repo_identity(&fixture.repo, &fixture.env, root.path()).unwrap();
    let config_dir = fixture.state.join("repos").join(repo.key);
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("config.json"),
        json!({"enabled":false}).to_string(),
    )
    .unwrap();
    let id = "conv-disabled-repo";
    assert_eq!(fixture.call(fixture.prompt(id)), (0, json!({})));
    let spool = fixture.spool(id);
    assert!(!spool.exists());
    fs::create_dir_all(spool.parent().unwrap()).unwrap();
    fs::write(&spool, "{\"seed\":true}\n").unwrap();
    assert_eq!(fixture.call(fixture.tool(id)), (0, json!({})));
    assert_eq!(fs::read_to_string(spool).unwrap(), "{\"seed\":true}\n");
}

#[test]
fn env_disable_and_sidecall_skip_recording() {
    let root = TempDir::new("cursor-record-env-skip");
    let mut fixture = Fixture::new(root.path());
    fixture.env.insert("TE_DISABLE".into(), json!("1"));
    assert_eq!(
        fixture.call(fixture.prompt("conv-env-skip")),
        (0, json!({}))
    );
    assert!(!fixture.spool("conv-env-skip").exists());
    fixture.env.remove("TE_DISABLE");
    fixture.env.insert("TE_SIDECALL".into(), json!("1"));
    assert_eq!(fixture.call(fixture.tool("conv-env-skip")), (0, json!({})));
    assert!(!fixture.spool("conv-env-skip").exists());
}

#[test]
fn recording_keeps_adapter_fields_and_drops_user_email() {
    let root = TempDir::new("cursor-record-email");
    let fixture = Fixture::new(root.path());
    let mut payload = fixture.prompt("conv-fields");
    payload["user_email"] = json!("person@example.com");
    payload["CURSOR_USER_EMAIL"] = json!("person@example.com");
    assert_eq!(fixture.call(payload), (0, json!({})));
    let rows = fixture.rows("conv-fields");
    let row = &rows[0];
    assert_eq!(row["hook_event_name"], "UserPromptSubmit");
    assert_eq!(row["prompt"], "Fix bug in payment handler");
    assert!(row.get("user_email").is_none());
    assert!(row.get("CURSOR_USER_EMAIL").is_none());
    assert!(!row.to_string().contains("user_email"));
}

#[test]
fn recording_caps_tool_output() {
    let root = TempDir::new("cursor-record-output-cap");
    let fixture = Fixture::new(root.path());
    let mut payload = fixture.tool("conv-cap");
    payload["tool_output"] =
        json!(json!({"exitCode":0,"output":"x".repeat(RESULT_CAP+40)}).to_string());
    assert_eq!(fixture.call(payload), (0, json!({})));
    let rows = fixture.rows("conv-cap");
    assert_eq!(rows[0]["tool_response"]["content"], "x".repeat(RESULT_CAP));
    assert!(rows[0].get("user_email").is_none());
}

#[test]
fn recording_caps_tool_input_and_keeps_its_short_fields() {
    let root = TempDir::new("cursor-record-input-cap");
    let fixture = Fixture::new(root.path());
    let mut payload = fixture.tool("conv-input");
    payload["tool_name"] = json!("Write");
    payload["tool_input"] = json!({"file_path":"src/big.py","contents":"y".repeat(RESULT_CAP+40)});
    assert_eq!(fixture.call(payload), (0, json!({})));
    assert_eq!(
        fixture.rows("conv-input")[0]["tool_input"],
        json!({"file_path":"src/big.py","contents":"y".repeat(RESULT_CAP)})
    );
}

#[test]
fn settle_deletes_processed_session_file() {
    let root = TempDir::new("cursor-record-settle");
    let spool = root.path().join("cursor-sessions/conv.jsonl");
    let other = root.path().join("elsewhere/conv.jsonl");
    for path in [&spool, &other] {
        fs::create_dir(path.parent().unwrap()).unwrap();
        fs::write(path, "{}\n").unwrap();
    }
    let environment = Environment {
        env: Map::new(),
        home: root.path().into(),
        cwd: root.path().into(),
    };
    for path in [&spool, &other] {
        session::settle(
            "cursor",
            Some(path),
            0.0,
            0.0,
            &mut session::NativeRuntime {
                environment: &environment,
            },
        )
        .unwrap();
    }
    assert!(!spool.exists());
    assert!(other.exists());
}
