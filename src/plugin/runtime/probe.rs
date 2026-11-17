use super::{NativeRuntime, path, strings, text};
use crate::plugin::{
    cli::{Error, Result, Runtime as CliRuntime},
    context::LiveContext,
    denylist,
    hosts::{
        self,
        session::{self, Environment},
    },
    probe::{self, Runtime},
    process, sidecall,
};
use serde_json::{Value, json};
use std::{path::Path, thread, time::Duration};

struct Probe<'a> {
    app: &'a mut NativeRuntime,
    ctx: Option<LiveContext>,
}
pub(super) fn run(app: &mut NativeRuntime, args: &Value) -> Result<Value> {
    let env = app.environment.clone();
    probe::run(
        args[0].as_i64().ok_or("expected session count")?,
        &args[1],
        args[2].as_bool().ok_or("expected keep flag")?,
        "copilot",
        &env,
        &mut Probe { app, ctx: None },
    )
    .map(|n| json!(n))
}
impl Probe<'_> {
    fn ctx(&self) -> Result<LiveContext> {
        self.ctx
            .clone()
            .ok_or_else(|| "probe context has not been created".into())
    }
    fn environment(&self) -> &Environment {
        &self.app.environment
    }
}
impl Runtime for Probe<'_> {
    fn say(&mut self, text: &str) -> Result<()> {
        CliRuntime::say(self.app, text)
    }
    fn call(&mut self, method: &str, args: Value) -> Result<Value> {
        Ok(match method {
            "which" => json!(process::which(text(&args[0])?)),
            "copilot_model" => json!(sidecall::copilot_model(
                text(&args[0])?,
                &self.environment().env
            )),
            "plugin_root" => {
                let base = self.app.executable.parent().unwrap_or(Path::new("."));
                json!(probe::plugin_root_candidates(
                    self.environment(),
                    &[base.join("../plugin"), base.join("..")]
                )?)
            }
            "trusted_folders" => json!(denylist::copilot_trusted_folders(
                &self.environment().env,
                &self.environment().home
            )),
            "writable" => {
                let path = path(&args[0])?;
                #[cfg(unix)]
                let value = {
                    use std::{ffi::CString, os::unix::ffi::OsStrExt};
                    let path =
                        CString::new(path.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
                    // access borrows this NUL-terminated path for the duration of the call.
                    unsafe { libc::access(path.as_ptr(), libc::W_OK) == 0 }
                };
                #[cfg(not(unix))]
                let value = !std::fs::metadata(path)?.permissions().readonly();
                json!(value)
            }
            "mkdtemp" => json!(sidecall::temporary_directory(
                ".te-probe-copilot-",
                args[0]
                    .as_str()
                    .map(Path::new)
                    .unwrap_or(&std::env::temp_dir())
            )?),
            "make_repo" => {
                let python = process::which("python3")
                    .or_else(|| process::which("python"))
                    .ok_or("the probe needs Python to run its unittest fixture")?;
                json!(probe::make_repo(
                    path(&args[0])?,
                    &python.to_string_lossy(),
                    self
                )?)
            }
            "git" => {
                let out = process::run_environment(
                    &strings(&args)?,
                    &self.environment().cwd,
                    &self.environment().env,
                    600.0,
                )?;
                if out.code != 0 {
                    return Err(Error::new("CalledProcessError", out.stderr.trim()));
                }
                Value::Null
            }
            "program" => json!(sidecall::Runtime::program(
                &mut self.app.sidecalls,
                text(&args[0])?,
                cfg!(windows),
                &self.app.environment
            )?),
            "process" => {
                let out = process::run_environment(
                    &strings(&args[0])?,
                    path(&args[1])?,
                    args[2].as_object().ok_or("expected session environment")?,
                    args[3].as_f64().ok_or("expected timeout")?,
                )?;
                json!({"stdout":out.stdout,"stderr":out.stderr})
            }
            "run_session" => probe::run_session(
                text(&args[0])?,
                &args[1],
                text(&args[2])?,
                &args[3],
                text(&args[4])?,
                &json!(600),
                self,
            )?,
            "settle" => {
                let environment = self.environment().clone();
                session::settle(
                    "copilot",
                    Some(path(&args[0])?),
                    15.0,
                    60.0,
                    &mut session::NativeRuntime {
                        environment: &environment,
                    },
                )
                .map_err(|e| e.to_string())?;
                Value::Null
            }
            "inspect_session" => {
                probe::inspect_session(text(&args[0])?, &self.environment().clone(), self)?
            }
            "wait_folded" => json!(probe::wait_folded(
                &self.ctx()?.store,
                text(&args[1])?,
                90.0,
                self
            )?),
            "time" => json!(crate::plugin::history::now()),
            "sleep" => {
                thread::sleep(Duration::from_secs(1));
                Value::Null
            }
            "refresh_catalog" => self.app.checks.refresh_catalog(
                path(&args[0])?,
                "",
                "copilot",
                &mut self.app.sidecalls,
            )?,
            "context" => {
                let env = args[1]
                    .as_object()
                    .cloned()
                    .ok_or("expected probe environment")?;
                let ctx = LiveContext::build(
                    "copilot",
                    hosts::profile("copilot").keeps_history,
                    path(&args[0])?,
                    env,
                    &self.environment().home,
                    Value::Null,
                    true,
                )?;
                let data = json!({"base":ctx.store.base});
                self.ctx = Some(ctx);
                data
            }
            "apply" => json!(denylist::apply(
                path(&args[0])?,
                &strings(&args[1])?,
                &self.ctx()?.store,
                "copilot",
                &mut denylist::NativeRuntime
            )?),
            "restore" => json!(denylist::restore(
                path(&args[0])?,
                &self.ctx()?.store,
                &[],
                "copilot",
                &mut denylist::NativeRuntime
            )?),
            _ => return Err(format!("unknown probe operation: {method}").into()),
        })
    }
}
