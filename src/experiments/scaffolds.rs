use super::swebench::{
    Failure, Result, Runtime,
    optimizer::code_points,
    providers::{boolean, copy, literal},
    runner_calls::{function, invoke, member},
};
use crate::{python_json::Text, value::truthy};
use serde_json::{Value, json};

mod merge;
mod registry;
mod repomap;
mod rtk;
pub use merge::merge_json_file;

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
fn set(rt: &mut dyn Runtime, name: &str, slot: &str) -> Result<()> {
    rt.call("stream_set", json!([name, slot])).map(|_| ())
}
fn binary(rt: &mut dyn Runtime, op: &str, a: &str, b: &str, target: &str) -> Result<()> {
    rt.call("web_binary", json!([op, a, b, target])).map(|_| ())
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
fn text(rt: &mut dyn Runtime, target: &str, value: Text) -> Result<()> {
    rt.call("logger_literal", json!([target, value.0]))
        .map(|_| ())
}
fn returned(slot: &str) -> Value {
    json!({"provider_ref":slot})
}
fn profile_new(rt: &mut dyn Runtime) -> Result<Value> {
    for name in ["kind", "home", "instructions_file"] {
        set(rt, name, name)?;
    }
    builtin(rt, "tuple")?;
    invoke(rt, &["nav_tools"], &[])?;
    set(rt, "nav_tools", "called")?;
    set(rt, "_mcp_args", "mcp_args")?;
    builtin(rt, "tuple")?;
    invoke(rt, &["patch_excludes"], &[])?;
    set(rt, "patch_excludes", "called")?;
    set(rt, "bin_dir", "bin_dir")?;
    builtin(rt, "dict")?;
    copy(rt, "dict", "function")?;
    copy(rt, "environment", "env")?;
    fallback(rt, "environment", json!({}))?;
    call(rt, "dict", &["environment"], "environment")?;
    set(rt, "env", "environment")?;
    Ok(Value::Null)
}
fn profile_mcp(rt: &mut dyn Runtime) -> Result<Value> {
    if !boolean(rt, "cfg_path")? {
        return Ok(json!([]));
    }
    builtin(rt, "list")?;
    copy(rt, "list", "function")?;
    member(rt, "_mcp_args", "function")?;
    invoke(rt, &["cfg_path"], &[])?;
    copy(rt, "mcp", "called")?;
    call(rt, "list", &["mcp"], "mcp")?;
    Ok(returned("mcp"))
}
fn context_new(rt: &mut dyn Runtime) -> Result<Value> {
    for name in [
        "profile",
        "container",
        "task",
        "run_dir",
        "exec_",
        "copy_in",
        "host_run",
    ] {
        set(rt, name, name)?;
    }
    set(rt, "_read_bytes", "read_bytes")?;
    copy(rt, "logger", "log")?;
    if !boolean(rt, "logger")? {
        rt.call(
            "eval_closure",
            json!(["logger", "logger", "context_noop", [], "scaffold"]),
        )?;
    }
    set(rt, "_log", "logger")?;
    Ok(Value::Null)
}
fn read(rt: &mut dyn Runtime) -> Result<Value> {
    member(rt, "_read_bytes", "read")?;
    if truthy(&rt.call("provider_is_none", json!(["read"]))?) {
        function(rt, "ScaffoldError")?;
        literal(
            rt,
            "message",
            json!(
                "this harness provides no `read_bytes` on the baseline context, which this baseline needs to stream the repository out of the container"
            ),
        )?;
        invoke(rt, &["message"], &[])?;
        rt.call("task_raise", json!(["called"]))?;
    }
    member(rt, "_read_bytes", "function")?;
    invoke(rt, &["script"], &[("timeout", "timeout")])?;
    Ok(returned("called"))
}
fn log(rt: &mut dyn Runtime) -> Result<Value> {
    member(rt, "_log", "function")?;
    invoke(rt, &["msg"], &[])?;
    Ok(Value::Null)
}
fn join_tmp(rt: &mut dyn Runtime, filename: &str) -> Result<()> {
    function(rt, "os.path.join")?;
    copy(rt, "join", "function")?;
    member(rt, "run_dir", "run_dir")?;
    literal(rt, "filename", json!(filename))?;
    call(rt, "join", &["run_dir", "filename"], "tmp")
}
fn write(rt: &mut dyn Runtime, json_data: bool) -> Result<()> {
    builtin(rt, "open")?;
    literal(rt, "write_mode", json!("w"))?;
    literal(rt, "utf8", json!("utf-8"))?;
    invoke(rt, &["tmp", "write_mode"], &[("encoding", "utf8")])?;
    rt.call("task_enter", json!(["file"]))?;
    let body = if json_data {
        (|| {
            function(rt, "json.dump")?;
            literal(rt, "doc", json!({}))?;
            rt.call("eval_field", json!(["doc", "mcpServers", "servers"]))?;
            literal(rt, "two", json!(2))?;
            invoke(rt, &["doc", "file_handle"], &[("indent", "two")])
        })()
    } else {
        method(rt, "file_handle", "write", &["text"], "written")
    };
    rt.call("task_exit", json!(["file", body.as_ref().err()]))?;
    if let Err(error) = body
        && !boolean(rt, "exit_result")?
    {
        return Err(error);
    }
    Ok(())
}
fn copy_tmp(rt: &mut dyn Runtime, dest: &str) -> Result<()> {
    member(rt, "copy_in", "function")?;
    invoke(rt, &["tmp", dest], &[])?;
    copy(rt, "res", "called")?;
    apply(rt, "os.unlink", &["tmp"], "removed")
}
fn success(rt: &mut dyn Runtime) -> Result<()> {
    attr(rt, "res", "returncode", "returncode")?;
    literal(rt, "zero", json!(0))?;
    rt.call("web_compare", json!(["eq", "returncode", "zero"]))?;
    Ok(())
}
fn mkdir_command(rt: &mut dyn Runtime, path: &str) -> Result<()> {
    apply(rt, "shlex.quote", &[path], "quoted")?;
    let mut command = Text::from("mkdir -p ");
    command.0.extend(formatted(rt, "quoted", "")?.0);
    text(rt, "command", command)?;
    literal(rt, "timeout", json!(30))?;
    copy(rt, "function", "exec")?;
    invoke(rt, &["command"], &[("timeout", "timeout")])
}
fn instructions(rt: &mut dyn Runtime) -> Result<Value> {
    member(rt, "profile", "profile")?;
    attr(rt, "profile", "instructions_file", "rel")?;
    literal(rt, "prefix", json!("/testbed/"))?;
    binary(rt, "add", "prefix", "rel", "path")?;
    apply(rt, "os.path.dirname", &["path"], "parent")?;
    if boolean(rt, "parent")? {
        literal(rt, "testbed", json!("/testbed"))?;
        rt.call("web_compare", json!(["ne", "parent", "testbed"]))?;
        if boolean(rt, "comparison")? {
            member(rt, "exec_", "exec")?;
            mkdir_command(rt, "parent")?;
        }
    }
    join_tmp(rt, "_scaffold_instructions")?;
    write(rt, false)?;
    copy_tmp(rt, "path")?;
    success(rt)?;
    Ok(returned("comparison"))
}
fn mcp(rt: &mut dyn Runtime) -> Result<Value> {
    member(rt, "profile", "profile")?;
    attr(rt, "profile", "home", "home")?;
    let mut cfg = formatted(rt, "home", "")?;
    push(&mut cfg, "/");
    cfg.0.extend(formatted(rt, "filename", "")?.0);
    text(rt, "cfg", cfg)?;
    join_tmp(rt, "_scaffold_mcp.json")?;
    write(rt, true)?;
    member(rt, "exec_", "exec")?;
    function(rt, "shlex.quote")?;
    copy(rt, "quote", "function")?;
    member(rt, "profile", "profile")?;
    attr(rt, "profile", "home", "home")?;
    call(rt, "quote", &["home"], "quoted")?;
    let mut command = Text::from("mkdir -p ");
    command.0.extend(formatted(rt, "quoted", "")?.0);
    text(rt, "command", command)?;
    literal(rt, "timeout", json!(30))?;
    copy(rt, "function", "exec")?;
    invoke(rt, &["command"], &[("timeout", "timeout")])?;
    copy_tmp(rt, "cfg")?;
    success(rt)?;
    Ok(if boolean(rt, "comparison")? {
        returned("cfg")
    } else {
        Value::Null
    })
}

// shortcut: the generated program uses Python until native container helpers are deployed.
const MERGE_BODY: &str = "os.makedirs(os.path.dirname(p), exist_ok=True)\ntry:\n    cur = json.load(open(p))\nexcept Exception:\n    cur = {}\nfor k, v in patch.items():\n    if isinstance(v, dict) and isinstance(cur.get(k), dict):\n        cur[k].update(v)\n    else:\n        cur[k] = v\njson.dump(cur, open(p, 'w'), indent=2)\nprint('merged')\n";

fn merge(rt: &mut dyn Runtime) -> Result<Value> {
    let mut program = Text::from("import json, os, sys\np = ");
    program.0.extend(formatted(rt, "path", "!r")?.0);
    push(&mut program, "\npatch = json.loads(");
    function(rt, "json.dumps")?;
    copy(rt, "dumps", "function")?;
    apply(rt, "json.dumps", &["patch"], "encoded")?;
    call(rt, "dumps", &["encoded"], "encoded")?;
    program.0.extend(formatted(rt, "encoded", "")?.0);
    push(&mut program, ")\n");
    push(&mut program, MERGE_BODY);
    text(rt, "text", program)?;
    function(rt, "hashlib.sha1")?;
    copy(rt, "sha1", "function")?;
    member(rt, "run_dir", "run_dir")?;
    let mut seed = formatted(rt, "run_dir", "")?;
    push(&mut seed, "|");
    seed.0.extend(formatted(rt, "path", "")?.0);
    push(&mut seed, "|");
    apply(rt, "os.getpid", &[], "pid")?;
    seed.0.extend(formatted(rt, "pid", "")?.0);
    text(rt, "seed", seed)?;
    method(rt, "seed", "encode", &[], "encoded")?;
    call(rt, "sha1", &["encoded"], "hash")?;
    method(rt, "hash", "hexdigest", &[], "hexdigest")?;
    rt.call("logger_slice", json!(["hexdigest", null, 12, "stamp"]))?;
    let mut remote = Text::from("/tmp/_scaffold_merge_");
    remote.0.extend(formatted(rt, "stamp", "")?.0);
    push(&mut remote, ".py");
    text(rt, "remote", remote)?;
    function(rt, "os.path.join")?;
    copy(rt, "join", "function")?;
    member(rt, "run_dir", "run_dir")?;
    let mut filename = Text::from("_scaffold_merge_");
    filename.0.extend(formatted(rt, "stamp", "")?.0);
    push(&mut filename, ".py");
    text(rt, "filename", filename)?;
    call(rt, "join", &["run_dir", "filename"], "tmp")?;
    write(rt, false)?;
    copy_tmp(rt, "remote")?;
    member(rt, "exec_", "exec")?;
    let mut command = Text::from("python3 ");
    command.0.extend(formatted(rt, "remote", "")?.0);
    push(&mut command, "; rm -f ");
    command.0.extend(formatted(rt, "remote", "")?.0);
    text(rt, "command", command)?;
    literal(rt, "timeout", json!(60))?;
    copy(rt, "function", "exec")?;
    invoke(rt, &["command"], &[("timeout", "timeout")])?;
    attr(rt, "called", "stdout", "stdout")?;
    fallback(rt, "stdout", json!(""))?;
    literal(rt, "marker", json!("merged"))?;
    let found = truthy(&rt.call("provider_contains", json!(["marker", "stdout"]))?);
    Ok(json!(found))
}

fn field(rt: &mut dyn Runtime, object: &str, key: &str, value: &str) -> Result<()> {
    rt.call("eval_field", json!([object, key, value]))
        .map(|_| ())
}
fn serena_server(rt: &mut dyn Runtime) -> Result<Value> {
    attr(rt, "profile", "home", "home")?;
    attr(rt, "profile", "kind", "kind")?;
    literal(rt, "codex", json!("codex"))?;
    rt.call("web_compare", json!(["eq", "kind", "codex"]))?;
    let context = if boolean(rt, "comparison")? {
        "codex"
    } else {
        "ide-assistant"
    };
    let mut command = formatted(rt, "home", "")?;
    push(&mut command, "/.local/bin/serena");
    text(rt, "command", command)?;
    literal(rt, "spec", json!({}))?;
    field(rt, "spec", "command", "command")?;
    literal(
        rt,
        "args",
        json!([
            "start-mcp-server",
            "--project",
            "/testbed",
            "--context",
            context,
            "--transport",
            "stdio",
            "--enable-web-dashboard",
            "false"
        ]),
    )?;
    field(rt, "spec", "args", "args")?;
    let mut path = formatted(rt, "home", "")?;
    push(&mut path, "/.local/bin:/usr/local/bin:/usr/bin:/bin");
    text(rt, "path", path)?;
    literal(rt, "env", json!({}))?;
    field(rt, "env", "PATH", "path")?;
    field(rt, "spec", "env", "env")?;
    Ok(returned("spec"))
}
fn serena_tool(rt: &mut dyn Runtime) -> Result<Value> {
    literal(rt, "tick", json!("`"))?;
    binary(rt, "add", "tick", "element", "quoted")?;
    binary(rt, "add", "quoted", "tick", "quoted")?;
    rt.call("eval_list", json!(["entry", ["quoted"]]))?;
    Ok(returned("entry"))
}
fn serena_nudge(rt: &mut dyn Runtime) -> Result<Value> {
    literal(rt, "separator", json!(", "))?;
    attr(rt, "separator", "join", "join")?;
    function(rt, "SERENA_TOOLS")?;
    rt.call(
        "eval_generator",
        json!(["tools_iter", "function", "serena_tool", "scaffold"]),
    )?;
    call(rt, "join", &["tools_iter"], "tools")?;
    literal(rt, "separator", json!("/"))?;
    attr(rt, "separator", "join", "join")?;
    attr(rt, "profile", "nav_tools", "nav_tools")?;
    call(rt, "join", &["nav_tools"], "forbid")?;
    fallback(rt, "forbid", json!("rg/grep/sed/cat in the shell"))?;
    let mut nudge = Text::from(
        "# MANDATORY code-navigation policy\nFor ANY code search, symbol lookup, or 'where is X defined/used' task, you MUST use the Serena MCP tools (use tool search to load them if needed): ",
    );
    nudge.0.extend(formatted(rt, "tools", "")?.0);
    push(&mut nudge, ". Do NOT use ");
    nudge.0.extend(formatted(rt, "forbid", "")?.0);
    push(
        &mut nudge,
        " to locate or browse code -- use Serena's semantic tools instead. They return precise, symbol-level results and are the required way to navigate this repository.\n",
    );
    text(rt, "nudge", nudge)?;
    Ok(returned("nudge"))
}
fn serena_setup(rt: &mut dyn Runtime) -> Result<Value> {
    attr(rt, "ctx", "profile", "profile")?;
    attr(rt, "profile", "home", "home")?;
    attr(rt, "ctx", "exec_", "exec")?;
    function(rt, "INSTALL")?;
    copy(rt, "install", "function")?;
    literal(rt, "timeout", json!(900))?;
    literal(rt, "env", json!({}))?;
    field(rt, "env", "HOME", "home")?;
    copy(rt, "function", "exec")?;
    invoke(rt, &["install"], &[("timeout", "timeout"), ("env", "env")])?;
    copy(rt, "ins", "called")?;
    attr(rt, "ins", "returncode", "returncode")?;
    literal(rt, "zero", json!(0))?;
    rt.call("web_compare", json!(["eq", "returncode", "zero"]))?;
    copy(rt, "installed", "comparison")?;
    literal(rt, "notes", json!({"arm_setup":"serena"}))?;
    field(rt, "notes", "serena_installed", "installed")?;
    attr(rt, "ins", "stdout", "stdout")?;
    fallback(rt, "stdout", json!(""))?;
    method(rt, "stdout", "strip", &[], "version")?;
    rt.call("logger_slice", json!(["version", -30, null, "version"]))?;
    field(rt, "notes", "serena_version", "version")?;
    if !boolean(rt, "installed")? {
        attr(rt, "ctx", "log", "logger")?;
        attr(rt, "ins", "stderr", "stderr")?;
        fallback(rt, "stderr", json!(""))?;
        rt.call("logger_slice", json!(["stderr", -200, null, "stderr"]))?;
        let mut message =
            Text::from("serena: install FAILED -- this task runs without the MCP server (");
        message.0.extend(formatted(rt, "stderr", "")?.0);
        push(&mut message, ")");
        text(rt, "message", message)?;
        call(rt, "logger", &["message"], "logged")?;
        literal(rt, "false", json!(false))?;
        field(rt, "notes", "serena_nudged", "false")?;
        return Ok(returned("notes"));
    }
    member(rt, "mcp_server", "server")?;
    attr(rt, "ctx", "profile", "profile")?;
    call(rt, "server", &["profile"], "spec")?;
    attr(rt, "ctx", "write_mcp_config", "writer")?;
    literal(rt, "servers", json!({}))?;
    field(rt, "servers", "serena", "spec")?;
    call(rt, "writer", &["servers"], "cfg")?;
    field(rt, "notes", "mcp_config", "cfg")?;
    attr(rt, "ctx", "profile", "profile")?;
    attr(rt, "profile", "kind", "kind")?;
    literal(rt, "codex", json!("codex"))?;
    rt.call("web_compare", json!(["eq", "kind", "codex"]))?;
    if boolean(rt, "comparison")? {
        literal(rt, "servers", json!({}))?;
        field(rt, "servers", "serena", "spec")?;
        field(rt, "notes", "mcp_servers", "servers")?;
    }
    attr(rt, "ctx", "write_instructions", "writer")?;
    member(rt, "nudge", "nudge_function")?;
    attr(rt, "ctx", "profile", "profile")?;
    call(rt, "nudge_function", &["profile"], "nudge")?;
    call(rt, "writer", &["nudge"], "nudged")?;
    field(rt, "notes", "serena_nudged", "nudged")?;
    Ok(returned("notes"))
}
pub fn dispatch(operation: &str, rt: &mut dyn Runtime) -> Result<Value> {
    if let Some(operation) = operation.strip_prefix("repomap_") {
        return repomap::dispatch(operation, rt);
    }
    if let Some(operation) = operation.strip_prefix("registry_") {
        return registry::dispatch(operation, rt);
    }
    if let Some(operation) = operation.strip_prefix("rtk_") {
        return rtk::dispatch(operation, rt);
    }
    match operation {
        "serena_server" => serena_server(rt),
        "serena_nudge" => serena_nudge(rt),
        "serena_tool" => serena_tool(rt),
        "serena_setup" => serena_setup(rt),
        "profile_new" => profile_new(rt),
        "profile_mcp" => profile_mcp(rt),
        "context_new" => context_new(rt),
        "context_read" => read(rt),
        "context_log" => log(rt),
        "context_noop" | "preflight" => Ok(Value::Null),
        "instructions" => instructions(rt),
        "mcp_config" => mcp(rt),
        "merge_json" => merge(rt),
        "mounts" => Ok(json!([])),
        "setup" => {
            builtin(rt, "NotImplementedError")?;
            invoke(rt, &[], &[])?;
            rt.call("task_raise", json!(["called"]))
        }
        _ => Err(Failure::new(
            "ValueError",
            format!("unknown scaffold operation: {operation}"),
        )),
    }
}
