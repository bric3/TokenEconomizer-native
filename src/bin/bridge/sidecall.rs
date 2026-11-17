use crate::job::{self, Channel};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tokeneconomizer::plugin::{
    history,
    hosts::session::Environment,
    paths, process,
    sidecall::{self, Runtime},
};
use tokeneconomizer::runner::Runner;

#[derive(Clone)]
pub(super) struct Transport {
    pub(super) channel: Channel,
    pub(super) flags: Value,
    pub(super) last: Option<(String, sidecall::Failure)>,
}
impl Transport {
    pub(super) fn response_error(&mut self, reply: &Value) -> Result<(), String> {
        if let Some(message) = reply["error"].as_str() {
            if let Some(failure) = reply.get("failure") {
                self.last = Some((
                    message.into(),
                    serde_json::from_value(failure.clone()).map_err(|e| e.to_string())?,
                ));
            }
            return Err(message.into());
        }
        Ok(())
    }
}
impl Runtime for Transport {
    fn override_call(
        &mut self,
        method: &str,
        args: &Value,
    ) -> Result<Option<(Value, Option<f64>)>, sidecall::Failure> {
        if self.flags[method] != true {
            return Ok(None);
        }
        let reply = self
            .channel
            .call("model_call", json!([method, args]))
            .map_err(|message| self.failure(message))?;
        if let Some(failure) = reply.get("failure") {
            return Err(
                serde_json::from_value(failure.clone()).map_err(|e| self.failure(e.to_string()))?
            );
        }
        let result = &reply["value"];
        Ok(Some((result[0].clone(), result[1].as_f64())))
    }
    fn override_available(&mut self) -> Result<Option<bool>, String> {
        if self.flags["fm_available"] != true {
            return Ok(None);
        }
        let reply = self.channel.call("fm_available", json!([]))?;
        self.response_error(&reply)?;
        Ok(Some(reply["value"] == true))
    }
    fn failure(&self, message: String) -> sidecall::Failure {
        self.last
            .as_ref()
            .filter(|(text, _)| *text == message)
            .map(|(_, failure)| failure.clone())
            .unwrap_or(sidecall::Failure::Exception {
                name: "RuntimeError".into(),
                message,
            })
    }
    fn effective(&mut self, usage: &Value) -> Result<Option<f64>, String> {
        if self.flags["effective"] != true {
            return tokeneconomizer::accounting::effective(usage)
                .map(|v| v.0)
                .map_err(|e| e.to_string());
        }
        let reply = self.channel.call("effective", json!([usage]))?;
        self.response_error(&reply)?;
        Ok(reply["value"][0].as_f64())
    }
    fn hermes_cli(&mut self, environment: &Environment) -> Result<String, String> {
        if self.flags["hermes_cli"] != true {
            return sidecall::installed_cli("hermes", environment, false, self);
        }
        let reply = self.channel.call("hermes_cli", json!([]))?;
        self.response_error(&reply)?;
        reply["value"]
            .as_str()
            .map(String::from)
            .ok_or("invalid Hermes executable".into())
    }
    fn executable(&mut self, path: &Path) -> Result<bool, String> {
        if self.flags["access"] != true {
            return paths::executable(path);
        }
        let reply = self.channel.call("access", json!([path, 1]))?;
        self.response_error(&reply)?;
        Ok(reply["value"] == true)
    }
    fn program(
        &mut self,
        cli: &str,
        windows: bool,
        environment: &Environment,
    ) -> Result<Vec<String>, String> {
        if self.flags["program"] != true {
            return sidecall::program(cli, windows, environment, self);
        }
        let reply = self.channel.call("program", json!([cli]))?;
        self.response_error(&reply)?;
        serde_json::from_value(reply["value"].clone()).map_err(|e| e.to_string())
    }
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
        self.response_error(&reply)?;
        reply["value"]
            .as_str()
            .map(|p| Some(PathBuf::from(p)))
            .ok_or("invalid temporary directory".into())
    }
    fn remove_temporary_directory(&mut self, path: &Path) -> Result<(), String> {
        let reply = self
            .channel
            .call("remove_temporary_directory", json!([path]))?;
        self.response_error(&reply)?;
        Ok(())
    }
    fn which(&mut self, cli: &str) -> Result<Option<String>, String> {
        let response = self.channel.call("which", json!([cli]))?;
        self.response_error(&response)?;
        Ok(response["value"].as_str().map(String::from))
    }
    fn claude_cli(&mut self, environment: &Environment, windows: bool) -> Result<String, String> {
        if self.flags["claude_cli"] != true {
            return sidecall::installed_cli("claude", environment, windows, self);
        }
        let reply = self.channel.call("claude_cli", json!([]))?;
        self.response_error(&reply)?;
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
            let result = match input {
                Some(input) => process::run_input(argv, cwd, env, input, timeout),
                None => process::run_environment(argv, cwd, env, timeout),
            };
            self.last = result.as_ref().err().map(|error| {
                (
                    error.to_string(),
                    sidecall::Failure::from_io(error, timeout),
                )
            });
            return result;
        }
        let reply = self
            .channel
            .call("run", json!([argv, cwd, env, input, timeout]))
            .map_err(std::io::Error::other)?;
        if let Some(error) = reply["error"].as_str() {
            let error = std::io::Error::new(
                match reply["error_kind"].as_str() {
                    Some("TimeoutExpired") => std::io::ErrorKind::TimedOut,
                    Some("FileNotFoundError") => std::io::ErrorKind::NotFound,
                    Some("PermissionError") => std::io::ErrorKind::PermissionDenied,
                    _ => std::io::ErrorKind::Other,
                },
                error.to_string(),
            );
            let failure = reply
                .get("failure")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_else(|| sidecall::Failure::from_io(&error, timeout));
            self.last = Some((error.to_string(), failure));
            return Err(error);
        }
        self.last = None;
        let out = &reply["value"];
        Ok(process::Output {
            code: out["code"].as_i64().unwrap_or(0) as i32,
            stdout: text(out, "stdout").into(),
            stderr: text(out, "stderr").into(),
        })
    }
}
struct Fallback(Channel);
impl Runner for Fallback {
    fn run(&mut self, prompt: &str, model: &str) -> Result<(String, Option<f64>), String> {
        let reply = self.0.call("fallback", json!([prompt, model]))?;
        if let Some(error) = reply["error"].as_str() {
            return Err(error.into());
        }
        serde_json::from_value(reply["value"].clone()).map_err(|e| e.to_string())
    }
}
fn text<'a>(args: &'a Value, key: &str) -> &'a str {
    args[key].as_str().unwrap_or("")
}
fn request(args: &Value) -> sidecall::Call<'_> {
    sidecall::Call {
        prompt: text(args, "prompt"),
        model: text(args, "model"),
        cli: text(args, "cli"),
        timeout: args["timeout"]
            .as_f64()
            .unwrap_or(sidecall::TIMEOUT_S as f64),
        effort: text(args, "effort"),
        windows: args["windows"] == true,
    }
}
fn execute(op: &str, args: &Value, runtime: &mut Transport) -> Result<Value, String> {
    let environment = Environment {
        env: args["env"].as_object().cloned().unwrap_or_default(),
        cwd: PathBuf::from(text(args, "process_cwd")),
        home: PathBuf::from(text(args, "home")),
    };
    let root = environment.path(Path::new(text(args, "root")));
    if matches!(
        op,
        "sidecall._host_call"
            | "sidecall.local_runner"
            | "sidecall.runner_for"
            | "sidecall.runner_run"
    ) {
        use sidecall::runner::{Context, RoutedRunner, State};
        let mut state: State = if op == "sidecall.runner_run" {
            serde_json::from_value(args["runner"].clone()).map_err(|e| e.to_string())?
        } else {
            State::default()
        };
        for (key, target) in [
            ("help", &mut state.help),
            ("oc_flags", &mut state.oc_flags),
            ("fm_ok", &mut state.fm_ok),
        ] {
            if let Some(cache) = args[key].as_object() {
                *target = cache.clone();
            }
        }
        let mut context = Context::new(environment, root);
        context.windows = args["windows"] == true;
        context.macos = args["macos"] == true;
        if let Some(value) = args["hermes_limits"]["delete_timeout"].as_f64() {
            context.hermes_limits.delete_timeout = value;
        }
        if let Some(value) = args["hermes_limits"]["title_wait"].as_f64() {
            context.hermes_limits.title_wait = value;
        }
        if let Some(value) = args["hermes_limits"]["abandon"].as_f64() {
            context.hermes_limits.abandon = value;
        }
        let mut routed = RoutedRunner::new(state, context, runtime.clone());
        if args["has_fallback"] == true {
            routed.fallback = Some(Box::new(Fallback(runtime.channel.clone())));
        }
        let result = if op == "sidecall.runner_run" {
            routed
                .run_raw(text(args, "prompt"), text(args, "model"))
                .map(|v| json!(v))
        } else {
            let present = if op == "sidecall.local_runner" {
                routed.select_local(text(args, "role"), &args["config"])
            } else {
                routed.select(
                    text(args, "host_name"),
                    &args["config"],
                    args["role"].as_str(),
                    op == "sidecall._host_call",
                )
            };
            present
                .map(|v| json!(v))
                .map_err(|message| routed.runtime.failure(message))
        };
        let mut out = match result {
            Ok(value) => json!({"value":value}),
            Err(failure) => {
                json!({"raised":"RuntimeError","message":failure.to_string(),"failure":failure})
            }
        };
        out["help"] = json!(routed.state.help);
        out["oc_flags"] = json!(routed.state.oc_flags);
        out["fm_ok"] = json!(routed.state.fm_ok);
        out["runner"] = json!(routed.state);
        return Ok(out);
    }
    let host = text(args, "host");
    if op == "sidecall.noted_call" {
        let reply = runtime
            .channel
            .call("noted", json!([text(args, "prompt"), text(args, "model")]))?;
        let result: Result<(), sidecall::Failure> = match reply.get("failure") {
            Some(failure) => {
                Err(serde_json::from_value(failure.clone()).map_err(|e| e.to_string())?)
            }
            None => Ok(()),
        };
        return Ok(json!({"value":null,"failed":sidecall::noted(host,&root,result).is_err()}));
    }
    if op.starts_with("sidecall._hermes_") && op != "sidecall._hermes_env"
        || matches!(op, "sidecall.hermes_home" | "sidecall.hermes_call")
    {
        let defaults = sidecall::hermes::Limits::default();
        let hermes = sidecall::hermes::Hermes {
            environment: &environment,
            root: &root,
            limits: sidecall::hermes::Limits {
                delete_timeout: args["hermes_limits"]["delete_timeout"]
                    .as_f64()
                    .unwrap_or(defaults.delete_timeout),
                title_wait: args["hermes_limits"]["title_wait"]
                    .as_f64()
                    .unwrap_or(defaults.title_wait),
                abandon: args["hermes_limits"]["abandon"]
                    .as_f64()
                    .unwrap_or(defaults.abandon),
            },
        };
        let sid = text(args, "session_id");
        let cli = text(args, "cli");
        let value = match op {
            "sidecall.hermes_home" => json!(hermes.home()),
            "sidecall._hermes_db" => json!(hermes.db()),
            "sidecall._hermes_ledger" => json!(hermes.ledger()?.base),
            "sidecall._hermes_charge" => json!(hermes.charge(
                sid,
                args["wait_s"].as_f64(),
                args["abandoned"] == true,
                runtime
            )?),
            "sidecall._hermes_delete" => json!(hermes.delete(sid, cli, runtime)),
            "sidecall._hermes_settle" => json!(hermes.settle(
                sid,
                cli,
                args["charged"] == true,
                args["wait_s"].as_f64(),
                args["abandoned"] == true,
                runtime
            )?),
            "sidecall._hermes_defer" => {
                hermes.defer(sid, &args["left"], runtime)?;
                Value::Null
            }
            "sidecall._hermes_settle_earlier" => json!(hermes.settle_earlier(cli, runtime)?),
            "sidecall.hermes_call" => json!(hermes.call(&request(args), runtime)?),
            _ => return Err(format!("unknown Hermes operation: {op}")),
        };
        return Ok(json!({"value":value}));
    }
    if op == "sidecall.fm_available" {
        let mut cache = args["fm_ok"].as_object().cloned().unwrap_or_default();
        let value = sidecall::fm_available(
            text(args, "cli"),
            args["macos"] == true,
            &mut cache,
            &environment,
            runtime,
        )?;
        return Ok(json!({"value":value,"fm_ok":cache}));
    }
    if op == "sidecall.fm_call" {
        let mut call = request(args);
        if args["timeout"].is_null() {
            call.timeout = sidecall::FM_TIMEOUT_S as f64;
        }
        return sidecall::fm_call(&call, &args["schema"], &environment, &root, runtime)
            .map(|v| json!({"value":v}));
    }
    if matches!(
        op,
        "sidecall.codex_call"
            | "sidecall.pi_call"
            | "sidecall.cursor_call"
            | "sidecall.grok_call"
            | "sidecall.opencode_call"
            | "sidecall.opencode_flags"
            | "sidecall.copilot_call"
            | "sidecall.droid_call"
    ) {
        let call = request(args);
        let mut cache = args["oc_flags"].as_object().cloned().unwrap_or_default();
        let value = match op {
            "sidecall.codex_call" => {
                sidecall::codex_call(&call, &environment, &root, runtime).map(|v| json!(v))
            }
            "sidecall.pi_call" => {
                let extensions: Vec<String> = serde_json::from_value(
                    args.get("extensions")
                        .filter(|v| !v.is_null())
                        .cloned()
                        .unwrap_or(json!([])),
                )
                .map_err(|e| e.to_string())?;
                sidecall::pi_call(&call, &extensions, &environment, &root, runtime)
                    .map(|v| json!(v))
            }
            "sidecall.cursor_call" => {
                sidecall::cursor_call(&call, &environment, &root, runtime).map(|v| json!(v))
            }
            "sidecall.grok_call" => {
                sidecall::grok_call(&call, &environment, &root, runtime).map(|v| json!(v))
            }
            "sidecall.opencode_call" => {
                sidecall::opencode_call(&call, &mut cache, &environment, &root, runtime)
                    .map(|v| json!(v))
            }
            "sidecall.copilot_call" => {
                sidecall::copilot_call(&call, &environment, &root, runtime).map(|v| json!(v))
            }
            "sidecall.droid_call" => {
                sidecall::droid_call(&call, &environment, &root, runtime).map(|v| json!(v))
            }
            _ => {
                sidecall::opencode_flags(call.cli, call.windows, &mut cache, &environment, runtime)
                    .map(|v| json!(v))
            }
        };
        return Ok(match value {
            Ok(value) => json!({"value":value,"oc_flags":cache}),
            Err(message) => json!({"raised":"RuntimeError","message":message,"oc_flags":cache}),
        });
    }
    if matches!(
        op,
        "sidecall._claude_help"
            | "sidecall.claude_flags"
            | "sidecall.claude_no_mcp"
            | "sidecall.claude_call"
    ) {
        let mut cache = args["help"].as_object().cloned().unwrap_or_default();
        let value = if op == "sidecall.claude_call" {
            sidecall::claude_call(&request(args), &environment, &root, &mut cache, runtime)
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
        "sidecall._droid_sessions_root" => json!(sidecall::droid_sessions_root(&environment)),
        "sidecall._droid_settings_usage" => {
            sidecall::droid_settings_usage(&args["sid"], &environment)?
        }
        "sidecall.pi_extension_args" => {
            let extensions: Vec<String> = serde_json::from_value(
                args.get("extensions")
                    .filter(|v| !v.is_null())
                    .cloned()
                    .unwrap_or(json!([])),
            )
            .map_err(|e| e.to_string())?;
            json!(sidecall::pi_extension_args(&extensions, &environment)?)
        }
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
        let schema = sidecall::runner::judge_schema();
        let mut constants = json!({"TIMEOUT_S":sidecall::TIMEOUT_S,"FM_CLI":sidecall::FM_CLI,"FM_TIMEOUT_S":sidecall::FM_TIMEOUT_S,"NO_SIDE_CALLS":sidecall::NO_SIDE_CALLS,"PI_FLAGS":sidecall::PI_FLAGS,"GROK_TOOLS":sidecall::GROK_TOOLS,"GROK_REMOVED":sidecall::GROK_REMOVED,"GROK_DENY_RULES":sidecall::GROK_DENY_RULES,"OPENCODE_CONFIG":sidecall::OPENCODE_CONFIG,"COPILOT_TOOLS":sidecall::COPILOT_TOOLS,"DROID_TOOLS":sidecall::DROID_TOOLS,"HERMES_SOURCE":sidecall::hermes::SOURCE,"HERMES_FLAGS":sidecall::hermes::FLAGS,"HERMES_DELETE_TIMEOUT_S":sidecall::hermes::DELETE_TIMEOUT_S,"HERMES_TITLE_WAIT_S":sidecall::hermes::TITLE_WAIT_S,"HERMES_ABANDON_S":sidecall::hermes::ABANDON_S});
        constants["JUDGE_SCHEMA"] = schema.clone();
        constants["ROLE_SCHEMAS"] = json!({"judge":schema});
        return Ok(Some(constants));
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
                last: None,
            },
        )
        .unwrap_or_else(|message| json!({"raised":"RuntimeError","message":message}))
    })
    .map(Some)
}
