use super::context_dispatch;
use crate::job::{self, Channel};
use serde_json::{Value, json};
use std::path::PathBuf;
use tokeneconomizer::plugin::{
    context::LiveContext,
    hook::{self, Host, Runtime},
    hosts::session::Environment,
};

struct Transport {
    channel: Channel,
    custom: Value,
    patched_run: bool,
    process: Environment,
}
impl Transport {
    fn call(&self, method: &str, args: Value) -> Result<Value, String> {
        let reply = self.channel.call(method, args)?;
        if let Some(error) = reply["error"].as_str() {
            return Err(error.into());
        }
        Ok(reply["value"].clone())
    }
}
impl Runtime for Transport {
    fn detect(
        &mut self,
        payload: &Value,
        environment: &Environment,
        hint: Option<&str>,
    ) -> Result<Host, String> {
        let reply = self.call("detect", json!([payload, environment.env, hint]))?;
        self.custom = reply["custom"].clone();
        serde_json::from_value(reply["host"].clone()).map_err(|e| e.to_string())
    }
    fn host(
        &mut self,
        host: &Host,
        method: &str,
        payload: &Value,
        environment: &Environment,
        render: Option<(&LiveContext, &Value)>,
    ) -> Result<Value, String> {
        if self.custom[method] == true {
            self.call(
                "host",
                json!([
                    method,
                    payload,
                    environment.env,
                    render.map(|(ctx, _)| context_dispatch::snapshot(ctx)),
                    render.map(|(_, out)| out)
                ]),
            )
        } else {
            hook::host_method(host, method, payload, environment, &self.process, render)
        }
    }
    fn handle(&mut self, ctx: &mut LiveContext, payload: Value) -> Result<Value, String> {
        let reply = self.call("handle", json!([context_dispatch::snapshot(ctx), payload]))?;
        *ctx = context_dispatch::context(&reply["ctx"])?;
        Ok(reply["output"].clone())
    }
    fn run(
        &mut self,
        payload: Value,
        environment: &Environment,
        hint: Option<&str>,
    ) -> Result<Value, String> {
        if self.patched_run {
            self.call("run", json!([payload, environment.env, hint]))
        } else {
            hook::run(payload, environment, hint, self)
        }
    }
}
fn execute(op: &str, args: &Value, runtime: &mut Transport) -> Result<Value, String> {
    let environment = Environment {
        env: args["env"].as_object().cloned().unwrap_or_default(),
        home: PathBuf::from(args["home"].as_str().unwrap_or(".")),
        cwd: PathBuf::from(args["process_cwd"].as_str().unwrap_or(".")),
    };
    let pid = args["pid"].as_u64().unwrap_or(0) as u32;
    match op {
        "hook.run" => hook::run(
            args["payload"].clone(),
            &environment,
            args["host_hint"].as_str(),
            runtime,
        ),
        "hook._record_error" => {
            hook::record_error(
                &environment,
                args["error"].as_str().unwrap_or("NoneType: None\n"),
                pid,
            );
            Ok(Value::Null)
        }
        "hook.main" => {
            let input = if let Some(error) = args["input_error"].as_str() {
                Err(error.into())
            } else {
                serde_json::from_value::<Vec<u8>>(args["input"].clone()).map_err(|e| e.to_string())
            };
            hook::main(
                &serde_json::from_value::<Vec<String>>(args["argv"].clone())
                    .map_err(|e| e.to_string())?,
                input,
                &environment,
                pid,
                runtime,
            )
            .map(|out| json!(out))
        }
        _ => Err(format!("unknown hook operation: {op}")),
    }
}
pub(super) fn dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
    if op == "hook._truthy" {
        return Ok(Some(json!(hook::enabled_flag(&args["v"]))));
    }
    if op == "hook.resume" {
        return job::advance(
            args["job"].as_u64().ok_or("expected hook job")?,
            args.get("reply").cloned(),
        )
        .map(Some);
    }
    let (op, args) = (op.to_string(), args.clone());
    job::start_polling("hook", move |channel| {
        let mut runtime = Transport {
            channel,
            custom: json!({}),
            patched_run: args["patched_run"] == true,
            process: Environment {
                env: args["process_env"].as_object().cloned().unwrap_or_default(),
                home: PathBuf::from(args["home"].as_str().unwrap_or(".")),
                cwd: PathBuf::from(args["process_cwd"].as_str().unwrap_or(".")),
            },
        };
        match execute(&op, &args, &mut runtime) {
            Ok(value) => json!({"value":value}),
            Err(message) => json!({"raised":"RuntimeError","message":message}),
        }
    })
    .map(Some)
}
