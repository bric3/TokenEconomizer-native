use super::{NativeRuntime, session::Services};
use crate::{
    plugin::{
        context::LiveContext,
        controller::{self, LiveController},
        fold, history, hook,
        hosts::session,
        judge, paths, repos, sidecall,
    },
    runner::Runner,
};
use serde_json::{Value, json};
use std::{io, path::Path};

impl NativeRuntime {
    pub fn run_hook(
        &mut self,
        argv: &[String],
        input: std::result::Result<Vec<u8>, String>,
    ) -> std::result::Result<String, String> {
        let prior = self.hook_output;
        self.hook_output = true;
        let result = hook::main(
            argv,
            input,
            &self.environment.clone(),
            std::process::id(),
            self,
        );
        self.hook_output = prior;
        result
    }
}
impl hook::Runtime for NativeRuntime {
    fn handle(
        &mut self,
        ctx: &mut LiveContext,
        payload: Value,
    ) -> std::result::Result<Value, String> {
        #[cfg(unix)]
        // getppid has no preconditions and identifies the agent's enclosing process.
        let parent = unsafe { libc::getppid() } as u32;
        #[cfg(not(unix))]
        let parent = 0;
        let mut controller = LiveController::new(
            ctx.clone(),
            payload,
            self.environment.clone(),
            parent,
            cfg!(windows),
        );
        let snapshot = controller.ctx.clone();
        let result = controller.handle(&mut HookServices {
            app: self,
            ctx: snapshot,
        });
        *ctx = controller.ctx;
        result
    }
}
struct HookServices<'a> {
    app: &'a mut NativeRuntime,
    ctx: LiveContext,
}
impl HookServices<'_> {
    fn services(&mut self) -> Services<'_> {
        Services {
            app: self.app,
            ctx: &self.ctx,
        }
    }
}
impl session::Runtime for HookServices<'_> {
    fn program(&mut self, cli: &str) -> session::Result<Vec<String>> {
        sidecall::Runtime::program(
            &mut self.app.sidecalls,
            cli,
            cfg!(windows),
            &self.app.environment,
        )
        .map_err(session::Error::Invalid)
    }
    fn ancestors(
        &mut self,
        limit: i64,
        pid: u32,
        windows: bool,
        cwd: &Path,
    ) -> session::Result<Vec<String>> {
        session::ancestor_args(limit, pid, windows, cwd, self)
    }
}
impl judge::Runtime for HookServices<'_> {
    fn paused(&mut self) -> std::result::Result<bool, String> {
        judge::Runtime::paused(&mut self.services())
    }
    fn runner(
        &mut self,
        role: &str,
        local_only: bool,
    ) -> std::result::Result<Option<Box<dyn Runner>>, String> {
        judge::Runtime::runner(&mut self.services(), role, local_only)
    }
    fn spawn_job(&mut self, sid: &str, id: &str) -> std::result::Result<(), String> {
        judge::Runtime::spawn_job(&mut self.services(), sid, id)
    }
    fn spawn(&mut self, argv: &[String], log: &str) -> std::result::Result<Value, String> {
        judge::Runtime::spawn(&mut self.services(), argv, log)
    }
}
impl controller::Runtime for HookServices<'_> {
    fn dispatch(
        &mut self,
        controller: &mut LiveController,
        method: &str,
    ) -> std::result::Result<Value, String> {
        // Settlement can reload configuration before a queued handler runs.
        self.ctx = controller.ctx.clone();
        controller.invoke(method, self)
    }
    fn launcher(&mut self, ctx: &LiveContext) -> io::Result<()> {
        paths::ensure_native_launcher(&ctx.env, &ctx.home, &self.app.executable, cfg!(windows))
            .map(|_| ())
            .map_err(io::Error::other)
    }
    fn paused_detail(&mut self) -> std::result::Result<Value, String> {
        Ok(sidecall::paused(&self.ctx.host, &self.ctx.root))
    }
    fn call_name(&mut self) -> std::result::Result<String, String> {
        Ok(sidecall::call_name(&self.ctx.host))
    }
    fn local_routes(&mut self) -> std::result::Result<bool, String> {
        Ok(sidecall::local_routes(
            &json!("judge"),
            &self.ctx.config,
            cfg!(target_os = "macos"),
        ))
    }
    fn map(
        &mut self,
        ctx: &LiveContext,
        method: &str,
        args: Value,
    ) -> std::result::Result<Value, String> {
        Services { app: self.app, ctx }
            .map(method, &args)
            .map_err(|e| e.to_string())
    }
    fn history_notice(&mut self, ctx: &LiveContext) -> std::result::Result<Value, String> {
        history::notice(ctx, &history::Limits::default()).map_err(|e| e.to_string())
    }
    fn history_start(&mut self, ctx: &LiveContext) -> std::result::Result<(), String> {
        history::maybe_start(
            ctx,
            history::now(),
            &history::Limits::default(),
            &mut Services { app: self.app, ctx },
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
    }
    fn migration_start(&mut self, ctx: &LiveContext) -> std::result::Result<(), String> {
        match repos::start_migration(&ctx.root, &ctx.env, &ctx.home)? {
            repos::StartMigration::Skip => (),
            repos::StartMigration::Inline(outcome) => self
                .app
                .refold(outcome.done.as_deref().unwrap_or_default())
                .map_err(|e| e.to_string())?,
            repos::StartMigration::Spawn { command, log } => {
                self.app
                    .spawn(
                        ctx,
                        &command.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                        log,
                    )
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
    fn pending_sessions(
        &mut self,
        ctx: &LiveContext,
        exclude: &[String],
    ) -> std::result::Result<Vec<String>, String> {
        Services { app: self.app, ctx }
            .pending(exclude)
            .map_err(|e| e.to_string())
    }
    fn spawn_folds(
        &mut self,
        ctx: &LiveContext,
        ids: &[String],
    ) -> std::result::Result<(), String> {
        fold::spawn_fold(
            ctx,
            ids,
            fold::FOLD_LOCK_WAIT_S,
            &mut Services { app: self.app, ctx },
        )
        .map(|_| ())
        .map_err(|e| e.to_string())
    }
}
