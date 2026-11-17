use crate::job::{self, Channel};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tokeneconomizer::plugin::{
    history,
    hosts::session::Environment,
    process,
    sidecall::{self, Runtime},
};

struct Transport(Channel);
impl Runtime for Transport {
    fn which(&mut self, cli: &str) -> Result<Option<String>, String> {
        let response = self.0.call("which", json!([cli]))?;
        if let Some(error) = response["error"].as_str() {
            return Err(error.into());
        }
        Ok(response["value"].as_str().map(String::from))
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
        execute(&op, &args, &mut Transport(channel))
            .unwrap_or_else(|message| json!({"raised":"RuntimeError","message":message}))
    })
    .map(Some)
}
