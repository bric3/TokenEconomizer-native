use crate::{
    job::{self, Channel},
    storage_dispatch,
};
use serde_json::{Value, json};
use std::{
    io,
    path::{Path, PathBuf},
};
use tokeneconomizer::{
    plugin::{
        context::{Entrypoint, LiveContext, Runtime},
        process::{self, Launch},
        winspawn,
    },
    store::Store,
};

pub(super) fn context(raw: &Value) -> Result<LiveContext, String> {
    Ok(LiveContext {
        root: PathBuf::from(raw["root"].as_str().unwrap_or(".")),
        store: Store::new(Path::new(
            raw["base"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or("."),
        ))
        .map_err(|e| e.to_string())?,
        repo: serde_json::from_value(
            raw.get("repo")
                .cloned()
                .unwrap_or(json!({"root":".","key":"","label":"","remote":"","how":""})),
        )
        .map_err(|e| e.to_string())?,
        host: raw["host"].as_str().unwrap_or("claude").into(),
        keeps_history: raw["keeps_history"] == true,
        config: raw.get("config").cloned().unwrap_or(json!({})),
        env: raw["env"].as_object().cloned().unwrap_or_default(),
        home: PathBuf::from(raw["home"].as_str().unwrap_or(".")),
        unsettled: serde_json::from_value(raw.get("unsettled").cloned().unwrap_or(Value::Null))
            .map_err(|e| e.to_string())?,
    })
}
pub(super) fn snapshot(ctx: &LiveContext) -> Value {
    json!({"root":ctx.root,"base":ctx.store.base,"repo":ctx.repo,"host":ctx.host,"keeps_history":ctx.keeps_history,"config":ctx.config,"env":ctx.env,"home":ctx.home,"unsettled":ctx.unsettled})
}
struct Transport {
    channel: Channel,
    flags: Value,
}
impl Transport {
    fn request(&self, method: &str, args: Value) -> io::Result<Value> {
        let reply = self.channel.call(method, args).map_err(io::Error::other)?;
        if let Some(error) = reply["error"].as_str() {
            return Err(io::Error::other(error.to_string()));
        }
        Ok(reply["value"].clone())
    }
}
impl Runtime for Transport {
    fn headless(
        &mut self,
        env: &serde_json::Map<String, Value>,
        transcript: Option<&str>,
    ) -> Result<bool, String> {
        self.request("headless", json!([env, transcript]))
            .map(|v| v == true)
            .map_err(|e| e.to_string())
    }
    fn spawn(
        &mut self,
        request: &Launch,
        entrypoint: &Entrypoint,
        windows: bool,
    ) -> Result<bool, String> {
        if windows {
            winspawn::spawn(
                request,
                &entrypoint.executable,
                &entrypoint.windows_runner,
                self,
            )
            .map_err(|e| e.to_string())
        } else if self.flags["popen"] == true {
            self.request("popen", json!([request, 0, true]))
                .map(|_| true)
                .map_err(|e| e.to_string())
        } else {
            process::spawn_logged(
                &request.cmd,
                &request.cwd,
                &request.env,
                &request.log,
                0,
                true,
            )
            .map(|_| true)
            .map_err(|e| e.to_string())
        }
    }
}
impl winspawn::Runtime for Transport {
    fn launch(&mut self, spec: &Launch, flags: u32) -> io::Result<()> {
        if self.flags["popen"] == true {
            self.request("popen", json!([spec, flags, false]))?;
            Ok(())
        } else {
            process::spawn_logged(&spec.cmd, &spec.cwd, &spec.env, &spec.log, flags, false)
        }
    }
    fn powershell(&mut self, script: &str) -> io::Result<i32> {
        if self.flags["run"] == true {
            self.request("powershell", json!([script]))?
                .as_i64()
                .map(|n| n as i32)
                .ok_or_else(|| io::Error::other("invalid PowerShell exit code"))
        } else {
            Ok(process::run_configured(
                &[
                    "powershell".into(),
                    "-NoProfile".into(),
                    "-NonInteractive".into(),
                    "-Command".into(),
                    script.into(),
                ],
                &std::env::current_dir()?,
                15.0,
                winspawn::NO_WINDOW,
            )?
            .code)
        }
    }
    fn wmi(&mut self, spec: &Launch, executable: &Path, runner: &Path) -> io::Result<bool> {
        if self.flags["wmi"] == true {
            self.request("wmi", json!([spec])).map(|v| v == true)
        } else {
            winspawn::spawn_wmi(spec, executable, runner, self)
        }
    }
    fn call(&mut self, spec: &Launch, flags: u32) -> io::Result<i32> {
        if self.flags["call"] == true {
            self.request("call", json!([spec, flags]))?
                .as_i64()
                .map(|n| n as i32)
                .ok_or_else(|| io::Error::other("invalid process exit code"))
        } else {
            process::call_logged(&spec.cmd, &spec.cwd, &spec.env, &spec.log, flags)
        }
    }
}
fn text<'a>(args: &'a Value, key: &str) -> &'a str {
    args[key].as_str().unwrap_or("")
}
fn execute(op: &str, args: &Value, runtime: &mut Transport) -> Result<Value, String> {
    if op == "context.build" {
        let ctx = LiveContext::build(
            text(args, "host"),
            args["keeps_history"] == true,
            Path::new(text(args, "cwd")),
            args["env"].as_object().cloned().unwrap_or_default(),
            Path::new(text(args, "home")),
            args["session_id"].clone(),
            args["settle"] != false,
        )?;
        return Ok(json!({"value":snapshot(&ctx)}));
    }
    if op.starts_with("winspawn.") {
        let executable = Path::new(text(args, "executable"));
        let runner = Path::new(text(args, "runner"));
        let result = (|| -> io::Result<Value> {
            Ok(match op {
                "winspawn._windowless_python" => json!(winspawn::windowless_python(executable)),
                "winspawn.command_line" => {
                    json!(winspawn::command_line(&serde_json::from_value::<
                        Vec<String>,
                    >(
                        args["cmd"].clone()
                    )?))
                }
                "winspawn.spawn" => json!(winspawn::spawn(
                    &serde_json::from_value(args["spec"].clone())?,
                    executable,
                    runner,
                    runtime
                )?),
                "winspawn._spawn_wmi" => json!(winspawn::spawn_wmi(
                    &serde_json::from_value(args["spec"].clone())?,
                    executable,
                    runner,
                    runtime
                )?),
                "winspawn._run" => {
                    json!(winspawn::run(Path::new(text(args, "spec_path")), runtime)?)
                }
                _ => {
                    return Err(io::Error::other(format!(
                        "unknown Windows spawn operation: {op}"
                    )));
                }
            })
        })();
        return Ok(storage_dispatch::response(result));
    }
    let mut ctx = context(&args["self"])?;
    let value = match op {
        "context.mech_dir" => json!(ctx.mech_dir()?),
        "context.is_headless" => json!(ctx.is_headless(args["transcript"].as_str(), runtime)?),
        "context.settle" => {
            let result = ctx.settle();
            return Ok(match result {
                Ok(()) => json!({"value":null,"self":snapshot(&ctx)}),
                Err(message) => {
                    json!({"raised":"RuntimeError","message":message,"self":snapshot(&ctx)})
                }
            });
        }
        "context.spawn" => json!(
            ctx.spawn(
                &serde_json::from_value::<Vec<String>>(args["argv"].clone())
                    .map_err(|e| e.to_string())?,
                text(args, "log_name"),
                &Entrypoint {
                    executable: PathBuf::from(text(args, "executable")),
                    core: PathBuf::from(text(args, "core")),
                    windows_runner: PathBuf::from(text(args, "runner"))
                },
                args["windows"] == true,
                runtime
            )?
        ),
        _ => return Err(format!("unknown context operation: {op}")),
    };
    Ok(json!({"value":value}))
}
pub(super) fn dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
    if !op.starts_with("context.") && !op.starts_with("winspawn.") {
        return Ok(None);
    }
    if op == "winspawn.constants" {
        return Ok(Some(
            json!({"_FLAGS":winspawn::FLAGS,"_BREAKAWAY":winspawn::BREAKAWAY}),
        ));
    }
    if op == "context.resume" {
        return job::advance(
            args["job"].as_u64().ok_or("missing context job")?,
            args.get("reply").cloned(),
        )
        .map(Some);
    }
    let (op, args) = (op.to_string(), args.clone());
    job::start_polling("context", move |channel| {
        let mut runtime = Transport {
            channel,
            flags: args["transport"].clone(),
        };
        execute(&op, &args, &mut runtime)
            .unwrap_or_else(|message| json!({"raised":"RuntimeError","message":message}))
    })
    .map(Some)
}
