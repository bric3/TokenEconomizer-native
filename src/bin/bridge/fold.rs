use crate::{
    job::{self, Channel},
    storage_dispatch,
};
use serde_json::{Value, json};
use std::{io, path::Path};
use tokeneconomizer::{
    plugin::{
        adapter::LiveAdapter,
        context::LiveContext,
        denylist,
        fold::{self, Error, Runtime},
    },
    runner::Runner,
    toolprune::stage::ToolingStage,
};

struct Transport {
    channel: Channel,
    flags: Value,
}
impl Transport {
    fn call(&self, method: &str, args: Value) -> fold::Result<Value> {
        let reply = self.channel.call(method, args)?;
        if reply["fatal"] == true {
            return Err(Error::Cancelled);
        }
        if let Some(error) = reply["error"].as_str() {
            return Err(match reply["error_kind"].as_str() {
                Some("FoldLater") => Error::Later,
                Some("ValueError") => Error::Value(error.into()),
                Some("OSError" | "PermissionError" | "FileNotFoundError") => {
                    Error::Io(io::Error::new(
                        match reply["error_kind"].as_str() {
                            Some("PermissionError") => io::ErrorKind::PermissionDenied,
                            Some("FileNotFoundError") => io::ErrorKind::NotFound,
                            _ => io::ErrorKind::Other,
                        },
                        error.to_string(),
                    ))
                }
                _ => Error::Failed(error.into()),
            });
        }
        Ok(reply["value"].clone())
    }
}
struct Proposer(Channel);
impl Runner for Proposer {
    fn run(&mut self, prompt: &str, model: &str) -> Result<(String, Option<f64>), String> {
        let reply = self.0.call("run", json!([prompt, model]))?;
        if let Some(error) = reply["error"].as_str() {
            return Err(error.into());
        }
        serde_json::from_value(reply["value"].clone()).map_err(|e| e.to_string())
    }
    fn estimate_effective(&mut self, prompt: &str, raw: &str) -> Result<f64, String> {
        let reply = self.0.call("estimate", json!([prompt, raw]))?;
        if let Some(error) = reply["error"].as_str() {
            return Err(error.into());
        }
        reply["value"]
            .as_f64()
            .ok_or_else(|| "invalid runner estimate".into())
    }
}
impl Runtime for Transport {
    fn host(&mut self, method: &str, args: Value) -> fold::Result<Value> {
        self.call("host", json!([method, args]))
    }
    fn proposer(&mut self, _ctx: &LiveContext) -> fold::Result<Option<Box<dyn Runner>>> {
        if self.call("proposer", json!([]))? == true {
            Ok(Some(Box::new(Proposer(self.channel.clone()))))
        } else {
            Ok(None)
        }
    }
    fn spawn(&mut self, argv: &[String], log: &str) -> fold::Result<Value> {
        self.call("spawn", json!([argv, log]))
    }
    fn blocks(&mut self, ctx: &LiveContext) -> fold::Result<bool> {
        if self.flags["import_blocks"] == true {
            Ok(self.call("import_blocks", json!([]))? == true)
        } else {
            fold::import_blocks(ctx)
        }
    }
    fn fold(&mut self, ctx: &LiveContext, sid: &str, path: Option<&str>) -> fold::Result<Value> {
        if self.flags["fold_locked"] == true {
            self.call("fold_locked", json!([sid, path]))
        } else {
            fold::fold_locked(ctx, sid, path, false, None, self)
        }
    }
    fn apply(
        &mut self,
        ctx: &LiveContext,
        root: &Path,
        items: &[String],
    ) -> fold::Result<Vec<String>> {
        if self.flags["deny"] == true {
            serde_json::from_value(self.call("apply", json!([root, items]))?)
                .map_err(|e| Error::Failed(e.to_string()))
        } else {
            Ok(denylist::apply(
                root,
                items,
                &ctx.store,
                &ctx.host,
                &mut denylist::NativeRuntime,
            )?)
        }
    }
    fn restore(&mut self, ctx: &LiveContext, items: &[String]) -> fold::Result<Vec<String>> {
        if self.flags["deny"] == true {
            serde_json::from_value(self.call("restore", json!([items]))?)
                .map_err(|e| Error::Failed(e.to_string()))
        } else {
            Ok(denylist::restore(
                &ctx.repo.root,
                &ctx.store,
                items,
                &ctx.host,
                &mut denylist::NativeRuntime,
            )?)
        }
    }
    fn git_tracked(&mut self, root: &Path, relative: &str) -> fold::Result<bool> {
        if self.flags["deny"] == true {
            Ok(self.call("git_tracked", json!([root, relative]))? == true)
        } else {
            Ok(denylist::git_tracked(root, relative))
        }
    }
}
fn text<'a>(args: &'a Value, key: &str) -> &'a str {
    args[key].as_str().unwrap_or("")
}
fn strings(value: &Value) -> fold::Result<Vec<String>> {
    if value.is_null() {
        Ok(Vec::new())
    } else {
        serde_json::from_value(value.clone()).map_err(|e| Error::Failed(e.to_string()))
    }
}
fn execute(op: &str, args: &Value, runtime: &mut Transport) -> fold::Result<Value> {
    let ctx = super::context_dispatch::context(&args["ctx"])?;
    let at = args["now"]
        .as_f64()
        .filter(|v| *v != 0.0)
        .unwrap_or_else(tokeneconomizer::plugin::history::now);
    let path = |key| Path::new(text(args, key));
    let adapter = || {
        serde_json::from_value::<LiveAdapter>(args["adapter"].clone())
            .map_err(|e| Error::Failed(e.to_string()))
    };
    let wait = args["lock_wait"].as_f64().unwrap_or(fold::FOLD_LOCK_WAIT_S);
    Ok(match op {
        "fold._run_dir" => json!(fold::run_dir(path("workdir"), path("trace_path"))?),
        "fold._microskills_config" => json!(fold::microskills_config(&ctx, text(args, "profile"))?),
        "fold._fold_microskills" => fold::fold_microskills(
            &ctx,
            &adapter()?,
            path("run_dir"),
            text(args, "session_id"),
            args.get("injected"),
            args["imported"] == true,
            runtime,
        )?,
        "fold.forget_microskills" => {
            fold::forget_microskills(&ctx, &strings(&args["ids"])?, args["undo"] == true)?
        }
        "fold.enough_evidence" => json!(fold::enough_evidence(
            args["sessions"].as_i64().unwrap_or(0),
            args["prompts"].as_i64().unwrap_or(0),
            &args["cfg"],
            args["high_risk"] == true
        )),
        "fold._fold_toolprune" => fold::fold_toolprune(
            &ctx,
            &mut adapter()?,
            path("run_dir"),
            text(args, "session_id"),
            args["transcript"].as_str(),
            args["workdir"].as_str().map(Path::new),
            args["prompts"].as_i64(),
            args["imported"] == true,
            runtime,
        )?,
        "fold._fold_toolprune_codex" => fold::fold_toolprune_codex(
            &ctx,
            &mut adapter()?,
            text(args, "session_id"),
            args["transcript"].as_str(),
            args["workdir"].as_str().map(Path::new),
            args["prompts"].as_i64(),
            runtime,
        )?,
        "fold._server_summary" => {
            let stage: ToolingStage = serde_json::from_value(args["stage"].clone())
                .map_err(|e| Error::Failed(e.to_string()))?;
            json!(fold::server_summary(
                &stage,
                &strings(&args["servers"])?,
                &serde_json::from_value(args["prompt_streaks"].clone())
                    .map_err(|e| Error::Failed(e.to_string()))?
            ))
        }
        "fold.unprotected" => json!(fold::unprotected(&ctx, &strings(&args["items"])?)),
        "fold._hold" => {
            let mut doc = args["doc"].clone();
            fold::hold(
                &mut doc,
                &args["prior"],
                &strings(&args["proposal"])?,
                path("root"),
            );
            json!({"doc":doc,"value":null})
        }
        "fold.last_activity" => json!(fold::last_activity(&ctx)?),
        "fold.apply_pending" => json!(fold::apply_pending(&ctx, at, runtime)?),
        "fold.schedule_pending" => json!(fold::schedule_pending(&ctx, runtime)?),
        "fold.import_blocks" => json!(fold::import_blocks(&ctx)?),
        "fold.fold_session" => fold::fold_session(
            &ctx,
            text(args, "session_id"),
            args["transcript_path"].as_str(),
            wait,
            runtime,
        )?,
        "fold.fold_locked" => fold::fold_locked(
            &ctx,
            text(args, "session_id"),
            args["transcript_path"].as_str(),
            args["imported"] == true,
            args["span"]
                .as_array()
                .and_then(|span| span.first().zip(span.get(1))),
            runtime,
        )?,
        "fold.pending_sessions" => json!(fold::pending_sessions(
            &ctx,
            &strings(&args["exclude"])?,
            args["stale_s"].as_f64().unwrap_or(fold::STALE_S),
            at,
            runtime
        )?),
        "fold.spawn_fold" => {
            fold::spawn_fold(&ctx, &strings(&args["session_ids"])?, wait, runtime)?
        }
        _ => return Err(Error::Failed(format!("unknown fold operation: {op}"))),
    })
}
fn response(result: fold::Result<Value>) -> Value {
    match result {
        Ok(value) => json!({"value":value}),
        Err(Error::Io(e)) => storage_dispatch::response(Err(e)),
        Err(Error::UnknownIds(ids)) => json!({"raised":"KeyError","message":ids}),
        Err(Error::Busy) => json!({"raised":"Busy","message":Error::Busy.to_string()}),
        Err(Error::Later) => json!({"raised":"FoldLater","message":Error::Later.to_string()}),
        Err(Error::Cancelled) => json!({"cancelled":true}),
        Err(Error::Value(message)) => json!({"raised":"ValueError","message":message}),
        Err(Error::Failed(message)) => json!({"raised":"RuntimeError","message":message}),
    }
}
pub(super) fn dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
    if !op.starts_with("fold.") {
        return Ok(None);
    }
    if op == "fold.constants" {
        return Ok(Some(
            json!({"STALE_S":fold::STALE_S,"FOLD_LOCK_WAIT_S":fold::FOLD_LOCK_WAIT_S,"PENDING_MAX_S":fold::PENDING_MAX_S,"WAITER_POLL_S":fold::WAITER_POLL_S,"CODEX_HIGH_RISK":fold::CODEX_HIGH_RISK,"HISTORY":fold::HISTORY,"HISTORY_BLOCKING":fold::HISTORY_BLOCKING}),
        ));
    }
    if op == "fold.resume" {
        return job::advance(
            args["job"].as_u64().ok_or("missing fold job")?,
            args.get("reply").cloned(),
        )
        .map(Some);
    }
    let (op, args) = (op.to_string(), args.clone());
    job::start_polling("fold", move |channel| {
        let mut runtime = Transport {
            channel,
            flags: args["transport"].clone(),
        };
        response(execute(&op, &args, &mut runtime))
    })
    .map(Some)
}
