use super::{
    Result, Runtime,
    optimizer::{capture_log, code_points},
    providers::{boolean, copy, literal},
    runner_calls::{function, invoke, member},
};
use crate::{python_json::Text, value::truthy};
use serde_json::{Value, json};

fn assign(rt: &mut dyn Runtime, field: &str, slot: &str) -> Result<()> {
    rt.call("stream_set", json!([field, slot])).map(|_| ())
}
fn store(rt: &mut dyn Runtime, field: &str) -> Result<()> {
    assign(rt, field, "called")
}
fn attribute(rt: &mut dyn Runtime, slot: &str, name: &str, target: &str) -> Result<()> {
    rt.call("stdout_member", json!([slot, name, target]))
        .map(|_| ())
}
fn apply(rt: &mut dyn Runtime, name: &str, args: &[&str]) -> Result<()> {
    function(rt, name)?;
    invoke(rt, args, &[])
}
fn call(rt: &mut dyn Runtime, slot: &str, args: &[&str]) -> Result<()> {
    copy(rt, "function", slot)?;
    invoke(rt, args, &[])
}
fn builtin(rt: &mut dyn Runtime, name: &str) -> Result<()> {
    rt.call("eval_builtin", json!([name])).map(|_| ())
}
fn list(rt: &mut dyn Runtime, slot: &str, refs: &[&str]) -> Result<()> {
    rt.call("eval_list", json!([slot, refs])).map(|_| ())
}
fn field(rt: &mut dyn Runtime, key: &str, slot: &str) -> Result<()> {
    rt.call("provider_collect", json!([key, slot])).map(|_| ())
}
fn data(rt: &mut dyn Runtime, key: &str, value: Value) -> Result<()> {
    literal(rt, "value", value)?;
    field(rt, key, "value")
}
fn points(rt: &mut dyn Runtime, slot: &str, text: Text) -> Result<()> {
    rt.call("logger_literal", json!([slot, text.0])).map(|_| ())
}
fn formatted(rt: &mut dyn Runtime, slot: &str) -> Result<Text> {
    code_points(rt.call("logger_format", json!([slot]))?)
}
fn append(text: &mut Text, suffix: &str) {
    text.0.extend(suffix.chars().map(u32::from));
}
fn log(rt: &mut dyn Runtime, text: Text) -> Result<()> {
    rt.call("logger_report", json!([text.0])).map(|_| ())
}
fn item(rt: &mut dyn Runtime, list: &str, index: i32, target: &str) -> Result<()> {
    rt.call("stream_item", json!([list, index, target]))
        .map(|_| ())
}
fn session_path(rt: &mut dyn Runtime, name: &str, field: &str) -> Result<()> {
    function(rt, "os.path.join")?;
    member(rt, "dir", "directory")?;
    literal(rt, "filename", json!(name))?;
    invoke(rt, &["directory", "filename"], &[])?;
    store(rt, field)
}
fn start_thread(rt: &mut dyn Runtime, method: &str, field: &str, http: bool) -> Result<()> {
    function(rt, "threading.Thread")?;
    if http {
        member(rt, "httpd", "server")?;
        attribute(rt, "server", method, "target")?;
        literal(rt, "thread_kwargs", json!({"poll_interval":0.2}))?;
    } else {
        member(rt, method, "target")?;
    }
    literal(rt, "daemon", json!(true))?;
    let mut kwargs = vec![("target", "target")];
    if http {
        kwargs.push(("kwargs", "thread_kwargs"));
    }
    kwargs.push(("daemon", "daemon"));
    invoke(rt, &[], &kwargs)?;
    store(rt, field)?;
    member(rt, field, "thread")?;
    attribute(rt, "thread", "start", "function")?;
    invoke(rt, &[], &[])
}
fn config(rt: &mut dyn Runtime) -> Result<()> {
    member(rt, "_full_task_ids", "function")?;
    invoke(rt, &[], &[])?;
    copy(rt, "full", "called")?;
    literal(rt, "output", json!({}))?;
    data(rt, "models", json!([]))?;
    let mut package = Text::from("@web-bench/");
    package.0.extend(formatted(rt, "project")?.0);
    points(rt, "package", package)?;
    list(rt, "projects", &["package"])?;
    field(rt, "projects", "projects")?;
    data(rt, "agentMode", json!("http"))?;
    member(rt, "port", "port")?;
    let mut endpoint = Text::from("http://127.0.0.1:");
    endpoint.0.extend(formatted(rt, "port")?.0);
    append(&mut endpoint, "/message");
    points(rt, "endpoint", endpoint)?;
    field(rt, "agentEndPoint", "endpoint")?;
    data(rt, "httpLimit", json!(1))?;
    data(rt, "maxdop", json!(1))?;
    data(rt, "taskMode", json!("sequential"))?;
    field(rt, "retry", "retry")?;
    field(rt, "logLevel", "log_level")?;
    data(rt, "production", json!(true))?;
    member(rt, "hash", "hash")?;
    field(rt, "hash", "hash")?;
    member(rt, "dir", "directory")?;
    field(rt, "agentDir", "directory")?;
    copy(rt, "config", "output")?;

    literal(rt, "ids", json!([]))?;
    rt.call("eval_iter", json!([]))?;
    while truthy(&rt.call("eval_next", json!([]))?) {
        rt.call(
            "provider_method",
            json!(["task", "get", ["instance_id"], "identifier"]),
        )?;
        rt.call("eval_append", json!(["ids", "identifier"]))?;
    }
    if boolean(rt, "ids")? && boolean(rt, "full")? {
        rt.call("web_compare", json!(["ne", "ids", "full"]))?;
        if boolean(rt, "comparison")? {
            item(rt, "ids", 0, "first")?;
            item(rt, "ids", -1, "last")?;
            rt.call("eval_field", json!(["config", "startTask", "first"]))?;
            rt.call("eval_field", json!(["config", "endTask", "last"]))?;
            capture_log(rt)?;
            let mut message = Text::from("webbench ");
            message.0.extend(formatted(rt, "project")?.0);
            append(&mut message, ": task range ");
            item(rt, "ids", 0, "first")?;
            message.0.extend(formatted(rt, "first")?.0);
            append(&mut message, "..");
            item(rt, "ids", -1, "last")?;
            message.0.extend(formatted(rt, "last")?.0);
            append(&mut message, " is a SUBSET of the ");
            builtin(rt, "len")?;
            invoke(rt, &["full"], &[])?;
            message.0.extend(formatted(rt, "called")?.0);
            append(
                &mut message,
                "-task stream -- the evaluator drops to development mode (Playwright timeout 2s, not 30s), so grades are stricter than a full run",
            );
            log(rt, message)?;
        }
    }
    session_path(rt, "evaluator_config.json", "config_path")?;
    builtin(rt, "open")?;
    member(rt, "config_path", "config_path")?;
    literal(rt, "write", json!("w"))?;
    literal(rt, "encoding", json!("utf-8"))?;
    invoke(rt, &["config_path", "write"], &[("encoding", "encoding")])?;
    rt.call("eval_enter", json!([]))?;
    let written: Result<()> = (|| {
        function(rt, "json.dump")?;
        literal(rt, "indent", json!(2))?;
        invoke(rt, &["config", "config_handle"], &[("indent", "indent")])
    })();
    rt.call("eval_exit", json!([written.as_ref().err()]))?;
    if let Err(error) = written
        && !boolean(rt, "exit_result")?
    {
        return Err(error);
    }
    Ok(())
}
fn launch(rt: &mut dyn Runtime) -> Result<()> {
    function(rt, "os.path.join")?;
    for (key, value) in [
        ("apps", "apps"),
        ("eval", "eval"),
        ("dist", "dist"),
        ("index", "index.js"),
    ] {
        literal(rt, key, json!(value))?;
    }
    invoke(rt, &["repo", "apps", "eval", "dist", "index"], &[])?;
    copy(rt, "entry", "called")?;
    apply(rt, "os.path.exists", &["entry"])?;
    if !boolean(rt, "called")? {
        let mut message = Text::from("the Web-Bench evaluator is not built: ");
        message.0.extend(formatted(rt, "entry")?.0);
        append(
            &mut message,
            " missing. Run `rush build --to @web-bench/eval` in ",
        );
        message.0.extend(formatted(rt, "repo")?.0);
        append(&mut message, ".");
        rt.call("runner_raise", json!([message.0]))?;
    }
    builtin(rt, "dict")?;
    copy(rt, "dict", "function")?;
    function(rt, "os.environ")?;
    copy(rt, "source_env", "function")?;
    call(rt, "dict", &["source_env"])?;
    copy(rt, "env", "called")?;
    attribute(rt, "env", "update", "update")?;
    literal(rt, "output", json!({}))?;
    data(rt, "NO_COLOR", json!("1"))?;
    data(rt, "FORCE_COLOR", json!("0"))?;
    builtin(rt, "str")?;
    invoke(rt, &["max_workers"], &[])?;
    field(rt, "MAX_TEST_WORKERS", "called")?;
    call(rt, "update", &["output"])?;
    attribute(rt, "env", "update", "update")?;
    function(rt, "webbench_undici_env")?;
    invoke(rt, &["env"], &[("repo", "repo")])?;
    copy(rt, "undici_env", "called")?;
    call(rt, "update", &["undici_env"])?;
    builtin(rt, "open")?;
    member(rt, "log_path", "log_path")?;
    invoke(rt, &["log_path", "write"], &[("encoding", "encoding")])?;
    store(rt, "_log_fh")?;
    function(rt, "subprocess.Popen")?;
    copy(rt, "popen", "function")?;
    literal(rt, "auto_run", json!("--auto-run"))?;
    literal(rt, "config_flag", json!("--config"))?;
    member(rt, "config_path", "config_path")?;
    literal(rt, "without_local", json!("--without-local-config"))?;
    list(
        rt,
        "argv",
        &[
            "node",
            "entry",
            "auto_run",
            "config_flag",
            "config_path",
            "without_local",
        ],
    )?;
    apply(rt, "os.path.join", &["repo", "apps", "eval"])?;
    copy(rt, "cwd", "called")?;
    function(rt, "subprocess.PIPE")?;
    copy(rt, "stdout", "function")?;
    function(rt, "subprocess.STDOUT")?;
    copy(rt, "stderr", "function")?;
    literal(rt, "text", json!(true))?;
    literal(rt, "one", json!(1))?;
    literal(rt, "new_session", json!(true))?;
    copy(rt, "function", "popen")?;
    invoke(
        rt,
        &["argv"],
        &[
            ("cwd", "cwd"),
            ("env", "env"),
            ("stdout", "stdout"),
            ("stderr", "stderr"),
            ("text", "text"),
            ("bufsize", "one"),
            ("start_new_session", "new_session"),
        ],
    )?;
    store(rt, "proc")?;
    apply(rt, "_LIVE_SESSIONS.add", &["self"])?;
    start_thread(rt, "_read_stdout", "_reader", false)?;
    capture_log(rt)?;
    let mut message = Text::from("webbench ");
    message.0.extend(formatted(rt, "project")?.0);
    append(&mut message, ": evaluator pid=");
    member(rt, "proc", "proc")?;
    attribute(rt, "proc", "pid", "pid")?;
    message.0.extend(formatted(rt, "pid")?.0);
    append(&mut message, " endpoint=:");
    member(rt, "port", "port")?;
    message.0.extend(formatted(rt, "port")?.0);
    append(&mut message, " retry=");
    message.0.extend(formatted(rt, "retry")?.0);
    append(&mut message, " log=");
    member(rt, "log_path", "log_path")?;
    message.0.extend(formatted(rt, "log_path")?.0);
    log(rt, message)
}
pub fn start(rt: &mut dyn Runtime) -> Result<Value> {
    for (field, slot) in [
        ("project", "project"),
        ("repo", "repo"),
        ("dir", "session_dir"),
        ("retry", "retry"),
        ("reply_timeout", "reply_timeout"),
    ] {
        assign(rt, field, slot)?;
    }
    apply(rt, "queue.Queue", &[])?;
    store(rt, "requests")?;
    apply(rt, "queue.Queue", &[])?;
    store(rt, "events")?;
    literal(rt, "false", json!(false))?;
    assign(rt, "finished", "false")?;
    literal(rt, "none", Value::Null)?;
    assign(rt, "exit_code", "none")?;
    literal(rt, "clock_format", json!("%Y%m%d-%H%M%S"))?;
    apply(rt, "time.strftime", &["clock_format"])?;
    store(rt, "hash")?;
    function(rt, "os.makedirs")?;
    member(rt, "dir", "directory")?;
    literal(rt, "true", json!(true))?;
    invoke(rt, &["directory"], &[("exist_ok", "true")])?;
    session_path(rt, "evaluator.log", "log_path")?;
    builtin(rt, "type")?;
    literal(rt, "handler_name", json!("_BoundAgentHandler"))?;
    rt.call(
        "provider_constant",
        json!(["handler_base", "_AgentHandler"]),
    )?;
    rt.call("provider_tuple", json!(["handler_base"]))?;
    literal(rt, "output", json!({}))?;
    field(rt, "session", "self")?;
    invoke(rt, &["handler_name", "tuple", "output"], &[])?;
    copy(rt, "handler", "called")?;
    function(rt, "ThreadingHTTPServer")?;
    literal(rt, "host", json!("127.0.0.1"))?;
    literal(rt, "zero", json!(0))?;
    rt.call("provider_tuple", json!(["host", "zero"]))?;
    invoke(rt, &["tuple", "handler"], &[])?;
    store(rt, "httpd")?;
    member(rt, "httpd", "server")?;
    attribute(rt, "server", "server_address", "address")?;
    item(rt, "address", 1, "port")?;
    assign(rt, "port", "port")?;
    start_thread(rt, "serve_forever", "_http_thread", true)?;
    config(rt)?;
    launch(rt)?;
    Ok(Value::Null)
}
