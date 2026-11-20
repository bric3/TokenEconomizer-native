// Converted from reference/upstream/experiments/tests/test_grading_integrity.py.
mod support;

use serde_json::{Map, Value, json};
use std::{fs, path::Path};
use support::TempDir;
use tokeneconomizer::{
    experiments::{
        aggregate, driver,
        swebench::{self, Failure, Task, settings},
    },
    ledger::{self, OVERHEAD_KEYS},
};

fn name(agent: &str, instance: &str, arm: &str, directory: &str) -> String {
    swebench::container_name(
        instance,
        &json!(arm),
        &json!(agent),
        &json!("claude-opus-4.8"),
        Path::new(directory),
        Path::new("/"),
    )
}

#[test]
fn container_name_differs_by_agent() {
    assert_ne!(
        name(
            "claude",
            "django__django-10097",
            "baseline",
            "/r/claude/baseline/django_s0/x"
        ),
        name(
            "copilot",
            "django__django-10097",
            "baseline",
            "/r/copilot/baseline/django_s0/x"
        ),
    );
}

#[test]
fn container_name_differs_by_run_dir() {
    assert_ne!(
        name(
            "claude",
            "sympy__sympy-11618",
            "trimech",
            "/r/t/trimech/sympy_s0/20260101T000000Z__000__x"
        ),
        name(
            "claude",
            "sympy__sympy-11618",
            "trimech",
            "/r/t/trimech/sympy_s1/20260101T000000Z__000__x"
        ),
    );
}

#[test]
fn container_name_is_docker_legal_and_readable() {
    let name = name(
        "claude",
        "scikit-learn__scikit-learn-25102",
        "trimech-repomap",
        "/r/t/a/b/c",
    );
    assert!(name.len() <= 63);
    assert!(name.chars().next().unwrap().is_alphanumeric());
    assert!(
        name.chars()
            .all(|c| c.is_alphanumeric() || "_.-".contains(c))
    );
    let stem = "te-scikit-learn-scikit-learn-25102-trimech-repomap";
    assert!(name.starts_with(&stem[..stem.len().min(50)]));
}

#[test]
fn container_name_is_stable_for_one_task_run() {
    let make = || {
        name(
            "claude",
            "django__django-10097",
            "baseline",
            "/r/t/baseline/django_s0/run",
        )
    };
    assert_eq!(make(), make());
}

#[test]
fn eval_exec_failure_is_flagged_not_graded() {
    let (ungraded, reason) = aggregate::not_graded(
        &json!({"resolved":false,"eval_report":{"eval_exec_failed":true,"eval_exec_rc":1,"eval_exec_error":"No such container"}}),
    );
    assert!(ungraded);
    assert!(reason.as_str().unwrap().contains("No such container"));
}

#[test]
fn grading_error_is_flagged_not_graded() {
    let (ungraded, reason) = aggregate::not_graded(
        &json!({"resolved":false,"eval_report":{"grading_error":"KeyError"}}),
    );
    assert!(ungraded);
    assert!(reason.as_str().unwrap().contains("KeyError"));
}

#[test]
fn a_real_miss_is_not_flagged() {
    assert!(!aggregate::not_graded(&json!({"resolved":false,"eval_report":{"patch_exists":true,"patch_successfully_applied":true,"resolved":false}})).0);
}

#[test]
fn a_resolve_is_not_flagged() {
    assert!(!aggregate::not_graded(&json!({"resolved":true,"eval_report":{"resolved":true}})).0);
}

#[test]
fn not_graded_rows_leave_the_resolve_denominator() {
    let cells = aggregate::summarize(&[
        json!({"arm":"trimech","repo":"django","seed":0,"resolved":true,"is_error":false,"num_turns":5,"cost_effective_tokens":100.0,"not_graded":false}),
        json!({"arm":"trimech","repo":"django","seed":0,"resolved":false,"is_error":false,"num_turns":4,"cost_effective_tokens":100.0,"not_graded":true}),
    ]).unwrap();
    let cell = &cells[&["trimech".into(), "django".into(), "0".into()]];
    assert_eq!(cell["n"], 2);
    assert_eq!(cell["graded"], 1);
    assert_eq!(cell["resolved"], 1);
    assert_eq!(cell["ungraded"], 1);
    assert_eq!(
        cell["resolved"].as_f64().unwrap() / cell["graded"].as_f64().unwrap(),
        1.0
    );
}

fn overhead_row(value: i32) -> Value {
    Value::Object(
        OVERHEAD_KEYS
            .iter()
            .map(|key| ((*key).into(), json!(value)))
            .collect(),
    )
}

#[test]
fn every_overhead_key_is_summed_by_the_ledger() {
    assert_eq!(
        ledger::overhead(&overhead_row(100)),
        json!(100 * OVERHEAD_KEYS.len())
    );
}

fn lift_overhead(row: Value, stream_row: Value) -> Value {
    struct Rows {
        slots: Map<String, Value>,
        keys: std::vec::IntoIter<Value>,
    }
    impl swebench::Runtime for Rows {
        fn call(&mut self, method: &str, args: Value) -> swebench::Result<Value> {
            let slot = |index: usize| args[index].as_str().unwrap();
            match method {
                "runner_function" => {
                    assert_eq!(args, json!(["ledger.OVERHEAD_KEYS"]));
                    self.slots.insert("function".into(), json!(OVERHEAD_KEYS));
                }
                "workspace_iter" => {
                    self.keys = self.slots[slot(0)].as_array().unwrap().clone().into_iter();
                }
                "workspace_next" => {
                    if let Some(key) = self.keys.next() {
                        self.slots.insert(slot(1).into(), key);
                        return Ok(json!(true));
                    }
                    return Ok(json!(false));
                }
                "provider_contains" => {
                    return Ok(json!(
                        self.slots[slot(1)]
                            .as_object()
                            .unwrap()
                            .contains_key(self.slots[slot(0)].as_str().unwrap())
                    ));
                }
                "stdout_member" => {
                    assert_eq!(args, json!(["srow", "get", "function"]));
                    self.slots.insert("function".into(), json!("srow.get"));
                }
                "runner_invoke" => {
                    assert_eq!(self.slots["function"], "srow.get");
                    let key = self.slots[args[0][0].as_str().unwrap()].as_str().unwrap();
                    self.slots
                        .insert("called".into(), self.slots["srow"][key].clone());
                }
                "provider_is_none" => return Ok(json!(self.slots[slot(0)].is_null())),
                "scaffold_item" => {
                    let value = self.slots[slot(0)][self.slots[slot(1)].as_str().unwrap()].clone();
                    self.slots.insert(slot(2).into(), value);
                }
                "workspace_put" => {
                    let key = self.slots[slot(1)].as_str().unwrap().to_owned();
                    let value = self.slots[slot(2)].clone();
                    self.slots[slot(0)][key] = value;
                }
                _ => panic!("unexpected callback: {method} {args}"),
            }
            Ok(Value::Null)
        }
    }
    let mut runtime = Rows {
        slots: json!({"row":row,"srow":stream_row})
            .as_object()
            .unwrap()
            .clone(),
        keys: Vec::new().into_iter(),
    };
    let result = driver::dispatch("lift_overhead", &mut runtime).unwrap();
    runtime
        .slots
        .remove(result["provider_ref"].as_str().unwrap())
        .unwrap()
}

#[test]
fn stream_only_overhead_is_copied_into_the_row() {
    let row = lift_overhead(
        json!({"tokens_total":1000}),
        json!({"untool_judge_tokens":5171,"resolved":true}),
    );
    assert_eq!(row["untool_judge_tokens"], 5171);
    assert_eq!(ledger::overhead(&row), 5171);
}

#[test]
fn the_copy_never_clobbers_a_value_the_runner_already_set() {
    let row = lift_overhead(
        json!({"spiral_judge_tokens":106916}),
        json!({"spiral_judge_tokens":0}),
    );
    assert_eq!(row["spiral_judge_tokens"], 106916);
}

#[test]
fn lift_overhead_covers_every_declared_overhead_key() {
    let row = lift_overhead(json!({}), overhead_row(7));
    assert_eq!(ledger::overhead(&row), json!(7 * OVERHEAD_KEYS.len()));
}

#[test]
fn lift_overhead_ignores_non_overhead_stream_fields() {
    let row = lift_overhead(
        json!({}),
        json!({"untool_judge_dropped":["WebFetch"],"resolved":true}),
    );
    assert!(row.get("untool_judge_dropped").is_none());
    assert!(row.get("resolved").is_none());
}

#[test]
fn a_grading_stall_flags_the_row_instead_of_ending_the_stream() {
    struct Host<'a> {
        fail: Option<(&'a str, &'a str)>,
        stops: usize,
        logs: Vec<String>,
    }
    impl swebench::Runtime for Host<'_> {
        fn call(&mut self, method: &str, args: Value) -> swebench::Result<Value> {
            if let Some((operation, kind)) = self.fail
                && operation == method
            {
                return Err(Failure::new(kind, "fixture grading stall"));
            }
            Ok(match method {
                "clock" => json!(100),
                "_container_name" => json!("te-fixture"),
                "_build_guard" => json!(false),
                "_record_env_activation" => args[2].clone(),
                "_apply_scaffold" => json!([{}, null]),
                "_install_net_guard" | "_install_answer_key_hosts" => json!(true),
                "transcript_open" => {
                    fs::File::create(args[0].as_str().unwrap())?;
                    Value::Null
                }
                "_segment" => {
                    json!({"usage":{"input_tokens":100,"output_tokens":50},"num_turns":2})
                }
                "_graded_diff" => json!("diff --git a/a b/a\n"),
                "_grade" => json!([true, {"resolved":true}]),
                "_net_guard_blocks" => json!(0),
                "_stop" => {
                    self.stops += 1;
                    Value::Null
                }
                "_row_from_segment" => swebench::row_from_segment("claude", &args[0])?,
                "_answer_key_attempts" | "scaffold_info" => Value::Null,
                "log" => {
                    self.logs.push(args[0].as_str().unwrap().into());
                    Value::Null
                }
                "optimizer_context" | "_write_run_config" | "_start" | "transcript_close" => {
                    Value::Null
                }
                _ => panic!("unexpected callback: {method} {args}"),
            })
        }
    }
    for operation in ["_graded_diff", "_grade"] {
        for kind in ["TimeoutExpired", "RuntimeError"] {
            let root = TempDir::new("grading-stall");
            let mut task = Task {
                task: json!({"instance_id":"django__django-10097","problem_statement":"fix"}),
                arm: json!("baseline"),
                injected: Value::Null,
                exclude_tools: json!([]),
                run_dir: root.path().join("failed"),
                kwargs: json!({}),
                metadata: json!({"agent":"claude","model":"claude-opus-4-8","effort":"high","cli_version":"fixture"}),
                environment: json!({}),
                process_cwd: root.path().into(),
            };
            let mut host = Host {
                fail: Some((operation, kind)),
                stops: 0,
                logs: Vec::new(),
            };
            let row = swebench::run_task(&task, &mut host).unwrap();
            // Both implementations overwrite these flags when merging segment fields.
            assert_eq!(row["is_error"], false);
            assert!(row["api_error_status"].is_null());
            assert!(row["grading_error"].as_str().unwrap().contains(kind));
            assert!(aggregate::not_graded(&row).0);
            assert_eq!(row["input_tokens"], 100);
            assert_eq!(row["output_tokens"], 50);
            assert!(host.logs.iter().any(|line| line.contains("GRADING FAILED")));
            assert_eq!(host.stops, 1);

            host.fail = None;
            task.run_dir = root.path().join("next");
            let next = swebench::run_task(&task, &mut host).unwrap();
            assert_eq!(next["resolved"], true);
            assert!(!aggregate::not_graded(&next).0);
            assert_eq!(host.stops, 2);
        }
    }
}

#[test]
fn choose_project_env_prefers_testbed_then_the_only_env() {
    let root = "/opt/miniconda3/envs";
    assert_eq!(
        settings::choose_project_env(&json!([
            format!("{root}/other/bin"),
            format!("{root}/testbed/bin")
        ]))
        .unwrap(),
        format!("{root}/testbed/bin")
    );
    assert_eq!(
        settings::choose_project_env(&json!([format!("{root}/py39/bin")])).unwrap(),
        format!("{root}/py39/bin")
    );
    assert!(
        settings::choose_project_env(&json!([format!("{root}/a/bin"), format!("{root}/b/bin")]))
            .unwrap()
            .is_null()
    );
    assert!(settings::choose_project_env(&json!([])).unwrap().is_null());
}

#[test]
fn activation_env_prepends_and_names_the_env() {
    let env = settings::activation_env(
        &json!("/opt/miniconda3/envs/testbed/bin"),
        &json!("/opt/miniconda3/bin:/usr/bin"),
    )
    .unwrap();
    assert_eq!(
        env["PATH"],
        "/opt/miniconda3/envs/testbed/bin:/opt/miniconda3/bin:/usr/bin"
    );
    assert_eq!(env["CONDA_PREFIX"], "/opt/miniconda3/envs/testbed");
    assert_eq!(env["CONDA_DEFAULT_ENV"], "testbed");
    assert_eq!(
        settings::activation_env(&json!("/x/envs/e/bin"), &json!("")).unwrap()["PATH"],
        "/x/envs/e/bin"
    );
}
