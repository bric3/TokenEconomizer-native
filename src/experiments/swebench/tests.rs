use super::*;
use std::{
    io::Write,
    time::{SystemTime, UNIX_EPOCH},
};

struct Host {
    calls: Vec<String>,
    segments: u64,
    resumes: u64,
    clocks: u64,
    start_error: bool,
}
impl Runtime for Host {
    fn call(&mut self, method: &str, args: Value) -> Result<Value> {
        self.calls.push(method.into());
        Ok(match method {
            "clock" => {
                let value = 100 + self.clocks;
                self.clocks += 1;
                json!(value)
            }
            "_container_name" => json!("fixture-container"),
            "_build_guard" => json!(true),
            "_start" if self.start_error => {
                return Err(Failure {
                    kind: "PoolUnavailable".into(),
                    message: "fixture start error".into(),
                    bases: vec!["builtins.RuntimeError".into()],
                    stdout: None,
                });
            }
            "_record_env_activation" => args[2].clone(),
            "_apply_scaffold" => json!([{}, null]),
            "_install_net_guard" | "_install_answer_key_hosts" => json!(true),
            "transcript_open" => {
                fs::File::create(args[0].as_str().unwrap())?;
                Value::Null
            }
            "_segment" => {
                let mut file = fs::OpenOptions::new()
                    .append(true)
                    .open(args[2].as_str().unwrap())?;
                writeln!(file, "segment {}", self.segments)?;
                self.segments += 1;
                json!({"usage":{"input_tokens":10,"output_tokens":5},"num_turns":2})
            }
            "_guard_continuation" => {
                if self.resumes < 2 {
                    self.resumes += 1;
                    json!("resume")
                } else {
                    Value::Null
                }
            }
            "guard_segment" => json!(self.resumes),
            "_merge_segments" => crate::experiments::usage::claude::merge_usage(&args[0], &args[1])
                .map_err(Failure::accounting)?,
            "_graded_diff" => json!("é😀\n"),
            "_grade" => json!([true,{"resolved":true}]),
            "_net_guard_blocks" => json!(3),
            "_row_from_segment" => row_from_segment("claude", &args[0])?,
            "_answer_key_attempts" => json!({}),
            "scaffold_info" => Value::Null,
            "_guard_row" => json!({"spiral_nudges":self.resumes}),
            "optimizer_context" | "_write_run_config" | "_start" | "_stop" | "log"
            | "transcript_close" => Value::Null,
            _ => {
                return Err(Failure::new(
                    "RuntimeError",
                    format!("unexpected operation: {method}"),
                ));
            }
        })
    }
}

#[test]
fn resumes_keep_the_transcript_and_cleanup_runs_once() {
    let root = std::env::temp_dir().join(format!(
        "te-task-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let task = Task {
        task: json!({"instance_id":"task","problem_statement":"fix"}),
        arm: json!("baseline"),
        injected: Value::Null,
        exclude_tools: json!([]),
        run_dir: root.clone(),
        kwargs: json!({}),
        metadata: json!({"agent":"claude","model":"claude-opus-4-8","effort":"high","cli_version":"fixture"}),
        environment: json!({}),
        process_cwd: std::env::current_dir().unwrap(),
    };
    let mut host = Host {
        calls: Vec::new(),
        segments: 0,
        resumes: 0,
        clocks: 0,
        start_error: false,
    };
    let row = run_task(&task, &mut host).unwrap();
    assert_eq!(row["resolved"], true);
    assert_eq!(row["input_tokens"], 30);
    assert_eq!(row["output_tokens"], 15);
    assert_eq!(row["spiral_nudges"], 2);
    assert_eq!(row["patch_bytes"], 3);
    assert_eq!(
        fs::read_to_string(root.join("transcript.jsonl")).unwrap(),
        "segment 0\nsegment 1\nsegment 2\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("model_patch.diff")).unwrap(),
        "é😀\n"
    );
    assert_eq!(host.calls.iter().filter(|s| *s == "_stop").count(), 1);
    let blocks = host
        .calls
        .iter()
        .position(|s| s == "_net_guard_blocks")
        .unwrap();
    assert_eq!(host.calls[blocks + 1], "_stop");
    fs::remove_dir_all(&root).unwrap();
    let mut host = Host {
        calls: Vec::new(),
        segments: 0,
        resumes: 0,
        clocks: 0,
        start_error: true,
    };
    let row = run_task(&task, &mut host).unwrap();
    assert_eq!(row["api_error_status"], "container_start_failed");
    assert_eq!(row["num_turns"], 0);
    assert!(!root.join("transcript.jsonl").exists());
    assert_eq!(host.calls.iter().filter(|s| *s == "_stop").count(), 1);
    assert!(!host.calls.iter().any(|s| s == "_apply_scaffold"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn grading_retains_timeout_output_and_recovers_score_errors() {
    struct Grader {
        executions: u32,
        timeout: bool,
        scored: bool,
    }
    impl Runtime for Grader {
        fn call(&mut self, method: &str, _: Value) -> Result<Value> {
            match method {
                "eval_script" => Ok(json!("echo result")),
                "_exec" => {
                    self.executions += 1;
                    if self.timeout && self.executions == 2 {
                        let mut error = Failure::new("EvalTimeout", "eval exceeded its cap");
                        error.bases = vec!["subprocess.TimeoutExpired".into()];
                        error.stdout = Some(Box::new(json!({"bytes":[195,169,255,10]})));
                        return Err(error);
                    }
                    Ok(json!({"returncode":0,"stdout":"test output\n","stderr":""}))
                }
                "score" => {
                    self.scored = true;
                    Err(Failure::new("ValueError", "invalid grader result"))
                }
                "log" => Ok(Value::Null),
                _ => panic!("unexpected grading operation: {method}"),
            }
        }
    }
    let root = std::env::temp_dir().join(format!(
        "te-grade-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let input = grading::Grade {
        name: "fixture-container".into(),
        instance_id: json!("fixture-task"),
        patch: "fix\n".into(),
        run_dir: root.clone(),
        eval_timeout: json!(90),
        process_cwd: std::env::current_dir().unwrap(),
    };
    for timeout in [true, false] {
        let mut host = Grader {
            executions: 0,
            timeout,
            scored: false,
        };
        let row = grading::grade(&input, &mut host).unwrap();
        assert_eq!(row[0], false);
        assert_eq!(host.scored, !timeout);
        assert_eq!(host.executions, 2);
        if timeout {
            assert_eq!(row[1], json!({"eval_timeout":90}));
            assert_eq!(
                fs::read_to_string(root.join("eval.log")).unwrap(),
                "é\u{fffd}\n"
            );
        } else {
            assert_eq!(row[1], json!({"grading_error":"invalid grader result"}));
            assert_eq!(
                fs::read_to_string(root.join("eval.log")).unwrap(),
                "test output\n"
            );
        }
    }
    fs::remove_dir_all(root).unwrap();
}
