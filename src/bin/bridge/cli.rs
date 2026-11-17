use super::context_dispatch;
use crate::job::{self, Channel};
use serde_json::{Value, json};
use std::path::Path;
use tokeneconomizer::{
    envelope::EnvelopeTracker,
    plugin::{
        cli::{self, Error, Result, Runtime},
        context::LiveContext,
    },
};

struct Transport {
    channel: Channel,
}
impl Transport {
    fn request(&self, method: &str, args: Value) -> Result<Value> {
        let reply = self.channel.call(method, args)?;
        if let Some(message) = reply["error"].as_str() {
            return Err(Error {
                kind: reply["kind"].as_str().unwrap_or("RuntimeError").into(),
                message: message.into(),
            });
        }
        Ok(reply["value"].clone())
    }
}
impl Runtime for Transport {
    fn context(&mut self, args: &Value) -> Result<LiveContext> {
        context_dispatch::context(&self.request("context", args.clone())?).map_err(Into::into)
    }
    fn call(&mut self, ctx: &LiveContext, method: &str, args: Value) -> Result<Value> {
        self.request(
            "service",
            json!([context_dispatch::snapshot(ctx), method, args]),
        )
    }
    fn say(&mut self, text: &str) -> Result<()> {
        self.request("say", json!(text)).map(|_| ())
    }
}
fn execute(op: &str, args: &Value, runtime: &mut Transport) -> Result<Value> {
    Ok(match op {
        "cli._split_argline" => json!(cli::split_argline(
            args["line"].as_str().ok_or("expected argument line")?
        )?),
        "cli._expand_argline" => json!(cli::expand_argline(&cli::words(&args["argv"])?)?),
        "cli._unwritable" => json!(cli::unwritable(
            Path::new(args["path"].as_str().ok_or("expected state path")?),
            args["pid"].as_u64().unwrap_or(0) as u32
        )),
        "cli._refuse_unwritable" => {
            let why = runtime.request("unwritable", json!([args["path"]]))?;
            if let Some(why) = why.as_str() {
                runtime.say(&cli::refusal(
                    Path::new(args["path"].as_str().ok_or("expected state path")?),
                    why,
                    &cli::words(&args["argv"])?,
                ))?;
                json!(1)
            } else {
                json!(0)
            }
        }
        "cli._config_view" => cli::config_view(&args["cfg"])?,
        "cli._session_note" => {
            cli::session_note(&context_dispatch::context(&args["ctx"])?, &args["env"])?
        }
        "cli._proposal" => json!(cli::proposal(
            &context_dispatch::context(&args["ctx"])?,
            &args["unt"]
        )?),
        "cli._status_explanations" => cli::status_explanations(
            &context_dispatch::context(&args["ctx"])?,
            &args["inj"],
            &args["unt"],
            &serde_json::from_value::<EnvelopeTracker>(args["tracker"].clone())
                .map_err(|e| e.to_string())?,
            &args["applied"],
            runtime,
        )?,
        "cli.cmd_status" => json!(cli::status(&args["args"], runtime)?),
        "cli.cmd_config" => json!(cli::config_command(&args["args"], runtime)?),
        _ => return Err(format!("unknown CLI operation: {op}").into()),
    })
}
pub(super) fn dispatch(op: &str, args: &Value) -> std::result::Result<Option<Value>, String> {
    if op == "cli.resume" {
        return job::advance(
            args["job"].as_u64().ok_or("expected CLI job")?,
            args.get("reply").cloned(),
        )
        .map(Some);
    }
    let (op, args) = (op.to_string(), args.clone());
    job::start_polling("cli", move |channel| {
        match execute(&op, &args, &mut Transport { channel }) {
            Ok(value) => json!({"value":value}),
            Err(error) => json!({"raised":error.kind,"message":error.message}),
        }
    })
    .map(Some)
}
