use super::{
    hosts::{self, EXIT_RE, RESULT_CAP},
    paths,
};
use crate::{
    envelope,
    loopguard::turn::{FoldOptions, TurnGuard},
    store::Store,
    value::{scalar_text, truthy},
};
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeMap,
    fmt, fs,
    io::{self, Write},
    path::Path,
    sync::LazyLock,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

pub const STATE: &str = "history.json";
pub const LOG: &str = "history.log";
pub const BUDGET_S: f64 = 1800.0;
pub const HEARTBEAT_STALE_S: f64 = 900.0;
pub const MAX_BYTES: u64 = 256 * 1024 * 1024;
pub const HISTORY_LOCK_S: f64 = 1.0;
pub const MAX_ATTEMPTS: i64 = 2;
pub const STALE_S: f64 = 600.0;
pub const FOLD_LOCK_WAIT_S: f64 = 240.0;
pub const HISTORY_BLOCKING: [&str; 2] = ["running", "partial"];
pub const HOST_LABEL: [(&str, &str); 8] = [
    ("claude", "Claude Code"),
    ("codex", "Codex"),
    ("copilot", "Copilot CLI"),
    ("pi", "Pi"),
    ("opencode", "opencode"),
    ("cursor", "Cursor"),
    ("hermes", "Hermes Agent"),
    ("droid", "Droid"),
];

#[derive(Debug)]
pub enum Error {
    Failed(String),
    Cancelled,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Failed(error) => error.fmt(f),
            Self::Cancelled => f.write_str("history import cancelled"),
        }
    }
}
impl std::error::Error for Error {}
impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Failed(error.to_string())
    }
}
impl From<String> for Error {
    fn from(error: String) -> Self {
        Self::Failed(error)
    }
}
pub type Result<T> = std::result::Result<T, Error>;

pub use super::context::LiveContext as Context;
pub struct Limits {
    pub max_bytes: u64,
    pub max_attempts: i64,
    pub heartbeat_stale: f64,
    pub history_lock: f64,
    pub fold_lock: f64,
    pub stale: f64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            max_bytes: MAX_BYTES,
            max_attempts: MAX_ATTEMPTS,
            heartbeat_stale: HEARTBEAT_STALE_S,
            history_lock: HISTORY_LOCK_S,
            fold_lock: FOLD_LOCK_WAIT_S,
            stale: STALE_S,
        }
    }
}
pub trait Runtime {
    fn host(&mut self, method: &str, args: Value) -> Result<Value>;
    fn plan(&mut self, ctx: &Context, now: f64, limits: &Limits) -> Result<Value>;
    fn fold(
        &mut self,
        sid: &str,
        path: Option<&str>,
        imported: bool,
        span: Option<(&Value, &Value)>,
    ) -> Result<Value>;
    fn pending(&mut self) -> Result<Vec<String>>;
    fn spawn(&mut self, argv: &[&str], log: &str) -> Result<Value>;
    fn say(&mut self, text: &str) -> Result<()>;
    fn clock(&mut self) -> Result<f64> {
        Ok(clock())
    }
}
pub fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}
pub fn clock() -> f64 {
    static START: LazyLock<Instant> = LazyLock::new(Instant::now);
    START.elapsed().as_secs_f64()
}
pub fn num(value: &Value, default: &Value) -> Value {
    if value.is_number() {
        value.clone()
    } else {
        default.clone()
    }
}
fn n(value: &Value) -> f64 {
    if value.is_number() {
        value.as_f64().unwrap_or(0.0)
    } else {
        0.0
    }
}
fn integer(value: &Value) -> i64 {
    // shortcut: counters use i64, extend their range during the final numeric parity audit.
    value.as_i64().unwrap_or_else(|| n(value) as i64)
}
fn object(value: &Value) -> Result<&Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| Error::Failed("history document must be an object".into()))
}
fn object_mut(value: &mut Value) -> Result<&mut Map<String, Value>> {
    value
        .as_object_mut()
        .ok_or_else(|| Error::Failed("history document must be an object".into()))
}
fn remove(doc: &mut Value, key: &str) -> Result<()> {
    object_mut(doc)?.remove(key);
    Ok(())
}
fn skip(skipped: &mut Map<String, Value>, why: &str) {
    let count = integer(&skipped.get(why).cloned().unwrap_or(Value::Null));
    skipped.insert(why.into(), json!(count + 1));
}
fn label(host: &str) -> &'static str {
    HOST_LABEL
        .iter()
        .find(|(name, _)| *name == host)
        .map(|(_, label)| *label)
        .unwrap_or("Claude Code")
}
pub fn record_error(root: &Path, error: &str) {
    let _ = (|| -> io::Result<()> {
        fs::create_dir_all(root)?;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join("errors.log"))?;
        writeln!(
            file,
            "--- {} pid={} (history)\n{error}",
            chrono::Local::now().format("%Y-%m-%dT%H:%M:%S"),
            std::process::id()
        )
    })();
}
pub fn read_state(store: &Store) -> Result<Option<Value>> {
    Ok(store
        .read(STATE, Value::Null)?
        .as_object()
        .map(|doc| json!(doc)))
}
pub fn stale(doc: &Value, at: f64, limit: f64) -> bool {
    at - n(&doc["heartbeat"]) >= limit
}
pub fn ledger_size(store: &Store) -> u64 {
    fs::metadata(store.base.join("ledger.jsonl"))
        .map(|m| m.len())
        .unwrap_or(0)
}
pub fn session_records(store: &Store) -> Result<Vec<Value>> {
    if ledger_size(store) == 0 {
        return Ok(Vec::new());
    }
    Ok(store
        .read_lines("ledger.jsonl", None)?
        .into_iter()
        .filter(|rec| rec.is_object() && rec["kind"] == "session")
        .collect())
}
pub fn nothing_folded(store: &Store) -> Result<bool> {
    Ok(!truthy(&store.read("folded.json", Value::Null)?) && session_records(store)?.is_empty())
}
pub fn live_folds(store: &Store) -> Result<bool> {
    let folded = store.read("folded.json", json!({}))?;
    if truthy(&folded)
        && object(&folded)?
            .values()
            .any(|value| !(value.is_object() && truthy(&value["imported"])))
    {
        return Ok(true);
    }
    Ok(session_records(store)?
        .iter()
        .any(|rec| !truthy(&rec["imported"])))
}
pub fn folded_by_import(store: &Store, sid: &str) -> Result<bool> {
    let doc = store.read("folded.json", json!({}))?;
    Ok(doc[sid].is_object() && truthy(&doc[sid]["imported"]))
}
pub fn refusal(ctx: &Context, resuming: bool) -> Result<Option<String>> {
    Ok(if resuming {
        if live_folds(&ctx.store)? {
            Some(format!(
                "{} has sessions folded since this import started; its earlier sessions would be learned after them",
                ctx.repo.label
            ))
        } else {
            None
        }
    } else if !nothing_folded(&ctx.store)? {
        Some(format!(
            "{} already has folded sessions; earlier ones would be learned after them, so its history is read only before the first session is folded",
            ctx.repo.label
        ))
    } else {
        None
    })
}
pub fn identity(
    cwd: &str,
    env: &Map<String, Value>,
    home: &Path,
    cache: &mut BTreeMap<String, Option<String>>,
) -> Option<String> {
    cache
        .entry(cwd.into())
        .or_insert_with(|| {
            paths::repo_identity(Path::new(cwd), env, home)
                .ok()
                .map(|repo| repo.key)
        })
        .clone()
}
pub fn is_this_repository(
    ctx: &Context,
    folder: &str,
    cwd: &Value,
    cache: &mut BTreeMap<String, Option<String>>,
    runtime: &mut dyn Runtime,
) -> Result<bool> {
    if let Some(cwd) = cwd
        .as_str()
        .filter(|cwd| !cwd.is_empty() && Path::new(cwd).is_dir())
    {
        return Ok(identity(cwd, &ctx.env, &ctx.home, cache).as_deref() == Some(&ctx.repo.key));
    }
    Ok(truthy(
        &runtime.host("folder_matches", json!([ctx.repo.root, folder]))?,
    ))
}
pub fn headless(head: &Value, prompts: f64) -> bool {
    prompts <= 1.0
        && head["entrypoint"]
            .as_str()
            .unwrap_or("")
            .to_lowercase()
            .starts_with("sdk")
}
pub fn plan(ctx: &Context, at: f64, limits: &Limits, runtime: &mut dyn Runtime) -> Result<Value> {
    let folded = ctx.store.read("folded.json", json!({}))?;
    let mut skipped = Map::new();
    let mut found = Vec::new();
    let mut cache = BTreeMap::new();
    let logs = runtime.host("history", json!([ctx.config["history_dirs"]]))?;
    for log in logs
        .as_array()
        .ok_or_else(|| Error::Failed("history logs must be a list".into()))?
    {
        let path = log["path"]
            .as_str()
            .ok_or_else(|| Error::Failed("history path must be a string".into()))?;
        let head = runtime.host("log_head", json!([path]))?;
        if !is_this_repository(
            ctx,
            log["folder"].as_str().unwrap_or(""),
            &head["cwd"],
            &mut cache,
            runtime,
        )? {
            continue;
        }
        let sid = runtime.host("safe_session_id", json!([log["session_id"]]))?;
        let mtime = n(&log["mtime"]);
        let why = if sid.is_null() {
            Some("unsafe_id")
        } else if mtime > at - limits.stale {
            Some("recent")
        } else if mtime < at - n(&ctx.config["history_days"]) * 86400.0 {
            Some("too_old")
        } else if n(&log["size"]) > limits.max_bytes as f64 {
            Some("too_large")
        } else if folded.get(sid.as_str().unwrap_or("")).is_some() {
            Some("already_folded")
        } else if ctx
            .store
            .base
            .join("sessions")
            .join(format!("{}.json", sid.as_str().unwrap_or("")))
            .exists()
        {
            Some("recorded_live")
        } else {
            None
        };
        if let Some(why) = why {
            skip(&mut skipped, why);
            continue;
        }
        let prompts = n(&runtime.host("prompt_count", json!([path]))?);
        if prompts <= 0.0 {
            skip(&mut skipped, "no_prompt");
        } else if headless(&head, prompts) {
            skip(&mut skipped, "headless");
        } else {
            found.push(json!({"id":sid,"path":path,"started":if truthy(&head["started"]) {head["started"].clone()} else {log["mtime"].clone()},"ended":log["mtime"]}));
        }
    }
    found.sort_by(|a, b| {
        n(&a["started"])
            .total_cmp(&n(&b["started"]))
            .then(n(&a["ended"]).total_cmp(&n(&b["ended"])))
            .then(a["id"].as_str().cmp(&b["id"].as_str()))
    });
    let over = (found.len() as i64 - integer(&ctx.config["history_max_sessions"])).max(0) as usize;
    Ok(
        json!({"sessions":found.into_iter().skip(over).collect::<Vec<_>>(),"skipped":skipped,"over_limit":over}),
    )
}
pub fn still_eligible(
    ctx: &Context,
    item: &Value,
    limits: &Limits,
    runtime: &mut dyn Runtime,
) -> Result<Option<String>> {
    let sid = if item.is_object() {
        runtime.host("safe_session_id", json!([item["id"]]))?
    } else {
        Value::Null
    };
    let path = item["path"].as_str();
    let Some((sid, path)) = sid.as_str().zip(path).filter(|(sid, path)| {
        !path.ends_with(std::path::MAIN_SEPARATOR)
            && Path::new(path)
                .file_name()
                .is_some_and(|name| name == format!("{sid}.jsonl").as_str())
    }) else {
        return Ok(Some("unsafe_id".into()));
    };
    if ctx.store.read("folded.json", json!({}))?.get(sid).is_some() {
        return Ok(Some("already_folded".into()));
    }
    if ctx
        .store
        .base
        .join("sessions")
        .join(format!("{sid}.json"))
        .exists()
    {
        return Ok(Some("recorded_live".into()));
    }
    Ok(match fs::metadata(path) {
        Ok(meta) if meta.len() > limits.max_bytes => Some("too_large".into()),
        Ok(_) => None,
        Err(_) => Some("gone".into()),
    })
}
pub fn replay_envelope(ctx: &Context, path: &str, runtime: &mut dyn Runtime) -> Result<i64> {
    if ctx.config["loopguard"] == "off" {
        return Ok(0);
    }
    let level = ctx.config["loopguard_aggr"].as_str().unwrap_or("default");
    let overrides = &ctx.config["loopguard_thresholds"];
    let thresholds = envelope::base_thresholds(level, overrides);
    let turns = runtime.host("prompt_turns", json!([path]))?;
    let turns = turns
        .as_array()
        .ok_or_else(|| Error::Failed("history prompt turns must be a list".into()))?;
    let mut recorded = 0;
    for (index, turn) in turns.iter().enumerate() {
        let mut guard = TurnGuard::new("", &json!({}))?;
        let calls = runtime.host("turn_calls", json!([turn]))?;
        for call in calls
            .as_array()
            .ok_or_else(|| Error::Failed("history calls must be a list".into()))?
        {
            let name = call[0].as_str().unwrap_or("");
            let args = if call[1].is_object() {
                call[1].clone()
            } else {
                json!({})
            };
            let result = call[2]
                .as_str()
                .unwrap_or("")
                .chars()
                .take(RESULT_CAP)
                .collect::<String>();
            let code = if truthy(&call[3]) {
                hosts::claude_success_code(name, &args, None)?
            } else {
                EXIT_RE
                    .captures(&result.chars().take(400).collect::<String>())
                    .and_then(|m| m[1].parse::<i64>().ok())
            };
            guard.fold(
                name,
                &args,
                &result,
                code,
                FoldOptions {
                    envelope: None,
                    thresholds: Some(&thresholds),
                    judge: None,
                    nudge_budget: 1,
                },
            )?;
        }
        let normal = !truthy(&runtime.host("turn_interrupted", json!([turn]))?)
            && (index + 1 < turns.len() || truthy(&runtime.host("turn_answered", json!([turn]))?));
        if guard.cum_turns != 0 && normal {
            envelope::record_turn(&ctx.store, &guard, level, true, overrides)?;
            recorded += 1;
        }
    }
    Ok(recorded)
}

pub fn update(
    ctx: &Context,
    timeout: f64,
    change: impl FnOnce(&mut Value) -> Result<bool>,
) -> Result<Option<Value>> {
    let lock = ctx.store.locked("history", timeout)?;
    let result = (|| {
        if !lock.acquired {
            return Ok(None);
        }
        let mut doc = read_state(&ctx.store)?.unwrap_or(json!({}));
        if change(&mut doc)? {
            ctx.store.write(STATE, &doc)?;
        }
        Ok(Some(doc))
    })();
    lock.release()?;
    result
}
pub fn save(ctx: &Context, doc: &Value) -> Result<()> {
    let lock = ctx.store.locked("history", 10.0)?;
    let result = ctx.store.write(STATE, doc).map(|_| ()).map_err(Error::from);
    lock.release()?;
    result
}
pub fn say(runtime: &mut dyn Runtime, text: &str) -> Result<()> {
    match runtime.say(text) {
        Err(Error::Cancelled) => Err(Error::Cancelled),
        _ => Ok(()),
    }
}
fn report(ctx: &Context, error: Error) -> Result<()> {
    if matches!(error, Error::Cancelled) {
        return Err(error);
    }
    record_error(&ctx.root, &error.to_string());
    Ok(())
}
pub fn run(
    ctx: &Context,
    auto: bool,
    at: f64,
    budget: f64,
    limits: &Limits,
    runtime: &mut dyn Runtime,
) -> Result<Value> {
    if !ctx.keeps_history {
        return Ok(
            json!({"state":"refused","reason":format!("{} keeps no session history the plugin reads",ctx.host)}),
        );
    }
    let lock = ctx.store.locked("fold", limits.fold_lock)?;
    let result = (|| {
        if !lock.acquired {
            return Ok(
                json!({"state":"busy","reason":"another fold holds this repository's lock"}),
            );
        }
        let mut doc = read_state(&ctx.store)?.unwrap_or(json!({}));
        let state = doc["state"].as_str().unwrap_or("").to_string();
        let blocking = HISTORY_BLOCKING.contains(&state.as_str());
        let resuming = (blocking || (!auto && state == "stopped")) && doc["plan"].is_array();
        if auto && !blocking {
            return Ok(
                json!({"state":if truthy(&doc["state"]) {doc["state"].clone()} else {json!("none")},"reason":"nothing to resume"}),
            );
        }
        if auto && !truthy(&ctx.config["history_import"]) {
            doc["state"] = json!("stopped");
            doc["reason"] = json!("history_import is off");
            doc["at"] = json!(now());
            save(ctx, &doc)?;
            return Ok(json!({"state":"stopped","reason":doc["reason"]}));
        }
        if let Some(why) = refusal(ctx, resuming)? {
            if doc["state"].is_null() || blocking {
                remove(&mut doc, "plan")?;
                doc["state"] = json!("skipped");
                doc["reason"] = json!(why);
                doc["at"] = json!(now());
                save(ctx, &doc)?;
            }
            return Ok(json!({"state":if auto {"skipped"}else{"refused"},"reason":why}));
        }
        match run_locked(ctx, &mut doc, resuming, at, budget, limits, runtime) {
            Ok(value) => Ok(value),
            Err(error) => {
                report(ctx, error)?;
                let mut doc = read_state(&ctx.store)?.unwrap_or(json!({}));
                doc["state"] = json!("stopped");
                doc["reason"] = json!("the import failed (see errors.log)");
                doc["at"] = json!(now());
                save(ctx, &doc)?;
                Ok(json!({"state":"error","reason":doc["reason"]}))
            }
        }
    })();
    lock.release()?;
    result
}
pub fn run_locked(
    ctx: &Context,
    doc: &mut Value,
    resuming: bool,
    at: f64,
    budget: f64,
    limits: &Limits,
    runtime: &mut dyn Runtime,
) -> Result<Value> {
    if !resuming {
        let mut initial = object(doc)?.clone();
        initial.insert("state".into(), json!("running"));
        initial.insert("heartbeat".into(), json!(now()));
        initial.remove("plan");
        save(ctx, &json!(initial))?;
        let p = runtime.plan(ctx, at, limits)?;
        let sessions = p["sessions"]
            .as_array()
            .ok_or_else(|| Error::Failed("history plan sessions must be a list".into()))?;
        *doc = json!({"state":"running","plan":sessions,"next":0,"imported":0,"skipped":p["skipped"],"over_limit":p["over_limit"],"planned":sessions.len(),"started":now(),"noticed":false});
        say(
            runtime,
            &format!(
                "{} earlier session(s) of {} to read",
                sessions.len(),
                ctx.repo.label
            ),
        )?;
    }
    doc["state"] = json!("running");
    doc["pid"] = json!(std::process::id());
    doc["heartbeat"] = json!(now());
    doc["runs"] = json!(integer(&doc["runs"]) + 1);
    save(ctx, doc)?;
    let mut skipped = doc["skipped"].as_object().cloned().unwrap_or_default();
    doc["skipped"] = json!(skipped);
    let items = doc["plan"]
        .as_array()
        .cloned()
        .ok_or_else(|| Error::Failed("history plan must be a list".into()))?;
    let deadline = runtime.clock()? + budget.max(0.0);
    let mut i = integer(&doc["next"]);
    while i < (items.len() as i64) {
        if runtime.clock()? >= deadline {
            break;
        }
        let index = if i < 0 { items.len() as i64 + i } else { i };
        let item = usize::try_from(index)
            .ok()
            .and_then(|index| items.get(index))
            .ok_or_else(|| Error::Failed("history plan index out of range".into()))?;
        import_one(ctx, doc, item, &mut skipped, limits, runtime)?;
        i += 1;
        remove(doc, "attempt")?;
        doc["next"] = json!(i);
        doc["heartbeat"] = json!(now());
        doc["skipped"] = json!(skipped);
        save(ctx, doc)?;
    }
    if i < (items.len() as i64) {
        doc["state"] = json!("partial");
    } else {
        doc["state"] = json!("done");
        doc["finished"] = json!(now());
        remove(doc, "plan")?;
    }
    save(ctx, doc)?;
    if doc["state"] == "done" {
        for sid in runtime.pending()? {
            if let Err(error) = runtime.fold(&sid, None, false, None) {
                report(ctx, error)?;
            }
        }
    }
    Ok(
        json!({"state":doc["state"],"imported":doc.get("imported").cloned().unwrap_or(json!(0)),"skipped":skipped,"over_limit":doc.get("over_limit").cloned().unwrap_or(json!(0)),"left":items.len() as i64-i,"reason":null}),
    )
}
pub fn import_one(
    ctx: &Context,
    doc: &mut Value,
    item: &Value,
    skipped: &mut Map<String, Value>,
    limits: &Limits,
    runtime: &mut dyn Runtime,
) -> Result<()> {
    let sid = item["id"].as_str();
    let tries = if sid.is_some() && doc["attempt"]["id"] == item["id"] {
        integer(&doc["attempt"]["n"])
    } else {
        0
    };
    let why = still_eligible(ctx, item, limits, runtime)?;
    if why.as_deref() == Some("already_folded") && folded_by_import(&ctx.store, sid.unwrap_or(""))?
    {
        doc["imported"] = json!(integer(&doc["imported"]) + 1);
        if tries < limits.max_attempts {
            doc["attempt"] = json!({"id":sid,"n":tries+1});
            save(ctx, doc)?;
            if let Err(error) = replay_envelope(ctx, item["path"].as_str().unwrap_or(""), runtime) {
                report(ctx, error)?;
            }
        }
        return Ok(());
    }
    if let Some(why) = why {
        skip(skipped, &why);
        return Ok(());
    }
    if tries >= limits.max_attempts {
        skip(skipped, "error");
        return Ok(());
    }
    let sid = sid.unwrap_or("");
    doc["attempt"] = json!({"id":sid,"n":tries+1});
    save(ctx, doc)?;
    let result = runtime.fold(
        sid,
        item["path"].as_str(),
        true,
        Some((&item["started"], &item["ended"])),
    );
    match result {
        Ok(rec) if !rec.is_null() && !truthy(&rec["error"]) => {
            doc["imported"] = json!(integer(&doc["imported"]) + 1);
            if let Err(error) = replay_envelope(ctx, item["path"].as_str().unwrap_or(""), runtime) {
                report(ctx, error)?;
            }
            say(runtime, &format!("read {sid}"))?;
        }
        Ok(rec) if !rec.is_null() => skip(skipped, "unreadable"),
        Ok(_) => (),
        Err(error) => {
            report(ctx, error)?;
            skip(skipped, "error");
        }
    }
    doc["skipped"] = json!(skipped);
    Ok(())
}
pub fn maybe_start(
    ctx: &Context,
    at: f64,
    limits: &Limits,
    runtime: &mut dyn Runtime,
) -> Result<Value> {
    if !ctx.keeps_history {
        return Ok(Value::Null);
    }
    let exists = ctx.store.base.join(STATE).exists();
    if exists && read_state(&ctx.store)?.is_none() {
        return Ok(Value::Null);
    }
    if !exists && !truthy(&ctx.config["history_import"]) {
        return Ok(Value::Null);
    }
    let mut start = false;
    let updated = update(ctx, limits.history_lock, |doc| {
        if !truthy(doc) {
            if !truthy(&ctx.config["history_import"]) {
                return Ok(false);
            }
            if !nothing_folded(&ctx.store)? {
                doc["state"] = json!("skipped");
                doc["at"] = json!(at);
                doc["reason"] = json!(format!("{} already had folded sessions", ctx.repo.label));
                return Ok(true);
            }
            doc["state"] = json!("running");
            doc["started"] = json!(at);
            doc["heartbeat"] = json!(at);
            doc["noticed"] = json!(false);
        } else if doc["state"] == "partial"
            || (doc["state"] == "running" && stale(doc, at, limits.heartbeat_stale))
        {
            if !truthy(&ctx.config["history_import"]) {
                doc["state"] = json!("stopped");
                doc["reason"] = json!("history_import is off");
                doc["at"] = json!(at);
                return Ok(true);
            }
            doc["heartbeat"] = json!(at);
        } else {
            return Ok(false);
        }
        start = true;
        Ok(true)
    })?;
    if updated.is_none() || !start {
        return Ok(Value::Null);
    }
    if ctx.env.get("TE_FOLD_SYNC") == Some(&json!("1")) {
        run(ctx, true, now(), BUDGET_S, limits, runtime)
    } else {
        runtime.spawn(&["history", "import", "--auto"], LOG)
    }
}
pub fn notice(ctx: &Context, limits: &Limits) -> Result<Value> {
    let Some(doc) = read_state(&ctx.store)? else {
        return Ok(Value::Null);
    };
    if doc["state"] != "done" || truthy(&doc["noticed"]) {
        return Ok(Value::Null);
    }
    let mut count = 0;
    let updated = update(ctx, limits.history_lock, |doc| {
        if doc["state"] != "done" || truthy(&doc["noticed"]) {
            return Ok(false);
        }
        doc["noticed"] = json!(true);
        count = integer(&doc["imported"]);
        Ok(true)
    })?;
    if updated.is_none() || count == 0 {
        return Ok(Value::Null);
    }
    Ok(json!(format!(
        "TokenEconomizer read {count} earlier session(s) of {} from {}'s history.",
        ctx.repo.label,
        label(&ctx.host)
    )))
}
pub fn summary(ctx: &Context) -> Result<Value> {
    let Some(mut doc) = read_state(&ctx.store)? else {
        return Ok(Value::Null);
    };
    let plan = object_mut(&mut doc)?.remove("plan");
    if let Some(Value::Array(plan)) = plan {
        doc["left"] = json!((plan.len() as i64 - integer(&doc["next"])).max(0));
    }
    Ok(doc)
}
pub fn describe(doc: &Value, host: &str) -> Value {
    if !truthy(doc) {
        return Value::Null;
    }
    let count = integer(&doc["imported"]);
    let who = label(host);
    let reason = |fallback: &str| {
        if truthy(&doc["reason"]) {
            scalar_text(&doc["reason"])
        } else {
            fallback.into()
        }
    };
    match doc["state"].as_str().unwrap_or("") {
        "done" => json!(format!(
            "read {count} earlier session(s) from {who}'s history"
        )),
        "running" | "partial" => json!(format!(
            "reading {who}'s history: {count} session(s) so far, {} to go; sessions ending meanwhile are folded after it",
            doc.get("left").map(scalar_text).unwrap_or("?".into())
        )),
        "skipped" => json!(format!("not read ({})", reason("skipped"))),
        "stopped" => json!(format!(
            "stopped after {count} session(s) ({})",
            reason("stopped")
        )),
        _ => Value::Null,
    }
}
