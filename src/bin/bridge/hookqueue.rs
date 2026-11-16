use crate::job::{self, Channel};
use serde_json::{Map, Value, json};
use std::path::Path;
use tokeneconomizer::plugin::hookqueue::{self, Hook, Limits, Replay, Runtime};

struct Transport {
    channel: Channel,
    enqueue_callback: bool,
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
    fn prepare_replay(&mut self, payload: &Value, event: &str) -> Result<Replay, String> {
        let value = self.call("prepare_replay", json!([payload, event]))?;
        Ok(Replay {
            session_id: value["session_id"].as_str().unwrap_or("").into(),
            has_handler: value["handler"] == true,
        })
    }
    fn replay_handler(&mut self, replaying: bool) -> Result<(), String> {
        self.call("replay_handler", json!([replaying])).map(|_| ())
    }
    fn record_error(&mut self, error: &str) -> Result<(), String> {
        self.call("record_error", json!([error])).map(|_| ())
    }
    fn handler(&mut self) -> Result<Value, String> {
        self.call("handler", json!([]))
    }
    fn enqueue(&mut self, queue: &Path, payload: &Value) -> bool {
        if self.enqueue_callback {
            self.call("enqueue", json!([queue, payload]))
                .is_ok_and(|v| v == true)
        } else {
            hookqueue::enqueue(queue, payload)
        }
    }
}
fn text<'a>(args: &'a Value, key: &str) -> &'a str {
    args[key].as_str().unwrap_or("")
}
fn execute(op: &str, args: &Value, runtime: &mut dyn Runtime) -> Result<Value, String> {
    let mut limits = Limits::for_event(text(args, "event"));
    limits.wait = args["wait"].as_f64().unwrap_or(limits.wait);
    limits.budget = args["budget"].as_f64().unwrap_or(limits.budget);
    limits.max_events = args["max_events"]
        .as_u64()
        .map(|v| v as usize)
        .unwrap_or(limits.max_events);
    limits.idle = args["idle"].as_f64().unwrap_or(limits.idle);
    Ok(match op {
        "hookqueue._safe" => json!(hookqueue::safe(&args["text"])),
        "hookqueue._queues" => json!(hookqueue::queues(
            text(args, "repomap"),
            text(args, "event")
        )),
        "hookqueue.enqueue" => json!(hookqueue::enqueue(
            Path::new(text(args, "queue")),
            &args["payload"]
        )),
        "hookqueue._prune" => {
            hookqueue::prune(Path::new(text(args, "base")), limits.idle);
            Value::Null
        }
        "hookqueue._replay" => {
            hookqueue::replay(text(args, "session_id"), &args["payload"], runtime)?;
            Value::Null
        }
        "hookqueue.drain" => {
            hookqueue::drain(
                text(args, "session_id"),
                Path::new(text(args, "queue")),
                limits.max_events,
                limits.budget,
                runtime,
            )?;
            Value::Null
        }
        "hookqueue.run" => hookqueue::run(
            Hook {
                root: Path::new(text(args, "root")),
                host: text(args, "host"),
                session_id: text(args, "session_id"),
                repomap: text(args, "repomap"),
                event: text(args, "event"),
                payload: &args["payload"],
            },
            &limits,
            runtime,
        )?,
        _ => return Err(format!("unknown hook queue operation: {op}")),
    })
}
pub(super) fn dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
    if !op.starts_with("hookqueue.") {
        return Ok(None);
    }
    if op == "hookqueue.constants" {
        return Ok(Some(
            json!({"WAITS":hookqueue::WAITS.into_iter().map(|(name,wait)|(name.into(),json!(wait))).collect::<Map<String,Value>>(),"QUEUED":hookqueue::QUEUED,"DRAIN_MAX_EVENTS":hookqueue::DRAIN_MAX_EVENTS,"DRAIN_BUDGET_S":hookqueue::DRAIN_BUDGET_S,"IDLE_S":hookqueue::IDLE_S}),
        ));
    }
    if op == "hookqueue.resume" {
        return job::advance(
            args["job"].as_u64().ok_or("missing queue job")?,
            args.get("reply").cloned(),
        )
        .map(Some);
    }
    let (op, args) = (op.to_string(), args.clone());
    job::start_polling("hookqueue", move |channel| {
        let mut runtime = Transport {
            channel,
            enqueue_callback: args["enqueue_callback"] == true,
        };
        match execute(&op, &args, &mut runtime) {
            Ok(value) => json!({"value":value}),
            Err(message) => json!({"raised":"RuntimeError","message":message}),
        }
    })
    .map(Some)
}
