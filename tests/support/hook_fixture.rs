use serde_json::{Value, json};
use std::{
    fs, io,
    path::{Path, PathBuf},
};
use tokeneconomizer::{
    plugin::{
        context::LiveContext,
        controller::{self, LiveController},
        history, hook,
        hosts::{
            session::{self, Environment},
            state,
        },
        judge, paths, sidecall,
    },
    runner::Runner,
    store::Store,
};

pub struct HookFixture {
    pub host: String,
    pub inline_judge: bool,
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
        panic!("Unexpected process lookup")
    }
}
impl judge::Runtime for HookFixture {
    fn paused(&mut self) -> Result<bool, String> {
        assert!(
            self.inline_judge,
            "These cold turns must not select a model"
        );
        Ok(false)
    }
    fn runner(
        &mut self,
        _role: &str,
        _local_only: bool,
    ) -> Result<Option<Box<dyn Runner>>, String> {
        assert!(self.inline_judge, "Unexpected model selection");
        Ok(None)
    }
    fn spawn_job(&mut self, sid: &str, id: &str) -> Result<(), String> {
        assert!(self.inline_judge, "Unexpected judge job");
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
        controller.invoke(method, self)
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
        panic!("Unexpected local model routing")
    }
    fn map(&mut self, _ctx: &LiveContext, _method: &str, _args: Value) -> Result<Value, String> {
        panic!("Unexpected repository map")
    }
    fn history_notice(&mut self, ctx: &LiveContext) -> Result<Value, String> {
        history::notice(ctx, &history::Limits::default()).map_err(|e| e.to_string())
    }
    fn history_start(&mut self, ctx: &LiveContext) -> Result<(), String> {
        assert_eq!(ctx.config["history_import"], false);
        Ok(())
    }
    fn migration_start(&mut self, _ctx: &LiveContext) -> Result<(), String> {
        Ok(())
    }
    fn pending_sessions(
        &mut self,
        _ctx: &LiveContext,
        _exclude: &[String],
    ) -> Result<Vec<String>, String> {
        Ok(vec![])
    }
    fn spawn_folds(&mut self, _ctx: &LiveContext, sessions: &[String]) -> Result<(), String> {
        self.folds.extend_from_slice(sessions);
        Ok(())
    }
}
