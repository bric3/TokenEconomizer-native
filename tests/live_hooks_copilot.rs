// Converted from reference/upstream/tests/test_live_hooks.py.
pub mod fixtures;
#[path = "support/live_hook_fixture.rs"]
pub mod live_fixture;
mod support;

use live_fixture::{HookFixture, cli, copilot_session, fixture, hook_fixture::ProposerReply};
use serde_json::{Value, json};
use std::fs;
use support::TempDir;
use tokeneconomizer::plugin::{adapter, denylist, sidecall};

const GITHUB_TOOLS: [&str; 3] = [
    "github-mcp-server-list_issues",
    "github-mcp-server-get_commit",
    "github-mcp-server-search_code",
];
const SIGNED_OUT: &str = "Error: No authentication information found. Copilot can be authenticated with GitHub using an OAuth Token or a Fine-Grained Personal Access Token. To authenticate, you can use any of the following methods: Start 'copilot' and run the '/login' command";

fn trap() -> [Value; 2] {
    [
        json!(["bash", {"command":"pytest -q tests/test_a.py"}, "bash: pytest: command not found\n<shellId: 0 completed with exit code 127>", true]),
        json!(["bash", {"command":"python -m pytest -q tests/test_a.py"}, "1 passed in 0.1s\n<shellId: 1 completed with exit code 0>", true]),
    ]
}

fn catalog(f: &HookFixture, builtins: &[&str]) {
    let path = adapter::catalog_path(&f.environment.home.join("state"), "copilot");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let tools: Vec<_> = builtins.iter().copied().chain(GITHUB_TOOLS).collect();
    fs::write(path, json!({"tools":tools}).to_string()).unwrap();
}

fn unused_sessions(f: &mut HookFixture, sessions: usize, prompts: usize) {
    let calls =
        [json!(["bash", {"command":"ls"}, "a.py\n<shellId: 0 completed with exit code 0>", true])];
    for i in 0..sessions {
        copilot_session(f, &format!("u{i}"), &calls, prompts);
    }
}

fn untool(f: &HookFixture) -> Value {
    f.store.read("untool.json", Value::Null).unwrap()
}

fn progress(output: &Value) -> &str {
    let [notice] = output["progress"].as_array().unwrap().as_slice() else {
        panic!("Expected one progress notice: {output}");
    };
    notice.as_str().unwrap()
}

#[test]
fn copilot_learns_from_its_own_logs_and_injects_at_top_level() {
    let tmp = TempDir::new("copilot-live-learn");
    let mut f = fixture(tmp.path(), "copilot");
    let starts: Vec<_> = (0..5)
        .map(|i| copilot_session(&mut f, &format!("cp{i}"), &trap(), 1))
        .collect();
    assert!(starts[..4].iter().all(|output| *output == json!({})));
    let learned = &starts[4];
    assert!(learned.get("hookSpecificOutput").is_none());
    assert!(learned.get("systemMessage").is_none());
    assert!(
        learned["additionalContext"]
            .as_str()
            .unwrap()
            .contains("`python -m pytest …` works.")
    );
    let notice = progress(learned);
    assert!(notice.contains("learned fact") && notice.contains("/tokeneconomizer:microskills"));

    let ledger = f.store.read_lines("ledger.jsonl", None).unwrap();
    let first = &ledger[0];
    assert_eq!(first["host"], "copilot");
    assert_eq!(first["requests"].as_f64(), Some(4.0));
    assert_eq!(first["priced_by"], "model");
    assert_eq!(
        first["input_tokens"].as_f64(),
        Some(4000.0 - 2400.0 - 100.0)
    );
    let events = f.store.read_lines("events.jsonl", None).unwrap();
    let turns: Vec<_> = events
        .iter()
        .filter(|event| event["kind"] == "turn")
        .collect();
    assert_eq!(turns.len(), 5);
    assert!(turns.iter().all(|event| event["tool_calls"] == 2));
}

#[test]
fn a_signed_out_copilot_pauses_the_model_calls_and_says_so_once() {
    let tmp = TempDir::new("copilot-live-auth");
    let mut f = fixture(tmp.path(), "copilot");
    f.environment
        .env
        .insert("TE_MICROSKILLS".into(), json!("on"));
    f.proposer_reply = Some(ProposerReply::SignedOut(SIGNED_OUT.into()));
    let starts: Vec<_> = (0..6)
        .map(|i| copilot_session(&mut f, &format!("b{i}"), &trap(), 1))
        .collect();
    assert_eq!(f.proposer_models.borrow().len(), 1);
    let said: Vec<_> = starts
        .iter()
        .flat_map(|output| output["progress"].as_array().into_iter().flatten())
        .filter_map(Value::as_str)
        .filter(|notice| notice.contains("not signed in"))
        .collect();
    assert_eq!(said.len(), 1);
    assert!(said[0].contains("Copilot does not pass it to the plugin's hooks"));
    assert!(
        starts[2]["progress"]
            .as_array()
            .unwrap()
            .contains(&json!(said[0]))
    );
    assert!(
        starts[4]["additionalContext"]
            .as_str()
            .unwrap()
            .contains("`python -m pytest …` works.")
    );
    let state = tmp.path().join("state");
    assert!(sidecall::paused("copilot", &state).is_object());
    assert!(
        cli(&f, &["config", "set", "microskills=on"])
            .status
            .success()
    );
    assert!(sidecall::paused("copilot", &state).is_null());
}

#[test]
fn copilot_proposes_only_whole_unused_mcp_servers() {
    let tmp = TempDir::new("copilot-server-proposal");
    let mut f = fixture(tmp.path(), "copilot");
    f.environment
        .env
        .insert("TE_TOOLPRUNE".into(), json!("propose"));
    catalog(
        &f,
        &["bash", "view", "sql", "fetch_copilot_cli_documentation"],
    );
    unused_sessions(&mut f, 5, 2);
    let data = untool(&f);
    assert_eq!(data["proposal"], json!(["github-mcp-server"]));
    assert_eq!(
        data["servers"]["github-mcp-server"],
        json!({"tools":3,"used":false,"unused_sessions":5,"unused_prompts":10})
    );
    assert!(!denylist::settings_path(&f.repo, "copilot").exists());
    let output = copilot_session(&mut f, "n", &[], 1);
    let notice = progress(&output);
    assert!(
        notice.contains("1 MCP server(s) went unused")
            && notice.contains("/tokeneconomizer:toolprune")
    );
}

#[test]
fn two_copilot_sessions_of_five_prompts_are_enough_to_propose_a_server() {
    let tmp = TempDir::new("copilot-server-prompts");
    let mut f = fixture(tmp.path(), "copilot");
    f.environment
        .env
        .insert("TE_TOOLPRUNE".into(), json!("propose"));
    catalog(&f, &["bash", "view"]);
    unused_sessions(&mut f, 2, 5);
    let data = untool(&f);
    assert_eq!(data["proposal"], json!(["github-mcp-server"]));
    assert_eq!(data["servers"]["github-mcp-server"]["unused_sessions"], 2);
    assert_eq!(data["servers"]["github-mcp-server"]["unused_prompts"], 10);
}

#[test]
fn by_default_copilot_turns_off_the_unused_server_and_says_so() {
    let tmp = TempDir::new("copilot-server-auto");
    let mut f = fixture(tmp.path(), "copilot");
    catalog(&f, &["bash", "view"]);
    unused_sessions(&mut f, 5, 2);
    let settings = denylist::read_settings(&denylist::settings_path(&f.repo, "copilot")).unwrap();
    assert_eq!(settings["disabledMcpServers"], json!(["github-mcp-server"]));
    let output = copilot_session(&mut f, "n", &[], 1);
    let notice = progress(&output);
    assert!(notice.contains("removed 1 MCP server(s)") && notice.contains("toolprune restore"));
}

#[test]
fn a_server_whose_tool_was_used_once_is_never_proposed() {
    let tmp = TempDir::new("copilot-server-used");
    let mut f = fixture(tmp.path(), "copilot");
    f.environment
        .env
        .insert("TE_TOOLPRUNE".into(), json!("propose"));
    catalog(&f, &["bash"]);
    copilot_session(
        &mut f,
        "used",
        &[json!(["github-mcp-server-search_code",{"query":"x"},"[]",true])],
        1,
    );
    for i in 0..8 {
        copilot_session(
            &mut f,
            &format!("u{i}"),
            &[json!(["bash",{"command":"ls"},"a.py",true])],
            1,
        );
    }
    let data = untool(&f);
    assert_eq!(data["proposal"], json!([]));
    assert_eq!(data["servers"]["github-mcp-server"]["used"], true);
}

#[test]
fn copilot_apply_and_restore_touch_only_the_plugins_own_servers() {
    let tmp = TempDir::new("copilot-server-ownership");
    let f = fixture(tmp.path(), "copilot");
    let path = denylist::settings_path(&f.repo, "copilot");
    assert!(path.ends_with(".github/copilot/settings.local.json"));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let user = json!({"disabledMcpServers":["mine"],"model":"gpt-5.4"});
    fs::write(&path, user.to_string()).unwrap();
    let mut runtime = denylist::NativeRuntime;
    let items = ["github-mcp-server".into()];
    assert_eq!(
        denylist::apply(&f.repo, &items, &f.store, "copilot", &mut runtime).unwrap(),
        items
    );
    assert_eq!(
        denylist::read_settings(&path).unwrap(),
        json!({"disabledMcpServers":["mine","github-mcp-server"],"model":"gpt-5.4"})
    );
    assert_eq!(
        denylist::restore(&f.repo, &f.store, &[], "copilot", &mut runtime).unwrap(),
        items
    );
    assert_eq!(denylist::read_settings(&path).unwrap(), user);
    fs::remove_file(path).unwrap();
    denylist::apply(&f.repo, &items, &f.store, "copilot", &mut runtime).unwrap();
    let rules = fs::read_to_string(f.repo.join(".git/info/exclude")).unwrap();
    assert!(
        rules
            .split_whitespace()
            .any(|rule| rule == "/.github/copilot/settings.local.json")
    );
}
