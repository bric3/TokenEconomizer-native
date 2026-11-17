use super::sidecall_dispatch::Transport;
use crate::job::{self, Channel};
use serde_json::{Map, Value, json};
use std::io;
use std::path::{Path, PathBuf};
use tokeneconomizer::plugin::{
    hosts::session::Environment,
    process::{self, Probe},
    toolcheck::{self, Checks},
};

impl toolcheck::Runtime for Transport {
    fn override_probe(&mut self, method: &str, args: Value) -> Result<Option<Value>, String> {
        if self.flags[method] != true {
            return Ok(None);
        }
        let reply = self.channel.call("probe", json!([method, args]))?;
        self.response_error(&reply)?;
        Ok(Some(reply["value"].clone()))
    }
    fn start_probe(
        &mut self,
        argv: &[String],
        cwd: &Path,
        env: &Map<String, Value>,
        new_session: bool,
    ) -> Result<Box<dyn Probe>, String> {
        if self.flags["popen"] != true {
            return process::start_probe(argv, cwd, env, new_session).map_err(|e| e.to_string());
        }
        let reply = self
            .channel
            .call("probe_start", json!([argv, cwd, env, new_session]))?;
        self.response_error(&reply)?;
        Ok(Box::new(ProxyProbe {
            channel: self.channel.clone(),
            pid: reply["value"]
                .as_u64()
                .and_then(|v| v.try_into().ok())
                .ok_or("invalid probe PID")?,
            windows: !new_session,
            closed: false,
        }))
    }
}
struct ProxyProbe {
    channel: Channel,
    pid: u32,
    windows: bool,
    closed: bool,
}
impl Probe for ProxyProbe {
    fn line(&mut self, timeout: f64) -> io::Result<Option<String>> {
        let reply = self
            .channel
            .call("probe_line", json!([timeout]))
            .map_err(io::Error::other)?;
        if let Some(error) = reply["error"].as_str() {
            return Err(io::Error::other(error.to_string()));
        }
        Ok(reply["value"].as_str().map(String::from))
    }
    fn kill_tree(&mut self) -> io::Result<()> {
        if !self.closed {
            process::kill_probe_group(self.pid, self.windows)?;
            let reply = self
                .channel
                .call("probe_stop", json!([]))
                .map_err(io::Error::other)?;
            if let Some(error) = reply["error"].as_str() {
                return Err(io::Error::other(error.to_string()));
            }
            self.closed = true;
        }
        Ok(())
    }
}
fn text<'a>(args: &'a Value, key: &str) -> &'a str {
    args[key].as_str().unwrap_or("")
}
fn strings(args: &Value, key: &str) -> Result<Vec<String>, String> {
    serde_json::from_value(args[key].clone()).map_err(|e| e.to_string())
}
fn execute(op: &str, args: &Value, runtime: &mut Transport) -> Result<Value, String> {
    let mut checks = Checks {
        environment: Environment {
            env: args["env"].as_object().cloned().unwrap_or_default(),
            cwd: PathBuf::from(text(args, "process_cwd")),
            home: PathBuf::from(text(args, "home")),
        },
        windows: args["windows"] == true,
        help: args["help"].as_object().cloned().unwrap_or_default(),
    };
    let cli = text(args, "cli");
    let timeout = |default| args["timeout"].as_f64().unwrap_or(default);
    let result = match op {
        "toolcheck._kill_tree" => {
            let mut probe = ProxyProbe {
                channel: runtime.channel.clone(),
                pid: args["pid"]
                    .as_u64()
                    .and_then(|v| v.try_into().ok())
                    .ok_or("invalid probe PID")?,
                windows: checks.windows,
                closed: false,
            };
            probe
                .kill_tree()
                .map(|_| Value::Null)
                .map_err(|e| e.to_string())
        }
        "toolcheck._init_tools" => checks
            .init_tools(&strings(args, "extra_args")?, cli, timeout(60.), runtime)
            .map(|v| json!(v)),
        "toolcheck.refresh_catalog" => checks.refresh_catalog(
            Path::new(text(args, "state_root")),
            cli,
            text(args, "host"),
            runtime,
        ),
        "toolcheck.copilot_capture" => checks.capture(
            &strings(args, "extra_args")?,
            args["cwd"].as_str().map(Path::new),
            cli,
            timeout(90.),
            runtime,
        ),
        "toolcheck._request_tools" => Ok(json!(toolcheck::request_tools(&args["body"]))),
        "toolcheck.measure" => {
            checks.measure(&strings(args, "tools")?, cli, text(args, "host"), runtime)
        }
        "toolcheck._prompt_tokens" => checks
            .prompt_tokens(&strings(args, "extra_args")?, cli, timeout(180.), runtime)
            .map(|v| json!(v)),
        "toolcheck._codex_prompt_chars" => {
            let triples: Vec<(Option<String>, String, String)> =
                serde_json::from_value(args["overrides"].clone()).map_err(|e| e.to_string())?;
            let triples: Vec<_> = triples
                .iter()
                .map(|(t, k, v)| (t.as_deref(), k.as_str(), v.as_str()))
                .collect();
            checks
                .codex_prompt_chars(&triples, cli, timeout(60.), runtime)
                .map(|v| json!(v))
        }
        "toolcheck.verify_codex" => checks.verify_codex(
            Path::new(text(args, "repo_root")),
            &strings(args, "capabilities")?,
            cli,
            timeout(30.),
            runtime,
        ),
        "toolcheck.verify_copilot" => checks.verify_copilot(
            Path::new(text(args, "repo_root")),
            &strings(args, "servers")?,
            cli,
            runtime,
        ),
        "toolcheck._copilot_version" => Ok(checks.copilot_version(cli, runtime)),
        _ => return Err(format!("unknown tool check operation: {op}")),
    };
    Ok(match result {
        Ok(value) => json!({"value":value,"help":checks.help}),
        Err(message) => json!({"raised":"RuntimeError","message":message,"help":checks.help}),
    })
}
pub(super) fn dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
    if op == "toolcheck.resume" {
        return job::advance(
            args["job"].as_u64().ok_or("expected job")?,
            args.get("reply").cloned(),
        )
        .map(Some);
    }
    let op = op.to_string();
    let args = args.clone();
    job::start_polling("toolcheck", move |channel| {
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
