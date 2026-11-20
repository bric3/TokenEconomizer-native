use crate::support::session_fixture;
use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::{Value, json};
use std::{
    fs::{self, File, FileTimes},
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{Duration, SystemTime},
};
use tokeneconomizer::{
    plugin::{
        context::LiveContext,
        fold, history,
        hosts::{
            self,
            session::{self, Environment},
        },
    },
    runner::Runner,
    trace::claude,
};

pub const CLAIM: &str = "`pytest …` fails here with command not found; `python -m pytest …` works.";

pub struct LogOptions {
    pub age: f64,
    pub cwd: Option<PathBuf>,
    pub folder: Option<String>,
    pub entrypoint: String,
    pub deferred: Vec<String>,
    pub interrupted: Vec<usize>,
}
impl Default for LogOptions {
    fn default() -> Self {
        Self {
            age: 3600.0,
            cwd: None,
            folder: None,
            entrypoint: "cli".into(),
            deferred: vec![],
            interrupted: vec![],
        }
    }
}

#[derive(Clone)]
pub struct HistoryFixture {
    pub root: PathBuf,
    pub claude_home: PathBuf,
    pub environment: Environment,
    pub ctx: LiveContext,
    pub spawned: Vec<Vec<String>>,
    pub clock_tick: Option<f64>,
    pub fail_plan: bool,
    pub proposer_calls: usize,
    pub before_fold: Option<fn(&mut Self, &str) -> history::Result<()>>,
    pub after_fold: Option<fn(&mut Self, &str) -> history::Result<()>>,
    pub block_after_check: bool,
    pub block_checks: usize,
}
impl HistoryFixture {
    pub fn new(root: &Path) -> Self {
        let repo = root.join("repo");
        fs::create_dir_all(repo.join(".git/info")).unwrap();
        fs::write(repo.join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(
            repo.join(".git/config"),
            "[remote \"origin\"]\n\turl = https://github.com/acme/widget.git\n",
        )
        .unwrap();
        fs::write(repo.join("pyproject.toml"), "[project]\nname='w'\n").unwrap();
        let claude_home = root.join("claude");
        fs::create_dir_all(claude_home.join("projects")).unwrap();
        let env = json!({"TOKENECONOMIZER_HOME":root.join("home"),"TE_FOLD_SYNC":"1","TE_HISTORY_IMPORT":"on","TE_TOOLPRUNE_QUIET_MINUTES":"0","TE_MICROSKILLS":"envfacts","CLAUDE_CONFIG_DIR":claude_home,"GIT_CEILING_DIRECTORIES":root,"PATH":""}).as_object().unwrap().clone();
        let ctx = LiveContext::build("claude", true, &repo, env.clone(), root, Value::Null, true)
            .unwrap();
        Self {
            root: root.into(),
            claude_home,
            environment: Environment {
                env,
                home: root.into(),
                cwd: root.into(),
            },
            ctx,
            spawned: vec![],
            clock_tick: None,
            fail_plan: false,
            proposer_calls: 0,
            before_fold: None,
            after_fold: None,
            block_after_check: false,
            block_checks: 0,
        }
    }
    pub fn reload(&mut self) {
        self.ctx = LiveContext::build(
            &self.ctx.host,
            hosts::profile(&self.ctx.host).keeps_history,
            &self.ctx.repo.root,
            self.environment.env.clone(),
            &self.environment.home,
            Value::Null,
            true,
        )
        .unwrap();
    }
    pub fn detach(&mut self) {
        self.environment.env.remove("TE_FOLD_SYNC");
        self.reload();
    }
    pub fn log(&self, sid: &str, prompts: &[Value], options: LogOptions) -> PathBuf {
        let cwd = options.cwd.as_deref().unwrap_or(&self.ctx.repo.root);
        let folder = options
            .folder
            .unwrap_or_else(|| claude::project_folder(cwd.to_str().unwrap()));
        let directory = self.claude_home.join("projects").join(folder);
        fs::create_dir_all(&directory).unwrap();
        let end = SystemTime::now() - Duration::from_secs_f64(options.age);
        let mut at = end - Duration::from_secs(300);
        let row = |kind: &str, fields: Value, at: SystemTime| {
            let date: DateTime<Utc> = at.into();
            let mut row = json!({"sessionId":sid,"cwd":cwd,"entrypoint":options.entrypoint,"version":"2.1.307","type":kind,"timestamp":date.to_rfc3339_opts(SecondsFormat::AutoSi,true)});
            row.as_object_mut()
                .unwrap()
                .extend(fields.as_object().unwrap().clone());
            row
        };
        let mut lines = vec![json!({"type":"file-history-snapshot","snapshot":{}})];
        if !options.deferred.is_empty() {
            lines.push(row(
                "attachment",
                json!({"attachment":{"type":"deferred_tools_delta","addedNames":options.deferred}}),
                at,
            ));
        }
        let mut n = 0;
        for (i, prompt) in prompts.iter().enumerate() {
            at += Duration::from_secs(1);
            let event = |kind: &str, message: Value| row(kind, json!({"message":message}), at);
            lines.push(event("user", json!({"role":"user","content":prompt[0]})));
            for call in prompt[1].as_array().unwrap() {
                n += 1;
                lines.push(event("assistant",json!({"id":format!("msg_{sid}_{n}"),"model":"claude-opus-5-5","role":"assistant","usage":{"input_tokens":10,"cache_read_input_tokens":1000,"cache_creation_input_tokens":100,"output_tokens":50},"content":[{"type":"tool_use","id":format!("tu{n}"),"name":call[0],"input":call[1]}]})));
                lines.push(event("user",json!({"role":"user","content":[{"type":"tool_result","tool_use_id":format!("tu{n}"),"content":call[2],"is_error":call[3]}]})));
            }
            if options.interrupted.contains(&i) {
                lines.push(event("user",json!({"role":"user","content":[{"type":"text","text":"[Request interrupted by user]"}]})));
            } else {
                lines.push(event("assistant",json!({"id":format!("msg_{sid}_{n}_done"),"model":"claude-opus-5-5","role":"assistant","content":[{"type":"text","text":"Done."}]})));
            }
        }
        let path = directory.join(format!("{sid}.jsonl"));
        fs::write(
            &path,
            lines
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n")
                + "\n",
        )
        .unwrap();
        File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(FileTimes::new().set_accessed(end).set_modified(end))
            .unwrap();
        path
    }
    pub fn trap(&self, i: i32) -> Vec<Value> {
        vec![json!([format!("fix bug {i}"),[
            ["Edit",{"file_path":self.ctx.repo.root.join(format!("pkg/mod{i}.py")),"old_string":"a","new_string":"b"},"ok",false],
            ["Bash",{"command":"pytest -q tests/test_a.py"},"Exit code 127\nbash: pytest: command not found",true],
            ["Bash",{"command":"python -m pytest -q tests/test_a.py"},"1 passed in 0.1s",false]
        ]])]
    }
    pub fn looked(&self) -> Vec<Value> {
        vec![
            json!(["look",[["Read",{"file_path":self.ctx.repo.root.join("a.py")},"x",false],["Bash",{"command":"ls"},"a.py",false]]]),
            json!(["and again", []]),
        ]
    }
    pub fn plan(&mut self, limits: &history::Limits) -> Value {
        let ctx = self.ctx.clone();
        history::plan(&ctx, history::now(), limits, self).unwrap()
    }
    pub fn run(&mut self, auto: bool, budget: f64) -> history::Result<Value> {
        let ctx = self.ctx.clone();
        history::run(
            &ctx,
            auto,
            history::now(),
            budget,
            &history::Limits::default(),
            self,
        )
    }
    pub fn maybe_start(&mut self) {
        let ctx = self.ctx.clone();
        history::maybe_start(&ctx, history::now(), &history::Limits::default(), self).unwrap();
    }
    pub fn mark_running(&self) {
        self.ctx
            .store
            .write(
                "history.json",
                &json!({"state":"running","heartbeat":history::now()}),
            )
            .unwrap();
    }
    pub fn live(&self, sid: &str, path: &Path) {
        let at = history::now();
        self.ctx.store.write(&format!("sessions/{sid}.json"),&json!({"session_id":sid,"host":"claude","started":at,"ended":at,"cwd":self.ctx.repo.root,"transcript":path,"turns":[]})).unwrap();
    }
    pub fn fold_live(&mut self, sid: &str) -> Value {
        let ctx = self.ctx.clone();
        fold::fold_session(&ctx, sid, None, fold::FOLD_LOCK_WAIT_S, self).unwrap()
    }
    pub fn ledger_ids(&self) -> Vec<String> {
        self.ctx
            .store
            .read_lines("ledger.jsonl", None)
            .unwrap()
            .iter()
            .filter(|row| row["kind"] == "session")
            .map(|row| row["session_id"].as_str().unwrap().to_owned())
            .collect()
    }
    pub fn hook(&self, sid: &str, event: &str, transcript: Option<&Path>) -> Value {
        let path = transcript
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.root.join(format!("{sid}.jsonl")));
        if !path.exists() {
            File::create(&path).unwrap();
        }
        let payload = json!({"hook_event_name":event,"session_id":sid,"source":"startup","reason":"other","transcript_path":path,"cwd":self.ctx.repo.root});
        let mut native = tokeneconomizer::plugin::runtime::NativeRuntime::new(
            self.environment.clone(),
            env!("CARGO_BIN_EXE_te").into(),
        );
        let out = native
            .run_hook(&[], Ok(payload.to_string().into_bytes()))
            .unwrap();
        serde_json::from_str(out.lines().next_back().unwrap()).unwrap()
    }
    pub fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_te"))
            .args([
                "--cwd",
                self.ctx.repo.root.to_str().unwrap(),
                "--host",
                &self.ctx.host,
            ])
            .args(args)
            .env_clear()
            .env("HOME", &self.environment.home)
            .env("USERPROFILE", &self.environment.home)
            .envs(
                self.environment
                    .env
                    .iter()
                    .map(|(key, value)| (key, value.as_str().unwrap())),
            )
            .output()
            .unwrap()
    }
    fn host_call(&self, method: &str, args: &Value) -> session::Result<Value> {
        session_fixture::host_call(
            &self.ctx.host,
            &self.ctx.env,
            &self.environment,
            method,
            args,
        )
    }
}

impl history::Runtime for HistoryFixture {
    fn host(&mut self, method: &str, args: Value) -> history::Result<Value> {
        self.host_call(method, &args)
            .map_err(|e| history::Error::Failed(e.to_string()))
    }
    fn plan(
        &mut self,
        ctx: &LiveContext,
        at: f64,
        limits: &history::Limits,
    ) -> history::Result<Value> {
        if self.fail_plan {
            return Err(history::Error::Failed("unreadable history".into()));
        }
        history::plan(ctx, at, limits, self)
    }
    fn fold(
        &mut self,
        sid: &str,
        path: Option<&str>,
        imported: bool,
        span: Option<(&Value, &Value)>,
    ) -> history::Result<Value> {
        if let Some(before) = self.before_fold {
            before(self, sid)?;
        }
        let ctx = self.ctx.clone();
        let result = fold::fold_locked(&ctx, sid, path, imported, span, self)
            .map_err(|e| history::Error::Failed(e.to_string()))?;
        if let Some(after) = self.after_fold {
            after(self, sid)?;
        }
        Ok(result)
    }
    fn pending(&mut self) -> history::Result<Vec<String>> {
        let ctx = self.ctx.clone();
        fold::pending_sessions(&ctx, &[], fold::STALE_S, history::now(), self)
            .map_err(|e| history::Error::Failed(e.to_string()))
    }
    fn spawn(&mut self, argv: &[&str], _: &str) -> history::Result<Value> {
        self.spawned
            .push(argv.iter().map(|s| s.to_string()).collect());
        Ok(json!(true))
    }
    fn say(&mut self, _text: &str) -> history::Result<()> {
        Ok(())
    }
    fn clock(&mut self) -> history::Result<f64> {
        if let Some(tick) = &mut self.clock_tick {
            let now = *tick;
            *tick += 1.0;
            Ok(now)
        } else {
            Ok(history::clock())
        }
    }
}
impl fold::Runtime for HistoryFixture {
    fn host(&mut self, method: &str, args: Value) -> fold::Result<Value> {
        self.host_call(method, &args).map_err(|e| match e {
            session::Error::Io(e) => fold::Error::Io(e),
            session::Error::Invalid(e) => fold::Error::Value(e),
            session::Error::Later(_) => fold::Error::Later,
            e => fold::Error::Failed(e.to_string()),
        })
    }
    fn proposer(&mut self, _: &LiveContext) -> fold::Result<Option<Box<dyn Runner>>> {
        self.proposer_calls += 1;
        Ok(None)
    }
    fn spawn(&mut self, argv: &[String], _: &str) -> fold::Result<Value> {
        self.spawned.push(argv.to_vec());
        Ok(json!(true))
    }
    fn fold(&mut self, ctx: &LiveContext, sid: &str, path: Option<&str>) -> fold::Result<Value> {
        fold::fold_locked(ctx, sid, path, false, None, self)
    }
    fn blocks(&mut self, ctx: &LiveContext) -> fold::Result<bool> {
        self.block_checks += 1;
        if self.block_after_check && self.block_checks == 1 {
            ctx.store.write(
                "history.json",
                &json!({"state":"partial","plan":[],"next":0}),
            )?;
            return Ok(false);
        }
        fold::import_blocks(ctx)
    }
}
