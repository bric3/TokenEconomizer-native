use super::{NativeRuntime, path, strings, text};
use crate::{
    plugin::{
        cli::{self, Error, Result, Runtime as CliRuntime},
        config,
        context::LiveContext,
        denylist, fold, history,
        hosts::session::{self, Environment},
        judge, repomap,
        sidecall::{
            self,
            runner::{self, RoutedRunner},
        },
    },
    runner::Runner,
    value::{float_or_zero, truthy},
};
use serde_json::{Value, json};
use std::{io, path::Path};

pub(super) struct Services<'a> {
    pub app: &'a mut NativeRuntime,
    pub ctx: &'a LiveContext,
}
pub(super) fn fold_error(error: fold::Error) -> Error {
    match error {
        fold::Error::Io(e) => e.into(),
        fold::Error::Value(e) => Error::new("ValueError", e),
        e => e.to_string().into(),
    }
}
fn host_error(error: session::Error) -> Error {
    match error {
        session::Error::Io(e) => e.into(),
        session::Error::Invalid(e) => Error::new("ValueError", e),
        session::Error::Later(e) => Error::new("FoldLater", e),
        session::Error::Unimplemented => {
            Error::new("NotImplementedError", "host does not provide session usage")
        }
    }
}
impl Services<'_> {
    fn environment(&self) -> Environment {
        Environment {
            env: self.ctx.env.clone(),
            home: self.ctx.home.clone(),
            cwd: self.app.environment.cwd.clone(),
        }
    }
    pub(super) fn pending(&mut self, exclude: &[String]) -> Result<Vec<String>> {
        fold::pending_sessions(self.ctx, exclude, fold::STALE_S, history::now(), self)
            .map_err(fold_error)
    }
    fn session_operation(&mut self, method: &str, args: &Value) -> Result<Value> {
        let env = self.environment();
        let host = &self.ctx.host;
        let input = || -> Result<_> { Ok(env.path(path(&args[0])?)) };
        let mut runtime = session::NativeRuntime { environment: &env };
        let result = match method {
            "complete" => session::complete(
                host,
                args[0].as_str(),
                text(&args[1])?,
                args[2].as_str().map(Path::new),
                &args[3],
                &env,
                &session::Limits::default(),
                &mut runtime,
            ),
            "mechanism_trace" => {
                session::mechanism_trace(host, args[0].as_str(), path(&args[1])?, &env)
                    .map(|p| json!(p))
            }
            "prompt_count" => Ok(json!(session::prompt_count(host, &input()?))),
            "settle" => session::settle(
                host,
                args[0].as_str().map(|p| env.path(Path::new(p))).as_deref(),
                15.0,
                60.0,
                &mut runtime,
            )
            .map(|_| Value::Null),
            "session_usage" => session::session_usage(host, &input()?),
            "last_active" => session::last_active(host, &input()?, &args[1]).map(|n| json!(n)),
            "history" => {
                session::history(host, &self.ctx.env, &strings(&args[0])?, &env).map(|v| json!(v))
            }
            "log_head" => Ok(session::log_head(host, &input()?)),
            "safe_session_id" => Ok(json!(session::safe_session_id(host, &args[0]))),
            "folder_matches" => {
                session::folder_matches(host, path(&args[0])?, &args[1]).map(|b| json!(b))
            }
            "prompt_turns" => Ok(json!(session::prompt_turns(host, &input()?))),
            "turn_calls" => Ok(json!(session::turn_calls(&strings(&args[0])?))),
            "turn_interrupted" => Ok(json!(session::turn_interrupted(&strings(&args[0])?))),
            "turn_answered" => Ok(json!(session::turn_answered(&strings(&args[0])?))),
            _ => return Err(format!("unknown session operation: {method}").into()),
        };
        result.map_err(host_error)
    }
    fn select_runner(&self, role: &str, local_only: bool) -> Result<Option<Box<dyn Runner>>> {
        let mut runner = RoutedRunner::new(
            runner::State::default(),
            runner::Context::new(self.app.environment.clone(), self.ctx.root.clone()),
            runner::NativeRuntime::default(),
        );
        let selected = if local_only {
            runner.select_local(role, &self.ctx.config)?
        } else {
            runner.select(&self.ctx.host, &self.ctx.config, Some(role), false)?
        };
        Ok(selected.then(|| Box::new(runner) as Box<dyn Runner>))
    }
    pub(super) fn map(&mut self, method: &str, args: &Value) -> Result<Value> {
        let env = self.environment();
        let generator = self
            .app
            .executable
            .with_file_name(format!("te-repomap-gen{}", std::env::consts::EXE_SUFFIX));
        let interpreter =
            crate::plugin::process::which("python3").map(|p| p.to_string_lossy().into_owned());
        let map = repomap::RepoMap {
            ctx: self.ctx,
            environment: &env,
            generator: &generator,
            interpreter: interpreter.as_deref(),
        };
        Ok(match method {
            "repomap_state" => map.read_state(),
            "repomap_python" => json!(map.find_python(self)?),
            "repomap_refresh" => map.refresh(self)?,
            "head_commit" => json!(map.head_commit(path(&args[0])?, self)),
            "stale" => json!(map.stale(self)),
            "spawn_refresh" => map.spawn_refresh(self)?,
            "cached_block" => json!(map.cached_block()?),
            "for_prompt" => json!(map.for_prompt(text(&args[0])?, self)?),
            _ => return Err(format!("unknown map operation: {method}").into()),
        })
    }
    pub(super) fn cli(&mut self, method: &str, args: &Value) -> Result<Value> {
        let ctx = self.ctx;
        Ok(match method {
            "session_note" => cli::session_note(ctx, &json!(self.app.environment.env))?,
            "paused" => sidecall::paused(&ctx.host, &ctx.root),
            "resume" => json!(sidecall::resume(&ctx.host, &ctx.root)),
            "call_name" => json!(sidecall::call_name(&ctx.host)),
            "proposal" => json!(cli::proposal(ctx, &args[0])?),
            "applied" => json!(denylist::applied_for(
                &ctx.store,
                &ctx.repo.root,
                &ctx.host
            )?),
            "kept" => json!(denylist::kept_for(&ctx.store, &ctx.repo.root, &ctx.host)?),
            "codex_trusted" => json!(denylist::codex_project_trusted(
                &ctx.repo.root,
                &self.app.environment.env,
                &self.app.environment.home
            )?),
            "copilot_trusted" => json!(denylist::copilot_folder_trusted(
                &ctx.repo.root,
                &self.app.environment.env,
                &self.app.environment.home
            )?),
            "copilot_folder_trusted" => json!(denylist::copilot_folder_trusted(
                path(&args[0])?,
                &self.app.environment.env,
                &self.app.environment.home
            )?),
            "refuse_unwritable" => json!(self.app.refusal(path(&args[0])?)?),
            "write_config" => json!(
                config::write_config(
                    path(&args[0])?,
                    args[1].as_object().ok_or("expected config updates")?
                )
                .map_err(|e| Error::new("ValueError", e))?
            ),
            "unset_config" => json!(
                config::unset_config(path(&args[0])?, &strings(&args[1])?)
                    .map_err(|e| Error::new("ValueError", e))?
            ),
            "deny_apply" => json!(denylist::apply(
                &ctx.repo.root,
                &strings(&args[0])?,
                &ctx.store,
                &ctx.host,
                &mut denylist::NativeRuntime
            )?),
            "deny_restore" => json!(denylist::restore(
                &ctx.repo.root,
                &ctx.store,
                &strings(&args[0])?,
                &ctx.host,
                &mut denylist::NativeRuntime
            )?),
            "set_kept" => json!(denylist::set_kept(
                &ctx.store,
                &ctx.repo.root,
                &ctx.host,
                &strings(&args[0])?,
                &strings(&args[1])?,
                &mut denylist::NativeRuntime
            )?),
            "verify_copilot" => self.app.checks.verify_copilot(
                &ctx.repo.root,
                &strings(&args[0])?,
                "copilot",
                &mut self.app.sidecalls,
            )?,
            "verify_codex" => self.app.checks.verify_codex(
                &ctx.repo.root,
                &strings(&args[0])?,
                "codex",
                30.0,
                &mut self.app.sidecalls,
            )?,
            "refresh_catalog" => self.app.checks.refresh_catalog(
                &ctx.root,
                "",
                &ctx.host,
                &mut self.app.sidecalls,
            )?,
            "measure" => self.app.checks.measure(
                &strings(&args[0])?,
                "",
                &ctx.host,
                &mut self.app.sidecalls,
            )?,
            "history_summary" => history::summary(ctx).map_err(|e| e.to_string())?,
            "history_plan" => history::plan(ctx, history::now(), &history::Limits::default(), self)
                .map_err(|e| e.to_string())?,
            "history_refusal" => {
                json!(history::refusal(ctx, truthy(&args[0])).map_err(|e| e.to_string())?)
            }
            "history_run" => history::run(
                ctx,
                truthy(&args[0]),
                history::now(),
                history::BUDGET_S,
                &history::Limits::default(),
                self,
            )
            .map_err(|e| e.to_string())?,
            "pending_sessions" => json!(self.pending(&[])?),
            "fold_session" => {
                fold::fold_session(ctx, text(&args[0])?, None, fold::FOLD_LOCK_WAIT_S, self)
                    .map_err(fold_error)?
            }
            "apply_pending" => {
                json!(fold::apply_pending(ctx, history::now(), self).map_err(fold_error)?)
            }
            "judge_job" => judge::run_job(
                &ctx.store,
                &ctx.host,
                text(&args[0])?,
                text(&args[1])?,
                float_or_zero(&ctx.config["local_judge_confidence"])?,
                self,
            )?,
            "repomap_state" | "repomap_refresh" | "repomap_python" | "head_commit" => {
                self.map(method, args)?
            }
            _ => return Err(format!("unknown CLI service: {method}").into()),
        })
    }
}
impl fold::Runtime for Services<'_> {
    fn host(&mut self, method: &str, args: Value) -> fold::Result<Value> {
        self.session_operation(method, &args)
            .map_err(|e| match e.kind.as_str() {
                "FoldLater" => fold::Error::Later,
                "ValueError" => fold::Error::Value(e.message),
                "FileNotFoundError" => {
                    fold::Error::Io(io::Error::new(io::ErrorKind::NotFound, e.message))
                }
                "PermissionError" => {
                    fold::Error::Io(io::Error::new(io::ErrorKind::PermissionDenied, e.message))
                }
                _ if e.is("OSError") => fold::Error::Io(io::Error::other(e.message)),
                _ => fold::Error::Failed(e.message),
            })
    }
    fn proposer(&mut self, _ctx: &LiveContext) -> fold::Result<Option<Box<dyn Runner>>> {
        self.select_runner(
            "proposer",
            truthy(&sidecall::paused(&self.ctx.host, &self.ctx.root)),
        )
        .map_err(|e| fold::Error::Failed(e.to_string()))
    }
    fn spawn(&mut self, argv: &[String], log: &str) -> fold::Result<Value> {
        self.app
            .spawn(self.ctx, argv, log)
            .map_err(|e| fold::Error::Failed(e.to_string()))
    }
    fn fold(&mut self, ctx: &LiveContext, sid: &str, path: Option<&str>) -> fold::Result<Value> {
        fold::fold_locked(ctx, sid, path, false, None, self)
    }
}
impl history::Runtime for Services<'_> {
    fn host(&mut self, method: &str, args: Value) -> history::Result<Value> {
        self.session_operation(method, &args)
            .map_err(|e| history::Error::Failed(e.to_string()))
    }
    fn plan(
        &mut self,
        ctx: &LiveContext,
        at: f64,
        limits: &history::Limits,
    ) -> history::Result<Value> {
        history::plan(ctx, at, limits, self)
    }
    fn fold(
        &mut self,
        sid: &str,
        path: Option<&str>,
        imported: bool,
        span: Option<(&Value, &Value)>,
    ) -> history::Result<Value> {
        fold::fold_locked(self.ctx, sid, path, imported, span, self)
            .map_err(|e| history::Error::Failed(e.to_string()))
    }
    fn pending(&mut self) -> history::Result<Vec<String>> {
        self.pending(&[])
            .map_err(|e| history::Error::Failed(e.to_string()))
    }
    fn spawn(&mut self, argv: &[&str], log: &str) -> history::Result<Value> {
        self.app
            .spawn(
                self.ctx,
                &argv.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                log,
            )
            .map_err(|e| history::Error::Failed(e.to_string()))
    }
    fn say(&mut self, text: &str) -> history::Result<()> {
        self.app
            .say(text)
            .map_err(|e| history::Error::Failed(e.to_string()))
    }
}
impl judge::Runtime for Services<'_> {
    fn paused(&mut self) -> std::result::Result<bool, String> {
        Ok(truthy(&sidecall::paused(&self.ctx.host, &self.ctx.root)))
    }
    fn runner(
        &mut self,
        role: &str,
        local_only: bool,
    ) -> std::result::Result<Option<Box<dyn Runner>>, String> {
        self.select_runner(role, local_only)
            .map_err(|e| e.to_string())
    }
    fn spawn_job(&mut self, sid: &str, job: &str) -> std::result::Result<(), String> {
        judge::spawn_job(
            &self.ctx.store,
            &self.ctx.host,
            sid,
            job,
            float_or_zero(&self.ctx.config["local_judge_confidence"])?,
            self.ctx.env.get("TE_FOLD_SYNC").is_some_and(|v| v == "1"),
            self,
        )
        .map(|_| ())
    }
    fn spawn(&mut self, argv: &[String], log: &str) -> std::result::Result<Value, String> {
        self.app
            .spawn(self.ctx, argv, log)
            .map_err(|e| e.to_string())
    }
}
impl sidecall::Runtime for Services<'_> {}
impl repomap::Runtime for Services<'_> {
    fn spawn(&mut self, argv: &[&str], log: &str) -> std::result::Result<Value, String> {
        self.app
            .spawn(
                self.ctx,
                &argv.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                log,
            )
            .map_err(|e| e.to_string())
    }
}
