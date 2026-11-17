use super::{Call, Runtime, environment, no_reply, with_directory};
use crate::{
    plugin::hosts::session::{self, Environment},
    store::Store,
    trace::hermes as trace,
    value::{float_or_zero, truthy},
};
use regex::Regex;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::LazyLock,
};

pub const SOURCE: &str = "tool";
pub const FLAGS: [&str; 10] = [
    "chat",
    "-Q",
    "--oneshot",
    "--source",
    SOURCE,
    "--ignore-rules",
    "--max-turns",
    "1",
    "-t",
    "bot_room",
];
pub const DELETE_TIMEOUT_S: f64 = 60.0;
pub const TITLE_WAIT_S: f64 = 3.0;
pub const ABANDON_S: f64 = 600.0;
static ID: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^session_id:\s*(\S+)").unwrap());
type Result<T> = std::result::Result<T, String>;

pub struct Limits {
    pub delete_timeout: f64,
    pub title_wait: f64,
    pub abandon: f64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            delete_timeout: DELETE_TIMEOUT_S,
            title_wait: TITLE_WAIT_S,
            abandon: ABANDON_S,
        }
    }
}
pub struct Hermes<'a> {
    pub environment: &'a Environment,
    pub root: &'a Path,
    pub limits: Limits,
}
impl Hermes<'_> {
    pub fn home(&self) -> PathBuf {
        session::hermes_home(&self.environment.env, &self.environment.home)
    }
    pub fn db(&self) -> PathBuf {
        self.home().join("state.db")
    }
    fn db_path(&self) -> PathBuf {
        self.environment.path(&self.db())
    }
    pub fn ledger(&self) -> Result<Store> {
        Store::new(&self.root.join("hosts/hermes")).map_err(|e| e.to_string())
    }
    pub fn charge(
        &self,
        sid: &str,
        wait: Option<f64>,
        abandoned: bool,
        runtime: &mut dyn Runtime,
    ) -> Result<Option<f64>> {
        let deadline = runtime.now()? + wait.unwrap_or(self.limits.title_wait);
        let db = self.db_path();
        let (row, usage, ended) = loop {
            let (row, usage) = trace::session_row(&db, sid);
            let ended = row.as_ref().is_some_and(|row| truthy(&row["ended_at"]));
            let titled = row.as_ref().is_some_and(|row| truthy(&row["title"]))
                || usage.iter().any(|u| u["task"] == "title_generation");
            if ended && titled || runtime.now()? >= deadline {
                break (row, usage, ended);
            }
            runtime.sleep(0.25)?;
        };
        if !(ended || abandoned && row.is_some()) {
            return Ok(None);
        }
        let mut total = 0.0;
        for row in usage {
            let mut usage = trace::row_usage(&row);
            if !truthy(&usage["model"]) {
                usage["model"] = json!("hermes");
            }
            total += runtime.effective(&usage)?.unwrap_or(0.0);
        }
        Ok(Some(total))
    }
    pub fn delete(&self, sid: &str, cli: &str, runtime: &mut dyn Runtime) -> bool {
        if runtime
            .run(
                &[
                    cli.into(),
                    "sessions".into(),
                    "delete".into(),
                    "--yes".into(),
                    sid.into(),
                ],
                &self.environment.cwd,
                &environment(&self.environment.env, true),
                None,
                self.limits.delete_timeout,
            )
            .is_err()
        {
            return false;
        }
        let db = self.db_path();
        trace::session_row(&db, sid).0.is_none() && db.exists()
    }
    pub fn settle(
        &self,
        sid: &str,
        cli: &str,
        charged: bool,
        wait: Option<f64>,
        abandoned: bool,
        runtime: &mut dyn Runtime,
    ) -> Result<(f64, Value)> {
        let spent = if charged {
            0.0
        } else {
            let Some(spent) = self.charge(sid, wait, abandoned, runtime)? else {
                return Ok((0.0, json!({"charged":false})));
            };
            spent
        };
        Ok((
            spent,
            if self.delete(sid, cli, runtime) {
                Value::Null
            } else {
                json!({"charged":true})
            },
        ))
    }
    pub fn defer(&self, sid: &str, left: &Value, runtime: &mut dyn Runtime) -> Result<()> {
        let store = self.ledger()?;
        let _lock = store.locked("sidecalls", 10.0).map_err(|e| e.to_string())?;
        let mut doc = store
            .read("sidecalls.json", Value::Null)
            .map_err(|e| e.to_string())?;
        if !truthy(&doc) {
            doc = json!({});
        }
        let mut entry = left
            .as_object()
            .cloned()
            .ok_or("deferred Hermes entry must be an object")?;
        let at = if truthy(&left["at"]) {
            left["at"].clone()
        } else {
            json!(runtime.now()?)
        };
        entry.insert("at".into(), at);
        doc.as_object_mut()
            .ok_or("Hermes ledger must be an object")?
            .insert(sid.into(), json!(entry));
        store
            .write("sidecalls.json", &doc)
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn settle_earlier(&self, cli: &str, runtime: &mut dyn Runtime) -> Result<f64> {
        let store = self.ledger()?;
        let doc = {
            let _lock = store.locked("sidecalls", 10.0).map_err(|e| e.to_string())?;
            let doc = store
                .read("sidecalls.json", Value::Null)
                .map_err(|e| e.to_string())?;
            if truthy(&doc) {
                store
                    .write("sidecalls.json", &json!({}))
                    .map_err(|e| e.to_string())?;
            }
            if truthy(&doc) { doc } else { json!({}) }
        };
        let mut spent = 0.0;
        for (sid, entry) in doc.as_object().ok_or("Hermes ledger must be an object")? {
            if trace::session_row(&self.db_path(), sid).0.is_none() {
                continue;
            }
            let age = runtime.now()? - float_or_zero(&entry["at"])?;
            let (now, mut left) = self.settle(
                sid,
                cli,
                truthy(&entry["charged"]),
                Some(0.0),
                age >= self.limits.abandon,
                runtime,
            )?;
            spent += now;
            if truthy(&left) && runtime.now()? - float_or_zero(&entry["at"])? < 7.0 * 86400.0 {
                left["at"] = entry["at"].clone();
                self.defer(sid, &left, runtime)?;
            }
        }
        Ok(spent)
    }
    pub fn call(
        &self,
        request: &Call<'_>,
        runtime: &mut dyn Runtime,
    ) -> Result<(Value, Option<f64>)> {
        let cli = if request.cli.is_empty() {
            runtime.hermes_cli(self.environment)?
        } else {
            request.cli.into()
        };
        let mut spent = self.settle_earlier(&cli, runtime)?;
        let started = runtime.now()?;
        let out = with_directory("te-side-", false, runtime, |cwd, runtime| {
            let query = cwd.join("query.txt");
            fs::write(&query, request.prompt).map_err(|e| e.to_string())?;
            let mut cmd = vec![cli.clone()];
            cmd.extend(FLAGS.map(String::from));
            if !request.model.is_empty() {
                cmd.extend(["--model".into(), request.model.into()]);
            }
            cmd.extend(["--query-file".into(), query.to_string_lossy().into_owned()]);
            Ok(runtime
                .run(
                    &cmd,
                    cwd,
                    &environment(&self.environment.env, true),
                    None,
                    request.timeout,
                )
                .ok())
        })?;
        let sid = out
            .as_ref()
            .and_then(|out| ID.captures(&out.stderr))
            .map(|m| m[1].to_string())
            .or_else(|| {
                trace::find_session(&self.db_path(), SOURCE, request.prompt, started - 1.0)
            });
        let text = out
            .as_ref()
            .filter(|out| out.code == 0)
            .map(|out| out.stdout.trim())
            .unwrap_or("");
        if text.is_empty() {
            let (stdout, stderr, code) = match &out {
                Some(out) => (out.stdout.as_str(), out.stderr.as_str(), json!(out.code)),
                None => ("", "no reply within the time limit", json!("timeout")),
            };
            no_reply("hermes", self.root, stdout, stderr, &code, runtime.now()?);
        }
        if let Some(sid) = sid.filter(|s| !s.is_empty()) {
            let (now, left) = self.settle(&sid, &cli, false, None, false, runtime)?;
            spent += now;
            if truthy(&left) {
                self.defer(&sid, &left, runtime)?;
            }
        }
        Ok((json!(text), if spent != 0.0 { Some(spent) } else { None }))
    }
}
