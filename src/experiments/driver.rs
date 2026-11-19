//! Benchmark runner selection and task filtering with caller-owned SDK objects.

use super::swebench::{
    Failure, Result, Runtime,
    optimizer::code_points,
    providers::{boolean, copy, literal},
    runner_calls::{function, invoke},
};
use crate::{python_json::Text, value::truthy};
use serde_json::{Value, json};

mod environment;
mod stream;
mod stub;
pub use environment::parse_env;

fn returned(slot: &str) -> Value {
    json!({"provider_ref":slot})
}
fn attr(rt: &mut dyn Runtime, object: &str, name: &str, target: &str) -> Result<()> {
    rt.call("stdout_member", json!([object, name, target]))
        .map(|_| ())
}
fn method(rt: &mut dyn Runtime, object: &str, name: &str, args: &[&str]) -> Result<()> {
    attr(rt, object, name, "function")?;
    invoke(rt, args, &[])
}
fn equal(rt: &mut dyn Runtime, slot: &str, value: &str) -> Result<bool> {
    rt.call("provider_equal", json!([slot, value]))
        .map(|value| truthy(&value))
}
fn contains(rt: &mut dyn Runtime, item: &str, collection: &str) -> Result<bool> {
    rt.call("provider_contains", json!([item, collection]))
        .map(|value| truthy(&value))
}
fn named(rt: &mut dyn Runtime, name: &str, collection: &str) -> Result<bool> {
    literal(rt, "key", json!(name))?;
    contains(rt, "key", collection)
}
fn append(message: &mut Text, text: &str) {
    message.0.extend(text.chars().map(u32::from));
}
fn formatted(rt: &mut dyn Runtime, slot: &str, spec: &str) -> Result<Text> {
    code_points(rt.call("task_format", json!([slot, spec]))?)
}
fn text(rt: &mut dyn Runtime, slot: &str, message: Text) -> Result<()> {
    rt.call("logger_literal", json!([slot, message.0]))
        .map(|_| ())
}
fn exit(rt: &mut dyn Runtime, message: Text) -> Result<Value> {
    rt.call("eval_builtin", json!(["SystemExit"]))?;
    text(rt, "message", message)?;
    invoke(rt, &["message"], &[])?;
    rt.call("task_raise", json!(["called"]))
}
fn load_runner(rt: &mut dyn Runtime) -> Result<Value> {
    if equal(rt, "spec", "stub")? {
        if boolean(rt, "scaffold")? {
            let mut message = Text::from("[runner stub] scaffold ");
            message.0.extend(formatted(rt, "scaffold", "!r")?.0);
            append(
                &mut message,
                " NOT installed -- synthetic rows, for wiring checks only",
            );
            rt.call("eval_builtin", json!(["print"]))?;
            text(rt, "message", message)?;
            invoke(rt, &["message"], &[])?;
        }
        function(rt, "StubRunner")?;
        invoke(rt, &["agent"], &[])?;
        return Ok(returned("called"));
    }
    literal(rt, "colon", json!(":"))?;
    if !contains(rt, "colon", "spec")? {
        let mut message = Text::from("--runner must be 'stub' or 'module:factory', got ");
        message.0.extend(formatted(rt, "spec", "!r")?.0);
        return exit(rt, message);
    }
    literal(rt, "one", json!(1))?;
    method(rt, "spec", "split", &["colon", "one"])?;
    rt.call("scaffold_pair", json!(["called", "mod_name", "fac_name"]))?;
    rt.call("eval_builtin", json!(["getattr"]))?;
    copy(rt, "getattr", "function")?;
    function(rt, "importlib.import_module")?;
    invoke(rt, &["mod_name"], &[])?;
    copy(rt, "module", "called")?;
    copy(rt, "function", "getattr")?;
    invoke(rt, &["module", "fac_name"], &[])?;
    copy(rt, "factory", "called")?;
    function(rt, "os.environ.setdefault")?;
    literal(rt, "key", json!("TE_AGENT"))?;
    copy(rt, "label", "agent")?;
    if !boolean(rt, "label")? {
        literal(rt, "label", json!(""))?;
    }
    invoke(rt, &["key", "label"], &[])?;
    let signature = (|| {
        function(rt, "inspect.signature")?;
        invoke(rt, &["factory"], &[])
    })();
    if let Err(error) = signature {
        if !truthy(&rt.call(
            "exception_matches",
            json!([error, ["TypeError", "ValueError"]]),
        )?) {
            return Err(error);
        }
        rt.call("handler_exception", json!([error]))?;
        copy(rt, "function", "factory")?;
        rt.call("handler_invoke", json!([[], []]))?;
        return Ok(returned("called"));
    }
    copy(rt, "signature", "called")?;
    attr(rt, "signature", "parameters", "parameters")?;
    method(rt, "parameters", "values", &[])?;
    rt.call("workspace_iter", json!(["called", "iterator"]))?;
    let mut takes_kwargs = false;
    while truthy(&rt.call("workspace_next", json!(["iterator", "parameter"]))?) {
        attr(rt, "parameter", "kind", "kind")?;
        function(rt, "inspect.Parameter.VAR_KEYWORD")?;
        rt.call("web_compare", json!(["is_", "kind", "function"]))?;
        if boolean(rt, "comparison")? {
            takes_kwargs = true;
            break;
        }
    }
    literal(rt, "kw", json!({}))?;
    attr(rt, "signature", "parameters", "parameters")?;
    if named(rt, "agent", "parameters")? || takes_kwargs {
        rt.call("eval_field", json!(["kw", "agent", "agent"]))?;
    }
    if boolean(rt, "model")? && !equal(rt, "model", "agent")? {
        attr(rt, "signature", "parameters", "parameters")?;
        if named(rt, "model", "parameters")? || takes_kwargs {
            rt.call("eval_field", json!(["kw", "model", "model"]))?;
        }
    }
    if boolean(rt, "scaffold")? {
        attr(rt, "signature", "parameters", "parameters")?;
        if named(rt, "scaffold", "parameters")? || takes_kwargs {
            rt.call("eval_field", json!(["kw", "scaffold", "scaffold"]))?;
        } else {
            let mut message = Text::from("runner ");
            message.0.extend(formatted(rt, "spec", "!r")?.0);
            append(
                &mut message,
                " does not accept a scaffold, but this arm needs ",
            );
            message.0.extend(formatted(rt, "scaffold", "!r")?.0);
            append(
                &mut message,
                ". A harness that ignored it would run NATIVE and file the rows under the scaffold's name. Add a `scaffold=` parameter (see experiments/te_bench/baselines/) or pick an arm without one.",
            );
            return exit(rt, message);
        }
        function(rt, "os.environ.setdefault")?;
        literal(rt, "key", json!("TE_SCAFFOLD"))?;
        invoke(rt, &["key", "scaffold"], &[])?;
    }
    copy(rt, "function", "factory")?;
    if boolean(rt, "kw")? {
        rt.call("task_call_kwargs", json!([[], [], "kw"]))?;
    } else {
        invoke(rt, &[], &[])?;
    }
    Ok(returned("called"))
}
fn load_adapter(rt: &mut dyn Runtime) -> Result<Value> {
    rt.call("driver_adapters", json!([]))?;
    if equal(rt, "name", "swebench")? {
        copy(rt, "function", "SwebenchAdapter")?;
        invoke(rt, &[], &[("tasks_path", "tasks_file")])?;
    } else if equal(rt, "name", "webbench")? {
        literal(rt, "slash", json!("/"))?;
        method(rt, "repo", "split", &["slash"])?;
        rt.call("stream_item", json!(["called", -1, "project"]))?;
        if !boolean(rt, "project")? {
            literal(rt, "project", json!("dom"))?;
        }
        copy(rt, "function", "WebBenchAdapter")?;
        invoke(rt, &["project"], &[])?;
    } else {
        literal(rt, "classes", json!({}))?;
        rt.call(
            "eval_field",
            json!(["classes", "creddata", "CredDataAdapter"]),
        )?;
        rt.call(
            "eval_field",
            json!(["classes", "reposvul", "ReposVulAdapter"]),
        )?;
        rt.call("scaffold_item", json!(["classes", "name", "function"]))?;
        invoke(rt, &[], &[])?;
    }
    Ok(returned("called"))
}
fn builtin(rt: &mut dyn Runtime, name: &str, args: &[&str], target: &str) -> Result<()> {
    rt.call("eval_builtin", json!([name]))?;
    invoke(rt, args, &[])?;
    copy(rt, target, "called")
}
fn read_json(rt: &mut dyn Runtime, path: &str, target: &str, groups: bool) -> Result<()> {
    rt.call("eval_builtin", json!(["open"]))?;
    literal(rt, "encoding", json!("utf-8"))?;
    invoke(rt, &[path], &[("encoding", "encoding")])?;
    rt.call("task_enter", json!(["input"]))?;
    let read = (|| {
        function(rt, "json.load")?;
        invoke(rt, &["input_handle"], &[])?;
        if groups {
            rt.call("stream_item", json!(["called", "groups", target]))?;
        } else {
            copy(rt, target, "called")?;
        }
        Ok(())
    })();
    match read {
        Ok(()) => {
            rt.call("task_exit", json!(["input", null]))?;
        }
        Err(error) => {
            rt.call("task_exit", json!(["input", error]))?;
            if !boolean(rt, "exit_result")? {
                return Err(error);
            }
        }
    }
    Ok(())
}
fn workload_ids(rt: &mut dyn Runtime) -> Result<Value> {
    read_json(rt, "workload_file", "groups", true)?;
    rt.call("workspace_local", json!(["groups", "groups"]))?;
    if boolean(rt, "repo")? && contains(rt, "repo", "groups")? {
        rt.call("scaffold_item", json!(["groups", "repo", "selected"]))?;
        builtin(rt, "set", &["selected"], "selected")?;
        return Ok(returned("selected"));
    }
    builtin(rt, "len", &["groups"], "length")?;
    literal(rt, "one", json!(1))?;
    rt.call("web_compare", json!(["eq", "length", "one"]))?;
    if boolean(rt, "comparison")? {
        method(rt, "groups", "values", &[])?;
        builtin(rt, "iter", &["called"], "iterator")?;
        builtin(rt, "next", &["iterator"], "selected")?;
        builtin(rt, "set", &["selected"], "selected")?;
        return Ok(returned("selected"));
    }
    if boolean(rt, "repo")? {
        let mut message = Text::from("--repo ");
        message.0.extend(formatted(rt, "repo", "!r")?.0);
        append(&mut message, " is not a group in ");
        message.0.extend(formatted(rt, "workload_file", "")?.0);
        append(&mut message, "; expected one of ");
        builtin(rt, "sorted", &["groups"], "names")?;
        message.0.extend(formatted(rt, "names", "")?.0);
        return exit(rt, message);
    }
    rt.call("workspace_set", json!(["selected"]))?;
    method(rt, "groups", "values", &[])?;
    rt.call("workspace_iter", json!(["called", "outer"]))?;
    while truthy(&rt.call("workspace_next", json!(["outer", "ids"]))?) {
        rt.call("workspace_iter", json!(["ids", "inner"]))?;
        while truthy(&rt.call("workspace_next", json!(["inner", "identifier"]))?) {
            method(rt, "selected", "add", &["identifier"])?;
        }
    }
    Ok(returned("selected"))
}
fn same_group(rt: &mut dyn Runtime) -> Result<Value> {
    rt.call("eval_builtin", json!(["str"]))?;
    copy(rt, "string", "function")?;
    literal(rt, "key", json!("repo"))?;
    method(rt, "task", "get", &["key"])?;
    if !boolean(rt, "called")? {
        literal(rt, "called", json!(""))?;
    }
    copy(rt, "argument", "called")?;
    copy(rt, "function", "string")?;
    invoke(rt, &["argument"], &[])?;
    copy(rt, "label", "called")?;
    if !boolean(rt, "label")? {
        return Ok(json!(false));
    }
    rt.call("web_compare", json!(["eq", "label", "repo"]))?;
    if boolean(rt, "comparison")? {
        return Ok(returned("comparison"));
    }
    literal(rt, "slash", json!("/"))?;
    method(rt, "label", "split", &["slash"])?;
    rt.call("stream_item", json!(["called", -1, "label_tail"]))?;
    method(rt, "repo", "split", &["slash"])?;
    rt.call("stream_item", json!(["called", -1, "repo_tail"]))?;
    rt.call("web_compare", json!(["eq", "label_tail", "repo_tail"]))?;
    Ok(returned("comparison"))
}
fn task_id(rt: &mut dyn Runtime) -> Result<()> {
    rt.call("eval_builtin", json!(["str"]))?;
    copy(rt, "string", "function")?;
    literal(rt, "key", json!("instance_id"))?;
    literal(rt, "empty", json!(""))?;
    method(rt, "task", "get", &["key", "empty"])?;
    copy(rt, "argument", "called")?;
    copy(rt, "function", "string")?;
    invoke(rt, &["argument"], &[])?;
    copy(rt, "identifier", "called")
}
fn legacy_group(rt: &mut dyn Runtime) -> Result<()> {
    literal(rt, "slash", json!("/"))?;
    method(rt, "repo", "split", &["slash"])?;
    rt.call("stream_item", json!(["called", -1, "tail"]))?;
    literal(rt, "hyphen", json!("-"))?;
    literal(rt, "underscore", json!("_"))?;
    method(rt, "tail", "replace", &["hyphen", "underscore"])?;
    copy(rt, "tok", "called")?;
    method(rt, "repo", "split", &["slash"])?;
    rt.call("stream_item", json!(["called", -1, "alt"]))?;
    literal(rt, "kept", json!([]))?;
    rt.call("workspace_iter", json!(["tasks", "iterator"]))?;
    while truthy(&rt.call("workspace_next", json!(["iterator", "task"]))?) {
        task_id(rt)?;
        method(rt, "identifier", "replace", &["hyphen", "underscore"])?;
        copy(rt, "normalized", "called")?;
        method(rt, "normalized", "startswith", &["tok"])?;
        let matched = boolean(rt, "called")?;
        let matched = if matched {
            true
        } else {
            task_id(rt)?;
            method(rt, "identifier", "startswith", &["alt"])?;
            boolean(rt, "called")?
        };
        if matched {
            rt.call("eval_append", json!(["kept", "task"]))?;
        }
    }
    if boolean(rt, "kept")? {
        copy(rt, "tasks", "kept")?;
    }
    Ok(())
}
fn load_tasks(rt: &mut dyn Runtime) -> Result<Value> {
    read_json(rt, "tasks_file", "tasks", false)?;
    if boolean(rt, "workload")? {
        function(rt, "workload_ids")?;
        invoke(rt, &["workload", "repo"], &[])?;
        copy(rt, "wanted", "called")?;
        rt.call("workspace_local", json!(["tasks", "tasks"]))?;
        literal(rt, "kept", json!([]))?;
        rt.call("workspace_iter", json!(["tasks", "iterator"]))?;
        while truthy(&rt.call("workspace_next", json!(["iterator", "task"]))?) {
            task_id(rt)?;
            if contains(rt, "identifier", "wanted")? {
                rt.call("eval_append", json!(["kept", "task"]))?;
            }
        }
        copy(rt, "tasks", "kept")?;
        if boolean(rt, "repo")? {
            literal(rt, "grouped", json!([]))?;
            rt.call("workspace_iter", json!(["tasks", "iterator"]))?;
            while truthy(&rt.call("workspace_next", json!(["iterator", "task"]))?) {
                function(rt, "_same_group")?;
                invoke(rt, &["task", "repo"], &[])?;
                if boolean(rt, "called")? {
                    rt.call("eval_append", json!(["grouped", "task"]))?;
                }
            }
            if boolean(rt, "grouped")? {
                copy(rt, "tasks", "grouped")?;
            }
        }
        if !boolean(rt, "tasks")? {
            let mut message = Text::from("no tasks left after applying workload ");
            message.0.extend(formatted(rt, "workload", "")?.0);
            if boolean(rt, "repo")? {
                append(&mut message, " (repo=");
                message.0.extend(formatted(rt, "repo", "")?.0);
                append(&mut message, ")");
            }
            append(&mut message, "; ");
            message.0.extend(formatted(rt, "tasks_file", "")?.0);
            append(&mut message, " does not contain any of its ");
            builtin(rt, "len", &["wanted"], "count")?;
            message.0.extend(formatted(rt, "count", "")?.0);
            append(&mut message, " ids");
            return exit(rt, message);
        }
    } else if boolean(rt, "repo")? {
        rt.call("workspace_local", json!(["tasks", "tasks"]))?;
        legacy_group(rt)?;
    }
    rt.call("workspace_local", json!(["tasks", "tasks"]))?;
    rt.call("workspace_iter", json!(["tasks", "iterator"]))?;
    while truthy(&rt.call("workspace_next", json!(["iterator", "task"]))?) {
        literal(rt, "key", json!("repo"))?;
        method(rt, "task", "setdefault", &["key", "repo"])?;
    }
    if boolean(rt, "limit")? {
        rt.call("workspace_slice", json!(["tasks", "limit", "tasks"]))?;
    }
    Ok(returned("tasks"))
}
fn lift_overhead(rt: &mut dyn Runtime) -> Result<Value> {
    function(rt, "ledger.OVERHEAD_KEYS")?;
    rt.call("workspace_iter", json!(["function", "iterator"]))?;
    while truthy(&rt.call("workspace_next", json!(["iterator", "key"]))?) {
        if contains(rt, "key", "row")? {
            continue;
        }
        method(rt, "srow", "get", &["key"])?;
        if truthy(&rt.call("provider_is_none", json!(["called"]))?) {
            continue;
        }
        rt.call("scaffold_item", json!(["srow", "key", "item"]))?;
        rt.call("workspace_put", json!(["row", "key", "item"]))?;
    }
    Ok(returned("row"))
}
pub fn dispatch(name: &str, rt: &mut dyn Runtime) -> Result<Value> {
    match name {
        "load_runner" => load_runner(rt),
        "load_adapter" => load_adapter(rt),
        "workload_ids" => workload_ids(rt),
        "_same_group" => same_group(rt),
        "load_tasks" => load_tasks(rt),
        "lift_overhead" => lift_overhead(rt),
        "main" => stream::main(rt),
        name if name.starts_with("stub_") => stub::run(&name[5..], rt),
        "env_path" | "_parse" | "load_env" | "require_keys" => environment::run(name, rt),
        _ => Err(Failure::new(
            "ValueError",
            format!("unknown experiment driver operation: {name}"),
        )),
    }
}
