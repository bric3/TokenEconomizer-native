mod probe;
mod session;

use super::{
    cli::{self, Error, Result, Runtime},
    context::LiveContext,
    history,
    hosts::{self, session::Environment},
    paths, process, repos, sidecall, toolcheck, winspawn,
};
use serde_json::{Value, json};
use session::Services;
use std::{
    fs::{File, FileTimes},
    io::{self, Write},
    path::{Path, PathBuf},
    thread,
    time::{Duration, SystemTime},
};

pub struct NativeRuntime {
    pub environment: Environment,
    pub executable: PathBuf,
    checks: toolcheck::Checks,
    sidecalls: sidecall::runner::NativeRuntime,
}
impl NativeRuntime {
    pub fn capture() -> Result<Self> {
        // shortcut: OS values must be UTF-8; preserve native encodings during the path parity audit.
        let environment = Environment {
            env: std::env::vars_os()
                .map(|(key, value)| {
                    let key = key
                        .into_string()
                        .map_err(|_| "environment contains a name that is not UTF-8")?;
                    let value = value
                        .into_string()
                        .map_err(|_| "environment contains a value that is not UTF-8")?;
                    Ok((key, json!(value)))
                })
                .collect::<std::result::Result<_, &str>>()?,
            home: std::env::home_dir().ok_or("cannot determine the home directory")?,
            cwd: std::env::current_dir()?,
        };
        if environment.cwd.to_str().is_none() || environment.home.to_str().is_none() {
            return Err("working and home directories must be UTF-8".into());
        }
        Ok(Self::new(environment, std::env::current_exe()?))
    }
    pub fn new(environment: Environment, executable: PathBuf) -> Self {
        let checks = toolcheck::Checks {
            environment: environment.clone(),
            windows: cfg!(windows),
            help: Default::default(),
        };
        Self {
            environment,
            executable,
            checks,
            sidecalls: Default::default(),
        }
    }
    pub fn run(&mut self, argv: &[String]) -> Result<Value> {
        let columns = cli::parser::terminal_columns(
            self.environment.env.get("COLUMNS").and_then(Value::as_str),
        );
        cli::main(argv, &self.environment.clone(), columns, self)
    }
    fn build(&self, args: &Value, sid: Value) -> Result<LiveContext> {
        let host = text(&args["host"])?;
        if !hosts::HOSTS.contains(&host) {
            return Err(Error::new("KeyError", host));
        }
        let cwd = self.environment.path(Path::new(text(&args["cwd"])?));
        LiveContext::build(
            host,
            hosts::profile(host).keeps_history,
            &cwd,
            self.environment.env.clone(),
            &self.environment.home,
            sid,
            true,
        )
        .map_err(Into::into)
    }
    fn execute(&mut self, args: &Value) -> Result<Value> {
        Ok(json!(match text(&args["fn"])? {
            "cmd_status" => cli::status(args, self)?,
            "cmd_microskills" => cli::facts(args, self)?,
            "cmd_toolprune" => cli::toolprune(args, self)?,
            "cmd_config" => cli::config_command(args, self)?,
            "cmd_fold" => cli::fold_command(args, self)?,
            "cmd_judge" => cli::judge_command(args, self)?,
            "cmd_doctor" => cli::doctor(args, &self.environment.clone(), self)?,
            "cmd_repos" => cli::repositories(args, self)?,
            "cmd_history" => cli::history_command(args, self)?,
            "cmd_repomap" => cli::map_command(args, self)?,
            "cmd_probe" => super::probe::command(args, self)?,
            name => return Err(format!("unknown command handler: {name}").into()),
        }))
    }
    fn refusal(&mut self, path: &Path) -> Result<i32> {
        if let Some(why) = cli::unwritable(path, std::process::id()) {
            self.say(&cli::refusal(
                path,
                &why,
                &std::env::args().skip(1).collect::<Vec<_>>(),
            ))?;
            Ok(1)
        } else {
            Ok(0)
        }
    }
    fn refold(&mut self, done: &[Value]) -> Result<()> {
        for (host, cwd) in repos::refold_targets(done) {
            let result = (|| -> Result<()> {
                let ctx = self.build(&json!({"host":host,"cwd":cwd}), Value::Null)?;
                let mut service = Services {
                    app: self,
                    ctx: &ctx,
                };
                let ids = service.pending(&[])?;
                super::fold::spawn_fold(&ctx, &ids, super::fold::FOLD_LOCK_WAIT_S, &mut service)
                    .map_err(session::fold_error)?;
                Ok(())
            })();
            if let Err(error) = result {
                repos::record_error(
                    &paths::state_root(&self.environment.env, &self.environment.home)?,
                    &error.to_string(),
                );
            }
        }
        Ok(())
    }
    fn spawn(&mut self, ctx: &LiveContext, argv: &[String], log: &str) -> Result<Value> {
        let mut command = vec![self.executable.to_string_lossy().into_owned()];
        command.extend_from_slice(argv);
        command.extend([
            "--host".into(),
            ctx.host.clone(),
            "--cwd".into(),
            ctx.repo.root.to_string_lossy().into_owned(),
        ]);
        ctx.store.ensure(&[] as &[&str])?;
        let launch = process::Launch {
            cmd: command,
            cwd: ctx.repo.root.clone(),
            env: ctx.env.clone(),
            log: ctx.store.base.join(log),
        };
        if cfg!(windows) {
            return Ok(json!(winspawn::spawn(
                &launch,
                &self.executable,
                &self.executable,
                &mut NativeSpawn
            )?));
        }
        process::spawn_logged(&launch.cmd, &launch.cwd, &launch.env, &launch.log, 0, true)?;
        Ok(json!(true))
    }
}
struct NativeSpawn;
impl winspawn::Runtime for NativeSpawn {
    fn worker_argv(
        &self,
        executable: &Path,
        _runner: &Path,
        spec: &Path,
    ) -> io::Result<Vec<String>> {
        Ok(vec![
            executable.to_string_lossy().into_owned(),
            "--run-spawn-spec".into(),
            spec.to_string_lossy().into_owned(),
        ])
    }
    fn wmi(
        &mut self,
        spec: &process::Launch,
        executable: &Path,
        runner: &Path,
    ) -> io::Result<bool> {
        winspawn::spawn_wmi(spec, executable, runner, self)
    }
}
fn text(value: &Value) -> Result<&str> {
    value
        .as_str()
        .ok_or_else(|| Error::new("TypeError", "expected text"))
}
fn path(value: &Value) -> Result<&Path> {
    text(value).map(Path::new)
}
fn strings(value: &Value) -> Result<Vec<String>> {
    cli::words(value)
}
impl Runtime for NativeRuntime {
    fn context(&mut self, args: &Value) -> Result<LiveContext> {
        self.build(args, Value::Null)
    }
    fn pinned_context(&mut self, args: &Value, session: &Value) -> Result<LiveContext> {
        self.build(args, session.clone())
    }
    fn say(&mut self, text: &str) -> Result<()> {
        let mut out = io::stdout().lock();
        writeln!(out, "{text}")?;
        out.flush()?;
        Ok(())
    }
    fn global(&mut self, method: &str, args: Value) -> Result<Value> {
        Ok(match method {
            "execute" => self.execute(&args[0])?,
            "stderr" => {
                let mut out = io::stderr().lock();
                out.write_all(text(&args[0])?.as_bytes())?;
                out.flush()?;
                Value::Null
            }
            "state_root" => json!(paths::state_root(
                &self.environment.env,
                &self.environment.home
            )?),
            "refuse_unwritable" => json!(self.refusal(path(&args[0])?)?),
            "runtime" => {
                json!({"name":"native","version":env!("CARGO_PKG_VERSION"),"parts":[],"minimum":[],"required":""})
            }
            "claude_cli" => json!(sidecall::Runtime::claude_cli(
                &mut self.sidecalls,
                &self.environment,
                cfg!(windows)
            )?),
            "hermes_cli" => json!(sidecall::Runtime::hermes_cli(
                &mut self.sidecalls,
                &self.environment
            )?),
            "which" => json!(process::which(text(&args[0])?)),
            "time" => json!(history::now()),
            "pid" => json!(std::process::id()),
            "sleep" => {
                thread::sleep(
                    Duration::try_from_secs_f64(args[0].as_f64().ok_or("expected delay")?)
                        .map_err(|e| Error::new("ValueError", e.to_string()))?,
                );
                Value::Null
            }
            "touch" => {
                let now = SystemTime::now();
                File::open(path(&args[0])?)?
                    .set_times(FileTimes::new().set_accessed(now).set_modified(now))?;
                Value::Null
            }
            "repos_run_migration" => {
                let outcome = repos::run_migration(
                    path(&args[0])?,
                    &self.environment.env,
                    &self.environment.home,
                )?;
                if let Some(done) = &outcome.done {
                    self.refold(done)?;
                }
                json!(outcome.done)
            }
            "repos_refold" => {
                self.refold(args[0].as_array().ok_or("expected migration results")?)?;
                Value::Null
            }
            "repos_merge" => {
                json!(repos::merge(
                    path(&args[0])?,
                    text(&args[1])?,
                    &args[2],
                    repos::LOCK_WAIT_S
                )?)
            }
            "repos_listing" => {
                let current = serde_json::from_value(args[1].clone()).map_err(|e| e.to_string())?;
                json!(repos::listing(
                    path(&args[0])?,
                    &self.environment.env,
                    &self.environment.home,
                    Some(&current)
                )?)
            }
            "probe_run" => probe::run(self, &args)?,
            _ => return Err(format!("unknown global CLI operation: {method}").into()),
        })
    }
    fn call(&mut self, ctx: &LiveContext, method: &str, args: Value) -> Result<Value> {
        Services { app: self, ctx }.cli(method, &args)
    }
}
