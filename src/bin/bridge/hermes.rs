use super::job::{self, Channel};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc, LazyLock, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use tokeneconomizer::plugin::{
    hermes::{self, Bridge, Limits, Runtime},
    paths,
};

static BRIDGES: LazyLock<Mutex<HashMap<u64, Arc<Bridge>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static NEXT: AtomicU64 = AtomicU64::new(1);
struct Transport {
    channel: Channel,
    args: Value,
}
impl Transport {
    fn call(&self, method: &str, args: Value) -> Result<Value, String> {
        let reply = self.channel.call(method, args)?;
        if let Some(error) = reply["error"].as_str() {
            return Err(error.into());
        }
        Ok(reply["value"].clone())
    }
}
impl Runtime for Transport {
    fn disabled(&mut self) -> Result<bool, String> {
        Ok(hermes::disabled(
            self.args["env"].as_object().ok_or("expected environment")?,
        ))
    }
    fn home(&mut self) -> Result<PathBuf, String> {
        self.call("home", json!([]))?
            .as_str()
            .map(PathBuf::from)
            .ok_or("home must be a path".into())
    }
    fn state_root(&mut self) -> Result<PathBuf, String> {
        paths::state_root(
            self.args["env"].as_object().ok_or("expected environment")?,
            Path::new(self.args["home"].as_str().ok_or("expected home")?),
        )
    }
    fn dispatch(
        &mut self,
        payload: Value,
        cwd: &str,
        mode: &str,
        home: &Path,
    ) -> Result<Value, String> {
        self.call("dispatch", json!([payload, cwd, mode, home]))
    }
    fn notice(&mut self, text: &str) -> Result<(), String> {
        self.call("notice", json!([text])).map(|_| ())
    }
}
pub(super) fn dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
    if !op.starts_with("hermes_gateway.") {
        return Ok(None);
    }
    match op {
        "hermes_gateway.constants" => {
            return Ok(Some(
                json!({"HOOK_TIMEOUT_S":hermes::HOOK_TIMEOUT_S,"LOCK_TIMEOUT_S":hermes::LOCK_TIMEOUT_S,"MAX_IGNORED":hermes::MAX_IGNORED,"MAX_ENDED":hermes::MAX_ENDED,"SECTION_ID":hermes::SECTION_ID,"REDIRECT_FIELD":hermes::REDIRECT_FIELD}),
            ));
        }
        "hermes_gateway.new" => {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            BRIDGES
                .lock()
                .map_err(|e| e.to_string())?
                .insert(id, Arc::default());
            return Ok(Some(json!(id)));
        }
        "hermes_gateway.with_redirect" => {
            return hermes::with_redirect(&args["result"], &args["text"]).map(Some);
        }
        "hermes_gateway.untrusted" => {
            return Ok(Some(json!(hermes::untrusted(&args["tool_name"]))));
        }
        "hermes_gateway.disabled" => {
            return Ok(Some(json!(hermes::disabled(
                args["env"].as_object().ok_or("expected environment")?
            ))));
        }
        "hermes_gateway.resume" => {
            return job::advance(
                args["job"].as_u64().ok_or("expected job")?,
                args.get("reply").cloned(),
            )
            .map(Some);
        }
        _ => (),
    }
    let bridge = BRIDGES
        .lock()
        .map_err(|e| e.to_string())?
        .get(&args["bridge"].as_u64().ok_or("expected Hermes bridge")?)
        .cloned()
        .ok_or("unknown Hermes bridge")?;
    let (op, args) = (op.to_string(), args.clone());
    job::start_polling("hermes_gateway", move |channel| {
        let result = (|| {
            let limits = Limits {
                max_ended: args["limits"]["max_ended"]
                    .as_u64()
                    .unwrap_or(hermes::MAX_ENDED as u64) as usize,
                max_ignored: args["limits"]["max_ignored"]
                    .as_u64()
                    .unwrap_or(hermes::MAX_IGNORED as u64) as usize,
                lock_timeout: args["limits"]["lock_timeout"]
                    .as_f64()
                    .unwrap_or(hermes::LOCK_TIMEOUT_S),
            };
            let mut runtime = Transport {
                channel,
                args: args.clone(),
            };
            if op == "hermes_gateway.hold" {
                let session = bridge
                    .session(
                        args["sid"].as_str().ok_or("expected session id")?,
                        &mut runtime,
                        &limits,
                    )?
                    .ok_or("unknown session")?;
                let _guard = session.lock()?;
                return runtime.call("locked", json!([]));
            }
            bridge.invoke(
                args["method"].as_str().ok_or("expected method")?,
                &args["args"],
                &mut runtime,
                &limits,
            )
        })();
        match result {
            Ok(value) => json!({"value":value}),
            Err(message) => json!({"raised":"RuntimeError","message":message}),
        }
    })
    .map(Some)
}
