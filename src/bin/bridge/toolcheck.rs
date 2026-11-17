use super::sidecall_dispatch::Transport;
use crate::job;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tokeneconomizer::plugin::{
    hosts::session::Environment,
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
    fn capture(
        &mut self,
        extra_args: &[String],
        cwd: Option<&Path>,
        cli: &str,
        timeout: f64,
    ) -> Result<Value, String> {
        let reply = self.channel.call("probe", json!(["copilot_capture",{"extra_args":extra_args,"cwd":cwd,"cli":cli,"timeout":timeout}]))?;
        self.response_error(&reply)?;
        Ok(reply["value"].clone())
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
