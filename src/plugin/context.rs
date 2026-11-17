use super::{
    config,
    paths::{self, RepoId},
    process::Launch,
    repos,
};
use crate::{store::Store, value::truthy};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct LiveContext {
    pub root: PathBuf,
    pub store: Store,
    pub repo: RepoId,
    pub host: String,
    pub keeps_history: bool,
    pub config: Value,
    pub env: Map<String, Value>,
    pub home: PathBuf,
    pub unsettled: Option<(PathBuf, Value)>,
}
pub struct Entrypoint {
    pub executable: PathBuf,
    pub core: PathBuf,
    pub windows_runner: PathBuf,
}
pub trait Runtime {
    fn headless(
        &mut self,
        env: &Map<String, Value>,
        transcript: Option<&str>,
    ) -> Result<bool, String>;
    fn spawn(
        &mut self,
        request: &Launch,
        entrypoint: &Entrypoint,
        windows: bool,
    ) -> Result<bool, String>;
}

impl LiveContext {
    pub fn build(
        host: &str,
        keeps_history: bool,
        cwd: &Path,
        env: Map<String, Value>,
        home: &Path,
        session: Value,
        settle: bool,
    ) -> Result<Self, String> {
        let root = paths::state_root(&env, home)?;
        let pinned = if truthy(&session) {
            repos::pinned(&root, host, &session)?
        } else {
            None
        };
        let (repo, unsettled) = match pinned {
            Some(repo) => (repo, None),
            None => (
                paths::repo_identity(cwd, &env, home)?,
                Some((cwd.into(), session)),
            ),
        };
        let base = paths::repo_state_dir(&repo, host, &env, home)?;
        let config = serde_json::to_value(config::load_config(&root, base.parent(), &env))
            .map_err(|e| e.to_string())?;
        let mut ctx = Self {
            root,
            store: Store::new(&base).map_err(|e| e.to_string())?,
            repo,
            host: host.into(),
            keeps_history,
            config,
            env,
            home: home.into(),
            unsettled,
        };
        if settle {
            ctx.settle()?;
        }
        Ok(ctx)
    }
    pub fn mech_dir(&self) -> Result<PathBuf, String> {
        self.store
            .ensure(&["mechanisms"])
            .map_err(|e| e.to_string())
    }
    pub fn is_headless(
        &self,
        transcript: Option<&str>,
        runtime: &mut dyn Runtime,
    ) -> Result<bool, String> {
        match self.headless_override() {
            Some(value) => Ok(value),
            None => runtime.headless(&self.env, transcript),
        }
    }
    pub fn headless_override(&self) -> Option<bool> {
        match self.config["mode"].as_str() {
            Some("headless") => Some(true),
            Some("interactive") => Some(false),
            _ => None,
        }
    }
    pub fn settle(&mut self) -> Result<(), String> {
        let Some((cwd, session)) = self.unsettled.take() else {
            return Ok(());
        };
        if repos::settle(
            &self.root,
            &self.host,
            &self.repo,
            &cwd,
            &self.env,
            &self.home,
            Some(&session),
        )? {
            self.config = serde_json::to_value(config::load_config(
                &self.root,
                self.store.base.parent(),
                &self.env,
            ))
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    pub fn spawn(
        &self,
        argv: &[String],
        log_name: &str,
        entrypoint: &Entrypoint,
        windows: bool,
        runtime: &mut dyn Runtime,
    ) -> Result<bool, String> {
        let mut env = self.env.clone();
        let prior = env
            .get("PYTHONPATH")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty());
        let core = entrypoint.core.to_string_lossy();
        env.insert(
            "PYTHONPATH".into(),
            json!(if let Some(prior) = prior {
                format!("{core}{}{prior}", if windows { ";" } else { ":" })
            } else {
                core.into_owned()
            }),
        );
        env.insert("PYTHONUTF8".into(), json!("1"));
        // shortcut: launch the migration's CLI, replace these entry points during native CLI packaging.
        let mut cmd = vec![
            entrypoint.executable.to_string_lossy().into_owned(),
            "-m".into(),
            "tokeneconomizer.plugin.cli".into(),
        ];
        cmd.extend_from_slice(argv);
        cmd.extend([
            "--host".into(),
            self.host.clone(),
            "--cwd".into(),
            self.repo.root.to_string_lossy().into_owned(),
        ]);
        self.store
            .ensure(&[] as &[&str])
            .map_err(|e| e.to_string())?;
        runtime.spawn(
            &Launch {
                cmd,
                cwd: self.repo.root.clone(),
                env,
                log: self.store.base.join(log_name),
            },
            entrypoint,
            windows,
        )
    }
}
