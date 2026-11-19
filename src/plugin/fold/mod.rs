mod tooling;
pub use tooling::*;

use super::{
    adapter::{self, LiveAdapter},
    context::LiveContext,
    denylist, history,
};
use crate::{
    accounting,
    config::InjectionConfig,
    microskills::{
        distiller::ProposeContext,
        stage::{self, PrefixStage, SavedFactError},
    },
    runner::Runner,
    value::{float_or_zero, int_value, truthy},
};
use serde_json::{Value, json};
use std::{
    fmt, fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

pub const STALE_S: f64 = 600.0;
pub const FOLD_LOCK_WAIT_S: f64 = 240.0;
pub const PENDING_MAX_S: f64 = 86400.0;
pub const WAITER_POLL_S: f64 = 600.0;
pub const CODEX_HIGH_RISK: &[&str] = &["image_media"];
pub const HISTORY: &str = "history.json";
pub const HISTORY_BLOCKING: [&str; 2] = ["running", "partial"];

#[derive(Debug)]
pub enum Error {
    Failed(String),
    Value(String),
    Io(io::Error),
    Busy,
    UnknownIds(Vec<String>),
    Later,
    Cancelled,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Failed(s) | Self::Value(s) => s.fmt(f),
            Self::Io(e) => e.fmt(f),
            Self::Busy => f.write_str("a fold of this repository's sessions holds its state"),
            Self::UnknownIds(ids) => write!(f, "{}", ids.join(", ")),
            Self::Later => f.write_str("session is still active"),
            Self::Cancelled => f.write_str("fold cancelled"),
        }
    }
}
impl std::error::Error for Error {}
impl From<String> for Error {
    fn from(e: String) -> Self {
        Self::Failed(e)
    }
}
impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<denylist::Error> for Error {
    fn from(e: denylist::Error) -> Self {
        match e {
            denylist::Error::Io(e) => Self::Io(e),
            denylist::Error::Refused(e) => Self::Failed(e),
        }
    }
}
impl From<SavedFactError> for Error {
    fn from(e: SavedFactError) -> Self {
        match e {
            SavedFactError::UnknownIds(ids) => Self::UnknownIds(ids),
            SavedFactError::State(e) => Self::Failed(e),
        }
    }
}
pub type Result<T> = std::result::Result<T, Error>;

pub trait Runtime {
    fn host(&mut self, method: &str, args: Value) -> Result<Value>;
    fn proposer(&mut self, ctx: &LiveContext) -> Result<Option<Box<dyn Runner>>>;
    fn spawn(&mut self, argv: &[String], log: &str) -> Result<Value>;
    fn fold(&mut self, ctx: &LiveContext, sid: &str, path: Option<&str>) -> Result<Value>;
    fn blocks(&mut self, ctx: &LiveContext) -> Result<bool> {
        import_blocks(ctx)
    }
    fn apply(&mut self, ctx: &LiveContext, root: &Path, items: &[String]) -> Result<Vec<String>> {
        Ok(denylist::apply(
            root,
            items,
            &ctx.store,
            &ctx.host,
            &mut denylist::NativeRuntime,
        )?)
    }
    fn restore(&mut self, ctx: &LiveContext, items: &[String]) -> Result<Vec<String>> {
        Ok(denylist::restore(
            &ctx.repo.root,
            &ctx.store,
            items,
            &ctx.host,
            &mut denylist::NativeRuntime,
        )?)
    }
    fn git_tracked(&mut self, root: &Path, relative: &str) -> Result<bool> {
        Ok(denylist::git_tracked(root, relative))
    }
}

fn number(value: &Value) -> Result<i64> {
    if let Some(n) = value.as_i64() {
        return Ok(n);
    }
    // shortcut: counters use i64, extend their range during the required numeric parity audit.
    int_value(value)
        .filter(|n| *n >= i64::MIN as f64 && *n < -(i64::MIN as f64))
        .map(|n| n as i64)
        .ok_or_else(|| Error::Value(format!("invalid integer: {value}")))
}
fn count(doc: &Value, key: &str) -> Result<i64> {
    doc.get(key).map(number).unwrap_or(Ok(0))
}
fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(String::from)
        .collect()
}
fn read(ctx: &LiveContext, name: &str) -> Result<Value> {
    let doc = ctx.store.read(name, Value::Null)?;
    if truthy(&doc) && !doc.is_object() {
        return Err(Error::Value(format!("expected an object in {name}")));
    }
    Ok(if truthy(&doc) { doc } else { json!({}) })
}
fn adapter(ctx: &LiveContext, catalog: serde_json::Map<String, Value>) -> LiveAdapter {
    LiveAdapter::new(
        &ctx.host,
        &ctx.repo.root,
        ctx.config["vocabulary"].as_str().unwrap_or("auto"),
        catalog,
        strings(&ctx.config["toolprune_protect"]),
    )
}
pub fn microskills_config(ctx: &LiveContext, profile: &str) -> Result<InjectionConfig> {
    Ok(InjectionConfig {
        enabled: true,
        profile: profile.into(),
        propose_every: count(&ctx.config, "microskills_propose_every")?,
        max_facts: count(&ctx.config, "microskills_max_facts")?,
        max_tokens: count(&ctx.config, "microskills_max_tokens")?,
    })
}
pub fn run_dir(workdir: &Path, transcript: &Path) -> Result<PathBuf> {
    let run = workdir.join("run");
    fs::create_dir_all(&run)?;
    let link = run.join("transcript.jsonl");
    #[cfg(unix)]
    let linked = std::os::unix::fs::symlink(transcript, &link);
    #[cfg(windows)]
    let linked = std::os::windows::fs::symlink_file(transcript, &link);
    if linked.is_err() {
        fs::copy(transcript, &link)?;
    }
    Ok(run)
}
pub fn fold_microskills(
    ctx: &LiveContext,
    adapter: &LiveAdapter,
    run: &Path,
    sid: &str,
    injected: Option<&Value>,
    imported: bool,
    runtime: &mut dyn Runtime,
) -> Result<Value> {
    let profile = ctx.config["microskills"].as_str().unwrap_or("off");
    if profile == "off" {
        return Ok(Value::Null);
    }
    let cfg = microskills_config(ctx, profile)?;
    let mut runner = if cfg.paid() && !imported {
        runtime.proposer(ctx)?
    } else {
        None
    };
    let mut context = ProposeContext {
        runner: runner.as_mut().map(|r| &mut **r as &mut dyn Runner),
        home: ctx.home.to_str().unwrap_or(""),
        ..Default::default()
    };
    let mut stage = PrefixStage::new(
        &cfg,
        &ctx.repo.label,
        &ctx.mech_dir()?,
        false,
        "agent",
        adapter.env_vocabulary(),
        &mut context,
    )?;
    let mut srow = json!({"resolved":null});
    let mut task = json!({"instance_id":sid});
    if let Some(ids) = injected.filter(|ids| !ids.is_null()) {
        task["injected_ids"] = ids.clone();
    }
    let (tokens, reason) = stage.after_task(&task, run, &mut srow, &mut context)?;
    let rendered = stage.render();
    let prior = read(ctx, "injection.json")?;
    ctx.store.write(
        "injection.json",
        &json!({
            "profile": profile,
            "reason": reason,
            "rendered": rendered,
            "rendered_ids": stage.injected_ids(),
            "facts": srow["manual_facts"],
            "updated": history::now(),
            "sessions": count(&prior,"sessions")? + 1,
            "imported_sessions": count(&prior,"imported_sessions")? + i64::from(imported)
        }),
    )?;
    Ok(json!({"optimizer_tokens":tokens,"rendered_chars":rendered.chars().count()}))
}
pub fn forget_microskills(ctx: &LiveContext, ids: &[String], undo: bool) -> Result<Value> {
    let adapter = adapter(ctx, Default::default());
    let vocab = adapter.env_vocabulary();
    let lock = ctx.store.locked("mechanisms", 10.0)?;
    if !lock.acquired {
        return Err(Error::Busy);
    }
    let out = stage::forget_facts(&ctx.repo.label, &ctx.mech_dir()?, ids, &vocab, undo)?;
    let prior = ctx.store.read("injection.json", Value::Null)?;
    let profile = if ctx.config["microskills"] != "off" {
        ctx.config["microskills"].as_str()
    } else {
        prior["profile"].as_str()
    };
    if !out.iter().any(|(_, changed, _)| truthy(changed))
        || prior.is_null()
        || profile.is_none_or(str::is_empty)
    {
        return Ok(json!(out));
    }
    let mut stage = PrefixStage::new(
        &microskills_config(ctx, profile.unwrap())?,
        &ctx.repo.label,
        &ctx.mech_dir()?,
        false,
        "agent",
        vocab,
        &mut ProposeContext::default(),
    )?;
    let rendered = stage.render();
    let mut facts = stage.distiller.debug_row();
    for (k, v) in prior["facts"].as_object().into_iter().flatten() {
        if k.starts_with('_') && facts.get(k).is_none() {
            facts[k] = v.clone();
        }
    }
    let mut doc = prior;
    doc["rendered"] = json!(rendered);
    doc["rendered_ids"] = json!(stage.injected_ids());
    doc["facts"] = facts;
    doc["updated"] = json!(history::now());
    ctx.store.write("injection.json", &doc)?;
    Ok(json!(out))
}
pub fn unprotected(ctx: &LiveContext, items: &[String]) -> Vec<String> {
    if ctx.host == "copilot" {
        return items.to_vec();
    }
    let protected = adapter(ctx, Default::default()).protected_tools();
    items
        .iter()
        .filter(|t| !protected.contains(t))
        .cloned()
        .collect()
}
pub fn hold(doc: &mut Value, prior: &Value, proposal: &[String], root: &Path) {
    doc["auto_pending"] = json!({"items":proposal,"root":root,"since":prior["auto_pending"].get("since").filter(|s| truthy(s)).cloned().unwrap_or_else(||json!(history::now()))});
    if truthy(&prior["auto_apply_refused"]) {
        doc["auto_apply_refused"] = prior["auto_apply_refused"].clone();
    }
}
fn modified(path: &Path) -> io::Result<f64> {
    Ok(fs::metadata(path)?
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or_else(|e| -e.duration().as_secs_f64()))
}
pub fn last_activity(ctx: &LiveContext) -> Result<f64> {
    let Ok(entries) = fs::read_dir(ctx.store.base.join("sessions")) else {
        return Ok(0.0);
    };
    let mut latest = 0.0_f64;
    for entry in entries {
        let entry = entry?;
        if !entry.file_name().to_string_lossy().ends_with(".json") {
            continue;
        }
        let Ok(at) = modified(&entry.path()) else {
            continue;
        };
        latest = latest.max(at);
        let doc = ctx.store.read(
            &format!("sessions/{}", entry.file_name().to_string_lossy()),
            Value::Null,
        )?;
        if let Some(tp) = doc["transcript"].as_str().filter(|s| !s.is_empty())
            && let Ok(at) = modified(Path::new(tp))
        {
            latest = latest.max(at);
        }
    }
    Ok(latest)
}
pub fn apply_pending(ctx: &LiveContext, at: f64, runtime: &mut dyn Runtime) -> Result<Option<f64>> {
    let _lock = ctx.store.locked("mechanisms", 10.0)?;
    let mut unt = read(ctx, "untool.json")?;
    let pend = &unt["auto_pending"];
    if !truthy(pend) {
        return Ok(None);
    }
    let root = pend["root"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| ctx.repo.root.clone());
    let mut done = denylist::applied_for(&ctx.store, &root, &ctx.host)?;
    done.extend(denylist::kept_for(&ctx.store, &root, &ctx.host)?);
    let items = unprotected(
        ctx,
        &strings(&pend["items"])
            .into_iter()
            .filter(|t| !done.contains(t))
            .collect::<Vec<_>>(),
    );
    if !items.is_empty() && ctx.config["toolprune"] == "auto" && !truthy(&ctx.config["shadow"]) {
        let idle = at - last_activity(ctx)?;
        let quiet = ctx.config["toolprune_quiet_minutes"]
            .as_f64()
            .unwrap_or(0.0)
            * 60.0;
        let since = pend
            .get("since")
            .filter(|v| truthy(v))
            .map(float_or_zero)
            .transpose()
            .map_err(Error::Value)?
            .unwrap_or(at);
        let waited = at - since;
        if idle < quiet && waited < PENDING_MAX_S {
            return Ok(Some((quiet - idle).min(PENDING_MAX_S - waited)));
        }
        match runtime.apply(ctx, &root, &items) {
            Ok(applied) => {
                unt["auto_applied"] = json!(applied);
                unt["proposal"] = json!(
                    strings(&unt["proposal"])
                        .into_iter()
                        .filter(|t| !items.contains(t))
                        .collect::<Vec<_>>()
                );
                unt.as_object_mut().unwrap().remove("auto_apply_refused");
            }
            Err(Error::Failed(e)) => unt["auto_apply_refused"] = json!(e),
            Err(e) => return Err(e),
        }
    }
    if unt.get("proposal").is_some() {
        unt["proposal"] = json!(unprotected(ctx, &strings(&unt["proposal"])));
    }
    unt.as_object_mut().unwrap().remove("auto_pending");
    ctx.store.write("untool.json", &unt)?;
    Ok(None)
}
pub fn schedule_pending(ctx: &LiveContext, runtime: &mut dyn Runtime) -> Result<Option<f64>> {
    let wait = apply_pending(ctx, history::now(), runtime)?;
    if wait.is_some() && ctx.env.get("TE_FOLD_SYNC").is_none_or(|v| v != "1") {
        runtime.spawn(&["toolprune".into(), "apply-pending".into()], "fold.log")?;
    }
    Ok(wait)
}
pub fn import_blocks(ctx: &LiveContext) -> Result<bool> {
    let doc = ctx.store.read(HISTORY, Value::Null)?;
    Ok(doc["state"]
        .as_str()
        .is_some_and(|s| HISTORY_BLOCKING.contains(&s)))
}
pub fn fold_session(
    ctx: &LiveContext,
    sid: &str,
    transcript: Option<&str>,
    wait: f64,
    runtime: &mut dyn Runtime,
) -> Result<Value> {
    if runtime.blocks(ctx)? {
        return Ok(Value::Null);
    }
    let lock = ctx.store.locked("fold", wait)?;
    if !lock.acquired || runtime.blocks(ctx)? {
        return Ok(Value::Null);
    }
    runtime.fold(ctx, sid, transcript)
}
struct WorkDir(PathBuf);
impl WorkDir {
    fn new() -> io::Result<Self> {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "te-fold-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e),
            }
        }
    }
}
impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = super::temporary::remove_tree(&self.0);
    }
}

pub fn fold_locked(
    ctx: &LiveContext,
    sid: &str,
    transcript_path: Option<&str>,
    imported: bool,
    span: Option<(&Value, &Value)>,
    runtime: &mut dyn Runtime,
) -> Result<Value> {
    let name = format!("sessions/{sid}.json");
    let mut folded = read(ctx, "folded.json")?;
    if folded.get(sid).is_some() {
        return Ok(Value::Null);
    }
    let mut sess = if imported {
        json!({})
    } else {
        read(ctx, &name)?
    };
    let transcript = if imported {
        transcript_path.map(|s| json!(s)).unwrap_or(Value::Null)
    } else {
        let path = transcript_path
            .filter(|s| !s.is_empty())
            .map(|s| json!(s))
            .unwrap_or_else(|| sess["transcript"].clone());
        let cwd = sess
            .get("cwd")
            .filter(|v| truthy(v))
            .cloned()
            .unwrap_or_else(|| json!(ctx.repo.root));
        match runtime.host("complete", json!([path, sid, cwd, sess])) {
            Err(Error::Later) => return Ok(Value::Null),
            result => result?,
        }
    };
    let (started, ended) = span.unwrap_or((&sess["started"], &sess["ended"]));
    let mut record = json!({"kind":"session","session_id":sid,"host":ctx.host,"repo":ctx.repo.label,"folded_at":history::now(),"started":started,"ended":ended,"shadow":ctx.config["shadow"]});
    if imported {
        record["imported"] = json!(true);
    }
    let work = WorkDir::new()?;
    let trace = if truthy(&transcript) {
        runtime.host("mechanism_trace", json!([transcript, work.0]))?
    } else {
        Value::Null
    };
    let prompts = if truthy(&transcript) {
        match runtime.host("prompt_count", json!([transcript])) {
            Ok(n) => n,
            Err(Error::Io(_) | Error::Value(_)) => Value::Null,
            Err(e) => return Err(e),
        }
    } else {
        Value::Null
    };
    record["prompts"] = prompts.clone();
    if !truthy(&trace) {
        record["error"] = json!("transcript unreadable");
    } else {
        let run = run_dir(
            &work.0,
            Path::new(
                trace
                    .as_str()
                    .ok_or_else(|| Error::Failed("expected transcript path".into()))?,
            ),
        )?;
        let catalog = if imported {
            Default::default()
        } else {
            adapter::load_catalog(&ctx.root, &ctx.host)?
        };
        let mut adapter = adapter(ctx, catalog);
        let (injection, untool) = {
            let _lock = ctx.store.locked("mechanisms", 10.0)?;
            let injection = fold_microskills(
                ctx,
                &adapter,
                &run,
                sid,
                sess.get("injected_ids"),
                imported,
                runtime,
            )?;
            let untool = if prompts.as_i64() == Some(0) {
                Value::Null
            } else {
                fold_toolprune(
                    ctx,
                    &mut adapter,
                    &run,
                    sid,
                    transcript.as_str(),
                    Some(&work.0),
                    prompts.as_i64(),
                    imported,
                    runtime,
                )?
            };
            (injection, untool)
        };
        record["injection"] = injection;
        record["untool"] = untool.clone();
        if truthy(&untool["pending"]) && !imported {
            schedule_pending(ctx, runtime)?;
        }
        if !imported {
            runtime.host("settle", json!([transcript]))?;
        }
        let usage = runtime.host("session_usage", json!([transcript]))?;
        let (effective, priced_by) =
            accounting::effective(&usage).map_err(|e| Error::Failed(e.to_string()))?;
        for key in [
            "model",
            "requests",
            "input_tokens",
            "cache_read_input_tokens",
            "cache_creation_input_tokens",
            "output_tokens",
        ] {
            record[key] = usage[key].clone();
        }
        if truthy(&usage["cache_creation_1h_input_tokens"]) {
            record["cache_creation_1h_input_tokens"] =
                usage["cache_creation_1h_input_tokens"].clone();
        }
        record["effective_tokens"] = json!(effective);
        record["priced_by"] = json!(priced_by);
    }
    record["turns"] = json!(sess["turns"].as_array().map_or(0, Vec::len));
    for key in ["nudges", "aborts", "injected_chars"] {
        record[key] = json!(count(&sess, key)?);
    }
    record["untool_applied"] = json!(if imported {
        Vec::new()
    } else {
        denylist::applied_for(&ctx.store, &ctx.repo.root, &ctx.host)?
    });
    record["overhead"] = json!({
        "optimizer_tokens": record["injection"].get("optimizer_tokens").filter(|v| truthy(v)).cloned().unwrap_or(json!(0)),
        "spiral_judge_tokens": float_or_zero(&sess["judge_tokens"]).map_err(Error::Value)?,
        "spiral_nudge_writer_tokens": float_or_zero(&sess["writer_tokens"]).map_err(Error::Value)?
    });
    ctx.store.append("ledger.jsonl", &record)?;
    let mut entry = json!({"at":history::now(),"transcript":transcript});
    if imported {
        entry["imported"] = json!(true);
    }
    folded[sid] = entry;
    ctx.store.write("folded.json", &folded)?;
    if truthy(&sess) {
        sess["folded"] = json!(true);
        ctx.store.write(&name, &sess)?;
    }
    Ok(record)
}
pub fn pending_sessions(
    ctx: &LiveContext,
    exclude: &[String],
    stale: f64,
    at: f64,
    runtime: &mut dyn Runtime,
) -> Result<Vec<String>> {
    let folded = read(ctx, "folded.json")?;
    let dir = ctx.store.base.join("sessions");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut names = fs::read_dir(dir)?
        .map(|e| e.map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect::<io::Result<Vec<_>>>()?;
    names.sort();
    let mut out = Vec::new();
    for name in names {
        let Some(sid) = name.strip_suffix(".json") else {
            continue;
        };
        if folded.get(sid).is_some() || exclude.iter().any(|s| s == sid) {
            continue;
        }
        let sess = read(ctx, &format!("sessions/{name}"))?;
        let tp = sess
            .get("transcript")
            .filter(|v| truthy(v))
            .cloned()
            .unwrap_or(json!(""));
        let quiet = match runtime.host("last_active", json!([tp, sess])) {
            Ok(at_last) => at - at_last.as_f64().unwrap_or(0.0) >= stale,
            Err(Error::Io(_)) => false,
            Err(e) => return Err(e),
        };
        if truthy(&sess["ended"]) || quiet {
            out.push((
                float_or_zero(&sess["started"]).map_err(Error::Value)?,
                sid.to_string(),
            ));
        }
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    Ok(out.into_iter().map(|(_, sid)| sid).collect())
}
pub fn spawn_fold(
    ctx: &LiveContext,
    ids: &[String],
    wait: f64,
    runtime: &mut dyn Runtime,
) -> Result<Value> {
    if ids.is_empty() || runtime.blocks(ctx)? {
        return Ok(Value::Null);
    }
    if ctx.env.get("TE_FOLD_SYNC").is_some_and(|v| v == "1") {
        return ids
            .iter()
            .map(|id| fold_session(ctx, id, None, wait, runtime))
            .collect::<Result<Vec<_>>>()
            .map(|v| json!(v));
    }
    let mut argv = vec!["fold".into()];
    for id in ids {
        argv.extend(["--session".into(), id.clone()]);
    }
    runtime.spawn(&argv, "fold.log")
}
