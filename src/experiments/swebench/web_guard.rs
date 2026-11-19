use super::{
    Failure, Result, Runtime,
    optimizer::{capture_log, code_points},
    providers::{boolean, copy, literal},
    runner_calls::{function, invoke, member},
};
use crate::{python_json::Text, value::truthy};
use serde_json::{Value, json};

fn attr(rt: &mut dyn Runtime, object: &str, name: &str, target: &str) -> Result<()> {
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
fn own(rt: &mut dyn Runtime, name: &str, args: &[&str], target: &str) -> Result<()> {
    member(rt, name, "function")?;
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
    attr(rt, object, name, "function")?;
    invoke(rt, args, &[])?;
    copy(rt, target, "called")
}
fn field(rt: &mut dyn Runtime, object: &str, key: &str, value: &str) -> Result<()> {
    rt.call("eval_field", json!([object, key, value]))
        .map(|_| ())
}
fn data(rt: &mut dyn Runtime, object: &str, key: &str, value: Value) -> Result<()> {
    literal(rt, "literal", value)?;
    field(rt, object, key, "literal")
}
fn binary(rt: &mut dyn Runtime, op: &str, a: &str, b: &str, target: &str) -> Result<()> {
    rt.call("web_binary", json!([op, a, b, target])).map(|_| ())
}
fn catches(rt: &mut dyn Runtime, error: &Failure, names: &[&str]) -> Result<bool> {
    rt.call("exception_matches", json!([error, names]))
        .map(|v| truthy(&v))
}
fn formatted(rt: &mut dyn Runtime, slot: &str) -> Result<Text> {
    code_points(rt.call("task_format", json!([slot, ""]))?)
}
fn push(text: &mut Text, suffix: &str) {
    text.0.extend(suffix.chars().map(u32::from));
}
fn returned(slot: &str) -> Value {
    json!({"provider_ref":slot})
}
fn report(rt: &mut dyn Runtime, message: Text) -> Result<()> {
    rt.call("logger_report", json!([message.0])).map(|_| ())
}
fn join(rt: &mut dyn Runtime, base: &str, suffix: &str) -> Result<Value> {
    function(rt, "os.path.join")?;
    literal(rt, "suffix", json!(suffix))?;
    invoke(rt, &[base, "suffix"], &[])?;
    Ok(returned("called"))
}
fn settings(rt: &mut dyn Runtime, agent: &str) -> Result<Value> {
    if agent == "codex" {
        return join(rt, "home", "hooks.json");
    }
    function(rt, "os.path.join")?;
    copy(rt, "join", "function")?;
    if agent == "copilot" {
        own(
            rt,
            "scaffold_copilot_home",
            &["run_dir_path"],
            "settings_home",
        )?;
        literal(rt, "name", json!("settings.json"))?;
        call(rt, "join", &["settings_home", "name"], "path")?;
    } else {
        literal(rt, "claude", json!(".claude"))?;
        literal(rt, "name", json!("settings.json"))?;
        call(rt, "join", &["home", "claude", "name"], "path")?;
    }
    Ok(returned("path"))
}
fn entry(rt: &mut dyn Runtime, copilot: bool) -> Result<Value> {
    literal(rt, "entry", json!({}))?;
    data(
        rt,
        "entry",
        "matcher",
        json!(if copilot { "bash" } else { "Bash" }),
    )?;
    if copilot {
        data(rt, "entry", "type", json!("command"))?;
        field(rt, "entry", "command", "command")?;
    } else {
        literal(rt, "hook", json!({}))?;
        data(rt, "hook", "type", json!("command"))?;
        field(rt, "hook", "command", "command")?;
        rt.call("eval_list", json!(["hooks", ["hook"]]))?;
        field(rt, "entry", "hooks", "hooks")?;
    }
    Ok(returned("entry"))
}
fn open(rt: &mut dyn Runtime, path: &str, write: bool) -> Result<()> {
    builtin(rt, "open")?;
    literal(rt, "utf8", json!("utf-8"))?;
    if write {
        literal(rt, "write", json!("w"))?;
        invoke(rt, &[path, "write"], &[("encoding", "utf8")])
    } else {
        invoke(rt, &[path], &[("encoding", "utf8")])
    }
}
fn exit(rt: &mut dyn Runtime, context: &str, body: Result<()>) -> Result<bool> {
    rt.call("task_exit", json!([context, body.as_ref().err()]))?;
    match body {
        Ok(()) => Ok(true),
        Err(error) => {
            if boolean(rt, "exit_result")? {
                Ok(false)
            } else {
                Err(error)
            }
        }
    }
}
fn load(rt: &mut dyn Runtime) -> Result<()> {
    open(rt, "path", false)?;
    rt.call("task_enter", json!(["settings"]))?;
    let body = apply(rt, "json.load", &["settings_handle"], "settings");
    exit(rt, "settings", body).map(|_| ())
}
fn launcher(rt: &mut dyn Runtime) -> Result<()> {
    join(rt, "home", "net_guard.py")?;
    copy(rt, "launcher", "called")?;
    apply(rt, "os.path.abspath", &["launcher"], "launcher")
}
fn command(rt: &mut dyn Runtime) -> Result<()> {
    let path = formatted(rt, "launcher")?;
    let mut command = Text::default();
    if !path.0.is_empty()
        && path.0.iter().all(|point| {
            u8::try_from(*point)
                .is_ok_and(|byte| byte.is_ascii_alphanumeric() || b"_@%+=:,./-".contains(&byte))
        })
    {
        command = path;
    } else {
        push(&mut command, "'");
        for point in path.0 {
            if point == 39 {
                push(&mut command, "'\"'\"'");
            } else {
                command.0.push(point);
            }
        }
        push(&mut command, "'");
    }
    rt.call("logger_literal", json!(["command", command.0]))
        .map(|_| ())
}
fn install(rt: &mut dyn Runtime) -> Result<Value> {
    own(
        rt,
        "net_guard_settings_path",
        &["home", "run_dir_path"],
        "path",
    )?;
    function(rt, "os.makedirs")?;
    copy(rt, "makedirs", "function")?;
    apply(rt, "os.path.dirname", &["path"], "parent")?;
    literal(rt, "true", json!(true))?;
    copy(rt, "function", "makedirs")?;
    invoke(rt, &["parent"], &[("exist_ok", "true")])?;
    match load(rt) {
        Err(error) if catches(rt, &error, &["Exception"])? => literal(rt, "settings", json!({}))?,
        result => result?,
    }
    member(rt, "net_guard_hook_entry", "hook_entry")?;
    launcher(rt)?;
    command(rt)?;
    call(rt, "hook_entry", &["command"], "entry")?;
    rt.call("workspace_local", json!(["settings", "settings_value"]))?;
    rt.call(
        "provider_method",
        json!(["settings_value", "setdefault", ["hooks", {}], "hooks"]),
    )?;
    attr(rt, "hooks", "setdefault", "setdefault")?;
    member(rt, "NET_GUARD_HOOK_KEY", "hook_key")?;
    literal(rt, "empty", json!([]))?;
    call(rt, "setdefault", &["hook_key", "empty"], "pre")?;
    builtin(rt, "any")?;
    copy(rt, "any", "function")?;
    rt.call(
        "eval_generator",
        json!(["entries", "pre", "network_hook_element"]),
    )?;
    call(rt, "any", &["entries"], "present")?;
    if !boolean(rt, "present")? {
        method(rt, "pre", "append", &["entry"], "appended")?;
    }
    open(rt, "path", true)?;
    rt.call("task_enter", json!(["settings_out"]))?;
    let body = (|| {
        function(rt, "json.dump")?;
        literal(rt, "two", json!(2))?;
        invoke(
            rt,
            &["settings", "settings_out_handle"],
            &[("indent", "two")],
        )
    })();
    exit(rt, "settings_out", body)?;
    own(rt, "_verify_net_guard", &["home"], "verified")?;
    Ok(returned("verified"))
}
fn install_codex(rt: &mut dyn Runtime) -> Result<Value> {
    builtin(rt, "bool")?;
    copy(rt, "bool", "function")?;
    rt.call("runner_super", json!(["_install_net_guard"]))?;
    invoke(rt, &["home", "run_dir_path"], &[])?;
    copy(rt, "installed", "called")?;
    call(rt, "bool", &["installed"], "armed")?;
    rt.call("stream_set", json!(["_net_guard_armed", "armed"]))?;
    member(rt, "_net_guard_armed", "armed")?;
    Ok(returned("armed"))
}
fn verify(rt: &mut dyn Runtime) -> Result<Value> {
    function(rt, "json.dumps")?;
    copy(rt, "dumps", "function")?;
    literal(rt, "payload", json!({"tool_name":"Bash","toolName":"bash"}))?;
    for key in ["tool_input", "toolArgs"] {
        member(rt, "NET_GUARD_PROBE", "command")?;
        literal(rt, "input", json!({}))?;
        field(rt, "input", "command", "command")?;
        field(rt, "payload", key, "input")?;
    }
    call(rt, "dumps", &["payload"], "probe")?;
    builtin(rt, "dict")?;
    copy(rt, "dict", "function")?;
    function(rt, "os.environ")?;
    copy(rt, "environ", "function")?;
    call(rt, "dict", &["environ"], "env")?;
    own(rt, "_net_guard_log", &["home"], "log_path")?;
    function(rt, "net_guard.LOG_ENV")?;
    copy(rt, "log_key", "function")?;
    rt.call("workspace_put", json!(["env", "log_key", "log_path"]))?;
    let ran: Result<()> = (|| {
        function(rt, "subprocess.run")?;
        copy(rt, "run", "function")?;
        launcher(rt)?;
        // Create the task home for Copilot, whose settings live in a separate directory.
        function(rt, "os.makedirs")?;
        literal(rt, "true", json!(true))?;
        invoke(rt, &["home"], &[("exist_ok", "true")])?;
        let helper = std::env::current_exe()?
            .with_file_name(format!("te-net-guard{}", std::env::consts::EXE_SUFFIX));
        let helper = helper
            .to_str()
            .ok_or_else(|| Failure::new("UnicodeError", "native guard path must be UTF-8"))?;
        // Keep the public filename while executing the native guard without a Python runtime.
        let program = format!(
            "#!/bin/sh\nexec {} \"$@\"\n",
            super::container_helper::quote(helper)
        );
        rt.call("host_guard_write", json!(["launcher", program]))?;
        rt.call("eval_list", json!(["argv", ["launcher"]]))?;
        literal(rt, "true", json!(true))?;
        literal(rt, "timeout", json!(60))?;
        copy(rt, "function", "run")?;
        invoke(
            rt,
            &["argv"],
            &[
                ("input", "probe"),
                ("capture_output", "true"),
                ("text", "true"),
                ("timeout", "timeout"),
                ("env", "env"),
            ],
        )?;
        copy(rt, "process", "called")
    })();
    if let Err(error) = ran {
        if !catches(rt, &error, &["Exception"])? {
            return Err(error);
        }
        rt.call("task_caught_log", json!([error]))?;
        let mut message = Text::from("net guard verify FAILED (");
        push(&mut message, &error.kind);
        push(&mut message, ": ");
        message
            .0
            .extend(code_points(rt.call("handler_format", json!(["error"]))?)?.0);
        push(&mut message, ") -- row runs UNGUARDED");
        rt.call("logger_literal", json!(["message", message.0]))?;
        copy(rt, "function", "active_log")?;
        rt.call("handler_invoke", json!([["message"], []]))?;
        return Ok(json!(false));
    }
    attr(rt, "process", "returncode", "rc")?;
    literal(rt, "two", json!(2))?;
    rt.call("web_compare", json!(["ne", "rc", "two"]))?;
    if boolean(rt, "comparison")? {
        capture_log(rt)?;
        let mut message = Text::from("net guard installed but did NOT refuse its probe (rc=");
        attr(rt, "process", "returncode", "rc")?;
        message.0.extend(formatted(rt, "rc")?.0);
        push(&mut message, ") -- row runs UNGUARDED");
        report(rt, message)?;
        return Ok(json!(false));
    }
    let removed = (|| {
        function(rt, "os.remove")?;
        copy(rt, "remove", "function")?;
        own(rt, "_net_guard_log", &["home"], "log_path")?;
        call(rt, "remove", &["log_path"], "removed")
    })();
    if let Err(error) = removed
        && !catches(rt, &error, &["OSError"])?
    {
        return Err(error);
    }
    Ok(json!(true))
}
fn blocks(rt: &mut dyn Runtime) -> Result<Value> {
    member(rt, "_net_guard_log", "guard_log")?;
    own(rt, "agent_home", &["run_dir_path"], "home")?;
    call(rt, "guard_log", &["home"], "path")?;
    let counted: Result<Value> = (|| {
        open(rt, "path", false)?;
        rt.call("task_enter", json!(["guardlog"]))?;
        let body = (|| {
            builtin(rt, "sum")?;
            copy(rt, "sum", "function")?;
            rt.call(
                "eval_generator",
                json!(["lines", "guardlog_handle", "network_line_element"]),
            )?;
            call(rt, "sum", &["lines"], "count")
        })();
        // A suppressed body error leaves the source's return statement unexecuted.
        if exit(rt, "guardlog", body)? {
            Ok(returned("count"))
        } else {
            Ok(Value::Null)
        }
    })();
    match counted {
        Err(error) if catches(rt, &error, &["OSError"])? => Ok(json!(0)),
        result => result,
    }
}
fn hosts(rt: &mut dyn Runtime) -> Result<Value> {
    join(rt, "run_dir_path", ".hosts-block")?;
    copy(rt, "path", "called")?;
    literal(rt, "hosts_path", json!("/etc/hosts"))?;
    let read = (|| {
        open(rt, "hosts_path", false)?;
        rt.call("task_enter", json!(["hostfile"]))?;
        let body = method(rt, "hostfile_handle", "read", &[], "base");
        exit(rt, "hostfile", body).map(|_| ())
    })();
    match read {
        Err(error) if catches(rt, &error, &["OSError"])? => {
            literal(rt, "base", json!("127.0.0.1 localhost\n"))?
        }
        result => result?,
    }
    open(rt, "path", true)?;
    rt.call("task_enter", json!(["hostfile_out"]))?;
    let body = (|| {
        attr(rt, "hostfile_out_handle", "write", "write_hosts")?;
        rt.call("workspace_local", json!(["base", "base_value"]))?;
        literal(rt, "newline", json!("\n"))?;
        method(rt, "base_value", "rstrip", &["newline"], "base_text")?;
        binary(rt, "add", "base_text", "newline", "host_text")?;
        apply(rt, "net_guard.hosts_block", &[], "block")?;
        binary(rt, "add", "host_text", "block", "host_text")?;
        binary(rt, "add", "host_text", "newline", "host_text")?;
        call(rt, "write_hosts", &["host_text"], "written")
    })();
    exit(rt, "hostfile_out", body)?;
    Ok(returned("path"))
}
fn quote(rt: &mut dyn Runtime) -> Result<Value> {
    builtin(rt, "str")?;
    invoke(rt, &["s"], &[])?;
    copy(rt, "text", "called")?;
    literal(rt, "quote", json!("'"))?;
    literal(rt, "escaped", json!("'\\''"))?;
    method(rt, "text", "replace", &["quote", "escaped"], "replaced")?;
    binary(rt, "add", "quote", "replaced", "quoted")?;
    binary(rt, "add", "quoted", "quote", "quoted")?;
    Ok(returned("quoted"))
}
pub fn dispatch(operation: &str, rt: &mut dyn Runtime) -> Result<Value> {
    match operation {
        "log" => join(rt, "home", "net_guard_blocks.jsonl"),
        "settings_claude" => settings(rt, "claude"),
        "settings_copilot" => settings(rt, "copilot"),
        "settings_codex" => settings(rt, "codex"),
        "entry_claude" => entry(rt, false),
        "entry_copilot" => entry(rt, true),
        "install" => install(rt),
        "install_codex" => install_codex(rt),
        "verify" => verify(rt),
        "blocks" => blocks(rt),
        "hosts" => hosts(rt),
        "quote" => quote(rt),
        "hook_element" => {
            apply(rt, "json.dumps", &["element"], "encoded")?;
            literal(rt, "needle", json!("net_guard"))?;
            let present = truthy(&rt.call("provider_contains", json!(["needle", "encoded"]))?);
            Ok(json!([present]))
        }
        "line_element" => {
            method(rt, "element", "strip", &[], "line")?;
            Ok(if boolean(rt, "line")? {
                json!([1])
            } else {
                json!([])
            })
        }
        _ => Err(Failure::new(
            "ValueError",
            format!("unknown Web-Bench network helper: {operation}"),
        )),
    }
}
