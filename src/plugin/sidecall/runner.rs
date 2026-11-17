use super::{Call, FM_CLI, FM_TIMEOUT_S, Failure, Runtime, TIMEOUT_S, hermes, noted};
use crate::{
    plugin::{hosts::session::Environment, paths, process},
    runner::Runner,
    value::truthy,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::{io, path::PathBuf};

pub fn judge_schema() -> Value {
    json!({"title":"Verdict","type":"object","additionalProperties":false,"x-order":["stuck","confidence","reason"],"required":["stuck","confidence","reason"],"properties":{"stuck":{"type":"boolean"},"confidence":{"type":"number","description":"0.0 to 1.0"},"reason":{"type":"string","description":"short leakage-safe justification"}}})
}
#[derive(Clone, Serialize, Deserialize)]
pub struct HostCall {
    pub method: String,
    pub args: Map<String, Value>,
}
pub fn host_call(
    host: &str,
    config: &Value,
    environment: &Environment,
) -> Result<Option<HostCall>, String> {
    if super::NO_SIDE_CALLS.contains(&host) {
        return Ok(None);
    }
    let model_key = format!("{host}_paid_model");
    let model = config
        .get(&model_key)
        .filter(|v| truthy(v))
        .unwrap_or(&config["paid_model"])
        .clone();
    let effort = config[format!("{host}_paid_effort")].clone();
    let mut args = json!({"model":model}).as_object().unwrap().clone();
    let method = match host {
        "pi" => {
            let cli = config["pi_cli"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or("pi");
            args.insert(
                "cli".into(),
                json!(paths::expand_user(
                    cli,
                    &environment.home,
                    &environment.env
                )?),
            );
            args.insert(
                "extensions".into(),
                config.get("pi_extensions").cloned().unwrap_or(json!([])),
            );
            "pi_call"
        }
        "hermes" => {
            let cli = config["hermes_cli"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(|p| paths::expand_user(p, &environment.home, &environment.env))
                .transpose()?;
            args.insert("cli".into(), json!(cli));
            "hermes_call"
        }
        "droid" => {
            if !(super::droid_enabled(&environment.env) || truthy(&config["droid_sidecall"])) {
                return Ok(None);
            }
            "droid_call"
        }
        "opencode" => "opencode_call",
        "cursor" => "cursor_call",
        "grok" => "grok_call",
        "copilot" => "copilot_call",
        "codex" => {
            if truthy(&effort) {
                args.insert("effort".into(), effort);
            }
            "codex_call"
        }
        _ => {
            if !truthy(&args["model"]) {
                args.insert("model".into(), json!("haiku"));
            }
            if truthy(&effort) {
                args.insert("effort".into(), effort);
            }
            "claude_call"
        }
    };
    Ok(Some(HostCall {
        method: method.into(),
        args,
    }))
}
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub host: Option<HostCall>,
    pub note: Option<String>,
    pub local: bool,
    pub schema: Value,
    pub answered_locally: bool,
    pub help: Map<String, Value>,
    pub oc_flags: Map<String, Value>,
    pub fm_ok: Map<String, Value>,
}
pub struct Context {
    pub environment: Environment,
    pub root: PathBuf,
    pub windows: bool,
    pub macos: bool,
    pub hermes_limits: hermes::Limits,
}
impl Context {
    pub fn new(environment: Environment, root: PathBuf) -> Self {
        Self {
            environment,
            root,
            windows: cfg!(windows),
            macos: cfg!(target_os = "macos"),
            hermes_limits: hermes::Limits::default(),
        }
    }
}
pub struct RoutedRunner<R> {
    pub state: State,
    pub context: Context,
    pub runtime: R,
    pub fallback: Option<Box<dyn Runner>>,
}
impl<R: Runtime> RoutedRunner<R> {
    pub fn new(state: State, context: Context, runtime: R) -> Self {
        Self {
            state,
            context,
            runtime,
            fallback: None,
        }
    }
    pub fn select(
        &mut self,
        host: &str,
        config: &Value,
        role: Option<&str>,
        raw: bool,
    ) -> Result<bool, String> {
        self.state.local = false;
        self.state.schema = Value::Null;
        self.state.answered_locally = false;
        self.state.host = host_call(host, config, &self.context.environment)?;
        self.state.note = (!raw).then(|| host.into());
        if !raw && let Some(role) = role {
            self.select_local(role, config)?;
        }
        Ok(self.state.host.is_some() || self.state.local)
    }
    pub fn select_local(&mut self, role: &str, config: &Value) -> Result<bool, String> {
        self.state.local = false;
        self.state.schema = Value::Null;
        self.state.answered_locally = false;
        if !super::local_routes(&json!(role), config, self.context.macos) {
            return Ok(false);
        }
        let available = match self.runtime.override_available()? {
            Some(value) => value,
            None => super::fm_available(
                FM_CLI,
                self.context.macos,
                &mut self.state.fm_ok,
                &self.context.environment,
                &mut self.runtime,
            )?,
        };
        if !available {
            return Ok(false);
        }
        self.state.local = true;
        self.state.schema = if role == "judge" {
            judge_schema()
        } else {
            Value::Null
        };
        Ok(true)
    }
    fn host(&mut self, host: &HostCall, prompt: &str) -> Result<(Value, Option<f64>), Failure> {
        if let Some(reply) = self
            .runtime
            .override_call(&host.method, &json!({"prompt":prompt,"kwargs":host.args}))?
        {
            return Ok(reply);
        }
        let default_cli = match host.method.as_str() {
            "pi_call" => "pi",
            "codex_call" => "codex",
            "copilot_call" => "copilot",
            "grok_call" => "grok",
            "droid_call" => "droid",
            "cursor_call" => "cursor-agent",
            "opencode_call" => "opencode",
            _ => "",
        };
        let call = Call {
            prompt,
            model: host.args.get("model").and_then(Value::as_str).unwrap_or(""),
            cli: host
                .args
                .get("cli")
                .and_then(Value::as_str)
                .unwrap_or(default_cli),
            timeout: TIMEOUT_S as f64,
            effort: host
                .args
                .get("effort")
                .and_then(Value::as_str)
                .unwrap_or(""),
            windows: self.context.windows,
        };
        let env = &self.context.environment;
        let root = &self.context.root;
        let result = match host.method.as_str() {
            "claude_call" => {
                super::claude_call(&call, env, root, &mut self.state.help, &mut self.runtime)
            }
            "codex_call" => super::codex_call(&call, env, root, &mut self.runtime),
            "pi_call" => {
                let extensions: Vec<String> = serde_json::from_value(
                    host.args.get("extensions").cloned().unwrap_or(json!([])),
                )
                .map_err(|e| self.runtime.failure(e.to_string()))?;
                super::pi_call(&call, &extensions, env, root, &mut self.runtime)
            }
            "grok_call" => super::grok_call(&call, env, root, &mut self.runtime),
            "cursor_call" => super::cursor_call(&call, env, root, &mut self.runtime),
            "opencode_call" => super::opencode_call(
                &call,
                &mut self.state.oc_flags,
                env,
                root,
                &mut self.runtime,
            ),
            "copilot_call" => super::copilot_call(&call, env, root, &mut self.runtime),
            "droid_call" => super::droid_call(&call, env, root, &mut self.runtime),
            "hermes_call" => hermes::Hermes {
                environment: env,
                root,
                limits: self.context.hermes_limits.clone(),
            }
            .call(&call, &mut self.runtime),
            _ => Err(format!("unknown side-call method: {}", host.method)),
        };
        result.map_err(|message| self.runtime.failure(message))
    }
    pub fn run_raw(&mut self, prompt: &str, model: &str) -> Result<(Value, Option<f64>), Failure> {
        if self.state.local {
            let reply = match self.runtime.override_call(
                "fm_call",
                &json!({"prompt":prompt,"kwargs":{"schema":self.state.schema}}),
            )? {
                Some(reply) => reply,
                None => {
                    let call = Call {
                        prompt,
                        model: "",
                        cli: FM_CLI,
                        timeout: FM_TIMEOUT_S as f64,
                        effort: "",
                        windows: self.context.windows,
                    };
                    let (text, eff) = super::fm_call(
                        &call,
                        &self.state.schema,
                        &self.context.environment,
                        &self.context.root,
                        &mut self.runtime,
                    )
                    .map_err(|message| self.runtime.failure(message))?;
                    (text, Some(eff as f64))
                }
            };
            self.state.answered_locally = !reply.0.is_null();
            if self.state.answered_locally {
                return Ok(reply);
            }
        }
        if let Some(host) = self.state.host.clone() {
            let result = self.host(&host, prompt);
            return if let Some(host) = &self.state.note {
                noted(host, &self.context.root, result)
            } else {
                result
            };
        }
        match &mut self.fallback {
            Some(fallback) => fallback
                .run(prompt, model)
                .map(|(text, eff)| (json!(text), eff))
                .map_err(|message| Failure::Exception {
                    name: "RuntimeError".into(),
                    message,
                }),
            None => Ok((json!(""), Some(0.0))),
        }
    }
}
impl<R: Runtime> Runner for RoutedRunner<R> {
    fn run(&mut self, prompt: &str, model: &str) -> Result<(String, Option<f64>), String> {
        let (text, eff) = self.run_raw(prompt, model).map_err(|e| e.to_string())?;
        Ok((
            text.as_str().ok_or("side-call reply must be text")?.into(),
            eff,
        ))
    }
    fn local_answer(&self) -> Option<Value> {
        self.state.answered_locally.then(|| json!("fm"))
    }
}
#[derive(Default)]
pub struct NativeRuntime {
    last: Option<(String, Failure)>,
}
impl Runtime for NativeRuntime {
    fn run(
        &mut self,
        argv: &[String],
        cwd: &std::path::Path,
        env: &Map<String, Value>,
        input: Option<&str>,
        timeout: f64,
    ) -> io::Result<process::Output> {
        let result = match input {
            Some(input) => process::run_input(argv, cwd, env, input, timeout),
            None => process::run_environment(argv, cwd, env, timeout),
        };
        self.last = result.as_ref().err().map(|error| {
            let failure = Failure::from_io(error, timeout);
            (error.to_string(), failure)
        });
        result
    }
    fn failure(&self, message: String) -> Failure {
        self.last
            .as_ref()
            .filter(|(text, _)| *text == message)
            .map(|(_, failure)| failure.clone())
            .unwrap_or(Failure::Exception {
                name: "RuntimeError".into(),
                message,
            })
    }
}
