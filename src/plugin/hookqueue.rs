use crate::{
    store::Store,
    value::{json_text, scalar_text, truthy},
};
use serde_json::{Value, json};
use std::{
    fs::{self, File, FileTimes},
    io,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

pub const WAITS: [(&str, f64); 6] = [
    ("SessionStart", 10.0),
    ("UserPromptSubmit", 5.0),
    ("PostToolUse", 3.0),
    ("PostToolUseFailure", 3.0),
    ("Stop", 5.0),
    ("SessionEnd", 1.0),
];
pub const QUEUED: [&str; 5] = [
    "UserPromptSubmit",
    "PostToolUse",
    "PostToolUseFailure",
    "Stop",
    "SessionEnd",
];
pub const DRAIN_MAX_EVENTS: usize = 200;
pub const DRAIN_BUDGET_S: f64 = 8.0;
pub const IDLE_S: f64 = 2.0 * 86400.0;
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub fn safe(text: &Value) -> String {
    let text = if truthy(text) {
        scalar_text(text)
    } else {
        "unknown".into()
    };
    let safe: String = text
        .chars()
        .take(200)
        .map(|c| {
            if c.is_ascii_alphanumeric() || "._-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    if safe.is_empty() {
        "unknown".into()
    } else {
        safe
    }
}
pub fn queues(repomap: &str, event: &str) -> bool {
    !(event == "UserPromptSubmit" && repomap == "on") && QUEUED.contains(&event)
}
pub fn enqueue(queue: &Path, payload: &Value) -> bool {
    (|| -> io::Result<()> {
        fs::create_dir_all(queue)?;
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let name = format!(
            "{time:020}-{:08}-{:06}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        let temporary = queue.join(format!("{name}.tmp"));
        fs::write(&temporary, json_text(payload))?;
        fs::rename(temporary, queue.join(format!("{name}.json")))
    })()
    .is_ok()
}

pub struct Replay {
    pub session_id: String,
    pub has_handler: bool,
}
pub trait Runtime {
    fn prepare_replay(&mut self, payload: &Value, event: &str) -> Result<Replay, String>;
    fn replay_handler(&mut self, replaying: bool) -> Result<(), String>;
    fn record_error(&mut self, error: &str) -> Result<(), String>;
    fn handler(&mut self) -> Result<Value, String>;
    fn enqueue(&mut self, queue: &Path, payload: &Value) -> bool {
        enqueue(queue, payload)
    }
}
pub fn replay(session_id: &str, payload: &Value, runtime: &mut dyn Runtime) -> Result<(), String> {
    let result: Result<(), String> = (|| {
        let other =
            runtime.prepare_replay(payload, payload["hook_event_name"].as_str().unwrap_or(""))?;
        if other.has_handler && other.session_id == session_id {
            runtime.replay_handler(true)?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        runtime.record_error(&error)?;
    }
    Ok(())
}
pub fn drain(
    session_id: &str,
    queue: &Path,
    max_events: usize,
    budget: f64,
    runtime: &mut dyn Runtime,
) -> Result<(), String> {
    let deadline = SystemTime::now()
        + Duration::try_from_secs_f64(budget.max(0.0)).map_err(|e| e.to_string())?;
    let mut done = 0;
    while done < max_events && SystemTime::now() < deadline {
        let Ok(entries) = directory_entries(queue) else {
            return Ok(());
        };
        let mut names: Vec<_> = entries
            .into_iter()
            .map(|e| e.file_name())
            .filter(|name| name.to_string_lossy().ends_with(".json"))
            .collect();
        names.sort();
        if names.is_empty() {
            return Ok(());
        }
        for name in names {
            let path = queue.join(name);
            let payload = fs::read(&path)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
            // Remove before replay so a failed handler cannot process the same event twice.
            if fs::remove_file(&path).is_err() {
                continue;
            }
            if let Some(payload) = payload.filter(Value::is_object) {
                replay(session_id, &payload, runtime)?;
                done += 1;
            }
            if done >= max_events || SystemTime::now() >= deadline {
                return Ok(());
            }
        }
    }
    Ok(())
}
fn directory_entries(path: &Path) -> io::Result<Vec<fs::DirEntry>> {
    fs::read_dir(path)?.collect()
}
fn age(path: &Path, now: SystemTime) -> io::Result<f64> {
    Ok(now
        .duration_since(fs::metadata(path)?.modified()?)
        .unwrap_or_default()
        .as_secs_f64())
}
pub fn prune(base: &Path, idle: f64) {
    let now = SystemTime::now();
    let marker = base.join(".pruned");
    if age(&marker, now).is_ok_and(|age| age < 86400.0) {
        return;
    }
    let Ok(entries) = directory_entries(base) else {
        return;
    };
    for entry in entries {
        let path = entry.path();
        if !age(&path, now).is_ok_and(|age| age > idle) {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.ends_with(".lock") {
            let _ = fs::remove_file(path);
        } else if name.ends_with(".queue")
            && fs::read_dir(&path).is_ok_and(|mut entries| entries.next().is_none())
        {
            let _ = fs::remove_dir(path);
        }
    }
    let _ = File::create(marker);
}

pub struct Hook<'a> {
    pub root: &'a Path,
    pub host: &'a str,
    pub session_id: &'a str,
    pub repomap: &'a str,
    pub event: &'a str,
    pub payload: &'a Value,
}
pub struct Limits {
    pub wait: f64,
    pub max_events: usize,
    pub budget: f64,
    pub idle: f64,
}
impl Limits {
    pub fn for_event(event: &str) -> Self {
        Self {
            wait: WAITS
                .iter()
                .find(|(name, _)| *name == event)
                .map(|(_, wait)| *wait)
                .unwrap_or(3.0),
            max_events: DRAIN_MAX_EVENTS,
            budget: DRAIN_BUDGET_S,
            idle: IDLE_S,
        }
    }
}
pub fn run(hook: Hook<'_>, limits: &Limits, runtime: &mut dyn Runtime) -> Result<Value, String> {
    let base = hook.root.join("locks").join(safe(&json!(hook.host)));
    let name = safe(&json!(hook.session_id));
    let queue = base.join(format!("{name}.queue"));
    let lock = Store::new(&base)
        .and_then(|store| store.locked(&name, limits.wait))
        .map_err(|e| e.to_string())?;
    let acquired = lock.acquired;
    if acquired {
        // Refresh the held lock so pruning cannot remove an old session's active inode.
        if let Ok(file) = fs::OpenOptions::new()
            .write(true)
            .open(base.join(format!(".{name}.lock")))
        {
            let now = SystemTime::now();
            let _ = file.set_times(FileTimes::new().set_accessed(now).set_modified(now));
        }
    }
    prune(&base, limits.idle);
    let mut result = Ok(json!({}));
    if acquired {
        result = (|| {
            drain(
                hook.session_id,
                &queue,
                limits.max_events,
                limits.budget,
                runtime,
            )?;
            let result = runtime.handler();
            drain(
                hook.session_id,
                &queue,
                limits.max_events,
                limits.budget,
                runtime,
            )?;
            result
        })();
    }
    lock.release().map_err(|e| e.to_string())?;
    if !acquired && !(queues(hook.repomap, hook.event) && runtime.enqueue(&queue, hook.payload)) {
        result = runtime.handler();
    }
    result.map(|value| if truthy(&value) { value } else { json!({}) })
}
