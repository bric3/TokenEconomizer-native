use super::{
    Result, Runtime,
    optimizer::{capture_log, code_points},
    providers::{boolean, copy, literal},
    runner_calls::{function, invoke, member},
};
use crate::{python_json::Text, value::truthy};
use serde_json::{Value, json};

fn attribute(rt: &mut dyn Runtime, object: &str, name: &str, target: &str) -> Result<()> {
    rt.call("stdout_member", json!([object, name, target]))
        .map(|_| ())
}
fn builtin(rt: &mut dyn Runtime, name: &str) -> Result<()> {
    rt.call("eval_builtin", json!([name])).map(|_| ())
}
fn call(rt: &mut dyn Runtime, callable: &str, args: &[&str], target: &str) -> Result<()> {
    copy(rt, "function", callable)?;
    invoke(rt, args, &[])?;
    copy(rt, target, "called")
}
fn apply(rt: &mut dyn Runtime, name: &str, args: &[&str], target: &str) -> Result<()> {
    function(rt, name)?;
    invoke(rt, args, &[])?;
    copy(rt, target, "called")
}
fn method(
    rt: &mut dyn Runtime,
    object: &str,
    name: &str,
    args: &[&str],
    target: &str,
) -> Result<()> {
    attribute(rt, object, name, "function")?;
    invoke(rt, args, &[])?;
    copy(rt, target, "called")
}
fn own(rt: &mut dyn Runtime, name: &str, args: &[&str], target: &str) -> Result<()> {
    member(rt, name, "function")?;
    invoke(rt, args, &[])?;
    copy(rt, target, "called")
}
fn item(rt: &mut dyn Runtime, object: &str, key: Value, target: &str) -> Result<()> {
    rt.call("stream_item", json!([object, key, target]))
        .map(|_| ())
}
fn get(rt: &mut dyn Runtime, object: &str, key: &str, target: &str) -> Result<()> {
    rt.call("provider_method", json!([object, "get", [key], target]))
        .map(|_| ())
}
fn field(rt: &mut dyn Runtime, object: &str, key: &str, slot: &str) -> Result<()> {
    rt.call("eval_field", json!([object, key, slot]))
        .map(|_| ())
}
fn row(rt: &mut dyn Runtime, key: &str, slot: &str) -> Result<()> {
    field(rt, "row", key, slot)
}
fn assign(rt: &mut dyn Runtime, name: &str, slot: &str) -> Result<()> {
    rt.call("stream_set", json!([name, slot])).map(|_| ())
}
fn is_none(rt: &mut dyn Runtime, slot: &str) -> Result<bool> {
    rt.call("provider_is_none", json!([slot]))
        .map(|v| truthy(&v))
}
fn compare(rt: &mut dyn Runtime, op: &str, a: &str, b: &str) -> Result<bool> {
    rt.call("web_compare", json!([op, a, b]))?;
    boolean(rt, "comparison")
}
fn binary(rt: &mut dyn Runtime, op: &str, a: &str, b: &str, target: &str) -> Result<()> {
    rt.call("web_binary", json!([op, a, b, target])).map(|_| ())
}
fn append(rt: &mut dyn Runtime, list: &str, slot: &str) -> Result<()> {
    rt.call("eval_append", json!([list, slot])).map(|_| ())
}
fn iter(rt: &mut dyn Runtime, slot: &str, iterator: &str) -> Result<()> {
    rt.call("workspace_iter", json!([slot, iterator]))
        .map(|_| ())
}
fn next(rt: &mut dyn Runtime, iterator: &str, slot: &str) -> Result<bool> {
    rt.call("workspace_next", json!([iterator, slot]))
        .map(|v| truthy(&v))
}
fn fallback(rt: &mut dyn Runtime, slot: &str, value: Value) -> Result<()> {
    if !boolean(rt, slot)? {
        literal(rt, slot, value)?;
    }
    Ok(())
}
fn formatted(rt: &mut dyn Runtime, slot: &str, spec: &str) -> Result<Text> {
    code_points(rt.call("task_format", json!([slot, spec]))?)
}
fn push(text: &mut Text, suffix: &str) {
    text.0.extend(suffix.chars().map(u32::from));
}
fn report(rt: &mut dyn Runtime, message: Text) -> Result<()> {
    rt.call("logger_report", json!([message.0])).map(|_| ())
}
fn log(rt: &mut dyn Runtime, parts: &[&str], slots: &[&str]) -> Result<()> {
    capture_log(rt)?;
    let mut text = Text::from(parts[0]);
    for (index, slot) in slots.iter().enumerate() {
        text.0.extend(formatted(rt, slot, "")?.0);
        push(&mut text, parts[index + 1]);
    }
    report(rt, text)
}
fn filename(rt: &mut dyn Runtime, prefix: &str, suffix: &str, target: &str) -> Result<()> {
    literal(rt, "one", json!(1))?;
    binary(rt, "add", "attempt", "one", "ordinal")?;
    let mut text = Text::from(prefix);
    text.0.extend(formatted(rt, "ordinal", "")?.0);
    push(&mut text, suffix);
    rt.call("logger_literal", json!([target, text.0]))
        .map(|_| ())
}
fn elapsed(rt: &mut dyn Runtime) -> Result<()> {
    builtin(rt, "round")?;
    copy(rt, "round", "function")?;
    apply(rt, "time.time", &[], "now")?;
    binary(rt, "sub", "now", "t0", "duration")?;
    literal(rt, "one", json!(1))?;
    call(rt, "round", &["duration", "one"], "wall_s")
}
fn excluded(rt: &mut dyn Runtime) -> Result<()> {
    builtin(rt, "list")?;
    copy(rt, "list", "function")?;
    copy(rt, "excluded", "exclude_tools")?;
    fallback(rt, "excluded", json!([]))?;
    call(rt, "list", &["excluded"], "excluded")
}
fn update_segment(rt: &mut dyn Runtime, result: &str) -> Result<()> {
    attribute(rt, "row", "update", "update")?;
    own(rt, "_row_from_segment", &[result], "segment_row")?;
    call(rt, "update", &["segment_row"], "updated")
}
fn open(rt: &mut dyn Runtime, context: &str, path: &str, captured: bool) -> Result<()> {
    if captured {
        copy(rt, "function", "open")?;
    } else {
        builtin(rt, "open")?;
    }
    literal(rt, "write", json!("w"))?;
    literal(rt, "encoding", json!("utf-8"))?;
    invoke(rt, &[path, "write"], &[("encoding", "encoding")])?;
    rt.call("task_enter", json!([context])).map(|_| ())
}
fn exit(rt: &mut dyn Runtime, context: &str, result: Result<()>) -> Result<()> {
    rt.call("task_exit", json!([context, result.as_ref().err()]))?;
    if let Err(error) = result
        && !boolean(rt, "exit_result")?
    {
        return Err(error);
    }
    Ok(())
}
fn broken(rt: &mut dyn Runtime) -> Result<()> {
    literal(rt, "true", json!(true))?;
    row(rt, "stream_broken", "true")?;
    assign(rt, "_stream_broken", "true")
}
fn no_event(rt: &mut dyn Runtime, verdict: bool) -> Result<()> {
    capture_log(rt)?;
    let mut message = formatted(rt, "task_id", "")?;
    if verdict {
        push(&mut message, ": no verdict within ");
        member(rt, "verdict_timeout", "timeout")?;
        message.0.extend(formatted(rt, "timeout", "")?.0);
        push(&mut message, "s (exited=");
    } else {
        push(&mut message, ": evaluator produced no attempt (exited=");
    }
    attribute(rt, "sess", "finished", "finished")?;
    message.0.extend(formatted(rt, "finished", "")?.0);
    push(
        &mut message,
        if verdict { ")" } else { ") -- stream broken" },
    );
    report(rt, message)?;
    broken(rt)
}
fn desync(rt: &mut dyn Runtime, verdict: bool) -> Result<()> {
    builtin(rt, "RuntimeError")?;
    copy(rt, "runtime_error", "function")?;
    let mut message = Text::from(if verdict {
        "webbench loop desync: verdict for "
    } else {
        "webbench loop desync: the driver is on "
    });
    if verdict {
        item(rt, "verdict", json!("task"), "unexpected_task")?;
        message.0.extend(formatted(rt, "unexpected_task", "!r")?.0);
        push(&mut message, " while the driver is on ");
        message.0.extend(formatted(rt, "task_id", "!r")?.0);
    } else {
        message.0.extend(formatted(rt, "task_id", "!r")?.0);
        push(&mut message, " but the evaluator started ");
        item(rt, "started", json!("task"), "unexpected_task")?;
        message.0.extend(formatted(rt, "unexpected_task", "!r")?.0);
        push(
            &mut message,
            ". The driver's task order must match projects/",
        );
        message.0.extend(formatted(rt, "project", "")?.0);
        push(&mut message, "/tasks.jsonl.");
    }
    raise(rt, message)
}

fn raise(rt: &mut dyn Runtime, message: Text) -> Result<()> {
    rt.call("logger_literal", json!(["error_message", message.0]))?;
    call(rt, "runtime_error", &["error_message"], "error")?;
    rt.call("task_raise", json!(["error"])).map(|_| ())
}
fn request_artifact(rt: &mut dyn Runtime) -> Result<()> {
    builtin(rt, "open")?;
    copy(rt, "open", "function")?;
    function(rt, "os.path.join")?;
    copy(rt, "join", "function")?;
    filename(rt, "agent_request_", ".json", "request_filename")?;
    call(
        rt,
        "join",
        &["run_dir_path", "request_filename"],
        "request_path",
    )?;
    open(rt, "request", "request_path", true)?;
    let result: Result<()> = (|| {
        function(rt, "json.dump")?;
        copy(rt, "dump", "function")?;
        literal(rt, "request_record", json!({}))?;
        for key in ["type", "task", "error"] {
            get(rt, "req", key, "value")?;
            field(rt, "request_record", key, "value")?;
        }
        builtin(rt, "sorted")?;
        invoke(rt, &["req_files"], &[])?;
        field(rt, "request_record", "files", "called")?;
        copy(rt, "function", "dump")?;
        literal(rt, "indent", json!(2))?;
        invoke(
            rt,
            &["request_record", "request_handle"],
            &[("indent", "indent")],
        )
    })();
    exit(rt, "request", result)
}
fn segment(rt: &mut dyn Runtime, prompt: &str, target: &str) -> Result<()> {
    own(
        rt,
        "_segment",
        &[
            "workspace",
            prompt,
            "transcript_handle",
            "exclude_tools",
            "max_turns",
            "guard",
        ],
        target,
    )
}
fn usage_value(rt: &mut dyn Runtime, key: &str, target: &str) -> Result<()> {
    get(rt, "usage", key, target)?;
    fallback(rt, target, json!(0))
}
fn answer(rt: &mut dyn Runtime) -> Result<()> {
    own(
        rt,
        "_collect",
        &["workspace", "req_files", "project_dir"],
        "collection",
    )?;
    rt.call(
        "task_triple",
        json!(["collection", "files", "deleted", "skipped"]),
    )?;
    binary(rt, "iadd", "deleted_all", "deleted", "deleted_all")?;
    binary(rt, "iadd", "skipped_all", "skipped", "skipped_all")?;
    builtin(rt, "sorted")?;
    invoke(rt, &["files"], &[])?;
    append(rt, "answered", "called")?;
    own(rt, "_segment_usage", &["result"], "usage")?;
    attribute(rt, "reply_q", "put", "put")?;
    literal(rt, "reply", json!({}))?;
    field(rt, "reply", "files", "files")?;
    usage_value(rt, "input_tokens", "input")?;
    usage_value(rt, "cache_read_input_tokens", "cache_read")?;
    binary(rt, "add", "input", "cache_read", "input_total")?;
    usage_value(rt, "cache_creation_input_tokens", "cache_creation")?;
    binary(rt, "add", "input_total", "cache_creation", "input_total")?;
    field(rt, "reply", "inputTokens", "input_total")?;
    usage_value(rt, "output_tokens", "output")?;
    field(rt, "reply", "outputTokens", "output")?;
    literal(rt, "transcript_name", json!("transcript.jsonl"))?;
    apply(
        rt,
        "os.path.join",
        &["run_dir_path", "transcript_name"],
        "trajectory",
    )?;
    field(rt, "reply", "trajectory", "trajectory")?;
    call(rt, "put", &["reply"], "put_result")
}
fn attempts(rt: &mut dyn Runtime) -> Result<()> {
    builtin(rt, "range")?;
    copy(rt, "range", "function")?;
    member(rt, "retry", "retry")?;
    call(rt, "range", &["retry"], "range_values")?;
    iter(rt, "range_values", "attempt_iter")?;
    let mut count = 0usize;
    while next(rt, "attempt_iter", "attempt")? {
        attribute(rt, "sess", "next_event", "function")?;
        literal(rt, "started_kind", json!("started"))?;
        member(rt, "start_timeout", "timeout")?;
        invoke(rt, &["started_kind", "timeout"], &[])?;
        copy(rt, "started", "called")?;
        if is_none(rt, "started")? {
            no_event(rt, false)?;
            break;
        }
        item(rt, "started", json!("task"), "announced_task")?;
        if compare(rt, "ne", "announced_task", "task_id")? {
            desync(rt, false)?;
        }
        attribute(rt, "sess", "next_request", "function")?;
        member(rt, "start_timeout", "timeout")?;
        invoke(rt, &["timeout"], &[])?;
        rt.call("workspace_pair", json!(["called", "req", "reply_q"]))?;
        if is_none(rt, "req")? {
            log(
                rt,
                &["", ": no agent request after the Started line"],
                &["task_id"],
            )?;
            broken(rt)?;
            break;
        }
        count += 1;
        literal(rt, "attempts", json!(count))?;
        get(rt, "req", "files", "req_files")?;
        fallback(rt, "req_files", json!({}))?;
        own(rt, "_make_guard", &["run_kwargs"], "guard")?;
        if !is_none(rt, "guard")? {
            append(rt, "guards", "guard")?;
        }
        function(rt, "os.path.join")?;
        copy(rt, "join", "function")?;
        filename(rt, "ws-attempt", "", "workspace_name")?;
        call(rt, "join", &["run_dir_path", "workspace_name"], "workspace")?;
        own(
            rt,
            "_materialize",
            &["workspace", "req_files"],
            "materialized",
        )?;
        member(rt, "scaffold", "scaffold")?;
        if !is_none(rt, "scaffold")? && !boolean(rt, "scaffold_notes")? {
            member(rt, "_apply_scaffold", "apply_scaffold")?;
            own(rt, "agent_home", &["run_dir_path"], "home")?;
            call(
                rt,
                "apply_scaffold",
                &["workspace", "home", "task", "run_dir_path"],
                "scaffold_notes",
            )?;
        }
        function(rt, "build_prompt")?;
        invoke(
            rt,
            &["task", "injected", "req", "project"],
            &[("workspace", "workspace")],
        )?;
        copy(rt, "prompt", "called")?;
        request_artifact(rt)?;
        segment(rt, "prompt", "result")?;
        while !is_none(rt, "guard")? {
            method(rt, "guard", "continuation", &["prompt"], "nxt")?;
            if !boolean(rt, "nxt")? {
                break;
            }
            capture_log(rt)?;
            let mut message = formatted(rt, "task_id", "")?;
            push(&mut message, ": spiral nudge -> segment ");
            attribute(rt, "guard", "segment", "segment")?;
            message.0.extend(formatted(rt, "segment", "")?.0);
            report(rt, message)?;
            segment(rt, "nxt", "seg")?;
            own(rt, "_merge_segments", &["result", "seg"], "result")?;
        }
        if boolean(rt, "merged")? {
            own(rt, "_merge_segments", &["merged", "result"], "merged")?;
        } else {
            copy(rt, "merged", "result")?;
        }
        answer(rt)?;
        attribute(rt, "sess", "next_event", "function")?;
        literal(rt, "verdict_kind", json!("verdict"))?;
        member(rt, "verdict_timeout", "timeout")?;
        invoke(rt, &["verdict_kind", "timeout"], &[])?;
        copy(rt, "verdict", "called")?;
        if is_none(rt, "verdict")? {
            no_event(rt, true)?;
            break;
        }
        item(rt, "verdict", json!("task"), "announced_task")?;
        if compare(rt, "ne", "announced_task", "task_id")? {
            desync(rt, true)?;
        }
        member(rt, "keep_workspaces", "keep_workspaces")?;
        if !boolean(rt, "keep_workspaces")? {
            function(rt, "shutil.rmtree")?;
            literal(rt, "true", json!(true))?;
            invoke(rt, &["workspace"], &[("ignore_errors", "true")])?;
        }
        builtin(rt, "bool")?;
        copy(rt, "bool", "function")?;
        item(rt, "verdict", json!("ok"), "ok")?;
        call(rt, "bool", &["ok"], "attempt_result")?;
        append(rt, "attempt_results", "attempt_result")?;
        item(rt, "verdict", json!("ok"), "ok")?;
        if boolean(rt, "ok")? {
            literal(rt, "resolved", json!(true))?;
            return Ok(());
        }
    }
    Ok(())
}
fn aggregate(
    rt: &mut dyn Runtime,
    operation: &str,
    attr: &str,
    target: &str,
    default: Option<Value>,
) -> Result<()> {
    builtin(rt, operation)?;
    let mut params = vec![json!("guards"), json!(attr)];
    if let Some(default) = default {
        params.push(default);
    }
    rt.call("task_attributes", json!(params))?;
    invoke(rt, &["attribute_values"], &[])?;
    copy(rt, target, "called")
}
fn telemetry(rt: &mut dyn Runtime) -> Result<()> {
    aggregate(rt, "any", "aborted", "aborted", None)?;
    row(rt, "aborted", "aborted")?;
    item(rt, "row", json!("aborted"), "aborted")?;
    row(rt, "spiral_abort", "aborted")?;
    aggregate(rt, "sum", "nudges_done", "total", None)?;
    row(rt, "spiral_nudges", "total")?;
    aggregate(
        rt,
        "any",
        "abort_suppressed",
        "suppressed",
        Some(json!(false)),
    )?;
    row(rt, "spiral_abort_suppressed", "suppressed")?;
    literal(rt, "events", json!([]))?;
    iter(rt, "guards", "guards_iter")?;
    while next(rt, "guards_iter", "g")? {
        attribute(rt, "g", "events", "guard_events")?;
        fallback(rt, "guard_events", json!([]))?;
        iter(rt, "guard_events", "events_iter")?;
        while next(rt, "events_iter", "event")? {
            append(rt, "events", "event")?;
        }
    }
    row(rt, "spiral_events", "events")?;
    aggregate(rt, "sum", "judge_tokens", "total", None)?;
    row(rt, "spiral_judge_tokens", "total")?;
    aggregate(rt, "sum", "nudge_writer_tokens", "total", None)?;
    row(rt, "spiral_nudge_writer_tokens", "total")
}
fn unique_sorted(rt: &mut dyn Runtime, slot: &str) -> Result<()> {
    builtin(rt, "sorted")?;
    copy(rt, "sorted", "function")?;
    builtin(rt, "set")?;
    invoke(rt, &[slot], &[])?;
    copy(rt, "unique", "called")?;
    call(rt, "sorted", &["unique"], "unique_sorted")
}
fn final_log(rt: &mut dyn Runtime) -> Result<()> {
    capture_log(rt)?;
    let mut message = formatted(rt, "task_id", "")?;
    push(&mut message, " arm=");
    message.0.extend(formatted(rt, "arm", "")?.0);
    push(&mut message, " resolved=");
    message.0.extend(formatted(rt, "resolved", "")?.0);
    push(&mut message, " attempts=");
    message.0.extend(formatted(rt, "attempts", "")?.0);
    push(&mut message, " turns=");
    get(rt, "row", "num_turns", "turns")?;
    message.0.extend(formatted(rt, "turns", "")?.0);
    push(&mut message, " eff_tok=");
    apply(rt, "pricing.effective_tokens", &["row"], "effective")?;
    message.0.extend(formatted(rt, "effective", ".0f")?.0);
    push(&mut message, " ");
    item(rt, "row", json!("wall_s"), "wall_s")?;
    message.0.extend(formatted(rt, "wall_s", "")?.0);
    push(&mut message, "s");
    report(rt, message)
}
fn finish(rt: &mut dyn Runtime) -> Result<()> {
    literal(rt, "init", json!("init"))?;
    if !boolean(rt, "resolved")? && compare(rt, "ne", "task_id", "init")? {
        member(rt, "retry", "retry")?;
        if compare(rt, "ge", "attempts", "retry")? {
            literal(rt, "true", json!(true))?;
            assign(rt, "_stream_broken", "true")?;
            log(
                rt,
                &[
                    "",
                    ": failed all ",
                    " attempts -- the Web-Bench stream ends here, later tasks are unreached",
                ],
                &["task_id", "attempts"],
            )?;
        }
    }
    update_segment(rt, "merged")?;
    row(rt, "resolved", "resolved")?;
    literal(rt, "p1", Value::Null)?;
    if boolean(rt, "attempt_results")? {
        item(rt, "attempt_results", json!(0), "p1")?;
    }
    row(rt, "resolved_p1", "p1")?;
    literal(rt, "p2", Value::Null)?;
    if boolean(rt, "attempt_results")? {
        builtin(rt, "any")?;
        invoke(rt, &["attempt_results"], &[])?;
        copy(rt, "p2", "called")?;
    }
    row(rt, "resolved_p2", "p2")?;
    row(rt, "webbench_attempt_results", "attempt_results")?;
    row(rt, "webbench_attempts", "attempts")?;
    row(rt, "webbench_answered_files", "answered")?;
    for (slot, key) in [
        ("deleted_all", "webbench_unexpressible_deletions"),
        ("skipped_all", "webbench_skipped_files"),
    ] {
        if boolean(rt, slot)? {
            unique_sorted(rt, slot)?;
            row(rt, key, "unique_sorted")?;
        }
    }
    elapsed(rt)?;
    row(rt, "wall_s", "wall_s")?;
    excluded(rt)?;
    row(rt, "untooled_excluded", "excluded")?;
    own(rt, "_net_guard_blocks", &["run_dir_path"], "blocks")?;
    row(rt, "net_guard_blocks", "blocks")?;
    row(rt, "net_guard_installed", "net_guard_installed")?;
    apply(
        rt,
        "answer_key_attempts",
        &["run_dir_path"],
        "answer_key_attempts",
    )?;
    row(rt, "answer_key_attempts", "answer_key_attempts")?;
    if boolean(rt, "scaffold_notes")? {
        method(rt, "row", "update", &["scaffold_notes"], "updated")?;
        member(rt, "scaffold", "scaffold")?;
        attribute(rt, "scaffold", "name", "scaffold_name")?;
        row(rt, "scaffold", "scaffold_name")?;
    }
    if boolean(rt, "guards")? {
        telemetry(rt)?;
    }
    final_log(rt)
}
fn unreached(rt: &mut dyn Runtime) -> Result<()> {
    builtin(rt, "open")?;
    copy(rt, "open", "function")?;
    literal(rt, "transcript_name", json!("transcript.jsonl"))?;
    apply(
        rt,
        "os.path.join",
        &["run_dir_path", "transcript_name"],
        "transcript",
    )?;
    open(rt, "transcript", "transcript", true)?;
    exit(rt, "transcript", Ok(()))?;
    literal(rt, "empty", json!({}))?;
    update_segment(rt, "empty")?;
    attribute(rt, "row", "update", "update")?;
    literal(
        rt,
        "unreached",
        json!({"resolved":false,"stream_broken":true,"unreached":true,"resolved_p1":null,"resolved_p2":null,"webbench_attempt_results":[],"webbench_attempts":0}),
    )?;
    elapsed(rt)?;
    field(rt, "unreached", "wall_s", "wall_s")?;
    excluded(rt)?;
    field(rt, "unreached", "untooled_excluded", "excluded")?;
    call(rt, "update", &["unreached"], "updated")?;
    log(
        rt,
        &["", " arm=", " UNREACHED (stream stopped earlier)"],
        &["task_id", "arm"],
    )
}

pub fn run(rt: &mut dyn Runtime) -> Result<Value> {
    item(rt, "task", json!("instance_id"), "task_id")?;
    assign(rt, "_current_task", "task_id")?;
    function(rt, "set_optimizer_context")?;
    copy(rt, "context", "function")?;
    member(rt, "model", "model")?;
    member(rt, "agent", "agent")?;
    apply(rt, "_cell_of", &["run_dir_path"], "cell")?;
    apply(rt, "os.path.basename", &["run_dir_path"], "run_dir")?;
    copy(rt, "function", "context")?;
    invoke(
        rt,
        &[],
        &[
            ("arm", "arm"),
            ("instance_id", "task_id"),
            ("model", "model"),
            ("agent", "agent"),
            ("cell", "cell"),
            ("run_dir", "run_dir"),
        ],
    )?;
    get(rt, "task", "repo", "repository")?;
    fallback(rt, "repository", json!(""))?;
    literal(rt, "slash", json!("/"))?;
    method(rt, "repository", "split", &["slash"], "parts")?;
    item(rt, "parts", json!(-1), "project")?;
    if !boolean(rt, "project")? {
        builtin(rt, "RuntimeError")?;
        copy(rt, "runtime_error", "function")?;
        let mut message = Text::from("webbench task ");
        message.0.extend(formatted(rt, "task_id", "!r")?.0);
        push(
            &mut message,
            " carries no project in `repo`; the driver's --repo selects the Web-Bench project",
        );
        raise(rt, message)?;
    }
    function(rt, "os.makedirs")?;
    literal(rt, "true", json!(true))?;
    invoke(rt, &["run_dir_path"], &[("exist_ok", "true")])?;
    builtin(rt, "int")?;
    copy(rt, "int", "function")?;
    get(rt, "run_kwargs", "max_turns", "raw_turns")?;
    if !boolean(rt, "raw_turns")? {
        rt.call("provider_env_get", json!(["raw_turns", "TE_MAX_TURNS", 80]))?;
    }
    call(rt, "int", &["raw_turns"], "max_turns")?;
    apply(rt, "time.time", &[], "t0")?;
    function(rt, "os.path.join")?;
    member(rt, "repo", "repo")?;
    literal(rt, "projects", json!("projects"))?;
    invoke(rt, &["repo", "projects", "project"], &[])?;
    copy(rt, "project_dir", "called")?;
    literal(rt, "row", json!({}))?;
    row(rt, "instance", "task_id")?;
    rt.call(
        "provider_method",
        json!(["task", "get", ["repo", ""], "task_repo"]),
    )?;
    row(rt, "repo", "task_repo")?;
    for name in ["model", "agent"] {
        member(rt, name, name)?;
        row(rt, name, name)?;
    }
    row(rt, "arm", "arm")?;
    for name in ["effort", "cli_version"] {
        member(rt, name, name)?;
        row(rt, name, name)?;
    }
    literal(rt, "domain", json!("webbench"))?;
    row(rt, "domain", "domain")?;
    row(rt, "webbench_project", "project")?;
    own(rt, "session_for", &["project"], "sess")?;
    member(rt, "_stream_broken", "stream_broken")?;
    let stopped = if boolean(rt, "stream_broken")? {
        true
    } else {
        attribute(rt, "sess", "finished", "finished")?;
        boolean(rt, "finished")?
    };
    if stopped {
        unreached(rt)?;
    } else {
        own(
            rt,
            "_write_run_config",
            &[
                "run_dir_path",
                "task",
                "arm",
                "injected",
                "exclude_tools",
                "max_turns",
                "run_kwargs",
                "sess",
                "project",
            ],
            "configured",
        )?;
        member(rt, "_install_net_guard", "install_guard")?;
        own(rt, "agent_home", &["run_dir_path"], "home")?;
        call(
            rt,
            "install_guard",
            &["home", "run_dir_path"],
            "net_guard_installed",
        )?;
        literal(rt, "transcript_name", json!("transcript.jsonl"))?;
        apply(
            rt,
            "os.path.join",
            &["run_dir_path", "transcript_name"],
            "transcript",
        )?;
        literal(rt, "merged", json!({}))?;
        literal(rt, "resolved", json!(false))?;
        literal(rt, "attempts", json!(0))?;
        for slot in [
            "attempt_results",
            "deleted_all",
            "skipped_all",
            "answered",
            "guards",
        ] {
            literal(rt, slot, json!([]))?;
        }
        literal(rt, "scaffold_notes", json!({}))?;
        open(rt, "transcript", "transcript", false)?;
        let result = attempts(rt);
        exit(rt, "transcript", result)?;
        finish(rt)?;
    }
    rt.call("provider_tuple", json!(["row", "run_dir_path"]))?;
    Ok(json!({"provider_ref":"tuple"}))
}

fn parent(rt: &mut dyn Runtime) -> Result<()> {
    rt.call("task_super", json!([]))?;
    rt.call(
        "task_call_kwargs",
        json!([
            ["task"],
            [
                ["arm", "arm"],
                ["injected", "injected"],
                ["exclude_tools", "exclude_tools"],
                ["run_dir_path", "run_dir_path"]
            ],
            "run_kwargs"
        ]),
    )?;
    rt.call("workspace_pair", json!(["called", "row", "out_dir"]))?;
    Ok(())
}
fn returned(rt: &mut dyn Runtime) -> Result<Value> {
    rt.call("provider_tuple", json!(["row", "out_dir"]))?;
    Ok(json!({"provider_ref":"tuple"}))
}

pub fn copilot(rt: &mut dyn Runtime) -> Result<Value> {
    parent(rt)?;
    rt.call(
        "provider_method",
        json!(["row", "pop", ["_debug_log", ""], "log_text"]),
    )?;
    if boolean(rt, "log_text")? {
        let saved: Result<()> = (|| {
            builtin(rt, "open")?;
            copy(rt, "open", "function")?;
            literal(rt, "filename", json!("copilot_debug.log"))?;
            apply(rt, "os.path.join", &["out_dir", "filename"], "debug_path")?;
            open(rt, "debug", "debug_path", true)?;
            let written = method(rt, "debug_handle", "write", &["log_text"], "written");
            exit(rt, "debug", written)
        })();
        if let Err(error) = saved {
            if !error.is_a("builtins.OSError") {
                return Err(error);
            }
            rt.call("task_caught_log", json!([error]))?;
            let mut message = Text::from("could not save copilot_debug.log: ");
            message
                .0
                .extend(code_points(rt.call("handler_format", json!(["error"]))?)?.0);
            rt.call("logger_literal", json!(["error_log", message.0]))?;
            copy(rt, "function", "active_log")?;
            rt.call("handler_invoke", json!([["error_log"], []]))?;
        }
    }
    returned(rt)
}

pub fn codex(rt: &mut dyn Runtime) -> Result<Value> {
    builtin(rt, "tuple")?;
    copy(rt, "tuple_function", "function")?;
    get(rt, "run_kwargs", "codex_config_capabilities", "caps")?;
    if !boolean(rt, "caps")? {
        rt.call("provider_tuple", json!([]))?;
        copy(rt, "caps", "tuple")?;
    }
    call(rt, "tuple_function", &["caps"], "capabilities")?;
    assign(rt, "_capabilities", "capabilities")?;
    parent(rt)?;
    literal(rt, "basis", json!("codex_steps"))?;
    row(rt, "turns_basis", "basis")?;
    builtin(rt, "list")?;
    copy(rt, "list", "function")?;
    member(rt, "_capabilities", "capabilities")?;
    call(rt, "list", &["capabilities"], "capabilities_list")?;
    row(rt, "codex_config_capabilities", "capabilities_list")?;
    builtin(rt, "int")?;
    copy(rt, "int", "function")?;
    member(rt, "_lean_instructions_tokens", "lean")?;
    fallback(rt, "lean", json!(0))?;
    call(rt, "int", &["lean"], "lean_integer")?;
    row(rt, "codex_lean_instructions_tokens", "lean_integer")?;
    builtin(rt, "bool")?;
    copy(rt, "bool", "function")?;
    member(rt, "answer_key_ns", "answer_key_ns")?;
    call(rt, "bool", &["answer_key_ns"], "hosts_blocked")?;
    row(rt, "answer_key_hosts_blocked", "hosts_blocked")?;
    member(rt, "scaffold_name", "scaffold_name")?;
    literal(rt, "rtk", json!("rtk"))?;
    if compare(rt, "eq", "scaffold_name", "rtk")? {
        attribute(rt, "row", "update", "update")?;
        function(rt, "codex.rtk_compliance")?;
        copy(rt, "compliance", "function")?;
        literal(rt, "filename", json!("transcript.jsonl"))?;
        apply(rt, "os.path.join", &["out_dir", "filename"], "transcript")?;
        call(rt, "compliance", &["transcript"], "compliance_row")?;
        call(rt, "update", &["compliance_row"], "updated")?;
    }
    own(rt, "_prune_agent_home", &["run_dir_path"], "pruned")?;
    returned(rt)
}

pub fn prune_home(rt: &mut dyn Runtime) -> Result<Value> {
    own(rt, "agent_home", &["run_dir_path"], "home")?;
    member(rt, "keep_workspaces", "keep_workspaces")?;
    if boolean(rt, "keep_workspaces")? {
        return Ok(Value::Null);
    }
    apply(rt, "os.path.isdir", &["home"], "is_directory")?;
    if !boolean(rt, "is_directory")? {
        return Ok(Value::Null);
    }
    apply(rt, "os.listdir", &["home"], "names")?;
    iter(rt, "names", "names_iter")?;
    while next(rt, "names_iter", "name")? {
        member(rt, "_AGENT_HOME_KEEP", "keep")?;
        if truthy(&rt.call("provider_contains", json!(["name", "keep"]))?) {
            continue;
        }
        apply(rt, "os.path.join", &["home", "name"], "path")?;
        let removed: Result<()> = (|| {
            apply(rt, "os.path.isdir", &["path"], "is_directory")?;
            let remove = if boolean(rt, "is_directory")? {
                "shutil.rmtree"
            } else {
                "os.remove"
            };
            apply(rt, remove, &["path"], "removed")
        })();
        match removed {
            Err(error) if error.is_a("builtins.OSError") => {}
            result => result?,
        }
    }
    let removed: Result<()> = (|| {
        function(rt, "shutil.rmtree")?;
        copy(rt, "remove_host", "function")?;
        literal(rt, "host_home", json!(".host-home"))?;
        apply(
            rt,
            "os.path.join",
            &["run_dir_path", "host_home"],
            "host_path",
        )?;
        call(rt, "remove_host", &["host_path"], "removed")
    })();
    match removed {
        Err(error) if error.is_a("builtins.OSError") => Ok(Value::Null),
        result => result.map(|_| Value::Null),
    }
}
