use super::{
    Result, Runtime,
    providers::{copy, literal},
    runner_calls::{function, invoke, member},
};
use crate::value::truthy;
use serde_json::{Value, json};

fn method(rt: &mut dyn Runtime, field: &str, name: &str, timeout: bool) -> Result<()> {
    member(rt, field, "object")?;
    rt.call("stdout_member", json!(["object", name, "function"]))?;
    if timeout {
        literal(rt, "timeout", json!(20))?;
        invoke(rt, &[], &[("timeout", "timeout")])
    } else {
        invoke(rt, &[], &[])
    }
}
fn ignore(rt: &mut dyn Runtime, field: &str, name: &str) -> Result<()> {
    match method(rt, field, name, false) {
        Err(error) if error.is_a("builtins.Exception") => Ok(()),
        result => result,
    }
}
fn kill(rt: &mut dyn Runtime, signal: &str) -> Result<()> {
    function(rt, "os.killpg")?;
    copy(rt, "killpg", "function")?;
    function(rt, "os.getpgid")?;
    member(rt, "proc", "proc")?;
    rt.call("stdout_member", json!(["proc", "pid", "pid"]))?;
    invoke(rt, &["pid"], &[])?;
    copy(rt, "group", "called")?;
    copy(rt, "function", "killpg")?;
    invoke(rt, &["group", signal], &[])
}
pub fn close(rt: &mut dyn Runtime) -> Result<Value> {
    function(rt, "_LIVE_SESSIONS.discard")?;
    invoke(rt, &["self"], &[])?;
    ignore(rt, "httpd", "shutdown")?;
    ignore(rt, "httpd", "server_close")?;
    method(rt, "proc", "poll", false)?;
    if truthy(&rt.call("provider_is_none", json!(["called"]))?) {
        function(rt, "signal.SIGTERM")?;
        copy(rt, "term", "function")?;
        function(rt, "signal.SIGKILL")?;
        copy(rt, "kill", "function")?;
        for signal in ["term", "kill"] {
            if let Err(error) = kill(rt, signal) {
                if error.is_a("builtins.ProcessLookupError")
                    || error.is_a("builtins.PermissionError")
                {
                    break;
                }
                return Err(error);
            }
            match method(rt, "proc", "wait", true) {
                Ok(()) => break,
                Err(error) if error.is_a("subprocess.TimeoutExpired") => continue,
                Err(error) => return Err(error),
            }
        }
    }
    ignore(rt, "_log_fh", "close")?;
    literal(rt, "finished", json!(true))?;
    rt.call("stream_set", json!(["finished", "finished"]))?;
    Ok(Value::Null)
}
