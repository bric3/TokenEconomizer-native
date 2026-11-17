use super::{context_dispatch, sidecall_dispatch::Transport};
use crate::job;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tokeneconomizer::plugin::{
    hosts::session::Environment,
    process,
    repomap::{self, RepoMap},
    sidecall::Runtime,
};

impl repomap::Runtime for Transport {
    fn run_generator(
        &mut self,
        argv: &[String],
        cwd: &Path,
        env: &serde_json::Map<String, Value>,
        timeout: f64,
    ) -> std::io::Result<process::Output> {
        if self.flags["run"] == true {
            self.run(argv, cwd, env, None, timeout)
        } else {
            process::run_group(argv, cwd, env, timeout)
        }
    }
    fn spawn(&mut self, args: &[&str], log: &str) -> Result<Value, String> {
        let reply = self.channel.call("spawn", json!([args, log]))?;
        self.response_error(&reply)?;
        Ok(reply["value"].clone())
    }
}
fn text<'a>(args: &'a Value, key: &str) -> &'a str {
    args[key].as_str().unwrap_or("")
}
fn execute(op: &str, args: &Value, runtime: &mut Transport) -> Result<Value, String> {
    if op == "repomap.prompt_idents" {
        return Ok(json!(repomap::prompt_idents(text(args, "text"))));
    }
    let ctx = context_dispatch::context(&args["ctx"])?;
    let environment = Environment {
        env: args["process_env"].as_object().cloned().unwrap_or_default(),
        cwd: PathBuf::from(text(args, "process_cwd")),
        home: PathBuf::from(text(args, "home")),
    };
    let maps = RepoMap {
        ctx: &ctx,
        environment: &environment,
        generator: Path::new(text(args, "generator")),
        interpreter: args["interpreter"].as_str(),
    };
    Ok(match op {
        "repomap._repo_dir" => json!(maps.repo_dir()),
        "repomap._state_path" => json!(maps.state_path()),
        "repomap._cache_dir" => json!(maps.cache_dir()),
        "repomap._gen_env" => json!(maps.gen_env()),
        "repomap.read_state" => maps.read_state(),
        "repomap._write_state" => {
            maps.write_state(&args["d"])?;
            Value::Null
        }
        "repomap.head_commit" => json!(maps.head_commit(Path::new(text(args, "root")), runtime)),
        "repomap._has_aider" => json!(maps.has_aider(
            text(args, "python"),
            args["env"].as_object().ok_or("expected environment")?,
            runtime
        )),
        "repomap._aider_command_python" => json!(maps.aider_command_python(runtime)?),
        "repomap.find_python" => json!(maps.find_python(runtime)?),
        "repomap.generate" => json!(
            maps.generate(
                text(args, "python"),
                &serde_json::from_value::<Vec<String>>(args["idents"].clone())
                    .map_err(|e| e.to_string())?,
                args["timeout"]
                    .as_f64()
                    .unwrap_or(repomap::REFRESH_TIMEOUT_S),
                runtime
            )?
        ),
        "repomap.refresh" => maps.refresh(runtime)?,
        "repomap.stale" => json!(maps.stale(runtime)),
        "repomap.spawn_refresh" => maps.spawn_refresh(runtime)?,
        "repomap.cached_block" => json!(maps.cached_block()?),
        "repomap.for_prompt" => json!(maps.for_prompt(text(args, "prompt"), runtime)?),
        _ => return Err(format!("unknown repository map operation: {op}")),
    })
}
pub(super) fn dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
    if op == "repomap.constants" {
        return Ok(Some(
            json!({"HEADER":repomap::HEADER,"PROBE":repomap::PROBE,"REFRESH_TIMEOUT_S":repomap::REFRESH_TIMEOUT_S}),
        ));
    }
    if op == "repomap.resume" {
        return job::advance(
            args["job"].as_u64().ok_or("expected job")?,
            args.get("reply").cloned(),
        )
        .map(Some);
    }
    let (op, args) = (op.to_string(), args.clone());
    job::start_polling("repomap", move |channel| {
        execute(
            &op,
            &args,
            &mut Transport {
                channel,
                flags: args["transport"].clone(),
                last: None,
            },
        )
        .map(|value| json!({"value":value}))
        .unwrap_or_else(|message| json!({"raised":"RuntimeError","message":message}))
    })
    .map(Some)
}
