// Converted from reference/upstream/experiments/tests/test_tooling.py.
pub mod fixtures;
#[path = "support/tooling_harness.rs"]
pub mod harness;
#[path = "support/codex_install_fixture.rs"]
pub mod install_fixture;
mod support;

#[cfg(unix)]
use install_fixture::Install;
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::Path,
    process::{Command, Output, Stdio},
};
use support::TempDir;
use tokeneconomizer::{
    config::ToolingConfig,
    experiments::{
        adapters::Adapter,
        net_guard,
        swebench::{codex_config, settings},
    },
    toolprune::{
        codex::{self as cu, CodexUntoolProfile},
        protected::ProtectedTools,
        stage::{ToolingAdapter, ToolingContext, ToolingStage},
        tools::{INAPPLICABLE_TOOLS, PROTECTED_TOOLS},
    },
};

const CATALOG: &[&str] = &[
    "Bash",
    "Edit",
    "Read",
    "Write",
    "WebFetch",
    "WebSearch",
    "Workflow",
    "Skill",
    "NotebookEdit",
    "CronCreate",
    "TaskCreate",
];
fn has(values: &Value, name: &str) -> bool {
    values.as_array().unwrap().contains(&json!(name))
}
fn empty_usage() -> Value {
    json!({"attempted":{},"completed":{},"failed":{}})
}
fn profile(root: &Path, repo: &str) -> CodexUntoolProfile {
    CodexUntoolProfile::new(repo, root, true).unwrap()
}
fn stage(root: &Path, adapter: &mut dyn ToolingAdapter, evidence: i64) -> ToolingStage {
    ToolingStage::new(
        ToolingConfig {
            evidence,
            judge: false,
            ..Default::default()
        },
        "claude",
        "agent",
        Some(("acme/widget", root)),
        true,
        &mut ToolingContext {
            adapter: Some(adapter),
            ..Default::default()
        },
    )
    .unwrap()
}
fn tools(
    stage: &mut ToolingStage,
    adapter: &mut dyn ToolingAdapter,
    task: &str,
    index: i64,
) -> Option<Vec<String>> {
    stage
        .build_tools(
            &json!({"instance_id":task}),
            index,
            &mut ToolingContext {
                adapter: Some(adapter),
                ..Default::default()
            },
        )
        .unwrap()
}
fn finish(
    stage: &mut ToolingStage,
    adapter: &mut dyn ToolingAdapter,
    root: &Path,
    index: i32,
    used: &[&str],
) {
    let directory = root.join(format!("run{index}"));
    fs::create_dir_all(&directory).unwrap();
    let calls: Vec<_> = used
        .iter()
        .map(|name| {
            json!([
                name,
                if *name == "Bash" {
                    json!({"command":"x"})
                } else {
                    json!({"file_path":"a.py"})
                },
                "ok"
            ])
        })
        .collect();
    fixtures::write_claude_transcript_with_catalog(
        &directory.join("transcript.jsonl"),
        CATALOG,
        &calls,
    );
    stage
        .after_task(
            &json!({}),
            &directory,
            &mut json!({}),
            &mut ToolingContext {
                adapter: Some(adapter),
                ..Default::default()
            },
        )
        .unwrap();
}
fn guard(root: &Path, input: &str, allow: Option<&str>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_te-net-guard"));
    command
        .env_clear()
        .env("HOME", root)
        .env("PATH", "")
        .current_dir(root);
    if let Some(allow) = allow {
        command.env("TE_ALLOW_NETWORK_TOOLS", allow);
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
        .unwrap_or_else(|error| assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe));
    child.wait_with_output().unwrap()
}

#[test]
fn codex_profile_task1_full_contract() {
    let root = TempDir::new("tooling-contract");
    let mut p = profile(root.path(), "pydata/xarray");
    let selection = p.begin_task(&json!("x-1"), &json!("fix concat")).unwrap();
    assert_eq!(selection["audit"], true);
    assert_eq!(selection["relay_capabilities"], json!([]));
    assert!(has(&selection["core_capabilities"], "deferred_wait"));
}

#[test]
fn codex_profile_never_prunes_core() {
    let root = TempDir::new("tooling-core");
    let mut p = profile(root.path(), "pydata/xarray");
    for i in 0..6 {
        let selection = p.begin_task(&json!(format!("x-{i}")), &json!("")).unwrap();
        assert!(!has(&selection["relay_capabilities"], "deferred_wait"));
        p.observe(&empty_usage(), json!(true)).unwrap();
    }
    assert!(!has(&p.state["current_pruned"], "deferred_wait"));
}

#[test]
fn codex_profile_demotes_unused_web() {
    let root = TempDir::new("tooling-unused-web");
    let mut p = profile(root.path(), "pydata/xarray");
    p.begin_task(&json!("x-1"), &json!("")).unwrap();
    p.observe(&empty_usage(), json!(true)).unwrap();
    assert!(has(
        &p.begin_task(&json!("x-2"), &json!("")).unwrap()["relay_capabilities"],
        "web"
    ));
}

#[test]
fn codex_profile_completed_use_protects() {
    let root = TempDir::new("tooling-completed-web");
    let mut p = profile(root.path(), "pydata/xarray");
    p.begin_task(&json!("x-1"), &json!("")).unwrap();
    p.observe(
        &json!({"attempted":{"web":1},"completed":{"web":1},"failed":{}}),
        json!(true),
    )
    .unwrap();
    assert!(has(&p.state["ever_completed"], "web"));
    assert!(!has(
        &p.begin_task(&json!("x-2"), &json!("")).unwrap()["relay_capabilities"],
        "web"
    ));
}

#[test]
fn inspect_usage_counts_web() {
    let root = TempDir::new("tooling-web-usage");
    let path = fixtures::write_codex_transcript(
        &root.path().join("t.jsonl"),
        &[
            json!({"type":"web_search","name":"web_search","status":"failed"}),
            json!({"type":"command_execution","command":"pytest"}),
        ],
    );
    let usage = cu::inspect_usage(&path);
    assert!(usage["attempted"]["web"].as_u64().unwrap() >= 1);
    assert_eq!(usage["completed"]["web"], 0);
}

#[test]
fn tooling_stage_codex_mask() {
    let root = TempDir::new("tooling-codex-stage");
    let mut adapter = Adapter::swebench(None, &Map::new());
    let mut stage = ToolingStage::new(
        ToolingConfig {
            codex_untool: true,
            ..Default::default()
        },
        "codex",
        "agent",
        Some(("pydata/xarray", root.path())),
        true,
        &mut ToolingContext {
            adapter: Some(&mut adapter),
            ..Default::default()
        },
    )
    .unwrap();
    let mask = stage
        .build_tools(
            &json!({"instance_id":"x-1","problem_statement":"fix"}),
            0,
            &mut ToolingContext {
                adapter: Some(&mut adapter),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(mask, Some(vec![]));
    let directory = root.path().join("run");
    fs::create_dir(&directory).unwrap();
    fixtures::write_codex_transcript(
        &directory.join("transcript.jsonl"),
        &[json!({"type":"command_execution","command":"pytest"})],
    );
    let mut row = json!({"resolved":true});
    stage
        .after_task(
            &json!({}),
            &directory,
            &mut row,
            &mut ToolingContext {
                adapter: Some(&mut adapter),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(row["codex_untool_active"], true);
    assert_eq!(row["codex_untool_profile"], cu::PROFILE_VERSION);
}

#[test]
fn evidence_untool_drops_after_streak() {
    struct Stub;
    impl ToolingAdapter for Stub {
        fn protected_tools(&mut self) -> Result<ProtectedTools, String> {
            Ok(ProtectedTools::default())
        }
        fn tool_usage(&mut self, _: &Path) -> Result<BTreeSet<String>, String> {
            Ok(BTreeSet::new())
        }
        fn parse_catalog_with_source(
            &mut self,
            _: &Path,
            _: &str,
        ) -> Result<(Map<String, Value>, Value), String> {
            Ok((
                json!({"web_fetch":"fetch a url","ask_user":"ask the human"})
                    .as_object()
                    .unwrap()
                    .clone(),
                json!("fixture"),
            ))
        }
        fn problem_statements(&mut self) -> Result<Map<String, Value>, String> {
            Ok(Map::new())
        }
    }
    let root = TempDir::new("tooling-streak");
    let mut adapter = Stub;
    let mut stage = ToolingStage::new(
        ToolingConfig {
            evidence: 2,
            ..Default::default()
        },
        "claude",
        "agent",
        None,
        false,
        &mut ToolingContext {
            adapter: Some(&mut adapter),
            ..Default::default()
        },
    )
    .unwrap();
    let directory = root.path().join("run");
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("transcript.jsonl"), "").unwrap();
    tools(&mut stage, &mut adapter, "w-1", 0);
    for _ in 0..2 {
        stage
            .after_task(
                &json!({}),
                &directory,
                &mut json!({"resolved":true}),
                &mut ToolingContext {
                    adapter: Some(&mut adapter),
                    ..Default::default()
                },
            )
            .unwrap();
    }
    let excluded = tools(&mut stage, &mut adapter, "w-1", 3).unwrap();
    assert!(excluded.contains(&"web_fetch".into()) && excluded.contains(&"ask_user".into()));
}

#[test]
fn first_task_of_a_stream_is_never_masked_swebench() {
    let root = TempDir::new("tooling-first-swe");
    let mut adapter = Adapter::swebench(None, &Map::new());
    let mut s = stage(root.path(), &mut adapter, 3);
    assert_eq!(tools(&mut s, &mut adapter, "w-0", 0), Some(vec![]));
}

#[test]
fn first_task_of_a_stream_is_never_masked_webbench() {
    let root = TempDir::new("tooling-first-web");
    let mut adapter = Adapter::webbench("zustand".into(), None, &Map::new());
    let mut s = stage(root.path(), &mut adapter, 3);
    assert_eq!(tools(&mut s, &mut adapter, "task-1", 0), Some(vec![]));
}

#[test]
fn mask_appears_only_after_the_catalog_is_learned() {
    let root = TempDir::new("tooling-learned-mask");
    let mut adapter = Adapter::webbench("zustand".into(), None, &Map::new());
    let mut s = stage(root.path(), &mut adapter, 3);
    assert_eq!(tools(&mut s, &mut adapter, "task-1", 0), Some(vec![]));
    for i in 0..3 {
        finish(
            &mut s,
            &mut adapter,
            root.path(),
            i,
            &["Bash", "Edit", "Read", "Write"],
        );
    }
    let mask = tools(&mut s, &mut adapter, "task-4", 3).unwrap();
    assert!(!mask.is_empty());
    assert!(
        mask.iter()
            .all(|tool| !PROTECTED_TOOLS.contains(&tool.as_str()))
    );
    for tool in ["WebFetch", "Workflow", "CronCreate"] {
        assert!(mask.contains(&tool.into()));
    }
}

#[test]
fn static_inapplicable_prior_is_not_applied() {
    let root = TempDir::new("tooling-no-prior");
    let mut adapter = Adapter::webbench("zustand".into(), None, &Map::new());
    assert!(INAPPLICABLE_TOOLS.contains(&"Workflow") && INAPPLICABLE_TOOLS.contains(&"CronCreate"));
    // ToolingAdapter exposes observations; the retired static-prior methods are outside its contract.
    let mut s = stage(root.path(), &mut adapter, 3);
    assert_eq!(tools(&mut s, &mut adapter, "task-1", 0), Some(vec![]));
    finish(
        &mut s,
        &mut adapter,
        root.path(),
        0,
        &["Bash", "Edit", "Read", "Write"],
    );
    assert_eq!(tools(&mut s, &mut adapter, "task-2", 1), Some(vec![]));
}

#[test]
fn untooling_is_off_entirely_without_an_evidence_threshold() {
    let root = TempDir::new("tooling-off");
    let mut adapter = Adapter::webbench("zustand".into(), None, &Map::new());
    let mut s = stage(root.path(), &mut adapter, 0);
    finish(&mut s, &mut adapter, root.path(), 0, &["Bash"]);
    assert_eq!(tools(&mut s, &mut adapter, "task-2", 1), None);
}

#[test]
fn network_tools_are_blocked_by_default() {
    assert_eq!(
        settings::network_tool_block(&Map::new())
            .into_iter()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["WebSearch", "WebFetch"])
    );
}

#[test]
fn network_block_is_liftable_but_only_explicitly() {
    for (value, blocked) in [
        ("1", false),
        ("true", false),
        ("yes", false),
        ("0", true),
        ("", true),
        ("no", true),
    ] {
        assert_eq!(
            !settings::network_tool_block(
                json!({"TE_ALLOW_NETWORK_TOOLS":value}).as_object().unwrap()
            )
            .is_empty(),
            blocked,
            "{value}"
        );
    }
}

#[test]
fn network_tools_are_not_protected() {
    assert!(!PROTECTED_TOOLS.contains(&"WebSearch") && !PROTECTED_TOOLS.contains(&"WebFetch"));
}

#[test]
fn net_guard_blocks_answer_key_fetches() {
    for cmd in [
        "curl -sL https://raw.githubusercontent.com/bytedance/web-bench/main/projects/threejs/test/task-7.spec.js",
        "curl https://api.github.com/repos/bytedance/web-bench/git/trees/main?recursive=1",
        "wget https://github.com/bytedance/web-bench/archive/main.tar.gz",
    ] {
        let (blocked, reason) = net_guard::decide(&json!(cmd)).unwrap();
        assert!(blocked && !reason.is_empty());
    }
}

#[test]
fn net_guard_allows_every_legitimate_shape_seen_in_the_stored_runs() {
    for cmd in [
        "npm install",
        "npx vite build",
        "yarn add three",
        "curl -s http://localhost:3211/",
        "curl -s http://127.0.0.1:3005/index.html",
        "ls -la src/",
        "npm run test -- -a",
    ] {
        assert!(!net_guard::decide(&json!(cmd)).unwrap().0);
    }
}

#[test]
fn net_guard_reads_both_agents_payload_spellings() {
    let cmd = "curl https://github.com/x/y.patch";
    assert_eq!(
        net_guard::hook_command(&json!({"tool_name":"Bash","tool_input":{"command":cmd}})),
        cmd
    );
    assert_eq!(
        net_guard::hook_command(&json!({"toolName":"bash","toolArgs":{"command":cmd}})),
        cmd
    );
    for junk in [
        json!({}),
        json!({"toolArgs":{}}),
        json!({"tool_input":null}),
        json!([]),
        json!("nope"),
        Value::Null,
    ] {
        assert_eq!(net_guard::hook_command(&junk), "");
    }
}

#[test]
fn net_guard_blocks_end_to_end_on_a_copilot_shaped_payload() {
    let root = TempDir::new("tooling-guard-payload");
    for key in ["tool_input", "toolArgs"] {
        let mut payload = json!({});
        payload[key] = json!({"command":"curl https://github.com/x"});
        assert_eq!(
            guard(root.path(), &payload.to_string(), None).status.code(),
            Some(2)
        );
    }
}

#[test]
fn net_guard_is_liftable_by_the_same_switch_as_the_tool_block() {
    let root = TempDir::new("tooling-guard-allow");
    let payload = json!({"tool_input":{"command":"curl https://github.com/x"}}).to_string();
    assert_eq!(guard(root.path(), &payload, None).status.code(), Some(2));
    assert_eq!(
        guard(root.path(), &payload, Some("1")).status.code(),
        Some(0)
    );
}

#[test]
fn net_guard_fails_open_on_unreadable_input() {
    let root = TempDir::new("tooling-guard-broken");
    assert_eq!(guard(root.path(), "not json", None).status.code(), Some(0));
}

#[test]
fn the_swebench_runner_installs_and_reads_the_container_side_guard() {
    let root = TempDir::new("tooling-task-guard");
    let (row, calls) = harness::task(root.path());
    assert!(
        calls.contains(&"_install_net_guard".into()) && calls.contains(&"_net_guard_blocks".into())
    );
    assert_eq!(row["net_guard_blocks"], 3);
}

#[test]
fn the_swebench_guard_honours_the_same_escape_hatch() {
    assert_eq!(harness::disabled_guard(), json!(false));
}

#[test]
fn the_recorded_tool_mask_is_the_one_the_cli_was_given() {
    let root = TempDir::new("tooling-recorded-mask");
    for web in [false, true] {
        let cfg = harness::config(root.path(), web);
        assert_eq!(
            cfg["tools"]["excluded_applied"]
                .as_array()
                .unwrap()
                .iter()
                .map(|tool| tool.as_str().unwrap())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["Workflow", "WebSearch", "WebFetch"])
        );
        assert_eq!(
            cfg["tools"]["excluded_requested"],
            json!(["Bash", "Workflow"])
        );
    }
}

#[test]
fn codex_profile_demotes_unused_goals_after_one_exposure() {
    let root = TempDir::new("tooling-unused-goals");
    let mut p = profile(root.path(), "sympy/sympy");
    let one = p.begin_task(&json!("s-1"), &json!("")).unwrap();
    assert!(!has(&one["config_capabilities"], "goals"));
    p.observe(&empty_usage(), json!(true)).unwrap();
    let two = p.begin_task(&json!("s-2"), &json!("")).unwrap();
    assert_eq!(two["config_capabilities"], json!(["collaboration", "web"]));
    p.observe(&empty_usage(), json!(true)).unwrap();
    let three = p.begin_task(&json!("s-3"), &json!("")).unwrap();
    assert!(
        has(&three["config_capabilities"], "goals") && has(&three["logical_capabilities"], "goals")
    );
    assert!(!has(&three["relay_capabilities"], "goals"));
}

#[test]
fn codex_profile_completed_goal_use_protects() {
    let root = TempDir::new("tooling-completed-goal");
    let mut p = profile(root.path(), "sympy/sympy");
    p.begin_task(&json!("s-1"), &json!("")).unwrap();
    p.observe(
        &json!({"attempted":{"goals":1},"completed":{"goals":1},"failed":{}}),
        json!(true),
    )
    .unwrap();
    assert!(has(&p.state["ever_completed"], "goals"));
    for i in 2..6 {
        let selection = p.begin_task(&json!(format!("s-{i}")), &json!("")).unwrap();
        assert!(!has(&selection["config_capabilities"], "goals"));
        p.observe(&empty_usage(), json!(true)).unwrap();
    }
}

#[test]
fn inspect_usage_counts_goal_items() {
    let root = TempDir::new("tooling-goal-usage");
    let path = fixtures::write_codex_transcript(
        &root.path().join("t.jsonl"),
        &[
            json!({"type":"create_goal","id":"g1","status":"completed","name":"create_goal"}),
            json!({"type":"custom_tool_call","id":"c1","status":"completed","name":"exec","input":"await tools.update_goal({id: 1});"}),
            json!({"type":"command_execution","command":"ls","aggregated_output":"","exit_code":0}),
        ],
    );
    let usage = cu::inspect_usage(&path);
    assert_eq!(usage["attempted"]["goals"], 2);
    assert_eq!(usage["completed"]["goals"], 2);
}

#[test]
fn harness_emits_the_goals_toggle_and_the_cli_accepts_the_whole_mask() {
    let (_, tables) = codex_config::config_toggles(&json!(["goals"]), &Map::new()).unwrap();
    assert!(tables.contains(&("features".into(), "goals = false".into())));
    #[cfg(unix)]
    if let Some(binary) = Install::installed() {
        let root = TempDir::new("tooling-full-config");
        let txt=codex_config::config_toml(&json!({"routing":{"base_url":"http://127.0.0.1:9/v1","api_key":"k"},"model":"gpt-5.6-terra","effort":"high","capabilities":cu::CONFIG_CAPABILITIES}),&Map::new()).unwrap();
        assert!(txt.contains("goals = false"));
        let error = Install::new(root.path())
            .run(
                "validate_config",
                json!({"text":txt,"binary":binary,"home":root.path(),"timeout":20}),
            )
            .unwrap();
        assert!(error.as_str().unwrap().is_empty(), "{error}");
    }
}

#[test]
fn codex_profile_demotes_interactive_guidance_after_one_exposure() {
    let root = TempDir::new("tooling-interactive");
    let mut p = profile(root.path(), "sympy/sympy");
    let mut seen = Vec::new();
    for i in 0..4 {
        seen.push(p.begin_task(&json!(format!("s-{i}")), &json!("")).unwrap());
        p.observe(&empty_usage(), json!(true)).unwrap();
    }
    assert_eq!(seen[0]["config_capabilities"], json!([]));
    assert!(has(&seen[2]["config_capabilities"], "interactive_guidance"));
    assert!(!has(
        &seen.last().unwrap()["relay_capabilities"],
        "interactive_guidance"
    ));
}
