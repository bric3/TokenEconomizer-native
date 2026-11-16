use crate::job::{self, Channel};
use serde_json::{Value, json};
use std::path::Path;
use tokeneconomizer::{
    loopguard::{
        judge as loop_judge,
        turn::{BackgroundJudge, TurnGuard},
    },
    plugin::judge::{self, Runtime},
    runner::Runner,
    store::Store,
};

struct Transport {
    channel: Channel,
    flags: Value,
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
struct Model {
    channel: Channel,
    role: String,
}
impl Model {
    fn call(&self, method: &str, args: Value) -> Result<Value, String> {
        let reply = self.channel.call(method, args)?;
        if let Some(error) = reply["error"].as_str() {
            return Err(error.into());
        }
        Ok(reply["value"].clone())
    }
}
impl Runner for Model {
    fn run(&mut self, prompt: &str, model: &str) -> Result<(String, Option<f64>), String> {
        serde_json::from_value(self.call("run", json!([self.role, prompt, model]))?)
            .map_err(|e| e.to_string())
    }
    fn estimate_effective(&mut self, prompt: &str, raw: &str) -> Result<f64, String> {
        self.call("estimate", json!([self.role, prompt, raw]))?
            .as_f64()
            .ok_or("invalid token estimate".into())
    }
    fn local_answer(&self) -> Option<Value> {
        let value = self.call("local_answer", json!([self.role])).ok()?;
        (value["answered"] == true).then(|| value["local"].clone())
    }
}
impl Runtime for Transport {
    fn read(&mut self, store: &Store, name: &str) -> Result<Value, String> {
        if self.flags["store_read"] == true {
            self.call("store_read", json!([name]))
        } else {
            store.read(name, Value::Null).map_err(|e| e.to_string())
        }
    }
    fn write(&mut self, store: &Store, name: &str, value: &Value) -> Result<(), String> {
        if self.flags["store_write"] == true {
            self.call("store_write", json!([name, value]))?;
        } else {
            store.write(name, value).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    fn append(&mut self, store: &Store, name: &str, value: &Value) -> Result<(), String> {
        if self.flags["store_append"] == true {
            self.call("store_append", json!([name, value]))?;
        } else {
            store.append(name, value).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    fn paused(&mut self) -> Result<bool, String> {
        Ok(self.call("paused", json!([]))? == true)
    }
    fn runner(&mut self, role: &str, local_only: bool) -> Result<Option<Box<dyn Runner>>, String> {
        Ok(if self.call("runner", json!([role, local_only]))? == true {
            Some(Box::new(Model {
                channel: self.channel.clone(),
                role: role.into(),
            }))
        } else {
            None
        })
    }
    fn judge(&mut self, digest: &str, runner: &mut dyn Runner) -> Result<Value, String> {
        if self.flags["judge"] == true {
            self.call("judge", json!([digest]))
        } else {
            Ok(loop_judge::llm_stuck_judge(digest, Some(runner), "agent"))
        }
    }
    fn nudge(&mut self, digest: &str, runner: &mut dyn Runner) -> Result<Value, String> {
        if self.flags["nudge"] == true {
            self.call("nudge", json!([digest]))
        } else {
            Ok(loop_judge::llm_nudge_writer(digest, Some(runner), "agent"))
        }
    }
    fn spawn_job(&mut self, session: &str, job: &str) -> Result<(), String> {
        self.call("spawn_job", json!([session, job])).map(|_| ())
    }
    fn spawn(&mut self, argv: &[String], log: &str) -> Result<Value, String> {
        self.call("spawn", json!([argv, log]))
    }
}
fn execute(op: &str, args: &Value, runtime: &mut dyn Runtime) -> Result<Value, String> {
    let store = Store::new(Path::new(
        args["base"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or("."),
    ))
    .map_err(|e| e.to_string())?;
    let sid = args["session_id"].as_str().unwrap_or("");
    let job = args["job_id"].as_str().unwrap_or("");
    let host = args["host"].as_str().unwrap_or("");
    let floor = args["local_floor"].as_f64().unwrap_or(0.9);
    Ok(match op {
        "live_judge.new" => {
            json!(judge::AsyncJudge::new(&store, sid, &args["threshold"], runtime)?.threshold)
        }
        "live_judge.run_job" => judge::run_job(&store, host, sid, job, floor, runtime)?,
        "live_judge.spawn_job" => judge::spawn_job(
            &store,
            host,
            sid,
            job,
            floor,
            args["synchronous"] == true,
            runtime,
        )?,
        _ => {
            let mut guard: TurnGuard =
                serde_json::from_value(args["guard"].clone()).map_err(|e| e.to_string())?;
            let mut judge = judge::AsyncJudge::new(&store, sid, &args["threshold"], runtime)?;
            match op {
                "live_judge.start" => {
                    judge.start(&mut guard, args["digest"].as_str().unwrap_or(""))?
                }
                "live_judge.poll" => json!(judge.poll(&mut guard)?),
                "live_judge.no_start" => judge::NoVerdict.start(&mut guard, "")?,
                "live_judge.no_poll" => json!(judge::NoVerdict.poll(&mut guard)?),
                _ => return Err(format!("unknown live judge operation: {op}")),
            }
        }
    })
}
pub(super) fn dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
    if !op.starts_with("live_judge.") {
        return Ok(None);
    }
    if op == "live_judge.resume" {
        return job::advance(
            args["job"].as_u64().ok_or("missing bridge job")?,
            Some(args["reply"].clone()),
        )
        .map(Some);
    }
    let (op, args) = (op.to_string(), args.clone());
    job::start("live_judge", move |channel| {
        let mut runtime = Transport {
            channel,
            flags: args["transport"].clone(),
        };
        match execute(&op, &args, &mut runtime) {
            Ok(value) => json!({"value":value}),
            Err(message) => json!({"raised":"RuntimeError","message":message}),
        }
    })
    .map(Some)
}
