use super::context_dispatch;
use crate::job::{self, Channel};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tokeneconomizer::{
    envelope::EnvelopeTracker,
    plugin::{
        cli::{self, Error, Result, Runtime},
        context::LiveContext,
        hosts::session::Environment,
        probe,
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
                parents: reply["parents"]
                    .as_array()
                    .map(|parents| {
                        parents
                            .iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
            });
        }
        Ok(reply["value"].clone())
    }
}
impl Runtime for Transport {
    fn context(&mut self, args: &Value) -> Result<LiveContext> {
        context_dispatch::context(&self.request("context", args.clone())?).map_err(Into::into)
    }
    fn pinned_context(&mut self, args: &Value, session: &Value) -> Result<LiveContext> {
        context_dispatch::context(&self.request("context_session", json!([args, session]))?)
            .map_err(Into::into)
    }
    fn global(&mut self, method: &str, args: Value) -> Result<Value> {
        self.request("global", json!([method, args]))
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
impl probe::Runtime for Transport {
    fn call(&mut self, method: &str, args: Value) -> Result<Value> {
        self.request("probe", json!([method, args]))
    }
    fn say(&mut self, text: &str) -> Result<()> {
        Runtime::say(self, text)
    }
}
fn execute(op: &str, args: &Value, runtime: &mut Transport) -> Result<Value> {
    let environment = Environment {
        env: args["process_env"].as_object().cloned().unwrap_or_default(),
        home: PathBuf::from(args["home"].as_str().unwrap_or(".")),
        cwd: PathBuf::from(args["process_cwd"].as_str().unwrap_or(".")),
    };
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
        "cli.cmd_microskills" => json!(cli::facts(&args["args"], runtime)?),
        "cli._microskills_forget" => json!(cli::forget_facts(&args["args"], runtime)?),
        "cli._short" => json!(cli::short(
            &args["text"],
            args["n"].as_i64().ok_or("expected abbreviation length")?
        )),
        "cli._session_ctx" => context_dispatch::snapshot(&cli::session_context(
            &args["args"],
            &args["session_id"],
            runtime,
        )?),
        "cli.cmd_fold" => json!(cli::fold_command(&args["args"], runtime)?),
        "cli.cmd_judge" => json!(cli::judge_command(&args["args"], runtime)?),
        "cli._when" => json!(cli::when(&args["ts"])?),
        "cli._print_history" => {
            cli::print_history(
                &context_dispatch::context(&args["ctx"])?,
                &args["doc"],
                runtime,
            )?;
            Value::Null
        }
        "cli.cmd_history" => json!(cli::history_command(&args["args"], runtime)?),
        "cli.cmd_repomap" => json!(cli::map_command(&args["args"], runtime)?),
        "cli.cmd_repos" => json!(cli::repositories(&args["args"], runtime)?),
        "cli._print_moves" => json!(cli::print_moves(&args["done"], runtime)?),
        "cli.cmd_toolprune" => json!(cli::toolprune(&args["args"], runtime)?),
        "cli._toolprune_show" => json!(cli::toolprune_show(
            &context_dispatch::context(&args["ctx"])?,
            &args["unt"],
            runtime
        )?),
        "cli._wait_and_apply" => json!(cli::wait_and_apply(&args["args"], runtime)?),
        "cli.cmd_doctor" => json!(cli::doctor(&args["args"], &environment, runtime)?),
        "cli.cmd_probe" => json!(probe::command(&args["args"], runtime)?),
        "probe.test_commands" => json!(probe::test_commands(&cli::words(&args["cmds"])?)),
        "probe._write" => {
            probe::write(
                &environment.path(Path::new(args["path"].as_str().ok_or("expected path")?)),
                args["text"].as_str().ok_or("expected text")?,
                args["mode"].as_u64().ok_or("expected file mode")? as u32,
            )?;
            Value::Null
        }
        "probe.make_repo" => json!(probe::make_repo(
            &environment.path(Path::new(
                args["root"].as_str().ok_or("expected probe root")?
            )),
            args["python"]
                .as_str()
                .ok_or("expected Python executable")?,
            runtime
        )?),
        "probe._plugin_root" => json!(probe::plugin_root(
            &environment,
            Path::new(
                args["module_file"]
                    .as_str()
                    .ok_or("expected package path")?
            )
        )?),
        "probe._session_env" => json!(probe::session_env(
            args["state"].as_str().ok_or("expected state path")?,
            args["trap"].as_str().ok_or("expected trap path")?,
            &environment.env
        )),
        "probe.run_session" => probe::run_session(
            args["repo"].as_str().ok_or("expected repository")?,
            &args["env"],
            args["plugin"].as_str().ok_or("expected plugin directory")?,
            &args["model"],
            args["cli"].as_str().ok_or("expected CLI")?,
            &args["timeout"],
            runtime,
        )?,
        "probe.inspect_session" => probe::inspect_session(
            args["sid"].as_str().ok_or("expected session ID")?,
            &environment,
            runtime,
        )?,
        "probe._wait_folded" => json!(probe::wait_folded(
            &context_dispatch::context(&args["ctx"])?.store,
            args["sid"].as_str().ok_or("expected session ID")?,
            args["timeout"].as_f64().ok_or("expected timeout")?,
            runtime
        )?),
        "probe._line" => json!(probe::line(&args["r"])?),
        "probe.run" => json!(probe::run(
            args["sessions"].as_i64().ok_or("expected session count")?,
            &args["model"],
            args["keep"].as_bool().ok_or("expected keep flag")?,
            args["cli"].as_str().ok_or("expected CLI")?,
            &environment,
            runtime
        )?),
        "cli._recent_host" => cli::recent_host(
            Path::new(args["cwd"].as_str().ok_or("expected working directory")?),
            &environment,
        )?,
        "cli._rtk_hook" => json!(cli::rtk_hooks(&environment.home)?),
        "cli.netloc" => json!(cli::netloc(args["url"].as_str().ok_or("expected URL")?)?),
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
