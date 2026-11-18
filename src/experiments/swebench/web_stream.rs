use super::{
    Result, Runtime,
    optimizer::{code_points, string},
    providers::{boolean, copy, literal},
    runner_calls::{function, invoke, member},
};
use crate::{python_json::Text, value::truthy};
use serde_json::{Value, json};

fn set(rt: &mut dyn Runtime, name: &str, slot: &str) -> Result<()> {
    rt.call("stream_set", json!([name, slot])).map(|_| ())
}
fn item(rt: &mut dyn Runtime, slot: &str, index: i32, target: &str) -> Result<()> {
    rt.call("stream_item", json!([slot, index, target]))
        .map(|_| ())
}
fn capture_join(rt: &mut dyn Runtime) -> Result<()> {
    rt.call("context_capture_path", json!(["join", "join"]))
        .map(|_| ())
}
fn join(rt: &mut dyn Runtime, target: &str, args: &[&str]) -> Result<()> {
    rt.call("context_apply_path", json!(["join", target, args]))
        .map(|_| ())
}
fn points(rt: &mut dyn Runtime, slot: &str, text: Text) -> Result<()> {
    rt.call("logger_literal", json!([slot, text.0])).map(|_| ())
}
fn last_component(rt: &mut dyn Runtime) -> Result<()> {
    if let Some(text) = string(rt, "project")? {
        if let Some(separator) = text.0.iter().rposition(|&point| point == 47) {
            points(rt, "project", Text(text.0[separator + 1..].to_vec()))?;
        }
    } else {
        rt.call(
            "provider_method",
            json!(["project", "split", ["/"], "parts"]),
        )?;
        item(rt, "parts", -1, "project")?;
    }
    Ok(())
}
fn failure(rt: &mut dyn Runtime, text: Text) -> Result<Value> {
    rt.call("runner_raise", json!([text.0]))
}
fn format(rt: &mut dyn Runtime, slot: &str, repr: bool) -> Result<Text> {
    code_points(rt.call(
        if repr { "runner_repr" } else { "logger_format" },
        json!([slot]),
    )?)
}
fn begin(rt: &mut dyn Runtime) -> Result<Value> {
    rt.call("stream_list_function", json!([]))?;
    if !boolean(rt, "tasks")? {
        literal(rt, "tasks", json!([]))?;
    }
    invoke(rt, &["tasks"], &[])?;
    set(rt, "_stream", "called")?;

    copy(rt, "project", "repo")?;
    if !boolean(rt, "project")? {
        literal(rt, "project", json!(""))?;
    }
    last_component(rt)?;
    if !boolean(rt, "project")? {
        member(rt, "_stream", "stream")?;
        if boolean(rt, "stream")? {
            member(rt, "_stream", "stream")?;
            item(rt, "stream", 0, "first")?;
            rt.call(
                "provider_method",
                json!(["first", "get", ["repo", ""], "project"]),
            )?;
        } else {
            literal(rt, "project", json!(""))?;
        }
    }
    last_component(rt)?;
    if !boolean(rt, "project")? {
        return failure(
            rt,
            Text::from("webbench: no project to run; pass --repo <project>"),
        );
    }

    function(rt, "os.path.isdir")?;
    capture_join(rt)?;
    member(rt, "repo", "checkout")?;
    literal(rt, "projects", json!("projects"))?;
    join(rt, "project_dir", &["checkout", "projects", "project"])?;
    invoke(rt, &["project_dir"], &[])?;
    if !boolean(rt, "called")? {
        capture_join(rt)?;
        member(rt, "repo", "checkout")?;
        join(rt, "project_dir", &["checkout", "projects", "project"])?;
        let mut message = Text::from("webbench: no such project ");
        message.0.extend(format(rt, "project_dir", false)?.0);
        return failure(rt, message);
    }
    member(rt, "close_session", "function")?;
    invoke(rt, &[], &[])?;
    literal(rt, "broken", json!(false))?;
    set(rt, "_stream_broken", "broken")?;
    capture_join(rt)?;
    copy(rt, "root", "results_root")?;
    if !boolean(rt, "root")? {
        function(rt, "os.getcwd")?;
        invoke(rt, &[], &[])?;
        copy(rt, "root", "called")?;
    }
    let mut directory = Text::from("_webbench_");
    directory.0.extend(format(rt, "project", false)?.0);
    points(rt, "directory", directory)?;
    join(rt, "session_dir", &["root", "directory"])?;

    function(rt, "_EvalSession")?;
    member(rt, "repo", "checkout")?;
    member(rt, "retry", "retry")?;
    member(rt, "_stream", "stream")?;
    member(rt, "node", "node")?;
    member(rt, "task_timeout", "timeout")?;
    member(rt, "retry", "retries")?;
    literal(rt, "two", json!(2))?;
    rt.call("web_binary", json!(["add", "retries", "two", "attempts"]))?;
    rt.call(
        "web_binary",
        json!(["mul", "timeout", "attempts", "reply_timeout"]),
    )?;
    member(rt, "max_workers", "max_workers")?;
    invoke(
        rt,
        &["project", "checkout", "session_dir"],
        &[
            ("retry", "retry"),
            ("tasks", "stream"),
            ("node", "node"),
            ("reply_timeout", "reply_timeout"),
            ("max_workers", "max_workers"),
        ],
    )?;
    set(rt, "_session", "called")?;
    set(rt, "_session_project", "project")?;
    Ok(Value::Null)
}
fn session_for(rt: &mut dyn Runtime) -> Result<Value> {
    member(rt, "_session", "session")?;
    if truthy(&rt.call("provider_is_none", json!(["session"]))?) {
        return failure(
            rt,
            Text::from(
                "webbench: no evaluator session. The driver calls begin_stream() before the first task; a custom caller must do the same.",
            ),
        );
    }
    member(rt, "_session_project", "current")?;
    rt.call("web_compare", json!(["ne", "current", "project"]))?;
    if boolean(rt, "comparison")? {
        member(rt, "_session_project", "current")?;
        let mut message = Text::from("webbench: session is running ");
        message.0.extend(format(rt, "current", true)?.0);
        message
            .0
            .extend(" but the task belongs to ".chars().map(u32::from));
        message.0.extend(format(rt, "project", true)?.0);
        return failure(rt, message);
    }
    member(rt, "_session", "session")?;
    Ok(json!({"provider_ref":"session"}))
}
fn close(rt: &mut dyn Runtime) -> Result<Value> {
    member(rt, "_session", "session")?;
    if !truthy(&rt.call("provider_is_none", json!(["session"]))?) {
        member(rt, "_session", "session")?;
        rt.call("stream_close_function", json!([]))?;
        invoke(rt, &[], &[])?;
        literal(rt, "none", Value::Null)?;
        set(rt, "_session", "none")?;
        set(rt, "_session_project", "none")?;
    }
    Ok(Value::Null)
}
pub fn run(method: &str, rt: &mut dyn Runtime) -> Result<Value> {
    match method {
        "begin_stream" => begin(rt),
        "end_stream" => {
            member(rt, "close_session", "function")?;
            invoke(rt, &[], &[])?;
            Ok(Value::Null)
        }
        "session_for" => session_for(rt),
        "close_session" => close(rt),
        _ => unreachable!(),
    }
}
