use super::{
    Result, Runtime,
    optimizer::{capture_log, code_points, string},
    providers::{boolean, copy, literal},
    runner_calls::{invoke, member},
};
use crate::{python_json::Text, value::truthy};
use regex::Regex;
use serde_json::{Value, json};
use std::sync::LazyLock;

const ATTEMPT: &str = r"^\[(?P<project>[^\]]+)\]\s+(?P<task>init|task-\d+)\s+(?P<ord>\d+)(?:st|nd|rd|th)\s+Attempt\s+(?P<mark>Started|✅|❌)";

enum Attempt {
    Captured(Value),
    Bound,
}

fn attribute(rt: &mut dyn Runtime, source: &str, name: &str, target: &str) -> Result<()> {
    rt.call("stdout_member", json!([source, name, target]))
        .map(|_| ())
}
fn call(rt: &mut dyn Runtime, function: &str, args: &[&str]) -> Result<()> {
    copy(rt, "function", function)?;
    invoke(rt, args, &[])
}
fn set(rt: &mut dyn Runtime, name: &str, slot: &str) -> Result<()> {
    rt.call("stream_set", json!([name, slot])).map(|_| ())
}
fn field(rt: &mut dyn Runtime, name: &str, slot: &str) -> Result<()> {
    rt.call("provider_collect", json!([name, slot])).map(|_| ())
}
fn strip(rt: &mut dyn Runtime, end_only: bool, target: &str) -> Result<()> {
    if let Some(text) = string(rt, "line")? {
        let whitespace = |point: &u32| {
            char::from_u32(*point)
                .is_some_and(|c| c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}'))
        };
        let start = if end_only {
            0
        } else {
            text.0.iter().take_while(|p| whitespace(p)).count()
        };
        let end = text
            .0
            .iter()
            .rposition(|p| !whitespace(p))
            .map_or(start, |i| i + 1);
        if start == 0 && end == text.0.len() {
            copy(rt, target, "line")?;
        } else {
            rt.call("logger_literal", json!([target, &text.0[start..end]]))?;
        }
    } else {
        rt.call(
            "provider_method",
            json!([
                "line",
                if end_only { "rstrip" } else { "strip" },
                [],
                target
            ]),
        )?;
    }
    Ok(())
}
fn group(rt: &mut dyn Runtime, attempt: &Attempt, name: &str, target: &str) -> Result<()> {
    match attempt {
        Attempt::Captured(groups) => literal(rt, target, groups[name].clone()),
        Attempt::Bound => rt.call("stdout_group", json!([name, target])).map(|_| ()),
    }
}
fn compare(rt: &mut dyn Runtime, a: &str, b: &str) -> Result<bool> {
    rt.call("web_compare", json!(["eq", a, b]))?;
    boolean(rt, "comparison")
}
fn matched(rt: &mut dyn Runtime) -> Result<Option<Attempt>> {
    static PATTERN: LazyLock<Regex> = LazyLock::new(|| {
        // Python 3.15 includes Unicode 17's Tolong Siki digits; regex uses Unicode 16.
        Regex::new(
            &ATTEMPT
                .replace(r"\s", r"[\s\x1c-\x1f]")
                .replace(r"\d", r"[\d\u{11de0}-\u{11de9}]"),
        )
        .unwrap()
    });
    let pattern = rt.call("stdout_capture_match", json!([]))?;
    strip(rt, false, "stripped")?;
    if pattern == json!([ATTEMPT, 32])
        && let Some(text) = string(rt, "stripped")?
    {
        // Surrogates cannot enter the task, ordinal or mark captures.
        return Ok(PATTERN.captures(&text.lossy()).map(|captures| {
            Attempt::Captured(json!({
                "task":&captures["task"], "ord":&captures["ord"], "mark":&captures["mark"],
            }))
        }));
    }
    rt.call("stdout_match", json!([]))?;
    Ok(boolean(rt, "match")?.then_some(Attempt::Bound))
}
fn emit(rt: &mut dyn Runtime, groups: Attempt) -> Result<()> {
    group(rt, &groups, "mark", "mark")?;
    literal(rt, "started", json!("Started"))?;
    let started = compare(rt, "mark", "started")?;
    literal(
        rt,
        "kind",
        json!(if started { "started" } else { "verdict" }),
    )?;
    member(rt, "events", "queue")?;
    attribute(rt, "queue", "put", "put")?;
    literal(rt, "output", json!({}))?;
    field(rt, "kind", "kind")?;
    group(rt, &groups, "task", "task")?;
    field(rt, "task", "task")?;
    rt.call("stdout_capture_integer", json!([]))?;
    group(rt, &groups, "ord", "ordinal")?;
    rt.call("stdout_integer", json!([]))?;
    field(rt, "attempt", "attempt")?;
    group(rt, &groups, "mark", "mark")?;
    rt.call("provider_constant", json!(["ok_mark", "_MARK_OK"]))?;
    rt.call("web_compare", json!(["eq", "mark", "ok_mark"]))?;
    copy(rt, "ok", "comparison")?;
    field(rt, "ok", "ok")?;
    strip(rt, true, "line_text")?;
    field(rt, "line", "line_text")?;
    call(rt, "put", &["output"])
}
pub fn read(rt: &mut dyn Runtime) -> Result<Value> {
    member(rt, "proc", "proc")?;
    attribute(rt, "proc", "stdout", "stdout")?;
    rt.call("stdout_iter", json!([]))?;
    while truthy(&rt.call("stdout_next", json!([]))?) {
        member(rt, "_log_fh", "log")?;
        attribute(rt, "log", "write", "write")?;
        call(rt, "write", &["line"])?;
        member(rt, "_log_fh", "log")?;
        attribute(rt, "log", "flush", "flush")?;
        call(rt, "flush", &[])?;
        if let Some(groups) = matched(rt)? {
            emit(rt, groups)?;
        }
    }
    member(rt, "proc", "proc")?;
    attribute(rt, "proc", "wait", "wait")?;
    call(rt, "wait", &[])?;
    member(rt, "proc", "proc")?;
    attribute(rt, "proc", "returncode", "code")?;
    set(rt, "exit_code", "code")?;
    literal(rt, "finished", json!(true))?;
    set(rt, "finished", "finished")?;
    member(rt, "events", "queue")?;
    attribute(rt, "queue", "put", "put")?;
    literal(rt, "output", json!({}))?;
    literal(rt, "kind", json!("exit"))?;
    field(rt, "kind", "kind")?;
    member(rt, "exit_code", "code")?;
    field(rt, "code", "code")?;
    call(rt, "put", &["output"])?;
    capture_log(rt)?;
    member(rt, "project", "project")?;
    let mut message = Text::from("webbench ");
    message
        .0
        .extend(code_points(rt.call("logger_format", json!(["project"]))?)?.0);
    message
        .0
        .extend(": evaluator exited rc=".chars().map(u32::from));
    member(rt, "exit_code", "code")?;
    message
        .0
        .extend(code_points(rt.call("logger_format", json!(["code"]))?)?.0);
    rt.call("logger_report", json!([message.0]))?;
    Ok(Value::Null)
}
