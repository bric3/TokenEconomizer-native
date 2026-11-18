use super::{Failure, Result, Runtime, log};
use crate::value::{number, scalar_text, truthy};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

#[derive(Deserialize)]
pub struct Grade {
    pub name: String,
    pub instance_id: Value,
    pub patch: String,
    pub run_dir: PathBuf,
    pub eval_timeout: Value,
    pub process_cwd: PathBuf,
}

#[derive(Deserialize)]
pub struct ExecFailure {
    pub instance_id: Value,
    pub host_log: PathBuf,
    pub out: Value,
    pub rc: Value,
    pub stderr: Value,
    pub phase: String,
    pub process_cwd: PathBuf,
}

fn strip(text: &str) -> &str {
    text.trim_matches(|c: char| c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}'))
}
fn text(value: &Value) -> Result<&str> {
    value
        .as_str()
        .ok_or_else(|| Failure::new("TypeError", "expected text from the task host"))
}
fn partial_output(value: &Value) -> Result<String> {
    if !truthy(value) {
        return Ok(String::new());
    }
    if let Some(bytes) = value.get("bytes") {
        let bytes: Vec<u8> = serde_json::from_value(bytes.clone())
            .map_err(|e| Failure::new("ValueError", e.to_string()))?;
        return Ok(String::from_utf8_lossy(&bytes).into_owned());
    }
    text(value).map(String::from)
}

pub fn eval_exec_failed(input: &ExecFailure, runtime: &mut dyn Runtime) -> Result<Value> {
    let stderr = if truthy(&input.stderr) {
        strip(text(&input.stderr)?)
    } else {
        ""
    };
    let error: String = stderr
        .chars()
        .skip(stderr.chars().count().saturating_sub(500))
        .collect();
    let mut output = if truthy(&input.out) {
        text(&input.out)?.into()
    } else {
        String::new()
    };
    if !error.is_empty() {
        output.push_str(&format!(
            "\n[te_harness] docker exec failed while {} (rc={}):\n{error}\n",
            input.phase,
            scalar_text(&input.rc)
        ));
    }
    // An unwritable diagnostic log must still leave an explicit ungraded report.
    let _ = fs::write(input.process_cwd.join(&input.host_log), output);
    log(
        runtime,
        format!(
            "{}: EVAL DID NOT RUN while {} (rc={}) -> NOT GRADED. {}",
            scalar_text(&input.instance_id),
            input.phase,
            scalar_text(&input.rc),
            if error.is_empty() {
                "no stderr"
            } else {
                &error
            }
        ),
    )?;
    Ok(
        json!({"eval_exec_failed":true,"eval_exec_rc":input.rc,"eval_exec_phase":input.phase,"eval_exec_error":error}),
    )
}

pub fn grade(input: &Grade, runtime: &mut dyn Runtime) -> Result<Value> {
    let iid = scalar_text(&input.instance_id);
    if strip(&input.patch).is_empty() {
        log(
            runtime,
            format!("{iid}: empty diff -> unresolved without running tests"),
        )?;
        return Ok(json!([false,{"empty_patch":true}]));
    }
    let script = runtime.call("eval_script", json!([input.instance_id]))?;
    let host_log = input.run_dir.join("eval.log");
    enum Execution {
        Failed(Value),
        Output(Value),
    }
    let execution = (|| -> Result<Execution> {
        let write = runtime.call(
            "_exec",
            json!([
                input.name,
                format!(
                    "cat > /tmp/eval.sh <<'TE_EOF'\n{}\nTE_EOF\nchmod +x /tmp/eval.sh",
                    text(&script)?
                ),
                {}
            ]),
        )?;
        if number(&write["returncode"]) != Some(0.0) {
            return Ok(Execution::Failed(runtime.call(
                "_eval_exec_failed",
                json!([
                    input.instance_id,
                    host_log,
                    "",
                    write["returncode"],
                    write["stderr"],
                    "writing eval.sh"
                ]),
            )?));
        }
        runtime.call("_exec", json!([input.name,"/tmp/eval.sh 2>&1 | tail -c 4000000",{"timeout":input.eval_timeout}])).map(Execution::Output)
    })();
    let process = match execution {
        Ok(Execution::Failed(report)) => return Ok(json!([false, report])),
        Ok(Execution::Output(process)) => process,
        Err(error) if error.is_a("subprocess.TimeoutExpired") => {
            fs::write(
                input.process_cwd.join(&host_log),
                partial_output(error.stdout.as_deref().unwrap_or(&Value::Null))?,
            )?;
            log(
                runtime,
                format!(
                    "{iid}: EVAL TIMEOUT after {}s -> unresolved (stream continues; see eval.log)",
                    scalar_text(&input.eval_timeout)
                ),
            )?;
            return Ok(json!([false,{"eval_timeout":input.eval_timeout}]));
        }
        Err(error) => return Err(error),
    };
    let output = text(&process["stdout"])?;
    if number(&process["returncode"]) != Some(0.0) && strip(output).is_empty() {
        let report = runtime.call(
            "_eval_exec_failed",
            json!([
                input.instance_id,
                host_log,
                output,
                process["returncode"],
                process["stderr"],
                "running eval.sh"
            ]),
        )?;
        return Ok(json!([false, report]));
    }
    fs::write(input.process_cwd.join(&host_log), output)?;
    if strip(output).is_empty() {
        log(
            runtime,
            format!("{iid}: eval script exited 0 but printed NOTHING -> not graded"),
        )?;
        return Ok(
            json!([false,{"eval_exec_failed":true,"eval_exec_rc":0,"eval_exec_error":"eval script produced no output"}]),
        );
    }
    match runtime.call("score", json!([input.instance_id, input.patch, host_log])) {
        Err(error) if error.is_a("builtins.Exception") => {
            log(runtime, format!("{iid}: grading failed: {}", error.message))?;
            Ok(json!([false,{"grading_error":error.message}]))
        }
        result => result,
    }
}
