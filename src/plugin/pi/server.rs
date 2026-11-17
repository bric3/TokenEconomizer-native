use super::{Bridge, HOOK_TIMEOUT_S, Prepared, Result, Runtime, carried_section, disabled, space};
use crate::plugin::{process, winspawn::NO_WINDOW};
use serde_json::{Map, Value, json};
use std::{
    io::{self, BufRead, BufWriter, Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
};

type Output = Arc<Mutex<BufWriter<io::Stdout>>>;

#[derive(Default)]
struct Session {
    bridge: Bridge,
    pending: Vec<(u64, Arc<Prepared>)>,
}
impl Session {
    fn checkpoint(&self) -> Value {
        // Complete accepted requests without hook effects if the backend stops before replying.
        let mut recovered = self.bridge.clone();
        let mut replies = Map::new();
        for (id, prepared) in &self.pending {
            let value = recovered
                .complete(prepared, prepared.failed_output())
                .unwrap_or_else(|_| json!({"actions":[]}));
            replies.insert(id.to_string(), value);
        }
        json!({"state":recovered,"replies":replies})
    }
    fn finish(&mut self, id: u64, prepared: &Prepared, out: Result<Value>) -> Value {
        self.pending.retain(|(pending, _)| *pending != id);
        let value = self
            .bridge
            .complete(prepared, out.unwrap_or_else(|_| prepared.failed_output()))
            .unwrap_or_else(|_| json!({"actions":[]}));
        json!({"id":id,"value":value,"checkpoint":self.checkpoint()})
    }
}
fn send(output: &Output, value: &Value) -> Result<()> {
    let mut output = output.lock().map_err(|e| e.to_string())?;
    serde_json::to_writer(&mut *output, value).map_err(|e| e.to_string())?;
    output
        .write_all(b"\n")
        .and_then(|_| output.flush())
        .map_err(|e| e.to_string())
}
struct NativeRuntime {
    executable: PathBuf,
}
impl NativeRuntime {
    fn run(
        &self,
        program: &Path,
        argv: &[String],
        input: &str,
        ctx: &Value,
        extra: Map<String, Value>,
    ) -> Result<String> {
        let mut env = ctx["env"]
            .as_object()
            .cloned()
            .ok_or("environment must be an object")?;
        env.extend(extra);
        env.insert("PYTHONUTF8".into(), json!("1"));
        if let Some(mode) = ctx.get("mode") {
            env.insert("TE_PI_MODE".into(), mode.clone());
        }
        let mut command = vec![program.to_string_lossy().into_owned()];
        command.extend_from_slice(argv);
        let Some(cwd) = ctx["process_cwd"].as_str() else {
            return Ok(String::new());
        };
        Ok(process::run_input_configured(
            &command,
            Path::new(cwd),
            &env,
            input,
            HOOK_TIMEOUT_S,
            if cfg!(windows) { NO_WINDOW } else { 0 },
        )
        .map(|out| out.stdout)
        .unwrap_or_default())
    }
}
impl Runtime for NativeRuntime {
    fn hook(&mut self, payload: &Value, ctx: &Value) -> Result<Value> {
        if disabled(
            ctx["env"]
                .as_object()
                .ok_or("environment must be an object")?,
        ) {
            return Ok(json!({}));
        }
        let hook = self
            .executable
            .with_file_name(format!("te-hook{}", std::env::consts::EXE_SUFFIX));
        let out = self.run(
            &hook,
            &["--host".into(), "pi".into()],
            &serde_json::to_string(payload).map_err(|e| e.to_string())?,
            ctx,
            Map::new(),
        )?;
        Ok(out
            .trim_matches(space)
            .split('\n')
            .next_back()
            .and_then(|line| serde_json::from_str::<Value>(line).ok())
            .filter(|v| !v.is_null())
            .unwrap_or(json!({})))
    }
    fn command(&mut self, args: &[String], ctx: &Value) -> Result<String> {
        let mut extra = Map::new();
        if let Some(sid) = ctx.get("session_id") {
            extra.insert("TE_SESSION_ID".into(), sid.clone());
        }
        self.run(&self.executable, args, "", ctx, extra)
    }
}
pub fn carried() -> Result<()> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|e| e.to_string())?;
    let input: Value = serde_json::from_str(&input).map_err(|e| e.to_string())?;
    let section = carried_section(input.as_array().ok_or("entries must be a list")?)?;
    let mut output = io::stdout().lock();
    serde_json::to_writer(&mut output, &section).map_err(|e| e.to_string())?;
    output.write_all(b"\n").map_err(|e| e.to_string())
}

/// Prepare in arrival order; complete in hook-response order, as Pi's async callbacks do.
pub fn run(executable: &Path) -> Result<()> {
    let session = Arc::new(Mutex::new(Session::default()));
    let output = Arc::new(Mutex::new(BufWriter::new(io::stdout())));
    let mut workers: Vec<thread::JoinHandle<()>> = Vec::new();
    let mut started = false;
    for line in io::stdin().lock().lines() {
        let request = match serde_json::from_str::<Value>(&line.map_err(|e| e.to_string())?) {
            Ok(value) => value,
            Err(error) => {
                send(&output, &json!({"id":null,"error":error.to_string()}))?;
                continue;
            }
        };
        let Some(id) = request["id"].as_u64() else {
            send(&output, &json!({"id":null,"error":"invalid request id"}))?;
            continue;
        };
        let mut state = session.lock().map_err(|e| e.to_string())?;
        if let Some(restore) = request.get("restore") {
            let restored = if started {
                Err("restore is only valid on the first request".into())
            } else {
                Bridge::restore(restore.clone())
            };
            match restored {
                Ok(bridge) => state.bridge = bridge,
                Err(error) => {
                    send(&output, &json!({"id":id,"error":error}))?;
                    continue;
                }
            }
        }
        started = true;
        if state.pending.iter().any(|(pending, _)| *pending == id) {
            send(&output, &json!({"id":id,"error":"duplicate Pi request id"}))?;
            continue;
        }
        let prepared = state.bridge.prepare(
            request["event"].as_str().unwrap_or(""),
            &request["context"],
            &request["body"],
        );
        let prepared = match prepared {
            Ok(value) => value,
            Err(error) => {
                send(&output, &json!({"id":id,"error":error}))?;
                continue;
            }
        };
        let prepared = Arc::new(prepared);
        state.pending.push((id, prepared.clone()));
        send(&output, &json!({"checkpoint":state.checkpoint()}))?;
        drop(state);
        workers.retain(|worker| !worker.is_finished());
        let (session, output, executable) =
            (session.clone(), output.clone(), executable.to_path_buf());
        workers.push(thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                prepared.execute(&mut NativeRuntime { executable })
            }))
            .unwrap_or_else(|_| Err("Pi callback panicked".into()));
            if let Ok(mut state) = session.lock() {
                let packet = state.finish(id, &prepared, result);
                let _ = send(&output, &packet);
            }
        }));
    }
    for worker in workers {
        let _ = worker.join();
    }
    Ok(())
}
