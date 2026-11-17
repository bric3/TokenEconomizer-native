use super::hook;
use crate::{
    trace,
    value::{json_text, scalar_text, truthy},
};
use indexmap::IndexMap;
use serde_json::{Value, json, value::RawValue};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, MutexGuard, TryLockError,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

pub type Result<T> = std::result::Result<T, String>;
pub mod server;
pub const HOOK_TIMEOUT_S: f64 = 20.0;
pub const LOCK_TIMEOUT_S: f64 = 5.0;
pub const MAX_IGNORED: usize = 4096;
pub const MAX_ENDED: usize = 4096;
pub const SECTION_ID: &str = "tokeneconomizer.facts";
pub const REDIRECT_FIELD: &str = "tokeneconomizer_redirect";

pub fn disabled(env: &serde_json::Map<String, Value>) -> bool {
    hook::enabled_flag(&env.get("TE_SIDECALL").cloned().unwrap_or(Value::Null))
        || hook::enabled_flag(&env.get("TE_DISABLE").cloned().unwrap_or(Value::Null))
}

pub fn untrusted(name: &Value) -> bool {
    name.as_str().is_some_and(|name| {
        ["web_extract", "web_search", "tool_call"].contains(&name)
            || name.starts_with("browser_")
            || name.starts_with("mcp_")
    })
}

pub fn with_redirect(result: &Value, text: &Value) -> Result<Value> {
    let Some(raw) = result.as_str() else {
        return Ok(Value::Null);
    };
    // shortcut: raw JSON validation is strict, add non-finite values in the JSON parity audit.
    let Ok(parsed) = serde_json::from_str::<&RawValue>(raw) else {
        return Ok(json!(format!("{raw}\n\n{}", scalar_text(text))));
    };
    if !parsed.get().starts_with('{') {
        return Ok(Value::Null);
    }
    let rest = raw
        .trim_start()
        .strip_prefix('{')
        .ok_or("missing JSON object")?
        .trim_start();
    let field = format!(
        "{{{}: {}",
        json_text(&json!(REDIRECT_FIELD)),
        json_text(text)
    );
    Ok(json!(if rest.starts_with('}') {
        format!("{field}}}")
    } else {
        format!("{field}, {rest}")
    }))
}

pub struct Limits {
    pub max_ignored: usize,
    pub max_ended: usize,
    pub lock_timeout: f64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_ignored: MAX_IGNORED,
            max_ended: MAX_ENDED,
            lock_timeout: LOCK_TIMEOUT_S,
        }
    }
}

pub trait Runtime {
    fn disabled(&mut self) -> Result<bool>;
    fn home(&mut self) -> Result<PathBuf>;
    fn state_root(&mut self) -> Result<PathBuf>;
    fn dispatch(&mut self, payload: Value, cwd: &str, mode: &str, home: &Path) -> Result<Value>;
    fn notice(&mut self, text: &str) -> Result<()>;
    fn lineage(&mut self, home: &Path, sid: &str) -> Result<Vec<Value>> {
        if !home.is_absolute() {
            return Err("Hermes home must be an absolute path".into());
        }
        trace::hermes::gateway_lineage(&home.join("state.db"), sid).map_err(|e| e.to_string())
    }
    fn is_directory(&mut self, path: &Path) -> bool {
        path.is_dir()
    }
}

#[derive(Default)]
struct Buffered {
    held: Vec<String>,
    notices: Vec<String>,
}
pub struct Session {
    id: String,
    cwd: String,
    mode: &'static str,
    // Pruning checks activity without waiting for a callback's session lock.
    started: AtomicBool,
    data: Mutex<Buffered>,
}
impl Session {
    fn data(&self) -> Result<MutexGuard<'_, Buffered>> {
        self.data.lock().map_err(|e| e.to_string())
    }
    pub fn lock(&self) -> Result<impl Drop + '_> {
        self.data()
    }
    fn timed(&self, seconds: f64) -> Result<Option<MutexGuard<'_, Buffered>>> {
        let deadline =
            Instant::now() + Duration::try_from_secs_f64(seconds).map_err(|e| e.to_string())?;
        loop {
            match self.data.try_lock() {
                Ok(guard) => return Ok(Some(guard)),
                Err(TryLockError::Poisoned(error)) => return Err(error.to_string()),
                Err(TryLockError::WouldBlock) => {
                    let left = deadline.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        return Ok(None);
                    }
                    thread::sleep(left.min(Duration::from_millis(5)));
                }
            }
        }
    }
    pub fn snapshot(&self) -> Result<Value> {
        let data = self.data()?;
        Ok(
            json!({"id":self.id,"cwd":self.cwd,"mode":self.mode,"started":self.started.load(Ordering::Relaxed),"held":data.held,"notices":data.notices}),
        )
    }
    fn send(
        &self,
        data: &mut Buffered,
        event: &str,
        fields: Value,
        runtime: &mut dyn Runtime,
    ) -> Result<Value> {
        let mut payload = fields
            .as_object()
            .cloned()
            .ok_or("hook fields must be an object")?;
        payload.extend(json!({"session_id":self.id,"cwd":self.cwd,"hook_event_name":event,
            "transcript_path":runtime.state_root()?.join("hermes-sessions").join(format!("{}.json",self.id))}).as_object().unwrap().clone());
        let home = runtime.home()?;
        let out = runtime.dispatch(json!(payload), &self.cwd, self.mode, &home)?;
        if let Some(notice) = out["systemMessage"].as_str().filter(|s| !s.is_empty()) {
            runtime.notice(notice)?;
            data.notices.push(notice.into());
        }
        Ok(out)
    }
    fn start(&self, data: &mut Buffered, source: &str, runtime: &mut dyn Runtime) -> Result<Value> {
        self.started.store(true, Ordering::Relaxed);
        let out = self.send(data, "SessionStart", json!({"source":source}), runtime)?;
        Ok(context(&out)
            .filter(|v| truthy(v))
            .cloned()
            .unwrap_or(json!("")))
    }
    fn end(&self, data: &mut Buffered, reason: Value, runtime: &mut dyn Runtime) -> Result<()> {
        if self.started.swap(false, Ordering::Relaxed) {
            self.send(data, "SessionEnd", json!({"reason":reason}), runtime)?;
        }
        Ok(())
    }
}

#[derive(Default)]
struct Registry {
    sessions: IndexMap<String, Arc<Session>>,
    ignored: BTreeSet<String>,
}
#[derive(Default)]
pub struct Bridge {
    registry: Mutex<Registry>,
}
fn context(out: &Value) -> Option<&Value> {
    out.get("hookSpecificOutput")
        .and_then(|v| v.get("additionalContext"))
}
fn text(args: &Value, key: &str) -> String {
    args.get(key).map(scalar_text).unwrap_or_default()
}

impl Bridge {
    pub fn session(
        &self,
        sid: &str,
        runtime: &mut dyn Runtime,
        limits: &Limits,
    ) -> Result<Option<Arc<Session>>> {
        if sid.is_empty() || runtime.disabled()? {
            return Ok(None);
        }
        {
            let registry = self.registry.lock().map_err(|e| e.to_string())?;
            if let Some(found) = registry.sessions.get(sid) {
                return Ok(Some(found.clone()));
            }
            if registry.ignored.contains(sid) {
                return Ok(None);
            }
        }
        let chain = match runtime.home().and_then(|home| runtime.lineage(&home, sid)) {
            Ok(chain) if !chain.is_empty() => chain,
            _ => return Ok(None),
        };
        let first = chain.first().unwrap();
        let leaf = chain.last().unwrap();
        let source = first
            .get("created_source")
            .filter(|v| truthy(v))
            .unwrap_or(&first["source"]);
        let cwd = leaf
            .get("cwd")
            .filter(|v| truthy(v))
            .unwrap_or(&first["cwd"]);
        let cwd = cwd.as_str().unwrap_or("");
        let mut registry = self.registry.lock().map_err(|e| e.to_string())?;
        if ![json!("cli"), json!("oneshot")].contains(source)
            || cwd.is_empty()
            || !runtime.is_directory(Path::new(cwd))
        {
            if registry.ignored.len() >= limits.max_ignored {
                registry.ignored.clear();
            }
            registry.ignored.insert(sid.into());
            return Ok(None);
        }
        let root = first["id"].as_str().ok_or("session id must be text")?;
        let found = if let Some(found) = registry.sessions.get(root) {
            found.clone()
        } else {
            let session = Arc::new(Session {
                id: root.into(),
                cwd: cwd.into(),
                mode: if source == "oneshot" {
                    "oneshot"
                } else {
                    "cli"
                },
                started: AtomicBool::new(false),
                data: Mutex::default(),
            });
            let ended = registry
                .sessions
                .iter()
                .filter(|(_, s)| !s.started.load(Ordering::Relaxed))
                .map(|(k, _)| k.clone())
                .collect::<Vec<_>>();
            for key in ended
                .iter()
                .take(ended.len().saturating_sub(limits.max_ended))
            {
                registry.sessions.shift_remove(key);
            }
            registry.sessions.insert(root.into(), session.clone());
            session
        };
        registry.sessions.insert(sid.into(), found.clone());
        Ok(Some(found))
    }
    pub fn snapshots(&self) -> Result<Value> {
        let sessions = self
            .registry
            .lock()
            .map_err(|e| e.to_string())?
            .sessions
            .clone();
        let mut out = serde_json::Map::new();
        for (key, session) in sessions {
            out.insert(key, session.snapshot()?);
        }
        Ok(json!(out))
    }
    pub fn invoke(
        &self,
        method: &str,
        args: &Value,
        runtime: &mut dyn Runtime,
        limits: &Limits,
    ) -> Result<Value> {
        if method == "snapshots" {
            return self.snapshots();
        }
        let sid = if method == "section" {
            args["info"].get("session_id")
        } else {
            args.get("session_id")
        }
        .filter(|v| truthy(v))
        .map(scalar_text)
        .unwrap_or_default();
        let Some(rec) = self.session(&sid, runtime, limits)? else {
            return Ok(if method == "section" {
                json!("")
            } else {
                Value::Null
            });
        };
        if method == "session" {
            return rec.snapshot();
        }
        if method == "transform_tool_result" && !args["result"].is_string() {
            return Ok(Value::Null);
        }
        let mut data = if ["section", "on_session_finalize"].contains(&method) {
            rec.data()?
        } else {
            let Some(data) = rec.timed(limits.lock_timeout)? else {
                return Ok(Value::Null);
            };
            data
        };
        match method {
            "section" => {
                if rec.started.load(Ordering::Relaxed) {
                    Ok(json!(""))
                } else {
                    rec.start(&mut data, "startup", runtime)
                }
            }
            "pre_llm_call" => {
                data.held.clear();
                if !rec.started.load(Ordering::Relaxed) {
                    rec.start(&mut data, "resume", runtime)?;
                }
                let prompt = match &args["user_message"] {
                    Value::String(text) => text.clone(),
                    Value::Null => String::new(),
                    Value::Array(parts) => parts
                        .iter()
                        .filter(|v| v.is_object())
                        .map(|v| {
                            v.get("text")
                                .map(|v| v.as_str().ok_or("message text must be text"))
                                .transpose()
                                .map(|v| v.unwrap_or(""))
                        })
                        .collect::<std::result::Result<Vec<_>, _>>()?
                        .join("\n"),
                    _ => return Err("user message must be text or a list".into()),
                };
                let out = rec.send(
                    &mut data,
                    "UserPromptSubmit",
                    json!({"prompt_id":text(args,"turn_id"),"prompt":prompt}),
                    runtime,
                )?;
                Ok(context(&out)
                    .filter(|v| truthy(v))
                    .map(|v| json!({"context":v}))
                    .unwrap_or(Value::Null))
            }
            "transform_tool_result" => {
                let out = rec.send(&mut data,"PostToolUse",json!({"prompt_id":text(args,"turn_id"),"tool_name":args["tool_name"],"tool_use_id":args.get("tool_call_id").cloned().unwrap_or(json!("")),"tool_input":args.get("args").filter(|v|truthy(v)).cloned().unwrap_or(json!({})),"tool_response":{"content":args["result"],"status":args["status"],"error":args["error_message"]}}),runtime)?;
                if let Some(text) = context(&out).filter(|v| truthy(v)) {
                    data.held
                        .push(text.as_str().ok_or("redirect must be text")?.into());
                }
                if data.held.is_empty() || untrusted(&args["tool_name"]) {
                    return Ok(Value::Null);
                }
                let answer = with_redirect(&args["result"], &json!(data.held.join("\n\n")))?;
                if !answer.is_null() {
                    data.held.clear();
                }
                Ok(answer)
            }
            "on_session_end" => {
                if truthy(&args["completed"])
                    && !truthy(&args["interrupted"])
                    && !truthy(&args["failed"])
                {
                    rec.send(
                        &mut data,
                        "Stop",
                        json!({"prompt_id":text(args,"turn_id"),"stop_hook_active":false}),
                        runtime,
                    )?;
                }
                if rec.mode == "oneshot" {
                    rec.end(&mut data, json!("oneshot"), runtime)?;
                }
                Ok(Value::Null)
            }
            "on_session_finalize" => {
                rec.end(
                    &mut data,
                    json!(
                        args.get("reason")
                            .filter(|v| truthy(v))
                            .map(scalar_text)
                            .unwrap_or("finalize".into())
                    ),
                    runtime,
                )?;
                let notices = std::mem::take(&mut data.notices);
                drop(data);
                self.registry
                    .lock()
                    .map_err(|e| e.to_string())?
                    .sessions
                    .retain(|_, session| !Arc::ptr_eq(session, &rec));
                Ok(if notices.is_empty() {
                    Value::Null
                } else {
                    json!(notices.join("\n"))
                })
            }
            _ => Err(format!("unknown Hermes callback: {method}")),
        }
    }
}
