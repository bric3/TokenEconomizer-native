use super::{
    Result, Runtime,
    providers::{copy, literal},
    runner_calls::{function, invoke},
};
use crate::value::truthy;
use serde_json::{Value, json};

fn snapshot(rt: &mut dyn Runtime, registry: &str) -> Result<()> {
    rt.call("eval_builtin", json!(["list"]))?;
    copy(rt, "list", "function")?;
    function(rt, registry)?;
    copy(rt, "live", "function")?;
    copy(rt, "function", "list")?;
    invoke(rt, &["live"], &[])?;
    copy(rt, "snapshot", "called")?;
    rt.call("cleanup_iter", json!([]))?;
    Ok(())
}

pub fn containers(rt: &mut dyn Runtime) -> Result<Value> {
    snapshot(rt, "_LIVE_CONTAINERS")?;
    while truthy(&rt.call("cleanup_next", json!([]))?) {
        function(rt, "subprocess.run")?;
        literal(rt, "docker", json!("docker"))?;
        literal(rt, "remove", json!("rm"))?;
        literal(rt, "force", json!("-f"))?;
        rt.call(
            "eval_list",
            json!(["command", ["docker", "remove", "force", "entry"]]),
        )?;
        literal(rt, "capture", json!(true))?;
        invoke(rt, &["command"], &[("capture_output", "capture")])?;
        function(rt, "_LIVE_CONTAINERS.discard")?;
        invoke(rt, &["entry"], &[])?;
    }
    Ok(Value::Null)
}

pub fn sessions(rt: &mut dyn Runtime) -> Result<Value> {
    snapshot(rt, "_LIVE_SESSIONS")?;
    while truthy(&rt.call("cleanup_next", json!([]))?) {
        let closed: Result<()> = (|| {
            rt.call("stdout_member", json!(["entry", "close", "function"]))?;
            invoke(rt, &[], &[])
        })();
        match closed {
            Err(error) if error.is_a("builtins.Exception") => {}
            result => result?,
        }
    }
    Ok(Value::Null)
}

pub fn install_signals(rt: &mut dyn Runtime) -> Result<Value> {
    rt.call("cleanup_handler", json!([]))?;
    for name in ["SIGTERM", "SIGINT", "SIGHUP"] {
        function(rt, &format!("signal.{name}"))?;
        copy(rt, name, "function")?;
    }
    for name in ["SIGTERM", "SIGINT", "SIGHUP"] {
        let installed: Result<()> = (|| {
            function(rt, "signal.signal")?;
            invoke(rt, &[name, "handler"], &[])
        })();
        match installed {
            Err(error) if error.is_a("builtins.ValueError") || error.is_a("builtins.OSError") => {}
            result => result?,
        }
    }
    Ok(Value::Null)
}

pub fn signal(rt: &mut dyn Runtime, cleanup: &str) -> Result<Value> {
    function(rt, cleanup)?;
    invoke(rt, &[], &[])?;
    function(rt, "signal.signal")?;
    copy(rt, "reset", "function")?;
    function(rt, "signal.SIG_DFL")?;
    copy(rt, "default", "function")?;
    copy(rt, "function", "reset")?;
    invoke(rt, &["signum", "default"], &[])?;
    function(rt, "os.kill")?;
    copy(rt, "kill_self", "function")?;
    function(rt, "os.getpid")?;
    invoke(rt, &[], &[])?;
    copy(rt, "pid", "called")?;
    copy(rt, "function", "kill_self")?;
    invoke(rt, &["pid", "signum"], &[])?;
    Ok(Value::Null)
}
