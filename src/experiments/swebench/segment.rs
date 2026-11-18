use super::{
    Result, Runtime, agent_stream,
    optimizer::code_points,
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
fn list(rt: &mut dyn Runtime, target: &str, slots: &[&str]) -> Result<()> {
    rt.call("eval_list", json!([target, slots])).map(|_| ())
}
fn item(rt: &mut dyn Runtime, object: &str, key: &str, target: &str) -> Result<()> {
    rt.call("stream_item", json!([object, key, target]))
        .map(|_| ())
}
fn append(rt: &mut dyn Runtime, target: &str, slot: &str) -> Result<()> {
    rt.call("eval_append", json!([target, slot])).map(|_| ())
}
fn extend(rt: &mut dyn Runtime, target: &str, slot: &str) -> Result<()> {
    rt.call("web_binary", json!(["iadd", target, slot, target]))
        .map(|_| ())
}
fn constants(rt: &mut dyn Runtime, args: &[&str]) -> Result<()> {
    literal(rt, "arguments", json!(args))?;
    extend(rt, "cmd", "arguments")
}
fn fallback(rt: &mut dyn Runtime, slot: &str, value: Value) -> Result<()> {
    if !boolean(rt, slot)? {
        literal(rt, slot, value)?;
    }
    Ok(())
}
fn format(rt: &mut dyn Runtime, slot: &str, spec: &str) -> Result<Text> {
    code_points(rt.call("task_format", json!([slot, spec]))?)
}
fn push(text: &mut Text, suffix: &str) {
    text.0.extend(suffix.chars().map(u32::from));
}
fn text(rt: &mut dyn Runtime, slot: &str, value: Text) -> Result<()> {
    rt.call("logger_literal", json!([slot, value.0]))
        .map(|_| ())
}
fn iter(rt: &mut dyn Runtime, slot: &str, target: &str) -> Result<()> {
    rt.call("workspace_iter", json!([slot, target])).map(|_| ())
}
fn next(rt: &mut dyn Runtime, iterator: &str, target: &str) -> Result<bool> {
    rt.call("workspace_next", json!([iterator, target]))
        .map(|v| truthy(&v))
}
fn environment(rt: &mut dyn Runtime) -> Result<()> {
    builtin(rt, "dict")?;
    copy(rt, "dict", "function")?;
    function(rt, "os.environ")?;
    copy(rt, "parent_env", "function")?;
    call(rt, "dict", &["parent_env"], "cenv")
}
fn activation(rt: &mut dyn Runtime, op: &str) -> Result<()> {
    own(rt, "_env_activation_args", &["name"], "activation")?;
    rt.call(
        if op == "star" {
            "eval_star"
        } else {
            "web_binary"
        },
        if op == "star" {
            json!(["cmd", "activation", "cmd"])
        } else {
            json!([op, "cmd", "activation", "cmd"])
        },
    )
    .map(|_| ())
}
fn env_args(rt: &mut dyn Runtime, env: &str) -> Result<()> {
    method(rt, env, "items", &[], "env_items")?;
    iter(rt, "env_items", "env_iter")?;
    while next(rt, "env_iter", "entry")? {
        rt.call("scaffold_pair", json!(["entry", "key", "value"]))?;
        literal(rt, "flag", json!("-e"))?;
        let mut entry = format(rt, "key", "")?;
        push(&mut entry, "=");
        entry.0.extend(format(rt, "value", "")?.0);
        text(rt, "entry_arg", entry)?;
        list(rt, "entry_args", &["flag", "entry_arg"])?;
        extend(rt, "cmd", "entry_args")?;
    }
    Ok(())
}
fn tail_argv(rt: &mut dyn Runtime) -> Result<()> {
    list(rt, "container", &["name"])?;
    rt.call("web_binary", json!(["add", "container", "argv", "tail"]))?;
    extend(rt, "cmd", "tail")
}
fn argv_claude(rt: &mut dyn Runtime) -> Result<()> {
    literal(
        rt,
        "cmd",
        json!([
            "docker",
            "exec",
            "-i",
            "-w",
            "/testbed",
            "-e",
            "ANTHROPIC_API_KEY",
            "-e",
            "HOME=/tmp/agent-home",
            "-e"
        ]),
    )?;
    function(rt, "net_guard.LOG_ENV")?;
    let mut entry = format(rt, "function", "")?;
    push(&mut entry, "=");
    member(rt, "NET_GUARD_LOG", "guard_log")?;
    entry.0.extend(format(rt, "guard_log", "")?.0);
    text(rt, "guard_env", entry)?;
    append(rt, "cmd", "guard_env")?;
    activation(rt, "star")?;
    constants(rt, &["-e", "IS_SANDBOX=1", "-e"])?;
    let mut entry = Text::from("CLAUDE_CODE_EFFORT_LEVEL=");
    member(rt, "effort", "effort")?;
    entry.0.extend(format(rt, "effort", "")?.0);
    text(rt, "effort_env", entry)?;
    append(rt, "cmd", "effort_env")?;
    constants(
        rt,
        &[
            "-e",
            "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1",
            "-e",
            "DISABLE_TELEMETRY=1",
            "-e",
            "DISABLE_AUTOUPDATER=1",
        ],
    )?;
    append(rt, "cmd", "name")?;
    constants(
        rt,
        &[
            "claude",
            "-p",
            "--output-format",
            "stream-json",
            "--verbose",
            "--dangerously-skip-permissions",
            "--model",
        ],
    )?;
    member(rt, "model", "model")?;
    append(rt, "cmd", "model")?;
    constants(rt, &["--max-turns"])?;
    builtin(rt, "str")?;
    invoke(rt, &["max_turns"], &[])?;
    append(rt, "cmd", "called")?;
    constants(rt, &["--effort"])?;
    member(rt, "effort", "effort")?;
    append(rt, "cmd", "effort")?;
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
    apply(rt, "network_tool_block", &[], "network")?;
    iter(rt, "network", "tools_iter")?;
    literal(rt, "network_drop", json!([]))?;
    while next(rt, "tools_iter", "tool")? {
        if !truthy(&rt.call("provider_contains", json!(["tool", "drop"]))?) {
            append(rt, "network_drop", "tool")?;
        }
    }
    extend(rt, "drop", "network_drop")?;
    if boolean(rt, "drop")? {
        literal(rt, "comma", json!(","))?;
        method(rt, "comma", "join", &["drop"], "dropped")?;
        literal(rt, "flag", json!("--disallowed-tools"))?;
        list(rt, "drop_args", &["flag", "dropped"])?;
        extend(rt, "cmd", "drop_args")?;
    }
    member(rt, "scaffold_profile", "profile")?;
    attr(rt, "profile", "mcp_args", "function")?;
    invoke(rt, &["mcp_config"], &[])?;
    copy(rt, "mcp_args", "called")?;
    extend(rt, "cmd", "mcp_args")?;
    environment(rt)?;
    member(rt, "api_key", "api_key")?;
    rt.call(
        "eval_field",
        json!(["cenv", "ANTHROPIC_API_KEY", "api_key"]),
    )?;
    Ok(())
}
fn argv_copilot(rt: &mut dyn Runtime) -> Result<()> {
    builtin(rt, "getattr")?;
    literal(rt, "segment_name", json!("segment"))?;
    literal(rt, "zero", json!(0))?;
    invoke(rt, &["guard", "segment_name", "zero"], &[])?;
    copy(rt, "segment", "called")?;
    function(rt, "copilot.CONTAINER_LOG_DIR")?;
    let mut dir = format(rt, "function", "")?;
    push(&mut dir, "/seg");
    dir.0.extend(format(rt, "segment", "")?.0);
    text(rt, "logdir", dir)?;
    function(rt, "_run")?;
    literal(rt, "mkdir", json!(["docker", "exec"]))?;
    append(rt, "mkdir", "name")?;
    literal(rt, "mkdir_tail", json!(["mkdir", "-p"]))?;
    extend(rt, "mkdir", "mkdir_tail")?;
    append(rt, "mkdir", "logdir")?;
    invoke(rt, &["mkdir"], &[])?;
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
            ("mcp_config", "mcp_config"),
        ],
    )?;
    copy(rt, "argv", "called")?;
    literal(
        rt,
        "cmd",
        json!([
            "docker",
            "exec",
            "-i",
            "-w",
            "/testbed",
            "-e",
            "HOME=/home/agent"
        ]),
    )?;
    activation(rt, "add")?;
    apply(rt, "copilot.provider_secret_names", &[], "secrets")?;
    iter(rt, "secrets", "secrets_iter")?;
    literal(rt, "env_flag", json!("-e"))?;
    while next(rt, "secrets_iter", "key")? {
        list(rt, "secret_args", &["env_flag", "key"])?;
        extend(rt, "cmd", "secret_args")?;
    }
    apply(rt, "copilot.run_env", &[], "run_env")?;
    env_args(rt, "run_env")?;
    tail_argv(rt)
}
fn argv_codex(rt: &mut dyn Runtime) -> Result<()> {
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
    member(rt, "_guard_python", "guard_python")?;
    call(rt, "bool", &["guard_python"], "hooked")?;
    if boolean(rt, "resume")? {
        function(rt, "codex.build_resume_argv")?;
        copy(rt, "builder", "function")?;
        item(rt, "resume", "session_id", "session_id")?;
        rt.call(
            "provider_method",
            json!(["resume", "get", ["effort"], "effort"]),
        )?;
        copy(rt, "function", "builder")?;
        invoke(
            rt,
            &["session_id"],
            &[("effort", "effort"), ("hook", "hooked")],
        )?;
    } else {
        function(rt, "codex.build_argv")?;
        invoke(rt, &[], &[("hook", "hooked")])?;
    }
    copy(rt, "argv", "called")?;
    literal(rt, "cmd", json!(["docker", "exec", "-i", "-w", "/testbed"]))?;
    activation(rt, "iadd")?;
    literal(rt, "env_flag", json!("-e"))?;
    function(rt, "codex.KEY_ENV")?;
    copy(rt, "key_env", "function")?;
    list(rt, "key_args", &["env_flag", "key_env"])?;
    extend(rt, "cmd", "key_args")?;
    function(rt, "codex.run_env")?;
    copy(rt, "run_env", "function")?;
    literal(rt, "overrides", json!({}))?;
    function(rt, "net_guard.LOG_ENV")?;
    copy(rt, "guard_key", "function")?;
    member(rt, "NET_GUARD_LOG", "guard_log")?;
    rt.call(
        "workspace_put",
        json!(["overrides", "guard_key", "guard_log"]),
    )?;
    call(rt, "run_env", &["overrides"], "run_env")?;
    env_args(rt, "run_env")?;
    tail_argv(rt)?;
    environment(rt)?;
    member(rt, "api_key", "api_key")?;
    function(rt, "codex.KEY_ENV")?;
    copy(rt, "key_env", "function")?;
    rt.call("workspace_put", json!(["cenv", "key_env", "api_key"]))?;
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
    let mut kwargs = vec![
        ("stdin", "stdin"),
        ("stdout", "stdout"),
        ("stderr", "stderr"),
        ("text", "true"),
        ("bufsize", "one"),
    ];
    if !copilot {
        kwargs.push(("env", "cenv"));
    }
    invoke(rt, &["cmd"], &kwargs)?;
    copy(rt, "proc", "called")
}
fn wait(rt: &mut dyn Runtime, stopped: bool, agent: &str) -> Result<()> {
    if stopped {
        for signal in ["-TERM", "-KILL"] {
            function(rt, "_run")?;
            literal(rt, "kill_args", json!(["docker", "exec"]))?;
            append(rt, "kill_args", "name")?;
            literal(
                rt,
                "kill_tail",
                json!([
                    "pkill",
                    signal,
                    if agent == "codex" { "-x" } else { "-f" },
                    agent
                ]),
            )?;
            extend(rt, "kill_args", "kill_tail")?;
            invoke(rt, &["kill_args"], &[])?;
            if signal == "-TERM" {
                literal(rt, "delay", json!(2))?;
                apply(rt, "time.sleep", &["delay"], "slept")?;
            }
        }
        method(rt, "proc", "kill", &[], "killed")?;
    }
    agent_stream::reap(rt)
}
pub fn run(rt: &mut dyn Runtime, agent: &str) -> Result<Value> {
    match agent {
        "claude" => argv_claude(rt)?,
        "copilot" => argv_copilot(rt)?,
        _ => argv_codex(rt)?,
    };
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
    let stopped = agent_stream::stream(rt, agent, false)?;
    wait(rt, stopped, agent)?;
    attr(rt, "proc", "stderr", "stderr_pipe")?;
    method(rt, "stderr_pipe", "read", &[], "err")?;
    fallback(rt, "err", json!(""))?;
    rt.call("logger_slice", json!(["err", -600, null, "err"]))?;
    if agent == "claude" {
        agent_stream::stderr_log(rt, agent, false)?;
        if !boolean(rt, "result")? {
            attr(rt, "recovery", "synthesize", "synthesize")?;
            fallback(rt, "stop_reason", json!("no_result_event"))?;
            own(rt, "_session_log_text", &["name"], "log_text")?;
            call(rt, "synthesize", &["stop_reason", "log_text"], "result")?;
        }
        return Ok(json!({"provider_ref":"result"}));
    }
    if agent == "copilot" {
        apply(rt, "copilot.fatal_config_error", &["err"], "bad")?;
        if boolean(rt, "bad")? {
            agent_stream::refused(rt, true, false)?;
        }
        function(rt, "_run")?;
        copy(rt, "run", "function")?;
        literal(rt, "log_cmd", json!(["docker", "exec"]))?;
        append(rt, "log_cmd", "name")?;
        literal(rt, "shell", json!(["bash", "-lc"]))?;
        extend(rt, "log_cmd", "shell")?;
        let mut command = Text::from("cat ");
        command.0.extend(format(rt, "logdir", "")?.0);
        push(&mut command, "/*.log 2>/dev/null");
        text(rt, "log_command", command)?;
        append(rt, "log_cmd", "log_command")?;
        literal(rt, "log_timeout", json!(180))?;
        copy(rt, "function", "run")?;
        invoke(rt, &["log_cmd"], &[("timeout", "log_timeout")])?;
        attr(rt, "called", "stdout", "log_text")?;
        fallback(rt, "log_text", json!(""))?;
    } else {
        literal(rt, "loading", json!("Error loading config.toml"))?;
        let mut bad = truthy(&rt.call("provider_contains", json!(["loading", "err"]))?);
        if !bad {
            literal(rt, "unknown", json!("unknown configuration field"))?;
            bad = truthy(&rt.call("provider_contains", json!(["unknown", "err"]))?);
        }
        if bad {
            agent_stream::refused(rt, false, false)?;
        }
    }
    function(
        rt,
        if agent == "copilot" {
            "copilot.row_usage"
        } else {
            "codex.row_usage"
        },
    )?;
    copy(rt, "row_usage", "function")?;
    literal(rt, "newline", json!("\n"))?;
    method(rt, "newline", "join", &["lines"], "stream_text")?;
    call(
        rt,
        "row_usage",
        if agent == "copilot" {
            &["stream_text", "log_text"]
        } else {
            &["stream_text"]
        },
        "usage",
    )?;
    agent_stream::stderr_log(rt, agent, false)?;
    if agent == "copilot" {
        rt.call("eval_field", json!(["usage", "_debug_log", "log_text"]))?;
    }
    Ok(json!({"provider_ref":"usage"}))
}
pub fn session_log(rt: &mut dyn Runtime) -> Result<Value> {
    member(rt, "scaffold_profile", "profile")?;
    attr(rt, "profile", "home", "home")?;
    let read = (|| {
        member(rt, "_exec", "exec")?;
        let mut command = Text::from("cat ");
        command.0.extend(format(rt, "home", "")?.0);
        push(&mut command, "/.claude/projects/*/*.jsonl 2>/dev/null");
        text(rt, "command", command)?;
        literal(rt, "timeout", json!(60))?;
        copy(rt, "function", "exec")?;
        invoke(rt, &["name", "command"], &[("timeout", "timeout")])?;
        attr(rt, "called", "stdout", "stdout")?;
        fallback(rt, "stdout", json!(""))
    })();
    if let Err(error) = read {
        if !truthy(&rt.call("exception_matches", json!([error, ["Exception"]]))?) {
            return Err(error);
        }
        rt.call("handler_exception", json!([error]))?;
        rt.call("handler_function", json!(["_log"]))?;
        copy(rt, "logger", "function")?;
        let mut message = Text::from("session log unreadable (");
        rt.call("handler_type", json!(["caught_error", "kind"]))?;
        message
            .0
            .extend(code_points(rt.call("handler_format", json!(["kind"]))?)?.0);
        push(&mut message, ": ");
        message
            .0
            .extend(code_points(rt.call("handler_format", json!(["caught_error"]))?)?.0);
        push(&mut message, ")");
        text(rt, "message", message)?;
        copy(rt, "function", "logger")?;
        rt.call("handler_invoke", json!([["message"], []]))?;
        return Ok(json!(""));
    }
    Ok(json!({"provider_ref":"stdout"}))
}
