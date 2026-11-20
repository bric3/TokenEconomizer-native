#[path = "recording_runner.rs"]
pub mod recording_runner;

use crate::support::session_fixture;
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    fs, io,
    path::{Path, PathBuf},
    rc::Rc,
};
use tokeneconomizer::{
    plugin::{
        context::LiveContext,
        controller::{self, LiveController},
        fold, history, hook, hookqueue,
        hosts::{
            session::{self, Environment},
            state,
        },
        judge, paths, sidecall,
    },
    runner::Runner,
    store::Store,
};

pub type MapHandler = fn(&LiveContext, &Environment, &str, Value) -> Result<Value, String>;

pub struct HookFixture {
    pub host: String,
    pub inline_judge: bool,
    pub inline_folds: bool,
    pub fold_wait: f64,
    pub proposer_reply: Option<(String, f64)>,
    pub proposer_models: Rc<RefCell<Vec<String>>>,
    pub judge_result: Option<bool>,
    pub judge_digests: Vec<String>,
    pub judge_jobs: Vec<String>,
    pub defer_judge: bool,
    pub paused_models: bool,
    pub queue_wait: Option<f64>,
    pub enqueue_after_post: bool,
    pub fail_replay: bool,
    pub refuse_enqueue: bool,
    pub history_failure: Option<String>,
    pub repo_map: Option<MapHandler>,
    pub repo: PathBuf,
    pub environment: Environment,
    pub store: Store,
    pub folds: Vec<String>,
}
impl HookFixture {
    pub fn new(root: &Path, host: &str) -> Self {
        let repo = root.join("repo");
        fs::create_dir_all(repo.join(".git/info")).unwrap();
        fs::write(repo.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(
            repo.join(".git/config"),
            "[remote \"origin\"]\n\turl = https://github.com/acme/widget.git\n",
        )
        .unwrap();
        let state = root.join("state");
        fs::create_dir(&state).unwrap();
        let mut env = json!({"TOKENECONOMIZER_HOME":state,"TE_LOOPGUARD":"off","GIT_CEILING_DIRECTORIES":root,"PATH":""}).as_object().unwrap().clone();
        if host == "cursor" {
            env.insert("CURSOR_PROJECT_DIR".into(), json!(repo));
        }
        let id = paths::repo_identity(&repo, &env, root).unwrap();
        let store = Store::new(&paths::repo_state_dir(&id, host, &env, root).unwrap()).unwrap();
        Self {
            host: host.into(),
            inline_judge: false,
            inline_folds: false,
            fold_wait: fold::FOLD_LOCK_WAIT_S,
            proposer_reply: None,
            proposer_models: Rc::default(),
            judge_result: None,
            judge_digests: vec![],
            judge_jobs: vec![],
            defer_judge: false,
            paused_models: false,
            queue_wait: None,
            enqueue_after_post: false,
            fail_replay: false,
            refuse_enqueue: false,
            history_failure: None,
            repo_map: None,
            repo,
            store,
            environment: Environment {
                env,
                home: root.into(),
                cwd: root.into(),
            },
            folds: vec![],
        }
    }
    pub fn run(&mut self, payload: Value) -> Value {
        let host = self.host.clone();
        hook::run(payload, &self.environment.clone(), Some(&host), self).unwrap()
    }
    pub fn main(&mut self, payload: Value) -> Value {
        let args = vec!["--host".into(), self.host.clone()];
        let output = hook::main(
            &args,
            Ok(payload.to_string().into_bytes()),
            &self.environment.clone(),
            0,
            self,
        )
        .unwrap();
        let errors = paths::state_root(&self.environment.env, &self.environment.home)
            .unwrap()
            .join("errors.log");
        assert!(!errors.exists(), "{}", fs::read_to_string(errors).unwrap());
        let mut lines: Vec<Value> = output
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        let mut result = lines.pop().unwrap();
        if !lines.is_empty() {
            assert!(lines.iter().all(|line| line["type"] == "progress"));
            result["progress"] = json!(
                lines
                    .into_iter()
                    .map(|line| line["message"].clone())
                    .collect::<Vec<_>>()
            );
        }
        result
    }
    pub fn resolved_turns(&self) -> Vec<f64> {
        let envelope = self.store.read("envelope.json", json!({})).unwrap();
        let turns = envelope.get("resolved_turns").cloned().unwrap_or(json!([]));
        turns
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_f64().unwrap())
            .collect()
    }
    pub fn prepare(&self, payload: &Value) -> Value {
        state::prepare(
            &self.host,
            payload,
            &json!(self.environment.env),
            &self.environment,
        )
        .unwrap()
    }
    pub fn tool(&mut self, base: &Value, fields: Value) -> Value {
        let mut payload = base.clone();
        payload
            .as_object_mut()
            .unwrap()
            .extend(fields.as_object().unwrap().clone());
        self.run(payload)
    }
    pub fn guard_on(&mut self) {
        self.environment
            .env
            .insert("TE_LOOPGUARD".into(), json!("on"));
    }
}
impl hook::Runtime for HookFixture {
    fn handle(&mut self, ctx: &mut LiveContext, payload: Value) -> Result<Value, String> {
        let mut controller = LiveController::new(
            ctx.clone(),
            payload,
            self.environment.clone(),
            0,
            cfg!(windows),
        );
        let output = controller.handle(self);
        *ctx = controller.ctx;
        output
    }
}
impl session::Runtime for HookFixture {
    fn program(&mut self, _cli: &str) -> session::Result<Vec<String>> {
        panic!("Unexpected program lookup")
    }
    fn ancestors(
        &mut self,
        _limit: i64,
        _pid: u32,
        _windows: bool,
        _cwd: &Path,
    ) -> session::Result<Vec<String>> {
        if self.inline_folds {
            return Ok(vec![]);
        }
        panic!("Unexpected process lookup")
    }
}
impl judge::Runtime for HookFixture {
    fn paused(&mut self) -> Result<bool, String> {
        assert!(
            self.inline_judge,
            "These cold turns must not select a model"
        );
        Ok(self.paused_models
            || (self.inline_folds
                && !sidecall::paused(
                    &self.host,
                    &paths::state_root(&self.environment.env, &self.environment.home)?,
                )
                .is_null()))
    }
    fn runner(
        &mut self,
        _role: &str,
        _local_only: bool,
    ) -> Result<Option<Box<dyn Runner>>, String> {
        assert!(self.inline_judge, "Unexpected model selection");
        Ok(self
            .judge_result
            .map(|_| Box::new(recording_runner::RecordingRunner::new("", 0.0)) as Box<dyn Runner>))
    }
    fn judge(&mut self, digest: &str, _: &mut dyn Runner) -> Result<Value, String> {
        self.judge_digests.push(digest.into());
        Ok(
            json!({"stuck":self.judge_result.unwrap(),"confidence":0.95,"reason":"loop","_tokens":1000.0}),
        )
    }
    fn nudge(&mut self, _: &str, _: &mut dyn Runner) -> Result<Value, String> {
        Ok(json!({"text":"Read the failing assertion before editing again.","_tokens":300.0}))
    }
    fn spawn_job(&mut self, sid: &str, id: &str) -> Result<(), String> {
        assert!(self.inline_judge, "Unexpected judge job");
        self.judge_jobs.push(id.into());
        if self.defer_judge {
            return Ok(());
        }
        let store = self.store.clone();
        let host = self.host.clone();
        judge::run_job(&store, &host, sid, id, 0.7, self).map(|_| ())
    }
    fn spawn(&mut self, _argv: &[String], _log: &str) -> Result<Value, String> {
        panic!("Unexpected process spawn")
    }
}
impl controller::Runtime for HookFixture {
    fn dispatch(&mut self, controller: &mut LiveController, method: &str) -> Result<Value, String> {
        if method == "post_tool" && controller.replaying && self.fail_replay {
            return Err("RuntimeError: replayed".into());
        }
        let output = controller.invoke(method, self)?;
        if method == "post_tool" && !controller.replaying && self.enqueue_after_post {
            let queue = controller
                .ctx
                .root
                .join("locks")
                .join(&self.host)
                .join(format!("{}.queue", hookqueue::safe(&json!(controller.sid))));
            assert!(hookqueue::enqueue(&queue, &controller.p));
        }
        Ok(output)
    }
    fn queue_limits(&self, event: &str) -> hookqueue::Limits {
        let mut limits = hookqueue::Limits::for_event(event);
        if let Some(wait) = self.queue_wait {
            limits.wait = wait;
        }
        limits
    }
    fn enqueue(&mut self, queue: &Path, payload: &Value) -> bool {
        !self.refuse_enqueue && hookqueue::enqueue(queue, payload)
    }
    fn launcher(&mut self, ctx: &LiveContext) -> io::Result<()> {
        paths::ensure_native_launcher(
            &ctx.env,
            &ctx.home,
            Path::new(env!("CARGO_BIN_EXE_te")),
            cfg!(windows),
        )
        .map(|_| ())
        .map_err(io::Error::other)
    }
    fn paused_detail(&mut self) -> Result<Value, String> {
        Ok(sidecall::paused(
            &self.host,
            &paths::state_root(&self.environment.env, &self.environment.home)?,
        ))
    }
    fn call_name(&mut self) -> Result<String, String> {
        Ok(sidecall::call_name(&self.host))
    }
    fn local_routes(&mut self) -> Result<bool, String> {
        if self.inline_folds {
            return Ok(false);
        }
        panic!("Unexpected local model routing")
    }
    fn map(&mut self, ctx: &LiveContext, method: &str, args: Value) -> Result<Value, String> {
        self.repo_map.expect("Unexpected repository map")(ctx, &self.environment, method, args)
    }
    fn history_notice(&mut self, ctx: &LiveContext) -> Result<Value, String> {
        if let Some(error) = &self.history_failure {
            return Err(error.clone());
        }
        history::notice(ctx, &history::Limits::default()).map_err(|e| e.to_string())
    }
    fn history_start(&mut self, ctx: &LiveContext) -> Result<(), String> {
        if let Some(error) = &self.history_failure {
            return Err(error.clone());
        }
        assert_eq!(ctx.config["history_import"], false);
        Ok(())
    }
    fn migration_start(&mut self, _ctx: &LiveContext) -> Result<(), String> {
        Ok(())
    }
    fn pending_sessions(
        &mut self,
        ctx: &LiveContext,
        exclude: &[String],
    ) -> Result<Vec<String>, String> {
        if self.inline_folds {
            return fold::pending_sessions(ctx, exclude, fold::STALE_S, history::now(), self)
                .map_err(|error| error.to_string());
        }
        Ok(vec![])
    }
    fn spawn_folds(&mut self, ctx: &LiveContext, sessions: &[String]) -> Result<(), String> {
        self.folds.extend_from_slice(sessions);
        if self.inline_folds {
            fold::spawn_fold(ctx, sessions, self.fold_wait, self)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}

impl fold::Runtime for HookFixture {
    fn host(&mut self, method: &str, args: Value) -> fold::Result<Value> {
        session_fixture::host_call(
            &self.host,
            &self.environment.env,
            &self.environment,
            method,
            &args,
        )
        .map_err(|error| match error {
            session::Error::Io(error) => fold::Error::Io(error),
            session::Error::Invalid(error) => fold::Error::Value(error),
            session::Error::Later(_) => fold::Error::Later,
            error => fold::Error::Failed(error.to_string()),
        })
    }
    fn proposer(&mut self, ctx: &LiveContext) -> fold::Result<Option<Box<dyn Runner>>> {
        let Some((text, tokens)) = &self.proposer_reply else {
            return Ok(None);
        };
        struct Reply {
            text: String,
            tokens: f64,
            models: Rc<RefCell<Vec<String>>>,
        }
        impl sidecall::Runtime for Reply {
            fn override_available(&mut self) -> Result<Option<bool>, String> {
                Ok(Some(false))
            }
            fn override_call(
                &mut self,
                method: &str,
                args: &Value,
            ) -> Result<Option<(Value, Option<f64>)>, sidecall::Failure> {
                assert_ne!(method, "fm_call");
                self.models
                    .borrow_mut()
                    .push(args["kwargs"]["model"].as_str().unwrap_or("").into());
                Ok(Some((json!(self.text), Some(self.tokens))))
            }
            fn which(&mut self, cli: &str) -> Result<Option<String>, String> {
                panic!("Unexpected side-call program lookup: {cli}")
            }
        }
        let mut runner = sidecall::runner::RoutedRunner::new(
            Default::default(),
            sidecall::runner::Context::new(self.environment.clone(), ctx.root.clone()),
            Reply {
                text: text.clone(),
                tokens: *tokens,
                models: self.proposer_models.clone(),
            },
        );
        let selected = if sidecall::paused(&ctx.host, &ctx.root).is_null() {
            runner.select(&ctx.host, &ctx.config, Some("proposer"), false)
        } else {
            runner.select_local("proposer", &ctx.config)
        }?;
        Ok(selected.then(|| Box::new(runner) as Box<dyn Runner>))
    }
    fn spawn(&mut self, argv: &[String], _: &str) -> fold::Result<Value> {
        panic!("Unexpected fold process spawn: {argv:?}")
    }
    fn fold(&mut self, ctx: &LiveContext, sid: &str, path: Option<&str>) -> fold::Result<Value> {
        fold::fold_locked(ctx, sid, path, false, None, self)
    }
}
