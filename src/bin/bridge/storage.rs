use std::{
    cell::RefCell,
    collections::HashMap,
    fs, io,
    path::Path,
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
};

use serde_json::{Value, json};
use tokeneconomizer::{
    envelope::{self, EnvelopeTracker},
    loopguard::turn::TurnGuard,
    store::{self, Store, StoreLock},
};

use super::callback::CallbackTape;

static LOCKS: LazyLock<Mutex<HashMap<u64, StoreLock>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static NEXT_LOCK: AtomicU64 = AtomicU64::new(1);
static LOCK_JOBS: LazyLock<Mutex<HashMap<u64, JoinHandle<io::Result<StoreLock>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn response(result: io::Result<Value>) -> Value {
    match result {
        Ok(value) => json!({"value": value}),
        Err(error) => {
            let kind = match error.kind() {
                io::ErrorKind::NotFound => "FileNotFoundError",
                io::ErrorKind::PermissionDenied => "PermissionError",
                io::ErrorKind::InvalidData => "ValueError",
                io::ErrorKind::IsADirectory => "IsADirectoryError",
                io::ErrorKind::NotADirectory => "NotADirectoryError",
                _ => "OSError",
            };
            json!({"raised": kind, "message": error.to_string()})
        }
    }
}

pub fn dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
    if !(op.starts_with("store.")
        || matches!(
            op,
            "lifecycle.write_json_atomic"
                | "envelope.load_envelope"
                | "envelope.save_envelope"
                | "envelope.record_turn"
        ))
    {
        return Ok(None);
    }
    let tape = RefCell::new(CallbackTape::new(args));
    let windows = args["windows"].as_bool().unwrap_or(cfg!(windows));
    let mut replace = |src: &Path, dst: &Path| {
        if args["replace_transport"] == true {
            // A replay cleans up its temporary file before the injected operation runs.
            let bytes = fs::read(src)?;
            tape.borrow_mut()
                .io_call(json!({"callback": "replace", "src": src, "dst": dst, "bytes": bytes}))?;
            Ok(())
        } else {
            fs::rename(src, dst)
        }
    };
    let result = (|| {
        if op == "lifecycle.write_json_atomic" {
            store::write_json_atomic_with(
                Path::new(args["path"].as_str().unwrap_or("")),
                &args["obj"],
                false,
                windows,
                &mut replace,
            )?;
            return Ok(Value::Null);
        }
        if op == "envelope.load_envelope" && args["store_transport"] == true {
            let reply = tape
                .borrow_mut()
                .io_call(json!({"callback": "store_read", "name": "envelope.json"}))?;
            return Ok(json!(envelope::tracker_from_history(
                &reply["value"],
                args["level"].as_str().unwrap_or("default"),
                &args["overrides"]
            )?));
        }
        if op == "store.release" {
            if let Some(lock) = LOCKS
                .lock()
                .unwrap()
                .remove(&args["id"].as_u64().unwrap_or(0))
            {
                lock.release()?;
            }
            return Ok(Value::Null);
        }
        if op == "store.acquire_result" {
            let id = args["id"].as_u64().unwrap_or(0);
            let mut jobs = LOCK_JOBS.lock().unwrap();
            if !jobs.get(&id).is_some_and(JoinHandle::is_finished) {
                return Ok(json!({"ready": false}));
            }
            let lock = jobs
                .remove(&id)
                .unwrap()
                .join()
                .map_err(|_| io::Error::other("lock acquisition panicked"))??;
            let acquired = lock.acquired;
            LOCKS.lock().unwrap().insert(id, lock);
            return Ok(json!({"ready": true, "acquired": acquired}));
        }
        let base = if op.starts_with("envelope.") {
            &args["store"]["base"]
        } else if op == "store.new" {
            &args["base"]
        } else {
            &args["self"]["base"]
        };
        let store = Store::new(Path::new(base.as_str().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "expected store base directory")
        })?))?;
        let name = args["name"].as_str().unwrap_or("");
        match op {
            "envelope.load_envelope" => Ok(json!(envelope::load_envelope(
                &store,
                args["level"].as_str().unwrap_or("default"),
                &args["overrides"]
            )?)),
            "envelope.save_envelope" => {
                let tracker: EnvelopeTracker =
                    serde_json::from_value(args["tracker"].clone()).map_err(io::Error::other)?;
                envelope::save_envelope(&store, &tracker)?;
                Ok(Value::Null)
            }
            "envelope.record_turn" => {
                let guard: TurnGuard =
                    serde_json::from_value(args["guard"].clone()).map_err(io::Error::other)?;
                Ok(json!(envelope::record_turn(
                    &store,
                    &guard,
                    args["level"].as_str().unwrap_or("default"),
                    args["stopped_normally"].as_bool().unwrap_or(true),
                    &args["overrides"]
                )?))
            }
            "store.new" => Ok(json!({"base": store.base})),
            "store.path" | "store.ensure" => {
                let parts: Vec<_> = args["parts"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .collect();
                let path = if op == "store.ensure" {
                    store.ensure(&parts)?
                } else {
                    store.path(&parts)
                };
                Ok(json!(path))
            }
            "store.read" => store::read_document(
                &store.base.join(name),
                args["default"].clone(),
                windows,
                |path| {
                    if args["read_transport"] == true {
                        let reply = tape
                            .borrow_mut()
                            .io_call(json!({"callback": "read", "path": path}))?;
                        Ok(reply["value"].as_str().unwrap_or("").as_bytes().to_vec())
                    } else {
                        fs::read(path)
                    }
                },
            ),
            "store.write" => {
                let target = store.base.join(name);
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                store::write_json_atomic_with(&target, &args["obj"], true, windows, &mut replace)?;
                Ok(json!(target))
            }
            "store.append" => {
                store.append(name, &args["record"])?;
                Ok(Value::Null)
            }
            "store.read_lines" => Ok(json!(store.read_lines(name, args["limit"].as_i64())?)),
            "store.acquire" => {
                let name = args["name"].as_str().unwrap_or("state");
                let timeout = args["timeout"].as_f64().unwrap_or(10.0);
                let id = NEXT_LOCK.fetch_add(1, Ordering::Relaxed);
                if args["async"] == true {
                    let name = name.to_owned();
                    LOCK_JOBS
                        .lock()
                        .unwrap()
                        .insert(id, thread::spawn(move || store.locked(&name, timeout)));
                    return Ok(json!({"id": id}));
                }
                let lock = store.locked(name, timeout)?;
                let acquired = lock.acquired;
                LOCKS.lock().unwrap().insert(id, lock);
                Ok(json!({"id": id, "acquired": acquired}))
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unknown storage operation: {op}"),
            )),
        }
    })();
    Ok(Some(tape.into_inner().finish(response(result))))
}
