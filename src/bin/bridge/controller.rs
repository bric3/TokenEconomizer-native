use crate::job::{self, Channel};
use serde_json::{Value, json};
use std::{
    io,
    path::{Path, PathBuf},
};
use tokeneconomizer::{
    plugin::{
        context::LiveContext,
        controller::{self, JudgeMode, LiveController},
        hookqueue,
        hosts::session::{self, Environment},
        judge, process,
    },
    runner::Runner,
};

struct Transport {
    channel: Channel,
    flags: Value,
    limits: Value,
    context: Option<Value>,
}
impl Transport {
    fn call(&self, method: &str, args: Value) -> Result<Value, String> {
        let reply = self
            .channel
            .call(method, json!({"ctx":self.context,"args":args}))?;
        if let Some(error) = reply["error"].as_str() {
            return Err(error.into());
        }
        Ok(reply["value"].clone())
    }
    fn context_call(&self, method: &str, ctx: &LiveContext, args: Value) -> Result<Value, String> {
        self.call(
            method,
            json!([super::context_dispatch::snapshot(ctx), args]),
        )
    }
}
impl session::Runtime for Transport {
    fn program(&mut self, cli: &str) -> session::Result<Vec<String>> {
        serde_json::from_value(self.call("program", json!([cli]))?)
            .map_err(|e| session::Error::Invalid(e.to_string()))
    }
    fn run(
        &mut self,
        argv: &[String],
        cwd: &Path,
        env: Option<&serde_json::Map<String, Value>>,
        timeout: f64,
    ) -> io::Result<process::Output> {
        if self.flags["process_run"] == true {
            let out = self
                .call("process_run", json!([argv, cwd, env, timeout]))
                .map_err(io::Error::other)?;
            Ok(process::Output {
                code: out["code"].as_i64().unwrap_or(0) as i32,
                stdout: out["stdout"].as_str().unwrap_or("").into(),
                stderr: out["stderr"].as_str().unwrap_or("").into(),
            })
        } else if let Some(env) = env {
            process::run_environment(argv, cwd, env, timeout)
        } else {
            process::run(argv, cwd, timeout, false)
        }
    }
    fn ancestors(
        &mut self,
        limit: i64,
        pid: u32,
        windows: bool,
        cwd: &Path,
    ) -> session::Result<Vec<String>> {
        if self.flags["ancestors"] == true {
            serde_json::from_value(self.call("ancestors", json!([]))?)
                .map_err(|e| session::Error::Invalid(e.to_string()))
        } else {
            session::ancestor_args(limit, pid, windows, cwd, self)
        }
    }
}
impl judge::Runtime for Transport {
    fn paused(&mut self) -> Result<bool, String> {
        Ok(self.call("paused_bool", json!([]))? == true)
    }
    fn runner(&mut self, role: &str, local_only: bool) -> Result<Option<Box<dyn Runner>>, String> {
        Ok(if self.call("runner", json!([role, local_only]))? == true {
            Some(super::judge_dispatch::model(self.channel.clone(), role))
        } else {
            None
        })
    }
    fn spawn_job(&mut self, sid: &str, id: &str) -> Result<(), String> {
        self.call("spawn_job", json!([sid, id])).map(|_| ())
    }
    fn spawn(&mut self, argv: &[String], log: &str) -> Result<Value, String> {
        self.call("spawn", json!([argv, log]))
    }
}
impl controller::Runtime for Transport {
    fn dispatch(&mut self, ctl: &mut LiveController, method: &str) -> Result<Value, String> {
        self.context = Some(super::context_dispatch::snapshot(&ctl.ctx));
        if self.flags[method] == true {
            self.call("handler", json!([snapshot(ctl), method]))
        } else {
            ctl.invoke(method, self)
        }
    }
    fn queue_limits(&self, event: &str) -> hookqueue::Limits {
        let mut limits = hookqueue::Limits::for_event(event);
        limits.wait = self.limits["wait"].as_f64().unwrap_or(limits.wait);
        limits.max_events = self.limits["max_events"]
            .as_u64()
            .map(|v| v as usize)
            .unwrap_or(limits.max_events);
        limits.budget = self.limits["budget"].as_f64().unwrap_or(limits.budget);
        limits.idle = self.limits["idle"].as_f64().unwrap_or(limits.idle);
        limits
    }
    fn enqueue(&mut self, queue: &Path, payload: &Value) -> bool {
        if self.flags["enqueue"] == true {
            self.call("enqueue", json!([queue, payload]))
                .is_ok_and(|v| v == true)
        } else {
            hookqueue::enqueue(queue, payload)
        }
    }
    fn launcher(&mut self, ctx: &LiveContext) -> io::Result<()> {
        self.context_call("launcher", ctx, json!([]))
            .map(|_| ())
            .map_err(io::Error::other)
    }
    fn paused_detail(&mut self) -> Result<Value, String> {
        self.call("paused", json!([]))
    }
    fn call_name(&mut self) -> Result<String, String> {
        self.call("call_name", json!([]))?
            .as_str()
            .map(String::from)
            .ok_or("invalid side-call name".into())
    }
    fn local_routes(&mut self) -> Result<bool, String> {
        Ok(self.call("local_routes", json!([]))? == true)
    }
    fn map(&mut self, ctx: &LiveContext, method: &str, args: Value) -> Result<Value, String> {
        self.context_call("map", ctx, json!([method, args]))
    }
    fn history_notice(&mut self, ctx: &LiveContext) -> Result<Value, String> {
        self.context_call("history_notice", ctx, json!([]))
    }
    fn history_start(&mut self, ctx: &LiveContext) -> Result<(), String> {
        self.context_call("history_start", ctx, json!([]))
            .map(|_| ())
    }
    fn migration_start(&mut self, ctx: &LiveContext) -> Result<(), String> {
        self.context_call("migration_start", ctx, json!([]))
            .map(|_| ())
    }
    fn pending_sessions(
        &mut self,
        ctx: &LiveContext,
        exclude: &[String],
    ) -> Result<Vec<String>, String> {
        serde_json::from_value(self.context_call("pending_sessions", ctx, json!([exclude]))?)
            .map_err(|e| e.to_string())
    }
    fn spawn_folds(&mut self, ctx: &LiveContext, sessions: &[String]) -> Result<(), String> {
        self.context_call("folds", ctx, json!([sessions]))
            .map(|_| ())
    }
}
fn text<'a>(args: &'a Value, key: &str) -> &'a str {
    args[key].as_str().unwrap_or("")
}
fn snapshot(ctl: &LiveController) -> Value {
    json!({"ctx":super::context_dispatch::snapshot(&ctl.ctx),"p":ctl.p,"sid":ctl.sid,"replaying":ctl.replaying})
}
fn parse(args: &Value, new: bool) -> Result<LiveController, String> {
    let raw = if new { args } else { &args["self"] };
    let ctx = super::context_dispatch::context(&raw["ctx"])?;
    let environment = Environment {
        env: args["process_env"].as_object().cloned().unwrap_or_default(),
        home: PathBuf::from(text(args, "home")),
        cwd: PathBuf::from(text(args, "process_cwd")),
    };
    let mut ctl = LiveController::new(
        ctx,
        raw[if new { "payload" } else { "p" }].clone(),
        environment,
        args["parent_pid"].as_u64().unwrap_or(0) as u32,
        args["windows"] == true,
    );
    if !new {
        ctl.sid = text(raw, "sid").into();
        ctl.replaying = raw["replaying"] == true;
    }
    Ok(ctl)
}
fn execute(op: &str, args: &Value, runtime: &mut Transport) -> Result<Value, String> {
    if op == "controller._judge" {
        return Ok(json!({"value":match controller::judge_mode(
            text(args, "host"),
            &args["envelope"],
            runtime
        )? {
            JudgeMode::None => "none",
            JudgeMode::NoVerdict => "no_verdict",
            JudgeMode::Async => "async",
        }}));
    }
    if op == "controller.new" {
        return Ok(json!({"value":snapshot(&parse(args, true)?)}));
    }
    if op == "controller.handler" {
        return Ok(json!({"value":controller::handler(text(args, "event"))}));
    }
    let mut ctl = parse(args, false)?;
    runtime.context = Some(super::context_dispatch::snapshot(&ctl.ctx));
    let value = match op {
        "controller.handle" => ctl.handle(runtime),
        "controller.session_start"
        | "controller.user_prompt"
        | "controller.post_tool"
        | "controller.stop"
        | "controller.session_end" => ctl.invoke(op.strip_prefix("controller.").unwrap(), runtime),
        "controller._sess_name" => Ok(json!(ctl.sess_name())),
        "controller._session" => ctl.session(),
        "controller._cmd" => Ok(json!(ctl.cmd(text(args, "name")))),
        "controller._record_error" => {
            ctl.record_error(text(args, "error"));
            Ok(Value::Null)
        }
        "controller._log" => ctl
            .log(text(args, "kind"), args["fields"].clone())
            .map(|_| Value::Null),
        "controller._save" => {
            let mut s = args["s"].clone();
            let result = ctl.save(&mut s);
            result.map(|_| json!({"s":s,"value":null}))
        }
        "controller._toolprune_notices" => {
            let mut seen = args["seen"].clone();
            let out = ctl.toolprune_notices(&mut seen)?;
            Ok(json!({"seen":seen,"value":out}))
        }
        "controller._inject_map" => {
            let mut s = args["s"].clone();
            let mut seen = args["seen"].clone();
            let out = ctl.inject_map(
                &mut s,
                &mut seen,
                text(args, "block"),
                text(args, "event"),
                text(args, "how"),
            )?;
            Ok(json!({"s":s,"seen":seen,"value":out}))
        }
        "controller._close_turn" => {
            let mut s = args["s"].clone();
            let out = ctl.close_turn(&mut s, args["stopped_normally"] == true)?;
            Ok(json!({"s":s,"value":out}))
        }
        _ => return Err(format!("unknown controller operation: {op}")),
    };
    Ok(match value {
        Ok(value) => json!({"value":value,"self":snapshot(&ctl)}),
        Err(message) => json!({"raised":"RuntimeError","message":message,"self":snapshot(&ctl)}),
    })
}
pub(super) fn dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
    if !op.starts_with("controller.") {
        return Ok(None);
    }
    if op == "controller.constants" {
        return Ok(Some(json!({"TURN_HISTORY":controller::TURN_HISTORY})));
    }
    if op == "controller.resume" {
        return job::advance(
            args["job"].as_u64().ok_or("missing controller job")?,
            args.get("reply").cloned(),
        )
        .map(Some);
    }
    let (op, args) = (op.to_string(), args.clone());
    job::start_polling("controller", move |channel| {
        let mut runtime = Transport {
            channel,
            flags: args["transport"].clone(),
            limits: args["queue_limits"].clone(),
            context: None,
        };
        execute(&op, &args, &mut runtime)
            .unwrap_or_else(|message| json!({"raised":"RuntimeError","message":message}))
    })
    .map(Some)
}
