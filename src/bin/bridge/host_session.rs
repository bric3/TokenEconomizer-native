use crate::{
    job::{self, Channel},
    storage_dispatch,
};
use serde_json::{Value, json};
use std::{
    io,
    path::{Path, PathBuf},
};
use tokeneconomizer::plugin::{
    hosts::session::{self, Environment, Error, Limits, Runtime},
    process,
};

struct Transport {
    channel: Channel,
    flags: Value,
}
impl Transport {
    fn call(&self, method: &str, args: Value) -> session::Result<Value> {
        let reply = self.channel.call(method, args)?;
        if let Some(message) = reply["error"].as_str() {
            return Err(match reply["error_kind"].as_str() {
                Some("OSError" | "TimeoutExpired" | "SubprocessError") => {
                    Error::Io(io::Error::other(message.to_string()))
                }
                _ => Error::Invalid(message.into()),
            });
        }
        Ok(reply["value"].clone())
    }
}
impl Runtime for Transport {
    fn program(&mut self, cli: &str) -> session::Result<Vec<String>> {
        serde_json::from_value(self.call("program", json!([cli]))?)
            .map_err(|e| Error::Invalid(e.to_string()))
    }
    fn run(
        &mut self,
        argv: &[String],
        cwd: &Path,
        env: Option<&serde_json::Map<String, Value>>,
        timeout: f64,
    ) -> io::Result<process::Output> {
        if self.flags["run"] == true {
            let out = self
                .call("run", json!([argv, cwd, env, timeout]))
                .map_err(io::Error::other)?;
            Ok(process::Output {
                code: out["code"].as_i64().unwrap_or(0) as i32,
                stdout: out["stdout"].as_str().unwrap_or("").into(),
                stderr: out["stderr"].as_str().unwrap_or("").into(),
            })
        } else if let Some(env) = env {
            process::run_environment(argv, cwd, env, timeout)
        } else {
            process::run(argv, cwd, timeout, false)
        }
    }
    fn ancestors(
        &mut self,
        limit: i64,
        pid: u32,
        windows: bool,
        cwd: &Path,
    ) -> session::Result<Vec<String>> {
        if self.flags["ancestors"] == true {
            serde_json::from_value(self.call("ancestors", json!([]))?)
                .map_err(|e| Error::Invalid(e.to_string()))
        } else {
            session::ancestor_args(limit, pid, windows, cwd, self)
        }
    }
}
fn text<'a>(args: &'a Value, key: &str) -> &'a str {
    args[key].as_str().unwrap_or("")
}
fn execute(op: &str, args: &Value, runtime: &mut Transport) -> session::Result<Value> {
    let host = text(args, "host");
    let environment = Environment {
        env: args["process_env"].as_object().cloned().unwrap_or_default(),
        home: PathBuf::from(text(args, "home")),
        cwd: PathBuf::from(text(args, "process_cwd")),
    };
    let input = |key| environment.path(Path::new(text(args, key)));
    let env = args["env"].as_object().cloned().unwrap_or_default();
    let transcript = args["transcript_path"].as_str();
    let parent = args["parent_pid"].as_u64().unwrap_or(0) as u32;
    let windows = args["windows"] == true;
    Ok(match op {
        "host_runtime.copilot_home" => json!(session::copilot_home(&env, &environment.home)),
        "host_runtime.cwd" => session::cwd(host, &args["p"], &args["env"], &environment.cwd),
        "host_runtime.transcript" => session::transcript(host, &args["p"], &environment)?,
        "host_runtime.headless" => json!(session::headless(
            host,
            &args["env"],
            args["transcript"].as_str(),
            &environment,
            parent,
            windows,
            runtime
        )?),
        "host_runtime._ancestor_args" => json!(session::ancestor_args(
            args["limit"].as_i64().unwrap_or(4),
            parent,
            windows,
            &environment.cwd,
            runtime
        )?),
        "host_runtime.mechanism_trace" => json!(session::mechanism_trace(
            host,
            transcript,
            Path::new(text(args, "workdir")),
            &environment
        )?),
        "host_runtime.session_usage" => session::session_usage(host, &input("transcript_path"))?,
        "host_runtime.prompt_count" => {
            json!(session::prompt_count(host, &input("transcript_path")))
        }
        "host_runtime.tool_usage" => json!(session::tool_usage(host, &input("trace_path"))?),
        "host_runtime.last_active" => json!(session::last_active(
            host,
            &input("transcript_path"),
            &args["session"]
        )?),
        "host_runtime.session_state" => session::session_state(host, &env, &environment.home),
        "host_runtime.settle" => {
            session::settle(
                host,
                transcript
                    .map(|p| environment.path(Path::new(p)))
                    .as_deref(),
                args["wait_s"].as_f64().unwrap_or(15.0),
                args["fresh_s"].as_f64().unwrap_or(60.0),
                runtime,
            )?;
            Value::Null
        }
        "host_runtime.history" => json!(session::history(
            host,
            &env,
            &serde_json::from_value::<Vec<String>>(args["extra_dirs"].clone())
                .map_err(|e| Error::Invalid(e.to_string()))?,
            &environment
        )?),
        "host_runtime.folder_matches" => json!(session::folder_matches(
            host,
            Path::new(text(args, "root")),
            &args["folder"]
        )?),
        "host_runtime.log_head" => session::log_head(host, &input("path")),
        "host_runtime.safe_session_id" => json!(session::safe_session_id(host, &args["sid"])),
        "host_runtime.prompt_turns" => json!(session::prompt_turns(host, &input("path"))),
        "host_runtime.turn_calls"
        | "host_runtime.turn_interrupted"
        | "host_runtime.turn_answered" => {
            let lines: Vec<String> = serde_json::from_value(args["turn_lines"].clone())
                .map_err(|e| Error::Invalid(e.to_string()))?;
            match op {
                "host_runtime.turn_calls" => json!(session::turn_calls(&lines)),
                "host_runtime.turn_interrupted" => json!(session::turn_interrupted(&lines)),
                _ => json!(session::turn_answered(&lines)),
            }
        }
        "host_runtime.complete" => {
            let limits = Limits {
                export_tries: args["limits"]["export_tries"].as_i64().unwrap_or(3),
                export_wait: args["limits"]["export_wait"].as_f64().unwrap_or(2.0),
                fold_retry: args["limits"]["fold_retry"]
                    .as_f64()
                    .unwrap_or(7.0 * 86400.0),
            };
            session::complete(
                host,
                transcript,
                text(args, "session_id"),
                args["cwd"].as_str().map(Path::new),
                &args["session"],
                &environment,
                &limits,
                runtime,
            )?
        }
        _ => {
            return Err(Error::Invalid(format!(
                "unknown host session operation: {op}"
            )));
        }
    })
}
fn response(result: session::Result<Value>) -> Value {
    match result {
        Ok(value) => json!({"value":value}),
        Err(Error::Io(e)) => storage_dispatch::response(Err(e)),
        Err(Error::Later(message)) => json!({"raised":"FoldLater","message":message}),
        Err(Error::Unimplemented) => json!({"raised":"NotImplementedError","message":""}),
        Err(Error::Invalid(message)) => json!({"raised":"ValueError","message":message}),
    }
}
pub(super) fn dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
    if !op.starts_with("host_runtime.") {
        return Ok(None);
    }
    if op == "host_runtime.resume" {
        return job::advance(
            args["job"].as_u64().ok_or("missing host session job")?,
            args.get("reply").cloned(),
        )
        .map(Some);
    }
    let (op, args) = (op.to_string(), args.clone());
    job::start_polling("host_runtime", move |channel| {
        let mut runtime = Transport {
            channel,
            flags: args["transport"].clone(),
        };
        response(execute(&op, &args, &mut runtime))
    })
    .map(Some)
}
