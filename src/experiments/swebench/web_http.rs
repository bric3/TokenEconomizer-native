use super::{
    Failure, Result, Runtime,
    optimizer::{capture_log, code_points},
    providers::{boolean, copy, literal},
    runner_calls::function,
};
use crate::python_json::Text;
use serde_json::{Value, json};

fn member(rt: &mut dyn Runtime, source: &str, name: &str, target: &str) -> Result<()> {
    rt.call("handler_member", json!([source, name, target]))
        .map(|_| ())
}
fn invoke(rt: &mut dyn Runtime, args: &[&str], kwargs: &[(&str, &str)]) -> Result<()> {
    rt.call("handler_invoke", json!([args, kwargs])).map(|_| ())
}
fn call(rt: &mut dyn Runtime, slot: &str, args: &[&str]) -> Result<()> {
    copy(rt, "function", slot)?;
    invoke(rt, args, &[])
}
fn method(rt: &mut dyn Runtime, source: &str, name: &str, args: &[&str]) -> Result<()> {
    member(rt, source, name, "function")?;
    invoke(rt, args, &[])
}
fn builtin(rt: &mut dyn Runtime, name: &str) -> Result<()> {
    rt.call("eval_builtin", json!([name])).map(|_| ())
}
fn handled(rt: &mut dyn Runtime, error: Option<&Failure>) -> Result<()> {
    rt.call("handler_exception", json!([error])).map(|_| ())
}
fn formatted(rt: &mut dyn Runtime, slot: &str) -> Result<Text> {
    code_points(rt.call("handler_format", json!([slot]))?)
}
fn text(rt: &mut dyn Runtime, slot: &str, value: Text) -> Result<()> {
    rt.call("logger_literal", json!([slot, value.0]))
        .map(|_| ())
}
fn report(rt: &mut dyn Runtime, value: Text) -> Result<()> {
    text(rt, "message", value)?;
    call(rt, "active_log", &["message"])
}
fn request(rt: &mut dyn Runtime) -> Result<()> {
    builtin(rt, "int")?;
    copy(rt, "integer", "function")?;
    member(rt, "self", "headers", "headers")?;
    literal(rt, "length_key", json!("content-length"))?;
    method(rt, "headers", "get", &["length_key"])?;
    copy(rt, "length_input", "called")?;
    if !boolean(rt, "length_input")? {
        literal(rt, "length_input", json!(0))?;
    }
    call(rt, "integer", &["length_input"])?;
    copy(rt, "length", "called")?;
    function(rt, "json.loads")?;
    copy(rt, "loads", "function")?;
    member(rt, "self", "rfile", "rfile")?;
    method(rt, "rfile", "read", &["length"])?;
    copy(rt, "raw", "called")?;
    literal(rt, "utf8", json!("utf-8"))?;
    method(rt, "raw", "decode", &["utf8"])?;
    copy(rt, "document", "called")?;
    if !boolean(rt, "document")? {
        literal(rt, "document", json!("{}"))?;
    }
    call(rt, "loads", &["document"])?;
    copy(rt, "request", "called")
}
fn reply(rt: &mut dyn Runtime) -> Result<()> {
    function(rt, "queue.Queue")?;
    literal(rt, "one", json!(1))?;
    invoke(rt, &[], &[("maxsize", "one")])?;
    copy(rt, "reply_queue", "called")?;
    member(rt, "self", "session", "session")?;
    member(rt, "session", "requests", "requests")?;
    member(rt, "requests", "put", "function")?;
    rt.call("provider_tuple", json!(["request", "reply_queue"]))?;
    invoke(rt, &["tuple"], &[])?;
    let received: Result<()> = (|| {
        member(rt, "reply_queue", "get", "function")?;
        member(rt, "self", "session", "session")?;
        member(rt, "session", "reply_timeout", "timeout")?;
        invoke(rt, &[], &[("timeout", "timeout")])?;
        copy(rt, "payload", "called")
    })();
    if let Err(error) = received {
        if !error.is_a("_queue.Empty") {
            return Err(error);
        }
        handled(rt, Some(&error))?;
        capture_log(rt)?;
        member(rt, "self", "session", "session")?;
        member(rt, "session", "reply_timeout", "timeout")?;
        let mut message = Text::from("webbench: no answer for an agent request within ");
        message.0.extend(formatted(rt, "timeout")?.0);
        message
            .0
            .extend("s -- replying with no files".chars().map(u32::from));
        report(rt, message)?;
        literal(rt, "payload", json!({"files":{}}))?;
        handled(rt, None)?;
    }
    Ok(())
}
fn respond(rt: &mut dyn Runtime) -> Result<()> {
    function(rt, "json.dumps")?;
    invoke(rt, &["payload"], &[])?;
    copy(rt, "encoded", "called")?;
    literal(rt, "utf8", json!("utf-8"))?;
    method(rt, "encoded", "encode", &["utf8"])?;
    copy(rt, "body", "called")?;
    literal(rt, "status", json!(200))?;
    method(rt, "self", "send_response", &["status"])?;
    literal(rt, "type_key", json!("content-type"))?;
    literal(rt, "type", json!("application/json"))?;
    method(rt, "self", "send_header", &["type_key", "type"])?;
    member(rt, "self", "send_header", "header")?;
    literal(rt, "length_key", json!("content-length"))?;
    builtin(rt, "str")?;
    copy(rt, "string", "function")?;
    builtin(rt, "len")?;
    invoke(rt, &["body"], &[])?;
    copy(rt, "length", "called")?;
    call(rt, "string", &["length"])?;
    copy(rt, "length_text", "called")?;
    call(rt, "header", &["length_key", "length_text"])?;
    method(rt, "self", "end_headers", &[])?;
    let wrote: Result<()> = (|| {
        member(rt, "self", "wfile", "wfile")?;
        method(rt, "wfile", "write", &["body"])
    })();
    if let Err(error) = wrote {
        if !error.is_a("builtins.BrokenPipeError") {
            return Err(error);
        }
        handled(rt, Some(&error))?;
        capture_log(rt)?;
        report(
            rt,
            Text::from(
                "webbench: the evaluator hung up before this answer was written (BrokenPipeError) -- the segment outran its ~300s request budget, so this attempt is LOST, not merely slow. Lower TE_TASK_TIMEOUT (default 280) so the segment is cut before the evaluator gives up.",
            ),
        )?;
    }
    Ok(())
}
pub fn post(rt: &mut dyn Runtime) -> Result<Value> {
    if let Err(error) = request(rt) {
        if !error.is_a("builtins.Exception") {
            return Err(error);
        }
        handled(rt, Some(&error))?;
        member(rt, "self", "send_error", "send_error")?;
        literal(rt, "status", json!(400))?;
        let mut message = Text::from("bad agent request: ");
        message.0.extend(formatted(rt, "caught_error")?.0);
        text(rt, "error_message", message)?;
        call(rt, "send_error", &["status", "error_message"])?;
        return Ok(Value::Null);
    }
    reply(rt)?;
    respond(rt)?;
    Ok(Value::Null)
}
