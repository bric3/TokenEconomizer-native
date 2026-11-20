// Converted from reference/upstream/experiments/tests/test_spiral.py.
#[path = "support/codex_argv.rs"]
mod invocation_fixture;
mod support;

use invocation_fixture::codex_argv;
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::Path};
use support::TempDir;
use tokeneconomizer::{
    config::SpiralConfig,
    experiments::{
        arms::{self, ARM_ORDER},
        codex_spiral::{self as cs, Policy, ResumeGuard},
        nudge_guard::NudgeGuard,
        spiral_stage::SpiralStage,
        swebench::{self, controls},
    },
    lifecycle,
    plugin::process,
    trace,
};

const PATCH: &str = "/bin/bash -lc \"apply_patch <<'PATCH'\n*** Begin Patch\n*** Update File: django/core/validators.py\n@@\n-old\n+new\n*** End Patch\nPATCH\n\ngit diff --check\"";

fn line(item: Value) -> String {
    json!({"type":"item.completed","item":item}).to_string()
}

fn command(command: &str, code: Value, output: &str) -> String {
    line(
        json!({"type":"command_execution","command":command,"aggregated_output":output,"exit_code":code}),
    )
}

fn thread(id: &str) -> String {
    json!({"type":"thread.started","thread_id":id}).to_string()
}

fn replay(lines: &[String], policy: &Policy) -> (Option<Value>, cs::State) {
    cs::replay_events(cs::iter_events(lines, true), policy).unwrap()
}

fn stage(cfg: SpiralConfig, agent: &str) -> SpiralStage {
    SpiralStage::new(cfg, Some(80.0), "", Some(agent.into()))
}

#[test]
fn shipped_codex_policy_flags() {
    let policy = Policy::shipped().at_threshold(30.0);
    assert!(policy.post_green_finalize_enabled);
    assert!(policy.post_green_finalize_require_cleared_failures);
    assert!(!policy.wide_failure_enabled);
    assert!(!policy.recovery_enabled);
    assert_eq!(policy.min_tool_turns, 30);
}

#[test]
fn green_is_trustworthy() {
    assert!(cs::green_is_trustworthy("python -m pytest tests/test_x.py"));
    assert!(!cs::green_is_trustworthy("pytest -k only_this"));
    assert!(!cs::green_is_trustworthy("pytest --last-failed"));
}

#[test]
fn generic_guard_run_kwargs() {
    let stage = stage(
        SpiralConfig {
            guard: true,
            judge: false,
            nudge: true,
            ..Default::default()
        },
        "claude",
    );
    let kwargs = stage.run_kwargs().unwrap();
    assert_eq!(kwargs["spiral_guard"], true);
    assert!(kwargs.get("spiral_thr").is_some());
    assert!(kwargs.get("abort_envelope").is_some());
}

#[test]
fn codex_spiral_run_kwargs_policy() {
    let stage = stage(
        SpiralConfig {
            codex_spiral: true,
            ..Default::default()
        },
        "codex",
    );
    assert!(
        stage
            .run_kwargs()
            .unwrap()
            .get("codex_spiral_policy")
            .is_some()
    );
}

#[test]
fn generic_guard_after_task_advances_envelope() {
    let mut stage = stage(
        SpiralConfig {
            guard: true,
            nudge: true,
            ..Default::default()
        },
        "claude",
    );
    let mut stream_row = json!({"resolved":true,"num_turns":12});
    stage
        .after_task(
            &json!({"resolved":true,"num_turns":12,"aborted":false}),
            Path::new("/tmp"),
            &mut stream_row,
            None,
        )
        .unwrap();
    assert_eq!(stream_row["aborted"], false);
    assert!(stream_row.get("spiral_nudges").is_some());
}

#[test]
fn codex_spiral_after_task_telemetry() {
    let root = TempDir::new("spiral-telemetry");
    let mut stage = stage(
        SpiralConfig {
            codex_spiral: true,
            ..Default::default()
        },
        "codex",
    );
    let mut stream_row = json!({"resolved":true,"num_turns":30});
    stage
        .after_task(
            &json!({"resolved":true,"num_turns":30,"codex_spiral_nudges":0}),
            root.path(),
            &mut stream_row,
            None,
        )
        .unwrap();
    assert_eq!(stream_row["codex_spiral_active"], true);
    assert_eq!(stream_row["codex_spiral_profile"], cs::PROFILE_VERSION);
}

#[test]
fn shell_applied_patch_is_an_edit_event_with_its_paths() {
    let events = cs::parse_events(&command(
        PATCH,
        json!(0),
        "Success. Updated the following files:\nM django/core/validators.py",
    ))
    .unwrap();
    assert_eq!(
        events
            .iter()
            .map(|event| event.kind.as_str())
            .collect::<Vec<_>>(),
        ["edit", "command"]
    );
    assert_eq!(events[0].paths, ["django/core/validators.py"]);
    let failed = cs::parse_events(&command(
        PATCH,
        json!(1),
        "apply_patch: error: Failed to find context",
    ))
    .unwrap();
    assert_eq!(
        failed
            .iter()
            .map(|event| event.kind.as_str())
            .collect::<Vec<_>>(),
        ["command"]
    );
    let plain = cs::parse_events(&command("ls", json!(0), "")).unwrap();
    assert_eq!(
        plain
            .iter()
            .map(|event| event.kind.as_str())
            .collect::<Vec<_>>(),
        ["command"]
    );
    assert_eq!(
        cs::parse_events(&command("pytest -q", json!("0"), "1 passed")).unwrap()[0].exit_code,
        0
    );
}

#[test]
fn shipped_policy_can_fire_on_a_shell_patched_trajectory() {
    let mut lines = vec![thread("t-1")];
    lines.extend((0..4).map(|i| command(&format!("rg pattern{i} src"), json!(0), "x")));
    lines.push(command(PATCH, json!(0), "Success. Updated"));
    lines.push(command(
        "python -m pytest tests/test_validators.py -q",
        json!(0),
        "3 passed",
    ));
    lines.extend((0..8).map(|i| {
        command(
            &format!("git log -p -{} -- django", i + 1),
            json!(0),
            "history",
        )
    }));
    let policy = Policy::shipped().at_threshold(6.0);
    let (decision, state) = replay(&lines, &policy);
    assert!(state.last_edit_turn > 0 && state.first_trustworthy_green_turn > state.last_edit_turn);
    assert_eq!(
        decision.unwrap()["reason"],
        "validated-patch-should-finalize"
    );
    let mut guard = ResumeGuard::new(policy, false).unwrap();
    let fired: Vec<_> = lines
        .iter()
        .map(|line| guard.monitor(line, &mut Vec::new()).unwrap())
        .collect();
    assert!(fired.iter().any(|fired| *fired));
    assert_eq!(guard.continuation().unwrap()["session_id"], "t-1");
}

#[test]
fn trajectory_seam_surfaces_shell_patches_as_apply_patch_calls() {
    let root = TempDir::new("spiral-patch");
    let path = root.path().join("t.jsonl");
    fs::write(
        &path,
        format!(
            "{}\n{}\n",
            command(PATCH, json!(0), "Success. Updated"),
            command("pytest -q", json!(0), "ok")
        ),
    )
    .unwrap();
    let calls = trace::calls::load_trajectory(&path);
    assert_eq!(
        calls
            .iter()
            .map(|call| call.0.as_str().unwrap())
            .collect::<Vec<_>>(),
        ["apply_patch", "shell", "shell"]
    );
    assert_eq!(calls[0].1["paths"], json!(["django/core/validators.py"]));
    assert_eq!(calls[0].1["via"], "shell");
    assert_eq!(
        lifecycle::edited_files(
            calls.iter().map(|call| (call.0.as_str().unwrap(), &call.1)),
            true
        ),
        BTreeSet::from(["validators.py".into()])
    );
}

#[test]
fn green_reverification_loop_fires_only_after_repeated_greens_without_an_edit() {
    let mut lines = vec![thread("t-2")];
    lines.extend((0..6).map(|i| command(&format!("rg x{i}"), json!(0), "")));
    lines.push(command(PATCH, json!(0), "Success. Updated"));
    for _ in 0..3 {
        lines.push(command("pytest tests/test_a.py -q", json!(0), "1 passed"));
    }
    let mut policy = Policy::defensive().at_threshold(8.0);
    let (decision, _) = replay(&lines, &policy);
    assert!(decision.is_none_or(|decision| decision["reason"] != "green-reverification-loop"));
    lines.push(command("pytest tests/test_a.py -q", json!(0), "1 passed"));
    assert_eq!(replay(&lines, &policy).1.green_reruns_since_edit, 3);
    policy.post_green_finalize_enabled = false;
    assert_eq!(
        replay(&lines, &policy).0.unwrap()["reason"],
        "green-reverification-loop"
    );
    lines.push(command(PATCH, json!(0), "Success. Updated"));
    assert_eq!(
        replay(&lines, &Policy::defensive().at_threshold(8.0))
            .1
            .green_reruns_since_edit,
        0
    );
    let policy = Policy {
        post_green_finalize_enabled: false,
        ..Policy::shipped().at_threshold(8.0)
    };
    assert!(replay(&lines[..lines.len() - 1], &policy).0.is_none());
}

#[test]
fn runaway_guard_fires_at_twice_the_envelope_even_with_no_edit() {
    let mut lines = vec![thread("t-3")];
    lines.extend((0..40).map(|i| command(&format!("rg needle{i} src"), json!(0), "")));
    let decision = replay(&lines, &Policy::defensive().at_threshold(16.0))
        .0
        .unwrap();
    assert_eq!(decision["reason"], "beyond-twice-the-resolved-envelope");
    assert_eq!(decision["turn"], 32);
    assert!(
        replay(&lines, &Policy::shipped().at_threshold(16.0))
            .0
            .is_none()
    );
    assert!(
        replay(&lines[..25], &Policy::defensive().at_threshold(8.0))
            .0
            .is_none()
    );
}

#[test]
fn defensive_guard_spends_its_budget_on_distinct_reasons_only() {
    let mut guard = ResumeGuard::new(Policy::defensive().at_threshold(16.0), false).unwrap();
    let mut lines = vec![thread("t-4")];
    lines.extend((0..40).map(|i| command(&format!("rg needle{i} src"), json!(0), "")));
    let fired = lines
        .iter()
        .filter(|line| guard.monitor(line, &mut Vec::new()).unwrap())
        .count();
    assert_eq!(fired, 1);
    assert!(
        guard.continuation().unwrap()["prompt"]
            .as_str()
            .unwrap()
            .starts_with("Automated process checkpoint")
    );
    for i in 0..5 {
        assert!(
            !guard
                .monitor(
                    &command(&format!("rg more{i}"), json!(0), ""),
                    &mut Vec::new()
                )
                .unwrap()
        );
    }
    assert_eq!(guard.policy.nudge_budget, 2);
    assert_eq!(guard.nudges_done, 1);
    assert!(!cs::nudge_text("green-reverification-loop").is_empty());
    assert!(!cs::nudge_text("beyond-twice-the-resolved-envelope").is_empty());
}

#[test]
fn defensive_profile_is_selectable_and_unpaid() {
    let config = arms::get("codex-spiral-defensive").unwrap().config;
    assert!(config.spiral.codex_spiral);
    assert_eq!(config.spiral.codex_spiral_profile, "defensive");
    assert!(!config.needs_optimizer());
    assert!(ARM_ORDER.contains(&"codex-spiral-defensive"));
    assert_eq!(
        serde_json::to_value(arms::get("codex-spiral").unwrap().config.spiral).unwrap(),
        serde_json::to_value(&config.spiral).unwrap()
    );
    assert_eq!(
        arms::get("codex-spiral-postgreen")
            .unwrap()
            .config
            .spiral
            .codex_spiral_profile,
        "post-green"
    );
    assert!(
        SpiralConfig {
            codex_spiral: true,
            codex_spiral_profile: "bogus".into(),
            ..Default::default()
        }
        .validate()
        .is_err()
    );
    let stage = stage(config.spiral, "codex");
    let policy: Policy =
        serde_json::from_value(stage.run_kwargs().unwrap()["codex_spiral_policy"].clone()).unwrap();
    assert!(policy.reverify_loop_enabled && policy.runaway_enabled);
    assert_eq!(policy.nudge_budget, 2);
    assert!(
        policy.post_green_finalize_enabled && policy.post_green_finalize_require_cleared_failures
    );
    assert!(!policy.effort_rescue_enabled);
    assert!(policy.resume_effort.is_empty());
}

#[test]
fn both_guards_expose_everything_the_shared_task_loop_reads() {
    let generic = NudgeGuard::new(None, None, 1, false, None);
    let codex = ResumeGuard::new(Policy::shipped().at_threshold(10.0), false).unwrap();
    // The SDK's Codex segment property reads the native issued-nudge count.
    assert_eq!(generic.segment, 0);
    assert_eq!(codex.nudges_done, 0);
    let mut guard = ResumeGuard::new(Policy::defensive().at_threshold(16.0), false).unwrap();
    guard.monitor(&thread("t-5"), &mut Vec::new()).unwrap();
    for i in 0..40 {
        if guard
            .monitor(
                &command(&format!("rg needle{i} src"), json!(0), ""),
                &mut Vec::new(),
            )
            .unwrap()
        {
            break;
        }
    }
    assert_eq!(guard.nudges_done, 0);
    assert!(guard.continuation().is_some());
    assert_eq!(guard.nudges_done, 1);
    assert_eq!(guard.telemetry()["nudges"], guard.nudges_done);
}

#[test]
fn the_codex_envelope_is_advanced_in_the_unit_its_trigger_compares() {
    let root = TempDir::new("spiral-envelope");
    let mut stage = stage(
        SpiralConfig {
            codex_spiral: true,
            codex_spiral_profile: "defensive".into(),
            ..Default::default()
        },
        "codex",
    );
    let mut stream_row = json!({});
    stage
        .after_task(
            &json!({"resolved":true,"num_turns":29,"codex_spiral_tool_turns":20}),
            root.path(),
            &mut stream_row,
            None,
        )
        .unwrap();
    assert_eq!(
        stage.codex_tracker.as_ref().unwrap().resolved_tool_turns,
        [20]
    );
    assert_eq!(stream_row["codex_spiral_envelope_unit"], "tool_events");
    assert_eq!(stream_row["codex_spiral_tool_turns"], 20);
    stage
        .after_task(
            &json!({"resolved":true,"num_turns":31}),
            root.path(),
            &mut stream_row,
            None,
        )
        .unwrap();
    assert_eq!(
        stage.codex_tracker.as_ref().unwrap().resolved_tool_turns,
        [20, 31]
    );
    assert_eq!(stream_row["codex_spiral_envelope_unit"], "steps_fallback");

    struct Telemetry {
        guard: ResumeGuard,
        row: Value,
    }
    impl swebench::Runtime for Telemetry {
        fn call(&mut self, method: &str, args: Value) -> swebench::Result<Value> {
            match method {
                "control_telemetry" => {}
                "control_row_value" => self.row[args[0].as_str().unwrap()] = args[1].clone(),
                "control_snapshot" => return Ok(json!(true)),
                "control_snapshot_get" => {
                    self.row[args[0].as_str().unwrap()] =
                        self.guard.telemetry()["snapshot"][args[1].as_str().unwrap()].clone()
                }
                "control_row_items" => {
                    for pair in args.as_array().unwrap() {
                        self.row[pair[0].as_str().unwrap()] =
                            self.guard.telemetry()[pair[1].as_str().unwrap()].clone();
                    }
                }
                _ => panic!("unexpected callback: {method} {args}"),
            }
            Ok(Value::Null)
        }
    }
    let mut telemetry = Telemetry {
        guard: ResumeGuard::new(Policy::defensive(), false).unwrap(),
        row: json!({}),
    };
    telemetry
        .guard
        .monitor(&command("rg pattern src", json!(0), ""), &mut Vec::new())
        .unwrap();
    controls::row("codex", &mut telemetry).unwrap();
    assert_eq!(telemetry.row["codex_spiral_tool_turns"], 1);
}

#[test]
fn the_resume_invocation_is_one_the_installed_cli_accepts() {
    let session = "01a09187-c54c-79a1-a40a-fe8ba406a935";
    let argv = codex_argv(
        "resume",
        json!({"session_id":session,"hook":true,"effort":"high"}),
    );
    let argv = argv.as_array().unwrap();
    assert_eq!(argv[1..4], [json!("exec"), json!("resume"), json!(session)]);
    assert_eq!(argv.last().unwrap(), "-");
    assert!(!argv.contains(&json!("-C")));
    assert!(!argv.contains(&json!("--color")));
    let binary = std::env::var_os("TE_CODEX_BIN")
        .map(Into::into)
        .or_else(|| process::which("codex"));
    let Some(binary) = binary else {
        eprintln!("codex-cli unavailable; checked argv only");
        return;
    };
    let args = vec![
        binary.to_string_lossy().into_owned(),
        "exec".into(),
        "resume".into(),
        "--help".into(),
    ];
    let output = process::run(&args, &std::env::current_dir().unwrap(), 120.0, false).unwrap();
    let text = format!("{}{}", output.stdout, output.stderr);
    assert!(text.contains("[SESSION_ID]"));
    for flag in argv
        .iter()
        .filter_map(Value::as_str)
        .filter(|arg| arg.starts_with("--"))
    {
        assert!(
            text.contains(flag),
            "codex exec resume does not accept {flag}"
        );
    }
}

#[test]
fn the_codex_verification_detector_sees_through_rtks_command_prefix() {
    for bare in [
        "python -m pytest tests/ -q",
        "pytest -q",
        "bin/test sympy/x.py",
        "K=1 pytest -q",
        "pip install foo",
        "git status",
    ] {
        for prefix in ["rtk ", "rtk proxy ", "rtk K=2 "] {
            assert_eq!(
                cs::is_verification_command(&format!("{prefix}{bare}")),
                cs::is_verification_command(bare)
            );
        }
    }
    assert!(cs::is_verification_command("cd x && rtk pytest -q"));
    assert!(cs::is_verification_command("rtk pytest -q"));
    assert!(!cs::is_verification_command("rtk git status"));
}
