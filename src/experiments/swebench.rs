use crate::{
    microskills::observe::posix_normpath,
    pricing,
    value::{int_value, round_decimal, scalar_text, truthy},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha1::{Digest, Sha1};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use crate::value::python_repr;

pub mod codex_config;
pub mod codex_install;
pub mod codex_trace;
pub mod containers;
pub mod controls;
pub mod copilot_install;
pub mod grading;
pub mod guard_install;
pub mod official;
pub mod optimizer;
pub mod optimizer_context;
pub mod optimizer_log;
pub mod preflight;
pub mod providers;
pub mod settings;
pub mod setup;
#[cfg(test)]
mod tests;
pub mod wrappers;

pub const TASK_TEMPLATE: &str = include_str!("swebench/task-prompt.txt");

#[derive(Debug, Serialize, Deserialize)]
pub struct Failure {
    pub kind: String,
    pub message: String,
    #[serde(default)]
    pub bases: Vec<String>,
    #[serde(default)]
    pub stdout: Option<Box<Value>>,
    #[serde(default)]
    pub details: Option<Box<Value>>,
}
impl Failure {
    pub fn for_path(error: std::io::Error, path: &Path) -> Self {
        #[cfg(unix)]
        if let Some(errno) = error.raw_os_error() {
            let display = error.to_string();
            let suffix = format!(" (os error {errno})");
            let details = json!({"errno":errno,"strerror":display.strip_suffix(&suffix).unwrap_or(&display),"filename":path.to_string_lossy()});
            let mut failure = Self::from(error);
            failure.message = format!(
                "[Errno {}] {}: {}",
                details["errno"],
                details["strerror"].as_str().unwrap(),
                python_repr(&details["filename"])
            );
            failure.details = Some(Box::new(details));
            return failure;
        }
        let _ = path;
        Self::from(error)
    }
    fn accounting(error: String) -> Self {
        let (kind, message) = error
            .split_once(": ")
            .unwrap_or(("ValueError", error.as_str()));
        Self::new(kind, message)
    }
    pub fn new(kind: &str, message: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            message: message.into(),
            bases: Vec::new(),
            stdout: None,
            details: None,
        }
    }
    fn recoverable(&self) -> bool {
        self.is_a("builtins.RuntimeError") || self.is_a("subprocess.TimeoutExpired")
    }
    fn is_a(&self, name: &str) -> bool {
        if !self.bases.is_empty() {
            return self.bases.iter().any(|base| base == name);
        }
        if name == "builtins.Exception" {
            return !matches!(
                self.kind.as_str(),
                "BaseException" | "KeyboardInterrupt" | "SystemExit" | "GeneratorExit"
            );
        }
        name.rsplit('.').next() == Some(self.kind.as_str())
    }
}
impl From<std::io::Error> for Failure {
    fn from(error: std::io::Error) -> Self {
        let kind = match error.kind() {
            std::io::ErrorKind::NotFound => "FileNotFoundError",
            std::io::ErrorKind::PermissionDenied => "PermissionError",
            std::io::ErrorKind::IsADirectory => "IsADirectoryError",
            std::io::ErrorKind::NotADirectory => "NotADirectoryError",
            _ => "OSError",
        };
        let mut failure = Self::new(kind, error.to_string());
        failure.bases = vec![
            format!("builtins.{kind}"),
            "builtins.OSError".into(),
            "builtins.Exception".into(),
            "builtins.BaseException".into(),
        ];
        failure
    }
}
pub type Result<T> = std::result::Result<T, Failure>;

/// The task host supplies its container, scaffold, grader and transcript handle.
pub trait Runtime {
    fn call(&mut self, method: &str, arguments: Value) -> Result<Value>;
}

#[derive(Deserialize)]
pub struct Task {
    pub task: Value,
    pub arm: Value,
    pub injected: Value,
    pub exclude_tools: Value,
    pub run_dir: PathBuf,
    pub kwargs: Value,
    pub metadata: Value,
    pub environment: Value,
    pub process_cwd: PathBuf,
}

pub fn build_prompt(task: &Value, injected: &Value) -> Result<String> {
    let body = TASK_TEMPLATE.replace(
        "{problem}",
        &scalar_text(task.get("problem_statement").unwrap_or(&json!(""))),
    );
    if !truthy(injected) {
        return Ok(body);
    }
    let text = injected.as_str().ok_or_else(|| {
        Failure::new(
            "AttributeError",
            "injected prefix has no attribute 'rstrip'",
        )
    })?;
    Ok(format!(
        "{}\n\n{body}",
        text.trim_end_matches(|c: char| c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}'))
    ))
}

fn basename(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}
fn dirname(path: &str) -> &str {
    let head = path.rfind('/').map_or("", |at| &path[..=at]);
    if head.chars().all(|c| c == '/') {
        head
    } else {
        head.trim_end_matches('/')
    }
}
pub fn cell_of(path: &str) -> String {
    let normalized = posix_normpath(path);
    let parent = dirname(&normalized);
    let (arm, repository) = (basename(dirname(parent)), basename(parent));
    if arm.is_empty() {
        repository.into()
    } else {
        format!("{arm}/{repository}")
    }
}

pub fn row_from_segment(agent: &str, result: &Value) -> Result<Value> {
    if agent == "claude" {
        return crate::experiments::usage::claude::row_from_result(result)
            .map_err(Failure::accounting);
    }
    let mut row = result.as_object().cloned().unwrap_or_default();
    row.entry("is_error").or_insert(json!(false));
    row.entry("cost_usd_vendor").or_insert(Value::Null);
    let source = row.get("tokens_source").cloned().unwrap_or(Value::Null);
    let incomplete = !source.is_null()
        && source
            != if agent == "copilot" {
                "debug_log"
            } else {
                "stream_usage"
            };
    row.insert("usage_incomplete".into(), json!(incomplete));
    row.insert(
        "output_tokens_missing".into(),
        json!(row.get("output_tokens").is_none_or(Value::is_null)),
    );
    row.insert("num_turns_recovered".into(), json!(false));
    if incomplete {
        row.entry("usage_incomplete_reason")
            .or_insert(json!(format!("tokens_source={}", scalar_text(&source))));
    }
    Ok(json!(row))
}

pub fn container_name(
    instance: &str,
    arm: &Value,
    agent: &Value,
    model: &Value,
    run_dir: &Path,
    cwd: &Path,
) -> String {
    let absolute = if run_dir.is_absolute() {
        run_dir.to_path_buf()
    } else {
        cwd.join(run_dir)
    };
    #[cfg(unix)]
    let absolute = posix_normpath(&absolute.to_string_lossy());
    #[cfg(not(unix))]
    let absolute = crate::plugin::paths::normalize(&absolute)
        .to_string_lossy()
        .into_owned();
    let key = format!("{}|{}|{absolute}", scalar_text(agent), scalar_text(model));
    let digest = format!("{:x}", Sha1::digest(key.as_bytes()));
    let stem = format!(
        "te-{}-{}",
        instance.replace("__", "-").replace('/', "-"),
        scalar_text(arm)
    )
    .to_lowercase();
    let stem = stem
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() || "_.-".contains(c) {
                c
            } else {
                '-'
            }
        })
        .take(50)
        .collect::<String>();
    format!("{}-{}", stem.trim_end_matches(['-', '.']), &digest[..8])
}

fn update(row: &mut Value, values: Value) -> Result<()> {
    row.as_object_mut()
        .ok_or_else(|| Failure::new("TypeError", "expected row object"))?
        .extend(
            values
                .as_object()
                .ok_or_else(|| Failure::new("TypeError", "expected row fields"))?
                .clone(),
        );
    Ok(())
}
fn now(runtime: &mut dyn Runtime) -> Result<f64> {
    runtime
        .call("clock", json!([]))?
        .as_f64()
        .ok_or_else(|| Failure::new("TypeError", "clock must return a number"))
}
fn log(runtime: &mut dyn Runtime, text: String) -> Result<()> {
    runtime.call("log", json!([text])).map(|_| ())
}
fn elapsed(runtime: &mut dyn Runtime, start: f64) -> Result<Value> {
    Ok(json!(round_decimal(now(runtime)? - start, 1)))
}

pub fn run_task(input: &Task, runtime: &mut dyn Runtime) -> Result<Value> {
    let iid = input
        .task
        .get("instance_id")
        .ok_or_else(|| Failure::new("KeyError", "instance_id"))?;
    let meta = &input.metadata;
    let mut context = json!({"arm":input.arm,"instance_id":iid,"model":meta["model"],"agent":meta["agent"],"cell":cell_of(&input.run_dir.to_string_lossy()),"run_dir":basename(&input.run_dir.to_string_lossy())});
    context.as_object_mut().unwrap().retain(|_, v| !v.is_null());
    runtime.call("optimizer_context", json!([context]))?;
    let physical = if input.run_dir.is_absolute() {
        input.run_dir.clone()
    } else {
        input.process_cwd.join(&input.run_dir)
    };
    fs::create_dir_all(&physical)?;
    let name = runtime.call("_container_name", json!([iid, input.arm, input.run_dir]))?;
    let max_turns = input
        .kwargs
        .get("max_turns")
        .filter(|v| truthy(v))
        .unwrap_or_else(|| {
            input
                .environment
                .get("TE_MAX_TURNS")
                .unwrap_or(&Value::Null)
        });
    let max_turns = if max_turns.is_null() {
        80
    } else {
        // shortcut: turn limits use signed 64-bit values; expand with the numeric parity audit.
        int_value(max_turns)
            .ok_or_else(|| Failure::new("ValueError", "invalid maximum turn count"))? as i64
    };
    let started = now(runtime)?;
    let guard = runtime.call("_build_guard", json!([]))?;
    let prompt = build_prompt(&input.task, &input.injected)?;
    let mut row = json!({"instance":iid,"repo":input.task.get("repo").cloned().unwrap_or(json!("")),"model":meta["model"],"agent":meta["agent"],"arm":input.arm,"effort":meta["effort"],"cli_version":meta["cli_version"]});
    runtime.call(
        "_write_run_config",
        json!([
            input.run_dir,
            input.task,
            input.arm,
            input.injected,
            input.exclude_tools,
            max_turns
        ]),
    )?;
    if let Err(error) = runtime.call("_start", json!([name, iid])) {
        if !error.recoverable() {
            return Err(error);
        }
        log(
            runtime,
            format!(
                "{}: CONTAINER START FAILED ({}) -> unresolved (stream continues)",
                scalar_text(iid),
                error.kind
            ),
        )?;
        runtime.call("_stop", json!([name]))?;
        update(
            &mut row,
            json!({"resolved":false,"is_error":true,"api_error_status":"container_start_failed","eval_report":{"start_error":error.message.chars().take(400).collect::<String>()},"num_turns":0,"patch_bytes":0,"wall_s":elapsed(runtime,started)?}),
        )?;
        return Ok(row);
    }
    let activation = runtime.call("_record_env_activation", json!([input.run_dir, name, row]))?;
    update(&mut row, activation)?;
    let body = (|| -> Result<(Value, Value, Value, String, Value, Value)> {
        let scaffold = runtime.call("_apply_scaffold", json!([name, input.task, input.run_dir]))?;
        update(&mut row, scaffold[0].clone())?;
        let mcp = &scaffold[1];
        let net_guard = runtime.call("_install_net_guard", json!([name]))?;
        let hosts = runtime.call("_install_answer_key_hosts", json!([name]))?;
        runtime.call(
            "transcript_open",
            json!([input.run_dir.join("transcript.jsonl")]),
        )?;
        let result = (|| -> Result<Value> {
            let mut result = runtime.call(
                "_segment",
                json!([
                    name,
                    prompt,
                    input.run_dir.join("transcript.jsonl"),
                    input.exclude_tools,
                    max_turns,
                    mcp
                ]),
            )?;
            while truthy(&guard) {
                let next = runtime.call("_guard_continuation", json!([prompt]))?;
                if !truthy(&next) {
                    break;
                }
                let segment = runtime.call("guard_segment", json!([]))?;
                log(
                    runtime,
                    format!(
                        "{}: spiral nudge -> segment {}",
                        scalar_text(iid),
                        scalar_text(&segment)
                    ),
                )?;
                let part = runtime.call(
                    "_segment",
                    json!([
                        name,
                        next,
                        input.run_dir.join("transcript.jsonl"),
                        input.exclude_tools,
                        max_turns,
                        mcp
                    ]),
                )?;
                result = runtime.call("_merge_segments", json!([result, part]))?;
            }
            Ok(result)
        })();
        runtime.call("transcript_close", json!([]))?;
        let result = result?;
        let (mut patch, mut resolved, mut report) = (String::new(), json!(false), json!({}));
        let grade = (|| -> Result<()> {
            patch = runtime
                .call("_graded_diff", json!([name]))?
                .as_str()
                .ok_or_else(|| Failure::new("TypeError", "patch must be text"))?
                .into();
            fs::write(physical.join("model_patch.diff"), &patch)?;
            let value = runtime.call("_grade", json!([name, iid, patch, input.run_dir]))?;
            resolved = value[0].clone();
            report = value[1].clone();
            Ok(())
        })();
        if let Err(error) = grade {
            if !error.recoverable() {
                return Err(error);
            }
            log(
                runtime,
                format!(
                    "{}: GRADING FAILED ({}) -> unresolved, flagged (stream continues)",
                    scalar_text(iid),
                    error.kind
                ),
            )?;
            row["is_error"] = json!(true);
            row["api_error_status"] = json!("grading_failed");
            row["grading_error"] = json!(format!(
                "{}: {}",
                error.kind,
                error.message.chars().take(400).collect::<String>()
            ));
            report = json!({"grading_error":row["grading_error"]});
        }
        Ok((result, resolved, report, patch, net_guard, hosts))
    })();
    // Cleanup runs once and its failures replace the task failure, matching Python finally.
    let blocks = runtime.call("_net_guard_blocks", json!([name]))?;
    runtime.call("_stop", json!([name]))?;
    let (result, resolved, report, patch, net_guard, hosts) = body?;
    update(
        &mut row,
        runtime.call("_row_from_segment", json!([result]))?,
    )?;
    update(
        &mut row,
        json!({"resolved":resolved,"eval_report":report,"patch_bytes":patch.chars().count(),"wall_s":elapsed(runtime,started)?,"untooled_excluded":if truthy(&input.exclude_tools) {input.exclude_tools.clone()} else {json!([])},"net_guard_installed":net_guard,"net_guard_blocks":blocks,"answer_key_hosts_blocked":hosts}),
    )?;
    row["answer_key_attempts"] = runtime.call("_answer_key_attempts", json!([input.run_dir]))?;
    let scaffold = runtime.call("scaffold_info", json!([]))?;
    if !scaffold.is_null() {
        row["scaffold"] = scaffold["name"].clone();
        row["patch_excludes"] = runtime.call("_patch_excludes", json!([]))?;
    }
    if truthy(&guard) {
        update(&mut row, runtime.call("_guard_row", json!([]))?)?;
    }
    log(
        runtime,
        format!(
            "{} arm={} resolved={} turns={} eff_tok={:.0} {}s",
            scalar_text(iid),
            scalar_text(&input.arm),
            scalar_text(&row["resolved"]),
            scalar_text(&row["num_turns"]),
            pricing::effective_tokens(&row, None)
                .map_err(|e| Failure::new("ValueError", e.to_string()))?,
            scalar_text(&row["wall_s"])
        ),
    )?;
    Ok(row)
}
