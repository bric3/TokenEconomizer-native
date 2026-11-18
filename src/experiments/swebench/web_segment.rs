use super::{
    Result, Runtime,
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
fn own(rt: &mut dyn Runtime, name: &str, args: &[&str], target: &str) -> Result<()> {
    member(rt, name, "function")?;
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
fn append(rt: &mut dyn Runtime, list: &str, value: &str) -> Result<()> {
    rt.call("eval_append", json!([list, value])).map(|_| ())
}
fn constant_arg(rt: &mut dyn Runtime, value: &str) -> Result<()> {
    literal(rt, "argument", json!(value))?;
    append(rt, "argv", "argument")
}
fn list(rt: &mut dyn Runtime, target: &str, refs: &[&str]) -> Result<()> {
    rt.call("eval_list", json!([target, refs])).map(|_| ())
}
fn item(rt: &mut dyn Runtime, object: &str, key: &str, target: &str) -> Result<()> {
    rt.call("stream_item", json!([object, key, target]))
        .map(|_| ())
}
fn iter(rt: &mut dyn Runtime, object: &str, target: &str) -> Result<()> {
    rt.call("workspace_iter", json!([object, target]))
        .map(|_| ())
}
fn next(rt: &mut dyn Runtime, iterator: &str, target: &str) -> Result<bool> {
    rt.call("workspace_next", json!([iterator, target]))
        .map(|v| truthy(&v))
}
fn is_none(rt: &mut dyn Runtime, object: &str) -> Result<bool> {
    rt.call("provider_is_none", json!([object]))
        .map(|v| truthy(&v))
}
fn fallback(rt: &mut dyn Runtime, object: &str, value: Value) -> Result<()> {
    if !boolean(rt, object)? {
        literal(rt, object, value)?;
    }
    Ok(())
}
fn binary(rt: &mut dyn Runtime, op: &str, a: &str, b: &str, target: &str) -> Result<()> {
    rt.call("web_binary", json!([op, a, b, target])).map(|_| ())
}
fn compare(rt: &mut dyn Runtime, op: &str, a: &str, b: &str) -> Result<bool> {
    rt.call("web_compare", json!([op, a, b]))?;
    boolean(rt, "comparison")
}
fn formatted(rt: &mut dyn Runtime, object: &str, spec: &str) -> Result<Text> {
    code_points(rt.call("task_format", json!([object, spec]))?)
}
fn push(text: &mut Text, suffix: &str) {
    text.0.extend(suffix.chars().map(u32::from));
}
fn report(rt: &mut dyn Runtime, message: Text) -> Result<()> {
    rt.call("logger_report", json!([message.0])).map(|_| ())
}
fn clock(rt: &mut dyn Runtime) -> Result<()> {
    apply(rt, "time.time", &[], "now")
}
fn catches(rt: &mut dyn Runtime, error: &super::Failure, names: &[&str]) -> Result<bool> {
    rt.call("exception_matches", json!([error, names]))
        .map(|v| truthy(&v))
}
fn environment(rt: &mut dyn Runtime) -> Result<()> {
    builtin(rt, "dict")?;
    copy(rt, "dict", "function")?;
    function(rt, "os.environ")?;
    copy(rt, "parent_env", "function")?;
    call(rt, "dict", &["parent_env"], "env")
}
fn mkdir(rt: &mut dyn Runtime, slot: &str) -> Result<()> {
    function(rt, "os.makedirs")?;
    literal(rt, "true", json!(true))?;
    invoke(rt, &[slot], &[("exist_ok", "true")])
}
fn mkdir_home(rt: &mut dyn Runtime) -> Result<()> {
    function(rt, "os.makedirs")?;
    item(rt, "env", "HOME", "env_home")?;
    literal(rt, "true", json!(true))?;
    invoke(rt, &["env_home"], &[("exist_ok", "true")])
}
fn guard_env(rt: &mut dyn Runtime) -> Result<()> {
    own(rt, "_net_guard_log", &["home"], "guard_log")?;
    function(rt, "net_guard.LOG_ENV")?;
    copy(rt, "guard_key", "function")?;
    rt.call("workspace_put", json!(["env", "guard_key", "guard_log"]))?;
    Ok(())
}
fn path(rt: &mut dyn Runtime, copilot: bool) -> Result<()> {
    function(rt, "os.pathsep")?;
    copy(rt, "pathsep", "function")?;
    attr(rt, "pathsep", "join", "join_path")?;
    member(rt, "scaffold_bin_dir", "bin_dir")?;
    if copilot {
        member(rt, "agent_home", "agent_home")?;
        apply(rt, "os.path.dirname", &["workspace"], "run_dir_for_path")?;
        call(rt, "agent_home", &["run_dir_for_path"], "path_home")?;
    } else {
        copy(rt, "path_home", "home")?;
    }
    call(rt, "bin_dir", &["path_home"], "bin")?;
    rt.call(
        "provider_method",
        json!(["env", "get", ["PATH", ""], "old_path"]),
    )?;
    list(rt, "path_parts", &["bin", "old_path"])?;
    call(rt, "join_path", &["path_parts"], "new_path")?;
    field(rt, "env", "PATH", "new_path")
}
fn argv_claude(rt: &mut dyn Runtime) -> Result<()> {
    literal(rt, "argv", json!([]))?;
    member(rt, "claude", "binary")?;
    append(rt, "argv", "binary")?;
    for arg in [
        "-p",
        "--output-format",
        "stream-json",
        "--verbose",
        "--dangerously-skip-permissions",
        "--model",
    ] {
        constant_arg(rt, arg)?;
    }
    member(rt, "model", "model")?;
    append(rt, "argv", "model")?;
    constant_arg(rt, "--max-turns")?;
    builtin(rt, "str")?;
    invoke(rt, &["max_turns"], &[])?;
    append(rt, "argv", "called")?;
    constant_arg(rt, "--effort")?;
    member(rt, "effort", "effort")?;
    append(rt, "argv", "effort")?;
    copy(rt, "excluded", "exclude_tools")?;
    fallback(rt, "excluded", json!([]))?;
    iter(rt, "excluded", "tools_iter")?;
    literal(rt, "drop", json!([]))?;
    while next(rt, "tools_iter", "tool")? {
        member(rt, "protected_tools", "protected")?;
        if !truthy(&rt.call("provider_contains", json!(["tool", "protected"]))?) {
            append(rt, "drop", "tool")?;
        }
    }
    apply(rt, "network_tool_block", &[], "network_tools")?;
    iter(rt, "network_tools", "tools_iter")?;
    literal(rt, "network_drop", json!([]))?;
    while next(rt, "tools_iter", "tool")? {
        if !truthy(&rt.call("provider_contains", json!(["tool", "drop"]))?) {
            append(rt, "network_drop", "tool")?;
        }
    }
    binary(rt, "iadd", "drop", "network_drop", "drop")?;
    if boolean(rt, "drop")? {
        literal(rt, "comma", json!(","))?;
        method(rt, "comma", "join", &["drop"], "drop_text")?;
        literal(rt, "disallowed", json!("--disallowed-tools"))?;
        list(rt, "drop_args", &["disallowed", "drop_text"])?;
        binary(rt, "iadd", "argv", "drop_args", "argv")?;
    }
    environment(rt)?;
    member(rt, "agent_home", "agent_home")?;
    apply(rt, "os.path.dirname", &["workspace"], "run_dir")?;
    call(rt, "agent_home", &["run_dir"], "home")?;
    attr(rt, "env", "update", "update_env")?;
    literal(rt, "updates", json!({}))?;
    field(rt, "updates", "HOME", "home")?;
    function(rt, "net_guard.LOG_ENV")?;
    copy(rt, "guard_key", "function")?;
    own(rt, "_net_guard_log", &["home"], "guard_log")?;
    rt.call(
        "workspace_put",
        json!(["updates", "guard_key", "guard_log"]),
    )?;
    member(rt, "effort", "effort")?;
    field(rt, "updates", "CLAUDE_CODE_EFFORT_LEVEL", "effort")?;
    for key in [
        "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC",
        "DISABLE_TELEMETRY",
        "DISABLE_AUTOUPDATER",
    ] {
        data(rt, "updates", key, json!("1"))?;
    }
    call(rt, "update_env", &["updates"], "updated")?;
    function(rt, "agent_credential_env")?;
    member(rt, "api_key", "api_key")?;
    invoke(rt, &["env"], &[("api_key", "api_key")])?;
    mkdir_home(rt)?;
    path(rt, false)
}
fn argv_copilot(rt: &mut dyn Runtime) -> Result<()> {
    builtin(rt, "getattr")?;
    literal(rt, "segment_name", json!("segment"))?;
    literal(rt, "zero", json!(0))?;
    invoke(rt, &["guard", "segment_name", "zero"], &[])?;
    copy(rt, "segment", "called")?;
    function(rt, "os.path.join")?;
    copy(rt, "join", "function")?;
    apply(rt, "os.path.dirname", &["workspace"], "run_dir")?;
    let mut name = Text::from(".copilot-runlog-");
    apply(rt, "os.path.basename", &["workspace"], "basename")?;
    name.0.extend(formatted(rt, "basename", "")?.0);
    push(&mut name, "-");
    name.0.extend(formatted(rt, "segment", "")?.0);
    rt.call("logger_literal", json!(["log_name", name.0]))?;
    call(rt, "join", &["run_dir", "log_name"], "logdir")?;
    mkdir(rt, "logdir")?;
    function(rt, "copilot.build_argv")?;
    member(rt, "model", "model")?;
    member(rt, "effort", "effort")?;
    invoke(
        rt,
        &["prompt"],
        &[
            ("log_dir", "logdir"),
            ("model", "model"),
            ("effort", "effort"),
            ("exclude_tools", "exclude_tools"),
        ],
    )?;
    copy(rt, "argv", "called")?;
    member(rt, "copilot", "binary")?;
    rt.call("eval_field", json!(["argv", 0, "binary"]))?;
    environment(rt)?;
    attr(rt, "env", "update", "update_env")?;
    apply(rt, "copilot.run_env", &[], "run_env")?;
    call(rt, "update_env", &["run_env"], "updated")?;
    member(rt, "scaffold_copilot_home", "copilot_home")?;
    apply(rt, "os.path.dirname", &["workspace"], "run_dir")?;
    call(rt, "copilot_home", &["run_dir"], "copilot_home_value")?;
    field(rt, "env", "COPILOT_HOME", "copilot_home_value")?;
    path(rt, true)
}
fn argv_codex(rt: &mut dyn Runtime) -> Result<()> {
    apply(rt, "os.path.dirname", &["workspace"], "run_dir")?;
    own(rt, "agent_home", &["run_dir"], "home")?;
    own(rt, "_write_config", &["home", "workspace"], "configured")?;
    builtin(rt, "isinstance")?;
    copy(rt, "isinstance", "function")?;
    builtin(rt, "dict")?;
    copy(rt, "dict_type", "function")?;
    call(rt, "isinstance", &["prompt", "dict_type"], "is_resume")?;
    if boolean(rt, "is_resume")? {
        copy(rt, "resume", "prompt")?;
    } else {
        literal(rt, "resume", Value::Null)?;
    }
    if boolean(rt, "resume")? {
        item(rt, "resume", "prompt", "text")?;
    } else {
        copy(rt, "text", "prompt")?;
    }
    builtin(rt, "bool")?;
    copy(rt, "bool", "function")?;
    member(rt, "_net_guard_armed", "armed")?;
    call(rt, "bool", &["armed"], "hooked")?;
    if boolean(rt, "resume")? {
        function(rt, "codex.build_resume_argv")?;
        copy(rt, "build_argv", "function")?;
        item(rt, "resume", "session_id", "session_id")?;
        rt.call(
            "provider_method",
            json!(["resume", "get", ["effort"], "resume_effort"]),
        )?;
        copy(rt, "function", "build_argv")?;
        invoke(
            rt,
            &["session_id"],
            &[("effort", "resume_effort"), ("hook", "hooked")],
        )?;
    } else {
        function(rt, "codex.build_argv")?;
        invoke(rt, &[], &[("hook", "hooked")])?;
    }
    copy(rt, "raw_argv", "called")?;
    iter(rt, "raw_argv", "argv_iter")?;
    literal(rt, "argv", json!([]))?;
    literal(rt, "testbed", json!("/testbed"))?;
    while next(rt, "argv_iter", "arg")? {
        if compare(rt, "ne", "arg", "testbed")? {
            append(rt, "argv", "arg")?;
        } else {
            append(rt, "argv", "workspace")?;
        }
    }
    member(rt, "agent_bin", "binary")?;
    rt.call("eval_field", json!(["argv", 0, "binary"]))?;
    environment(rt)?;
    attr(rt, "env", "update", "update_env")?;
    apply(rt, "codex.run_env", &[], "run_env")?;
    call(rt, "update_env", &["run_env"], "updated")?;
    field(rt, "env", "CODEX_HOME", "home")?;
    guard_env(rt)?;
    literal(rt, "host_home_name", json!(".host-home"))?;
    apply(
        rt,
        "os.path.join",
        &["run_dir", "host_home_name"],
        "host_home",
    )?;
    field(rt, "env", "HOME", "host_home")?;
    mkdir_home(rt)?;
    path(rt, false)?;
    member(rt, "api_key", "api_key")?;
    function(rt, "codex.KEY_ENV")?;
    copy(rt, "key_env", "function")?;
    rt.call("workspace_put", json!(["env", "key_env", "api_key"]))?;
    member(rt, "answer_key_ns", "namespace")?;
    if boolean(rt, "namespace")? {
        own(rt, "_hosts_file", &["run_dir"], "hosts")?;
        apply(rt, "_shq", &["hosts"], "quoted_hosts")?;
        literal(
            rt,
            "mount_template",
            json!("mount --bind %s /etc/hosts && exec \"$@\""),
        )?;
        binary(rt, "mod", "mount_template", "quoted_hosts", "inner")?;
        literal(
            rt,
            "namespace_args",
            json!([
                "unshare",
                "--user",
                "--map-root-user",
                "--mount",
                "bash",
                "-c"
            ]),
        )?;
        append(rt, "namespace_args", "inner")?;
        literal(rt, "underscore", json!("_"))?;
        append(rt, "namespace_args", "underscore")?;
        binary(rt, "add", "namespace_args", "argv", "argv")?;
    }
    Ok(())
}
fn launch(rt: &mut dyn Runtime, copilot: bool) -> Result<()> {
    function(rt, "subprocess.Popen")?;
    copy(rt, "popen", "function")?;
    function(
        rt,
        if copilot {
            "subprocess.DEVNULL"
        } else {
            "subprocess.PIPE"
        },
    )?;
    copy(rt, "stdin", "function")?;
    function(rt, "subprocess.PIPE")?;
    copy(rt, "stdout", "function")?;
    function(rt, "subprocess.PIPE")?;
    copy(rt, "stderr", "function")?;
    literal(rt, "true", json!(true))?;
    literal(rt, "one", json!(1))?;
    copy(rt, "function", "popen")?;
    invoke(
        rt,
        &["argv"],
        &[
            ("cwd", "workspace"),
            ("env", "env"),
            ("stdin", "stdin"),
            ("stdout", "stdout"),
            ("stderr", "stderr"),
            ("text", "true"),
            ("bufsize", "one"),
            ("start_new_session", "true"),
        ],
    )?;
    copy(rt, "proc", "called")
}
fn kill_group(rt: &mut dyn Runtime, signal: &str) -> Result<()> {
    function(rt, "os.killpg")?;
    copy(rt, "killpg", "function")?;
    function(rt, "os.getpgid")?;
    attr(rt, "proc", "pid", "pid")?;
    invoke(rt, &["pid"], &[])?;
    copy(rt, "pgid", "called")?;
    function(rt, signal)?;
    copy(rt, "signal", "function")?;
    call(rt, "killpg", &["pgid", "signal"], "killed")
}
fn wait(rt: &mut dyn Runtime, stopped: bool) -> Result<()> {
    if stopped {
        let killed: Result<()> = (|| {
            kill_group(rt, "signal.SIGTERM")?;
            literal(rt, "delay", json!(2))?;
            apply(rt, "time.sleep", &["delay"], "slept")?;
            kill_group(rt, "signal.SIGKILL")
        })();
        if let Err(error) = killed
            && !catches(rt, &error, &["ProcessLookupError", "PermissionError"])?
        {
            return Err(error);
        }
        method(rt, "proc", "kill", &[], "killed")?;
    }
    let waited = (|| {
        attr(rt, "proc", "wait", "function")?;
        literal(rt, "wait_timeout", json!(60))?;
        invoke(rt, &[], &[("timeout", "wait_timeout")])
    })();
    match waited {
        Err(error) if catches(rt, &error, &["subprocess.TimeoutExpired"])? => {
            rt.call("handler_exception", json!([error]))?;
            rt.call("handler_member", json!(["proc", "kill", "function"]))?;
            rt.call("handler_invoke", json!([[], []])).map(|_| ())
        }
        result => result,
    }
}
fn stream(rt: &mut dyn Runtime, agent: &str) -> Result<bool> {
    let claude = agent == "claude";
    if claude {
        literal(rt, "result", json!({}))?;
    } else {
        literal(rt, "lines", json!([]))?;
    }
    clock(rt)?;
    member(rt, "task_timeout", "task_timeout")?;
    binary(rt, "add", "now", "task_timeout", "deadline")?;
    if claude {
        apply(rt, "_UsageRecovery", &[], "recovery")?;
        literal(rt, "stop_reason", Value::Null)?;
    }
    attr(rt, "proc", "stdout", "stdout")?;
    iter(rt, "stdout", "stdout_iter")?;
    while next(rt, "stdout_iter", "line")? {
        super::web_output::strip(rt, false, "line")?;
        if !boolean(rt, "line")? {
            continue;
        }
        attr(rt, "transcript_fh", "write", "write_line")?;
        literal(rt, "newline", json!("\n"))?;
        binary(rt, "add", "line", "newline", "record")?;
        call(rt, "write_line", &["record"], "written")?;
        method(rt, "transcript_fh", "flush", &[], "flushed")?;
        if claude {
            match apply(rt, "json.loads", &["line"], "event") {
                Err(error) if catches(rt, &error, &["Exception"])? => {
                    literal(rt, "event", json!({}))?
                }
                result => result?,
            }
            method(rt, "recovery", "observe", &["event"], "observed")?;
            rt.call(
                "provider_method",
                json!(["event", "get", ["type"], "event_type"]),
            )?;
            literal(rt, "result_type", json!("result"))?;
            if compare(rt, "eq", "event_type", "result_type")? {
                copy(rt, "result", "event")?;
            }
        } else {
            append(rt, "lines", "line")?;
        }
        if !is_none(rt, "guard")? {
            method(rt, "guard", "monitor", &["line"], "stop")?;
            if boolean(rt, "stop")? {
                if claude {
                    literal(rt, "stop_reason", json!("spiral_guard_stop"))?;
                }
                return Ok(true);
            }
        }
        clock(rt)?;
        if compare(rt, "gt", "now", "deadline")? {
            capture_log(rt)?;
            report(
                rt,
                Text::from(match agent {
                    "claude" => "webbench segment hit TE_TASK_TIMEOUT; stopping the agent",
                    "copilot" => "webbench copilot segment hit TE_TASK_TIMEOUT; stopping the agent",
                    _ => "webbench codex segment hit TE_TASK_TIMEOUT; stopping the agent",
                }),
            )?;
            if claude {
                literal(rt, "stop_reason", json!("task_timeout"))?;
            }
            return Ok(true);
        }
    }
    Ok(false)
}
fn stderr_log(rt: &mut dyn Runtime, agent: &str) -> Result<()> {
    method(rt, "err", "strip", &[], "stripped_err")?;
    if !boolean(rt, "stripped_err")? {
        return Ok(());
    }
    if agent == "claude" {
        if boolean(rt, "result")? {
            return Ok(());
        }
    } else {
        rt.call(
            "provider_method",
            json!(["usage", "get", ["num_turns"], "turns"]),
        )?;
        if boolean(rt, "turns")? {
            return Ok(());
        }
    }
    capture_log(rt)?;
    method(rt, "err", "strip", &[], "stripped_err")?;
    rt.call(
        "logger_slice",
        json!(["stripped_err", null, 300, "err_head"]),
    )?;
    let mut message = Text::from(match agent {
        "claude" => "webbench agent stderr: ",
        "copilot" => "webbench copilot stderr: ",
        _ => "agent stderr: ",
    });
    message.0.extend(formatted(rt, "err_head", "")?.0);
    report(rt, message)
}
fn refused(rt: &mut dyn Runtime, copilot: bool) -> Result<()> {
    builtin(rt, "RuntimeError")?;
    copy(rt, "error_constructor", "function")?;
    if copilot {
        let mut message = Text::from("copilot refused this configuration: ");
        message.0.extend(formatted(rt, "bad", "")?.0);
        push(&mut message, "\n  model=");
        member(rt, "model", "model")?;
        message.0.extend(formatted(rt, "model", "!r")?.0);
        push(&mut message, " effort=");
        member(rt, "effort", "effort")?;
        message.0.extend(formatted(rt, "effort", "!r")?.0);
        push(&mut message, "\n  fix COPILOT_MODEL / TE_EFFORT_LEVEL.");
        rt.call("logger_literal", json!(["error_message", message.0]))?;
    } else {
        method(rt, "err", "strip", &[], "stripped_err")?;
        rt.call(
            "logger_slice",
            json!(["stripped_err", null, 600, "err_head"]),
        )?;
        literal(
            rt,
            "error_prefix",
            json!("codex refused its config at run time:\n"),
        )?;
        binary(rt, "add", "error_prefix", "err_head", "error_message")?;
    }
    call(rt, "error_constructor", &["error_message"], "error")?;
    rt.call("task_raise", json!(["error"])).map(|_| ())
}
pub fn run(rt: &mut dyn Runtime, agent: &str) -> Result<Value> {
    match agent {
        "claude" => argv_claude(rt)?,
        "copilot" => argv_copilot(rt)?,
        _ => argv_codex(rt)?,
    }
    launch(rt, agent == "copilot")?;
    if agent != "copilot" {
        attr(rt, "proc", "stdin", "stdin_pipe")?;
        method(
            rt,
            "stdin_pipe",
            "write",
            &[if agent == "claude" { "prompt" } else { "text" }],
            "sent",
        )?;
        attr(rt, "proc", "stdin", "stdin_pipe")?;
        method(rt, "stdin_pipe", "close", &[], "closed")?;
    }
    let stopped = stream(rt, agent)?;
    wait(rt, stopped)?;
    attr(rt, "proc", "stderr", "stderr_pipe")?;
    method(rt, "stderr_pipe", "read", &[], "err")?;
    fallback(rt, "err", json!(""))?;
    rt.call(
        "logger_slice",
        json!([
            "err",
            if agent == "codex" { -1200 } else { -600 },
            null,
            "err"
        ]),
    )?;
    if agent == "claude" {
        stderr_log(rt, agent)?;
        if !boolean(rt, "result")? {
            attr(rt, "recovery", "synthesize", "synthesize")?;
            fallback(rt, "stop_reason", json!("no_result_event"))?;
            apply(rt, "_session_log_text_host", &["home"], "log_text")?;
            call(rt, "synthesize", &["stop_reason", "log_text"], "result")?;
        }
        return Ok(json!({"provider_ref":"result"}));
    }
    if agent == "copilot" {
        apply(rt, "copilot.fatal_config_error", &["err"], "bad")?;
        if boolean(rt, "bad")? {
            refused(rt, true)?;
        }
        apply(rt, "copilot.read_log_dir", &["logdir"], "log_text")?;
        function(rt, "copilot.row_usage")?;
        copy(rt, "row_usage", "function")?;
        literal(rt, "newline", json!("\n"))?;
        method(rt, "newline", "join", &["lines"], "stream_text")?;
        call(rt, "row_usage", &["stream_text", "log_text"], "usage")?;
    } else {
        method(rt, "err", "lower", &[], "lower_err")?;
        literal(rt, "unknown", json!("unknown configuration"))?;
        let mut bad = truthy(&rt.call("provider_contains", json!(["unknown", "lower_err"]))?);
        if !bad {
            method(rt, "err", "lower", &[], "lower_err")?;
            literal(rt, "loading", json!("error loading config.toml"))?;
            bad = truthy(&rt.call("provider_contains", json!(["loading", "lower_err"]))?);
        }
        if bad {
            refused(rt, false)?;
        }
        function(rt, "codex.row_usage")?;
        copy(rt, "row_usage", "function")?;
        literal(rt, "newline", json!("\n"))?;
        method(rt, "newline", "join", &["lines"], "stream_text")?;
        call(rt, "row_usage", &["stream_text"], "usage")?;
    }
    stderr_log(rt, agent)?;
    if agent == "copilot" {
        field(rt, "usage", "_debug_log", "log_text")?;
    }
    Ok(json!({"provider_ref":"usage"}))
}

fn read_session_file(rt: &mut dyn Runtime) -> Result<()> {
    builtin(rt, "open")?;
    literal(rt, "utf8", json!("utf-8"))?;
    literal(rt, "replace", json!("replace"))?;
    invoke(
        rt,
        &["session_path"],
        &[("encoding", "utf8"), ("errors", "replace")],
    )?;
    rt.call("task_enter", json!(["sessionlog"]))?;
    let read: Result<()> = (|| {
        method(rt, "sessionlog_handle", "read", &[], "part")?;
        append(rt, "parts", "part")
    })();
    rt.call("task_exit", json!(["sessionlog", read.as_ref().err()]))?;
    if let Err(error) = read
        && !boolean(rt, "exit_result")?
    {
        return Err(error);
    }
    Ok(())
}
pub fn session_log(rt: &mut dyn Runtime) -> Result<Value> {
    let read: Result<()> = (|| {
        literal(rt, "parts", json!([]))?;
        builtin(rt, "sorted")?;
        copy(rt, "sorted", "function")?;
        function(rt, "glob.glob")?;
        copy(rt, "glob", "function")?;
        function(rt, "os.path.join")?;
        for (slot, value) in [
            ("claude_dir", ".claude"),
            ("projects", "projects"),
            ("wildcard", "*"),
            ("jsonl", "*.jsonl"),
        ] {
            literal(rt, slot, json!(value))?;
        }
        invoke(
            rt,
            &["home", "claude_dir", "projects", "wildcard", "jsonl"],
            &[],
        )?;
        copy(rt, "pattern", "called")?;
        call(rt, "glob", &["pattern"], "matches")?;
        call(rt, "sorted", &["matches"], "paths")?;
        iter(rt, "paths", "paths_iter")?;
        while next(rt, "paths_iter", "session_path")? {
            match read_session_file(rt) {
                Err(error) if catches(rt, &error, &["OSError"])? => {}
                result => result?,
            }
        }
        literal(rt, "newline", json!("\n"))?;
        method(rt, "newline", "join", &["parts"], "session_text")
    })();
    match read {
        Ok(()) => Ok(json!({"provider_ref":"session_text"})),
        Err(error) if catches(rt, &error, &["Exception"])? => {
            rt.call("task_caught_log", json!([error]))?;
            let mut message = Text::from("session log unreadable (");
            push(&mut message, &error.kind);
            push(&mut message, ": ");
            message
                .0
                .extend(code_points(rt.call("handler_format", json!(["error"]))?)?.0);
            push(&mut message, ")");
            rt.call("logger_literal", json!(["error_log", message.0]))?;
            copy(rt, "function", "active_log")?;
            rt.call("handler_invoke", json!([["error_log"], []]))?;
            Ok(json!(""))
        }
        Err(error) => Err(error),
    }
}
