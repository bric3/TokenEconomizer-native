use super::{Bridge, Id, Result};
use crate::{
    plugin::{
        javascript::{disabled, space},
        process,
        winspawn::NO_WINDOW,
    },
    store::Store,
};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::{
    collections::HashMap,
    fs::{DirBuilder, OpenOptions},
    io::{self, BufRead, BufReader, BufWriter, Read, Write},
    net::{Ipv4Addr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    sync::{Arc, LazyLock, Mutex, mpsc},
    thread,
    time::Duration,
};

const TIMEOUT_S: f64 = 60.0;
const CONTROL_LIMIT: u64 = 1_048_576;
type Output = Arc<Mutex<BufWriter<io::Stdout>>>;
fn send(output: &Output, packet: &Value) -> Result<()> {
    let mut output = output.lock().map_err(|e| e.to_string())?;
    serde_json::to_writer(&mut *output, packet).map_err(|e| e.to_string())?;
    output
        .write_all(b"\n")
        .and_then(|_| output.flush())
        .map_err(|e| e.to_string())
}
fn mode(context: &Value) -> String {
    context["env"]["TE_OPENCODE_MODE"]
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| {
            if context["argv"]
                .as_array()
                .is_some_and(|args| args.iter().any(|v| v == "run"))
            {
                "run".into()
            } else {
                "tui".into()
            }
        })
}
fn flags(context: &Value) -> u32 {
    if cfg!(windows) && context["windows_hide"] != false {
        NO_WINDOW
    } else {
        0
    }
}

struct NativeRuntime {
    id: u64,
    context: Value,
    output: Option<Output>,
    replies: Option<mpsc::Receiver<Value>>,
    executable: PathBuf,
}
impl NativeRuntime {
    fn call(&mut self, method: &str, args: Value) -> Result<Value> {
        send(
            self.output.as_ref().ok_or("SDK is exiting")?,
            &json!({"id":self.id,"callback":method,"args":args}),
        )?;
        let reply = self
            .replies
            .as_ref()
            .ok_or("SDK is exiting")?
            .recv()
            .map_err(|e| e.to_string())?;
        if let Some(error) = reply["error"].as_str() {
            return Err(error.into());
        }
        Ok(reply["value"].clone())
    }
    fn context(&mut self) -> Result<Value> {
        if self.context.is_null() && self.output.is_some() {
            self.context = self.call("context", json!([]))?;
        }
        Ok(self.context.clone())
    }
    fn env(context: &Value) -> Result<Map<String, Value>> {
        let mut env = context["env"]
            .as_object()
            .cloned()
            .ok_or("environment must be an object")?;
        env.insert("PYTHONUTF8".into(), json!("1"));
        env.insert("TE_OPENCODE_MODE".into(), json!(mode(context)));
        Ok(env)
    }
    fn write_export(path: &Path, content: &str) -> Result<()> {
        let mut directory = DirBuilder::new();
        directory.recursive(true);
        let mut options = OpenOptions::new();
        options.create(true).write(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
            directory.mode(0o700);
            options.mode(0o600);
        }
        directory
            .create(path.parent().ok_or("export has no parent directory")?)
            .map_err(|e| e.to_string())?;
        options
            .open(path)
            .and_then(|mut file| file.write_all(content.as_bytes()))
            .map_err(|e| e.to_string())
    }
}
impl super::Runtime for NativeRuntime {
    fn checkpoint(&mut self, changes: Vec<Value>) {
        if let Some(output) = &self.output {
            let _ = send(output, &json!({"checkpoint":changes}));
        }
    }
    fn hook(&mut self, payload: &Value) -> Result<Value> {
        let ctx = self.context()?;
        let env = Self::env(&ctx)?;
        if disabled(&env) {
            return Ok(json!({}));
        }
        let Some(cwd) = ctx["process_cwd"].as_str() else {
            return Ok(json!({}));
        };
        let argv = vec![
            self.executable
                .with_file_name(format!("te-hook{}", std::env::consts::EXE_SUFFIX))
                .to_string_lossy()
                .into_owned(),
            "--host".into(),
            "opencode".into(),
        ];
        Ok(process::run_input_configured(
            &argv,
            Path::new(cwd),
            &env,
            &payload.to_string(),
            TIMEOUT_S,
            flags(&ctx),
        )
        .ok()
        .and_then(|out| {
            out.stdout
                .trim_matches(space)
                .split('\n')
                .next_back()
                .and_then(|line| serde_json::from_str::<Value>(line).ok())
        })
        .filter(|out| !out.is_null())
        .unwrap_or_else(|| json!({})))
    }
    fn export(&mut self, sid: &Id, directory: &Id, records: Option<Value>) -> Result<String> {
        let ctx = self.context()?;
        let root = ctx["env"]["TOKENECONOMIZER_HOME"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                Path::new(ctx["home"].as_str().unwrap_or("")).join(".tokeneconomizer")
            });
        let root = if root.is_absolute() {
            root
        } else {
            Path::new(
                ctx["process_cwd"]
                    .as_str()
                    .ok_or("working directory is unavailable")?,
            )
            .join(root)
        };
        let path = Store::new(Path::new(&format!(
            "{}/opencode-sessions/{}.json",
            root.display(),
            super::id_text(sid)
        )))
        .map_err(|e| e.to_string())?
        .base;
        let text = if let Some(document) = records {
            if document["messages"].as_array().is_none_or(Vec::is_empty) && path.exists() {
                return Ok(path.to_string_lossy().into_owned());
            }
            document.to_string()
        } else {
            static EMBEDDER: LazyLock<Regex> =
                LazyLock::new(|| Regex::new(r"(?i)(^|[\\/])(node|bun)(\.exe)?$").unwrap());
            let executable = ctx["exec_path"].as_str().ok_or("missing SDK executable")?;
            let argv = vec![
                if EMBEDDER.is_match(executable) {
                    "opencode".into()
                } else {
                    executable.into()
                },
                "export".into(),
                super::id_text(sid),
            ];
            let cwd = match directory {
                Some(Value::String(path)) => path.as_str(),
                None | Some(Value::Null) => ctx["process_cwd"].as_str().unwrap_or(""),
                _ => return Ok(String::new()),
            };
            let out = process::run_input_configured(
                &argv,
                Path::new(cwd),
                &Self::env(&ctx)?,
                "",
                TIMEOUT_S,
                flags(&ctx),
            );
            let Some(out) = out
                .ok()
                .filter(|out| out.code == 0 && out.stdout.len() <= 256 * 1024 * 1024)
            else {
                return Ok(String::new());
            };
            if !out.stdout.trim_start_matches(space).starts_with('{') {
                return Ok(String::new());
            }
            out.stdout
        };
        Ok(if Self::write_export(&path, &text).is_ok() {
            path.to_string_lossy().into_owned()
        } else {
            String::new()
        })
    }
    fn notice(&mut self, text: &str, stderr: bool) -> Result<()> {
        self.call(if stderr { "stderr" } else { "toast" }, json!([text]))
            .map(|_| ())
    }
    fn apply(&mut self, action: Value) -> Result<()> {
        self.call("apply", json!([action])).map(|_| ())
    }
    fn mode(&mut self) -> Result<String> {
        Ok(mode(&self.context()?))
    }
}

fn control(bridge: Arc<Bridge>, listener: TcpListener, token: String, executable: PathBuf) {
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else {
            continue;
        };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let request = (|| -> Result<Value> {
            let mut input = Vec::new();
            BufReader::new((&mut stream).take(CONTROL_LIMIT + 1))
                .read_until(b'\n', &mut input)
                .map_err(|e| e.to_string())?;
            if input.len() as u64 > CONTROL_LIMIT {
                return Err("control request is too large".into());
            }
            serde_json::from_slice(&input).map_err(|e| e.to_string())
        })();
        let Ok(request) = request else {
            continue;
        };
        if request["token"].as_str() != Some(token.as_str()) {
            continue;
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let job = bridge.prepare("exit", &json!({}))?;
            bridge.run(
                job,
                &mut NativeRuntime {
                    id: 0,
                    context: request["context"].clone(),
                    output: None,
                    replies: None,
                    executable: executable.clone(),
                },
            )
        }))
        .unwrap_or_else(|_| Err("opencode exit panicked".into()));
        let packet = match result {
            Ok(_) => json!({"done":true}),
            Err(error) => json!({"error":error}),
        };
        let _ = serde_json::to_writer(&mut stream, &packet);
        let _ = stream.write_all(b"\n");
        let _ = stream.flush();
        std::process::exit(0);
    }
}

/// The synchronous SDK exit handler delegates to the backend that owns its open sessions.
pub fn exit() -> Result<()> {
    let request: Value = serde_json::from_reader(io::stdin()).map_err(|e| e.to_string())?;
    let port = request["port"]
        .as_u64()
        .and_then(|v| u16::try_from(v).ok())
        .filter(|p| *p != 0);
    let connection = port.map(|port| {
        TcpStream::connect_timeout(&(Ipv4Addr::LOCALHOST, port).into(), Duration::from_secs(5))
    });
    let mut stream = match connection {
        Some(Ok(stream)) => stream,
        _ => {
            let version = request["version"]
                .as_u64()
                .and_then(|v| u8::try_from(v).ok())
                .ok_or("invalid SDK version")?;
            let bridge = Bridge::new(version)?;
            bridge.restore(version, request["restore"].clone(), &request["bootstrap"])?;
            let job = bridge.prepare("exit", &json!({}))?;
            bridge.run(
                job,
                &mut NativeRuntime {
                    id: 0,
                    context: request["context"].clone(),
                    output: None,
                    replies: None,
                    executable: std::env::current_exe().map_err(|e| e.to_string())?,
                },
            )?;
            return Ok(());
        }
    };
    // The live backend already owns the transcript; recovery data stays in this helper.
    let control = json!({"token":request["token"],"context":request["context"]});
    serde_json::to_writer(&mut stream, &control).map_err(|e| e.to_string())?;
    stream.write_all(b"\n").map_err(|e| e.to_string())?;
    let mut reply = String::new();
    BufReader::new(stream)
        .read_line(&mut reply)
        .map_err(|e| e.to_string())?;
    let reply: Value = serde_json::from_str(&reply).map_err(|e| e.to_string())?;
    if reply["done"] != true {
        return Err(reply["error"]
            .as_str()
            .unwrap_or("exit was not acknowledged")
            .into());
    }
    Ok(())
}

pub fn run(executable: &Path, version: u8) -> Result<()> {
    let bridge = Arc::new(Bridge::new(version)?);
    let token = std::env::var("TE_NATIVE_CONTROL_TOKEN").map_err(|e| e.to_string())?;
    if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("invalid control token".into());
    }
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let (closing, executable_for_exit) = (bridge.clone(), executable.to_path_buf());
    thread::spawn(move || control(closing, listener, token, executable_for_exit));
    let output = Arc::new(Mutex::new(BufWriter::new(io::stdout())));
    send(&output, &json!({"ready":port}))?;
    let waiters = Arc::new(Mutex::new(HashMap::<u64, mpsc::Sender<Value>>::new()));
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
        if let Some(reply) = request.get("reply") {
            if let Some(waiter) = waiters.lock().map_err(|e| e.to_string())?.get(&id) {
                let _ = waiter.send(reply.clone());
            }
            continue;
        }
        if waiters.lock().map_err(|e| e.to_string())?.contains_key(&id) {
            send(
                &output,
                &json!({"id":id,"error":"duplicate opencode request id"}),
            )?;
            continue;
        }
        if let Some(saved) = request.get("restore") {
            let result = if started {
                Err("restore is only valid on the first request".into())
            } else {
                bridge.restore(version, saved.clone(), &request["bootstrap"])
            };
            if let Err(error) = result {
                send(&output, &json!({"id":id,"error":error}))?;
                continue;
            }
        }
        started = true;
        let job = match bridge.prepare(request["method"].as_str().unwrap_or(""), &request["body"]) {
            Ok(job) => job,
            Err(error) => {
                send(&output, &json!({"id":id,"error":error}))?;
                continue;
            }
        };
        let (tx, replies) = mpsc::channel();
        waiters.lock().map_err(|e| e.to_string())?.insert(id, tx);
        bridge.checkpoint(&mut NativeRuntime {
            id,
            context: Value::Null,
            output: Some(output.clone()),
            replies: None,
            executable: executable.to_path_buf(),
        })?;
        workers.retain(|worker| !worker.is_finished());
        let (bridge, output, waiters, executable) = (
            bridge.clone(),
            output.clone(),
            waiters.clone(),
            executable.to_path_buf(),
        );
        workers.push(thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                bridge.run(
                    job,
                    &mut NativeRuntime {
                        id,
                        context: request["context"].clone(),
                        output: Some(output.clone()),
                        replies: Some(replies),
                        executable,
                    },
                )
            }))
            .unwrap_or_else(|_| Err("opencode callback panicked".into()));
            let packet = match result {
                Ok(value) => json!({"id":id,"value":value}),
                Err(error) => json!({"id":id,"error":error}),
            };
            let _ = send(&output, &packet);
            if let Ok(mut waiters) = waiters.lock() {
                waiters.remove(&id);
            }
        }));
    }
    // Finish accepted native I/O after EOF, releasing callbacks whose SDK caller has stopped.
    waiters.lock().map_err(|e| e.to_string())?.clear();
    for worker in workers {
        let _ = worker.join();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::opencode::Runtime;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn an_empty_copy_keeps_an_existing_export() {
        let root = std::env::temp_dir().join(format!(
            "te-opencode-export-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let mut runtime = NativeRuntime {
            id: 0,
            context: json!({"env":{"TOKENECONOMIZER_HOME":root},"home":root,"process_cwd":root}),
            output: None,
            replies: None,
            executable: PathBuf::from("/unused"),
        };
        let sid = Some(json!("shared"));
        let empty = json!({"info":{"id":"shared"},"messages":[]});
        let file = runtime.export(&sid, &None, Some(empty.clone())).unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), empty.to_string());
        let existing =
            "{\"info\":{\"id\":\"shared\"},\"messages\":[{\"prompt\":\"from another copy\"}]}";
        fs::write(&file, existing).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&file, fs::Permissions::from_mode(0o640)).unwrap();
            assert_eq!(
                fs::metadata(root.join("opencode-sessions"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
        assert_eq!(runtime.export(&sid, &None, Some(empty)).unwrap(), file);
        assert_eq!(fs::read_to_string(&file).unwrap(), existing);
        let seen = json!({"info":{"id":"shared"},"messages":[{"prompt":"seen here"}]});
        runtime.export(&sid, &None, Some(seen.clone())).unwrap();
        assert_eq!(fs::read_to_string(&file).unwrap(), seen.to_string());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&file).unwrap().permissions().mode() & 0o777,
                0o640
            );
        }
        fs::remove_dir_all(root).unwrap();
    }
}
