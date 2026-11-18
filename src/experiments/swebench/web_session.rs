use super::{
    Result, Runtime,
    optimizer::{capture_log, code_points},
    providers::{boolean, copy, literal},
    runner_calls::{function, invoke, member},
};
use crate::python_json::Text;
use serde_json::{Value, json};

fn now(rt: &mut dyn Runtime) -> Result<()> {
    function(rt, "time.time")?;
    invoke(rt, &[], &[])?;
    copy(rt, "now", "called")
}
fn binary(rt: &mut dyn Runtime, operation: &str, a: &str, b: &str, target: &str) -> Result<()> {
    rt.call("web_binary", json!([operation, a, b, target]))
        .map(|_| ())
}
fn compare(rt: &mut dyn Runtime, operation: &str, a: &str, b: &str) -> Result<bool> {
    rt.call("web_compare", json!([operation, a, b]))?;
    boolean(rt, "comparison")
}
fn item(rt: &mut dyn Runtime, target: &str) -> Result<()> {
    rt.call("stream_item", json!(["event", "kind", target]))
        .map(|_| ())
}
fn formatted(rt: &mut dyn Runtime, slot: &str) -> Result<Text> {
    code_points(rt.call("logger_format", json!([slot]))?)
}
fn append(text: &mut Text, suffix: &str) {
    text.0.extend(suffix.chars().map(u32::from));
}
fn unexpected(rt: &mut dyn Runtime) -> Result<()> {
    capture_log(rt)?;
    member(rt, "project", "project")?;
    let mut message = Text::from("webbench ");
    message.0.extend(formatted(rt, "project")?.0);
    append(&mut message, ": unexpected ");
    item(rt, "event_kind")?;
    message.0.extend(formatted(rt, "event_kind")?.0);
    append(&mut message, " event ");
    rt.call("provider_method", json!(["event", "get", ["task"], "task"]))?;
    message.0.extend(formatted(rt, "task")?.0);
    append(&mut message, " #");
    rt.call(
        "provider_method",
        json!(["event", "get", ["attempt"], "attempt"]),
    )?;
    message.0.extend(formatted(rt, "attempt")?.0);
    append(&mut message, " while waiting for ");
    message.0.extend(formatted(rt, "kind")?.0);
    rt.call("logger_report", json!([message.0])).map(|_| ())
}
fn empty(request: bool, rt: &mut dyn Runtime) -> Result<Value> {
    if request {
        literal(rt, "none", Value::Null)?;
        rt.call("provider_tuple", json!(["none", "none"]))?;
        Ok(json!({"provider_ref":"tuple"}))
    } else {
        Ok(Value::Null)
    }
}
pub fn poll(request: bool, rt: &mut dyn Runtime) -> Result<Value> {
    now(rt)?;
    binary(rt, "add", "now", "timeout", "deadline")?;
    loop {
        now(rt)?;
        binary(rt, "sub", "deadline", "now", "left")?;
        literal(rt, "zero", json!(0))?;
        if compare(rt, "le", "left", "zero")? {
            return empty(request, rt);
        }
        let received: Result<()> = (|| {
            member(rt, if request { "requests" } else { "events" }, "queue")?;
            rt.call("session_get_function", json!([]))?;
            literal(rt, "interval", json!(5.0))?;
            // min(left, 5.0) compares the second value with the first and retains its reference.
            if compare(rt, "lt", "interval", "left")? {
                copy(rt, "wait", "interval")?;
            } else {
                copy(rt, "wait", "left")?;
            }
            invoke(rt, &[], &[("timeout", "wait")])
        })();
        if let Err(error) = received {
            if !error.is_a("_queue.Empty") {
                return Err(error);
            }
            member(rt, "finished", "finished")?;
            if boolean(rt, "finished")? {
                return empty(request, rt);
            }
            continue;
        }
        if request {
            return Ok(json!({"provider_ref":"called"}));
        }
        copy(rt, "event", "called")?;
        item(rt, "event_kind")?;
        literal(rt, "exit", json!("exit"))?;
        if compare(rt, "eq", "event_kind", "exit")? {
            return empty(false, rt);
        }
        item(rt, "event_kind")?;
        if compare(rt, "eq", "event_kind", "kind")? {
            return Ok(json!({"provider_ref":"event"}));
        }
        unexpected(rt)?;
    }
}
