use super::{Bridge, HOOK_TIMEOUT_S, Limits, Result, Runtime};
use crate::{
    plugin::{paths, process, winspawn::NO_WINDOW},
    value::json_text,
};
use serde_json::{Map, Value, json};
use std::{
    collections::HashMap,
    io::{self, BufRead, BufWriter, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    thread,
};

type Output = Arc<Mutex<BufWriter<io::Stdout>>>;
fn send(output: &Output, value: &Value) -> Result<()> {
    let mut output = output.lock().map_err(|e| e.to_string())?;
    serde_json::to_writer(&mut *output, value).map_err(|e| e.to_string())?;
    output
        .write_all(b"\n")
        .and_then(|_| output.flush())
        .map_err(|e| e.to_string())
}
struct NativeRuntime {
    request: Value,
    output: Output,
    replies: mpsc::Receiver<Value>,
    hook: PathBuf,
}
impl NativeRuntime {
    fn call(&self, method: &str, args: Value) -> Result<Value> {
        send(
            &self.output,
            &json!({"id":self.request["id"],"callback":method,"args":args}),
        )?;
        let reply = self.replies.recv().map_err(|e| e.to_string())?;
        if let Some(error) = reply["error"].as_str() {
            return Err(error.into());
        }
        Ok(reply["value"].clone())
    }
    fn env(&self) -> Result<Map<String, Value>> {
        self.request["env"]
            .as_object()
            .cloned()
            .ok_or("environment must be an object".into())
    }
}
impl Runtime for NativeRuntime {
    fn disabled(&mut self) -> Result<bool> {
        Ok(super::disabled(&self.env()?))
    }
    fn home(&mut self) -> Result<PathBuf> {
        if let Some(home) = self.call("home", json!([]))?.as_str() {
            return Ok(home.into());
        }
        Ok(self
            .env()?
            .get("HERMES_HOME")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or(
                Path::new(
                    self.request["home"]
                        .as_str()
                        .ok_or("missing home directory")?,
                )
                .join(".hermes"),
            ))
    }
    fn state_root(&mut self) -> Result<PathBuf> {
        let mut env = self.env()?;
        let home = Path::new(
            self.request["home"]
                .as_str()
                .ok_or("missing home directory")?,
        );
        let root = env
            .get("TOKENECONOMIZER_HOME")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or(home.join(".tokeneconomizer"));
        let root = if root.is_absolute() {
            root
        } else {
            Path::new(
                self.request["cwd"]
                    .as_str()
                    .ok_or("gateway working directory is unavailable")?,
            )
            .join(root)
        };
        env.insert("TOKENECONOMIZER_HOME".into(), json!(root));
        paths::state_root(&env, home)
    }
    fn dispatch(&mut self, payload: Value, cwd: &str, mode: &str, home: &Path) -> Result<Value> {
        if self.disabled()? {
            return Ok(json!({}));
        }
        let mut env = self.env()?;
        env.extend(
            json!({"PYTHONUTF8":"1","TE_HERMES_MODE":mode,"HERMES_HOME":home})
                .as_object()
                .unwrap()
                .clone(),
        );
        let argv = vec![
            self.hook.to_string_lossy().into_owned(),
            "--host".into(),
            "hermes".into(),
        ];
        let out = process::run_input_configured(
            &argv,
            Path::new(cwd),
            &env,
            &json_text(&payload),
            HOOK_TIMEOUT_S,
            if cfg!(windows) { NO_WINDOW } else { 0 },
        );
        Ok(out
            .ok()
            .and_then(|out| {
                out.stdout
                    .trim()
                    .lines()
                    .last()
                    .and_then(|line| serde_json::from_str::<Value>(line).ok())
            })
            .filter(Value::is_object)
            .unwrap_or(json!({})))
    }
    fn notice(&mut self, text: &str) -> Result<()> {
        self.call("notice", json!([text])).map(|_| ())
    }
}

/// Serve one SDK bridge. Each request keeps its caller's environment and profile callbacks.
pub fn run(executable: &Path) -> Result<()> {
    let bridge = Arc::new(Bridge::default());
    let output = Arc::new(Mutex::new(BufWriter::new(io::stdout())));
    let requests = Arc::new(Mutex::new(HashMap::<u64, mpsc::Sender<Value>>::new()));
    let hook = executable.with_file_name(format!("te-hook{}", std::env::consts::EXE_SUFFIX));
    let mut workers: Vec<thread::JoinHandle<()>> = Vec::new();
    for line in io::stdin().lock().lines() {
        let request = match serde_json::from_str::<Value>(&line.map_err(|e| e.to_string())?) {
            Ok(request) => request,
            Err(error) => {
                send(&output, &json!({"id":null,"error":error.to_string()}))?;
                continue;
            }
        };
        let Some(id) = request["id"].as_u64() else {
            send(
                &output,
                &json!({"id":null,"error":"request id must be an unsigned integer"}),
            )?;
            continue;
        };
        if request["cancel"] == true {
            requests.lock().map_err(|e| e.to_string())?.remove(&id);
            continue;
        }
        if let Some(reply) = request.get("reply") {
            if let Some(waiter) = requests.lock().map_err(|e| e.to_string())?.get(&id) {
                let _ = waiter.send(reply.clone());
            }
            continue;
        }
        let (reply_send, replies) = mpsc::channel();
        {
            let mut requests = requests.lock().map_err(|e| e.to_string())?;
            if requests.contains_key(&id) {
                return Err("duplicate request id".into());
            }
            requests.insert(id, reply_send);
        }
        workers.retain(|worker| !worker.is_finished());
        let (bridge, output, requests, hook) = (
            bridge.clone(),
            output.clone(),
            requests.clone(),
            hook.clone(),
        );
        workers.push(thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut runtime = NativeRuntime {
                    request: request.clone(),
                    output: output.clone(),
                    replies,
                    hook,
                };
                bridge.invoke(
                    request["method"]
                        .as_str()
                        .ok_or("missing callback method")?,
                    &request["args"],
                    &mut runtime,
                    &Limits::default(),
                )
            }))
            .unwrap_or_else(|_| Err("Hermes callback panicked".into()));
            let response = match result {
                Ok(value) => json!({"id":id,"value":value}),
                Err(error) => json!({"id":id,"error":error}),
            };
            let _ = send(&output, &response);
            if let Ok(mut requests) = requests.lock() {
                requests.remove(&id);
            }
        }));
    }
    requests.lock().map_err(|e| e.to_string())?.clear();
    for worker in workers {
        let _ = worker.join();
    }
    Ok(())
}
