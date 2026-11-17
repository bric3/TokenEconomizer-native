use crate::job::{self, Channel};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tokeneconomizer::plugin::{
    history,
    hosts::session::Environment,
    process,
    sidecall::{self, Runtime},
};

struct Transport {
    channel: Channel,
    flags: Value,
}
impl Runtime for Transport {
    fn temporary_directory(
        &mut self,
        prefix: &str,
        ignore_cleanup_errors: bool,
    ) -> Result<Option<PathBuf>, String> {
        if self.flags["temporary_directory"] != true {
            return Ok(None);
        }
        let reply = self.channel.call(
            "temporary_directory",
            json!([prefix, ignore_cleanup_errors]),
        )?;
        if let Some(error) = reply["error"].as_str() {
            return Err(error.into());
        }
        reply["value"]
            .as_str()
            .map(|p| Some(PathBuf::from(p)))
            .ok_or("invalid temporary directory".into())
    }
    fn remove_temporary_directory(&mut self, path: &Path) -> Result<(), String> {
        let reply = self
            .channel
            .call("remove_temporary_directory", json!([path]))?;
        if let Some(error) = reply["error"].as_str() {
            return Err(error.into());
        }
        Ok(())
    }
    fn which(&mut self, cli: &str) -> Result<Option<String>, String> {
        let response = self.channel.call("which", json!([cli]))?;
        if let Some(error) = response["error"].as_str() {
            return Err(error.into());
        }
        Ok(response["value"].as_str().map(String::from))
    }
    fn claude_cli(&mut self, environment: &Environment, windows: bool) -> Result<String, String> {
        if self.flags["claude_cli"] != true {
            return sidecall::installed_cli("claude", environment, windows, self);
        }
        let reply = self.channel.call("claude_cli", json!([]))?;
        if let Some(error) = reply["error"].as_str() {
            return Err(error.into());
        }
        reply["value"]
            .as_str()
            .map(String::from)
            .ok_or("invalid Claude executable".into())
    }
    fn run(
        &mut self,
        argv: &[String],
        cwd: &Path,
        env: &serde_json::Map<String, Value>,
        input: Option<&str>,
        timeout: f64,
    ) -> std::io::Result<process::Output> {
        if self.flags["run"] != true {
            return match input {
                Some(input) => process::run_input(argv, cwd, env, input, timeout),
                None => process::run_environment(argv, cwd, env, timeout),
            };
        }
        let reply = self
            .channel
            .call("run", json!([argv, cwd, env, input, timeout]))
            .map_err(std::io::Error::other)?;
        if let Some(error) = reply["error"].as_str() {
            return Err(std::io::Error::new(
                if reply["error_kind"] == "TimeoutExpired" {
                    std::io::ErrorKind::TimedOut
                } else {
                    std::io::ErrorKind::Other
                },
                error.to_string(),
            ));
        }
        let out = &reply["value"];
        Ok(process::Output {
            code: out["code"].as_i64().unwrap_or(0) as i32,
            stdout: text(out, "stdout").into(),
            stderr: text(out, "stderr").into(),
        })
    }
}
fn text<'a>(args: &'a Value, key: &str) -> &'a str {
    args[key].as_str().unwrap_or("")
}
fn execute(op: &str, args: &Value, runtime: &mut Transport) -> Result<Value, String> {
    let environment = Environment {
        env: args["env"].as_object().cloned().unwrap_or_default(),
        cwd: PathBuf::from(text(args, "process_cwd")),
        home: PathBuf::from(text(args, "home")),
    };
    let root = environment.path(Path::new(text(args, "root")));
    let host = text(args, "host");
    if matches!(
        op,
        "sidecall._claude_help"
            | "sidecall.claude_flags"
            | "sidecall.claude_no_mcp"
            | "sidecall.claude_call"
    ) {
        let mut cache = args["help"].as_object().cloned().unwrap_or_default();
        let value = if op == "sidecall.claude_call" {
            sidecall::claude_call(
                &sidecall::ClaudeCall {
                    prompt: text(args, "prompt"),
                    model: text(args, "model"),
                    cli: text(args, "cli"),
                    timeout: args["timeout"]
                        .as_f64()
                        .unwrap_or(sidecall::TIMEOUT_S as f64),
                    effort: text(args, "effort"),
                    windows: args["windows"] == true,
                },
                &environment,
                &root,
                &mut cache,
                runtime,
            )
            .map(|value| json!(value))
        } else {
            let cli = if op == "sidecall.claude_flags" && text(args, "cli").is_empty() {
                runtime.claude_cli(&environment, args["windows"] == true)?
            } else {
                text(args, "cli").into()
            };
            let help = sidecall::claude_help(&cli, &mut cache, &environment, runtime);
            match op {
                "sidecall._claude_help" => Ok(json!(help)),
                "sidecall.claude_flags" => Ok(json!(sidecall::claude_flags(&help))),
                _ => {
                    sidecall::claude_no_mcp(&help, &environment.path(Path::new(text(args, "cwd"))))
                        .map(|value| json!(value))
                        .map_err(|e| e.to_string())
                }
            }
        };
        return Ok(match value {
            Ok(value) => json!({"value":value,"help":cache}),
            Err(message) => json!({"raised":"RuntimeError","message":message,"help":cache}),
        });
    }
    let value = match op {
        "sidecall.process_input" => {
            let argv: Vec<String> =
                serde_json::from_value(args["argv"].clone()).map_err(|e| e.to_string())?;
            return Ok(
                match process::run_input(
                    &argv,
                    &environment.cwd,
                    &environment.env,
                    text(args, "input"),
                    args["timeout"].as_f64().unwrap_or(5.0),
                ) {
                    Ok(out) => {
                        json!({"value":{"code":out.code,"stdout":out.stdout,"stderr":out.stderr}})
                    }
                    Err(error) => {
                        json!({"raised":if error.kind() == std::io::ErrorKind::TimedOut { "TimeoutExpired" } else { "OSError" },"message":error.to_string()})
                    }
                },
            );
        }
        "sidecall._obj" => {
            if args["v"].is_object() {
                args["v"].clone()
            } else {
                json!({})
            }
        }
        "sidecall._env" | "sidecall._hermes_env" => json!(sidecall::environment(
            &environment.env,
            op == "sidecall._hermes_env"
        )),
        "sidecall._pause_path" => json!(sidecall::pause_path(host, &root)),
        "sidecall.paused" => sidecall::paused(host, &root),
        "sidecall.resume" => json!(sidecall::resume(host, &root)),
        "sidecall._log" => {
            sidecall::log(&root, text(args, "text"));
            Value::Null
        }
        "sidecall._no_reply" => {
            sidecall::no_reply(
                host,
                &root,
                text(args, "stdout"),
                text(args, "stderr"),
                &args["code"],
                history::now(),
            );
            Value::Null
        }
        "sidecall.claude_cli" | "sidecall.hermes_cli" => json!(sidecall::installed_cli(
            if op == "sidecall.claude_cli" {
                "claude"
            } else {
                "hermes"
            },
            &environment,
            args["windows"] == true,
            runtime
        )?),
        "sidecall._shim_argv" => json!(sidecall::shim_argv(
            Path::new(text(args, "path")),
            &environment,
            runtime
        )?),
        "sidecall.program" => match sidecall::program(
            text(args, "cli"),
            args["windows"] == true,
            &environment,
            runtime,
        ) {
            Ok(argv) => json!(argv),
            Err(message) => return Ok(json!({"raised":"FileNotFoundError","message":message})),
        },
        "sidecall.claude_json" => sidecall::claude_json(text(args, "stdout")),
        "sidecall.copilot_model" => json!(sidecall::copilot_model(
            text(args, "paid_model"),
            &environment.env
        )),
        "sidecall.droid_enabled" => json!(sidecall::droid_enabled(&environment.env)),
        "sidecall._droid_usage" => sidecall::droid_usage(&args["d"]),
        "sidecall.local_routes" => json!(sidecall::local_routes(
            &args["role"],
            &args["config"],
            args["macos"] == true
        )),
        "sidecall.call_name" => json!(sidecall::call_name(text(args, "host_name"))),
        _ => return Err(format!("unknown side-call operation: {op}")),
    };
    Ok(json!({"value":value}))
}
pub(super) fn dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
    if op == "sidecall.constants" {
        return Ok(Some(
            json!({"TIMEOUT_S":sidecall::TIMEOUT_S,"FM_CLI":sidecall::FM_CLI,"FM_TIMEOUT_S":sidecall::FM_TIMEOUT_S,"NO_SIDE_CALLS":sidecall::NO_SIDE_CALLS}),
        ));
    }
    if op == "sidecall.resume_job" {
        return job::advance(
            args["job"].as_u64().ok_or("missing side-call job")?,
            args.get("reply").cloned(),
        )
        .map(Some);
    }
    let (op, args) = (op.to_string(), args.clone());
    job::start_polling("sidecall", move |channel| {
        execute(
            &op,
            &args,
            &mut Transport {
                channel,
                flags: args["transport"].clone(),
            },
        )
        .unwrap_or_else(|message| json!({"raised":"RuntimeError","message":message}))
    })
    .map(Some)
}
