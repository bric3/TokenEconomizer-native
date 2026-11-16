use crate::job::{self, Channel};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use tokeneconomizer::plugin::history::{self, Context, Error, Limits, Runtime};

struct Transport {
    channel: Channel,
    flags: Value,
    log: bool,
}
impl Transport {
    fn call(&self, method: &str, args: Value) -> history::Result<Value> {
        let reply = self.channel.call(method, args)?;
        if reply["fatal"] == true {
            return Err(Error::Cancelled);
        }
        if let Some(error) = reply["error"].as_str() {
            return Err(Error::Failed(error.into()));
        }
        Ok(reply["value"].clone())
    }
}
impl Runtime for Transport {
    fn host(&mut self, method: &str, args: Value) -> history::Result<Value> {
        self.call("host", json!([method, args]))
    }
    fn plan(&mut self, ctx: &Context, at: f64, limits: &Limits) -> history::Result<Value> {
        if self.flags["plan"] == true {
            self.call("plan", json!([at]))
        } else {
            history::plan(ctx, at, limits, self)
        }
    }
    fn fold(
        &mut self,
        sid: &str,
        path: Option<&str>,
        imported: bool,
        span: Option<(&Value, &Value)>,
    ) -> history::Result<Value> {
        self.call("fold", json!([sid, path, imported, span]))
    }
    fn pending(&mut self) -> history::Result<Vec<String>> {
        serde_json::from_value(self.call("pending_sessions", json!([]))?)
            .map_err(|e| Error::Failed(e.to_string()))
    }
    fn spawn(&mut self, argv: &[&str], log: &str) -> history::Result<Value> {
        self.call("spawn", json!([argv, log]))
    }
    fn say(&mut self, text: &str) -> history::Result<()> {
        if self.log {
            self.call("log", json!([text]))?;
        }
        Ok(())
    }
    fn clock(&mut self) -> history::Result<f64> {
        if self.flags["clock"] == true {
            self.call("clock", json!([]))?
                .as_f64()
                .ok_or_else(|| Error::Failed("history clock must be numeric".into()))
        } else {
            Ok(history::clock())
        }
    }
}
fn text<'a>(args: &'a Value, key: &str) -> &'a str {
    args[key].as_str().unwrap_or("")
}
fn context(args: &Value) -> history::Result<Context> {
    super::context_dispatch::context(&args["ctx"]).map_err(Error::Failed)
}
fn limits(args: &Value) -> Limits {
    let raw = &args["limits"];
    let mut limits = Limits::default();
    limits.max_bytes = raw["max_bytes"].as_u64().unwrap_or(limits.max_bytes);
    limits.max_attempts = raw["max_attempts"].as_i64().unwrap_or(limits.max_attempts);
    limits.heartbeat_stale = raw["heartbeat_stale"]
        .as_f64()
        .unwrap_or(limits.heartbeat_stale);
    limits.history_lock = raw["history_lock"].as_f64().unwrap_or(limits.history_lock);
    limits.fold_lock = raw["fold_lock"].as_f64().unwrap_or(limits.fold_lock);
    limits.stale = raw["stale"].as_f64().unwrap_or(limits.stale);
    limits
}
fn execute(op: &str, args: &Value, runtime: &mut Transport) -> history::Result<Value> {
    let ctx = context(args)?;
    let limits = limits(args);
    let at = args["now"].as_f64().unwrap_or_else(history::now);
    Ok(match op {
        "history._record_error" => {
            history::record_error(
                std::path::Path::new(text(args, "root")),
                text(args, "error"),
            );
            Value::Null
        }
        "history.read_state" => history::read_state(&ctx.store)?.unwrap_or(Value::Null),
        "history._num" => history::num(&args["v"], &args["default"]),
        "history._stale" => json!(history::stale(&args["doc"], at, limits.heartbeat_stale)),
        "history._ledger_size" => json!(history::ledger_size(&ctx.store)),
        "history._session_records" => json!(history::session_records(&ctx.store)?),
        "history._nothing_folded" => json!(history::nothing_folded(&ctx.store)?),
        "history._live_folds" => json!(history::live_folds(&ctx.store)?),
        "history._folded_by_import" => {
            json!(history::folded_by_import(&ctx.store, text(args, "sid"))?)
        }
        "history._refusal" => json!(history::refusal(&ctx, args["resuming"] == true)?),
        "history._identity" => {
            let mut cache: BTreeMap<String, Option<String>> =
                serde_json::from_value(args["cache"].clone())
                    .map_err(|e| Error::Failed(e.to_string()))?;
            let env = args["env"].as_object().cloned().unwrap_or_default();
            let value = history::identity(text(args, "cwd"), &env, &ctx.home, &mut cache);
            json!({"value":value,"cache":cache})
        }
        "history.is_this_repository" => {
            let mut cache: BTreeMap<String, Option<String>> =
                serde_json::from_value(if args["cache"].is_null() {
                    json!({})
                } else {
                    args["cache"].clone()
                })
                .map_err(|e| Error::Failed(e.to_string()))?;
            let value = history::is_this_repository(
                &ctx,
                text(args, "folder"),
                &args["cwd"],
                &mut cache,
                runtime,
            )?;
            json!({"value":value,"cache":cache})
        }
        "history._headless" => json!(history::headless(
            &args["head"],
            args["prompts"].as_f64().unwrap_or(0.0)
        )),
        "history.plan" => history::plan(&ctx, at, &limits, runtime)?,
        "history._still_eligible" => json!(history::still_eligible(
            &ctx,
            &args["item"],
            &limits,
            runtime
        )?),
        "history.replay_envelope" => json!(history::replay_envelope(
            &ctx,
            text(args, "transcript_path"),
            runtime
        )?),
        "history._update" => {
            let value = history::update(&ctx, args["timeout"].as_f64().unwrap_or(10.0), |doc| {
                let reply = runtime.call("change", json!([doc]))?;
                *doc = reply["doc"].clone();
                Ok(reply["value"] != false)
            })?;
            value.unwrap_or(Value::Null)
        }
        "history._save" => {
            history::save(&ctx, &args["doc"])?;
            Value::Null
        }
        "history._say" => {
            history::say(runtime, text(args, "text"))?;
            Value::Null
        }
        "history.run" => history::run(
            &ctx,
            args["auto"] == true,
            at,
            args["budget_s"].as_f64().unwrap_or(history::BUDGET_S),
            &limits,
            runtime,
        )?,
        "history._run_locked" => {
            let mut doc = args["doc"].clone();
            let value = history::run_locked(
                &ctx,
                &mut doc,
                args["resuming"] == true,
                at,
                args["budget_s"].as_f64().unwrap_or(history::BUDGET_S),
                &limits,
                runtime,
            )?;
            json!({"doc":doc,"value":value})
        }
        "history._import_one" => {
            let mut doc = args["doc"].clone();
            let mut skipped: Map<String, Value> = serde_json::from_value(args["skipped"].clone())
                .map_err(|e| Error::Failed(e.to_string()))?;
            history::import_one(
                &ctx,
                &mut doc,
                &args["item"],
                &mut skipped,
                &limits,
                runtime,
            )?;
            json!({"doc":doc,"skipped":skipped,"value":null})
        }
        "history.maybe_start" => history::maybe_start(&ctx, at, &limits, runtime)?,
        "history.notice" => history::notice(&ctx, &limits)?,
        "history.summary" => history::summary(&ctx)?,
        "history.describe" => {
            history::describe(&args["doc"], args["host"].as_str().unwrap_or("claude"))
        }
        _ => return Err(Error::Failed(format!("unknown history operation: {op}"))),
    })
}
pub(super) fn dispatch(op: &str, args: &Value) -> std::result::Result<Option<Value>, String> {
    if !op.starts_with("history.") {
        return Ok(None);
    }
    if op == "history.constants" {
        return Ok(Some(
            json!({"STATE":history::STATE,"LOG":history::LOG,"BUDGET_S":history::BUDGET_S,"HEARTBEAT_STALE_S":history::HEARTBEAT_STALE_S,"MAX_BYTES":history::MAX_BYTES,"HISTORY_LOCK_S":history::HISTORY_LOCK_S,"MAX_ATTEMPTS":history::MAX_ATTEMPTS,"HOST_LABEL":history::HOST_LABEL.into_iter().map(|(name,label)|(name.into(),json!(label))).collect::<Map<String,Value>>()}),
        ));
    }
    if op == "history.resume" {
        return job::advance(
            args["job"].as_u64().ok_or("missing history job")?,
            args.get("reply").cloned(),
        )
        .map(Some);
    }
    let (op, args) = (op.to_string(), args.clone());
    job::start_polling("history", move |channel| {
        let mut runtime = Transport {
            channel,
            flags: args["transport"].clone(),
            log: args["log"] == true,
        };
        match execute(&op, &args, &mut runtime) {
            Ok(value) => json!({"value":value}),
            Err(Error::Cancelled) => json!({"cancelled":true}),
            Err(error) => json!({"raised":"RuntimeError","message":error.to_string()}),
        }
    })
    .map(Some)
}
