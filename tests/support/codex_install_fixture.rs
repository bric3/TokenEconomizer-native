use serde_json::{Map, Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};
#[cfg(unix)]
use std::{
    io::{self, Read},
    os::unix::process::CommandExt,
    process::{Child, Command, Stdio},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tokeneconomizer::experiments::swebench::{Failure, Result, Runtime, codex_install};

pub struct Install {
    root: PathBuf,
    pub extra_env: Map<String, Value>,
    #[cfg(unix)]
    environment: Map<String, Value>,
    #[cfg(unix)]
    child: Option<Child>,
    #[cfg(unix)]
    readers: Vec<JoinHandle<io::Result<Vec<u8>>>>,
    #[cfg(unix)]
    clock: Instant,
}
impl Install {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.into(),
            extra_env: Map::new(),
            #[cfg(unix)]
            environment: Map::new(),
            #[cfg(unix)]
            child: None,
            #[cfg(unix)]
            readers: Vec::new(),
            #[cfg(unix)]
            clock: Instant::now(),
        }
    }
    pub fn run(&mut self, method: &str, input: Value) -> Result<Value> {
        codex_install::dispatch(method, &input, self)
    }
    #[cfg(unix)]
    pub fn installed() -> Option<String> {
        std::env::var("TE_CODEX_BIN")
            .ok()
            .filter(|value| !value.is_empty())
            .or_else(|| {
                std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
                    .map(|path| path.join("codex"))
                    .find(|path| tokeneconomizer::plugin::paths::executable(path).unwrap_or(false))
                    .map(|path| path.to_str().unwrap().to_owned())
            })
    }
    #[cfg(unix)]
    fn wait(&mut self, seconds: f64) -> Result<Value> {
        let deadline = Instant::now() + Duration::from_secs_f64(seconds);
        loop {
            if let Some(status) = self.child.as_mut().unwrap().try_wait()? {
                return Ok(json!(status.code().unwrap_or(-9)));
            }
            if Instant::now() >= deadline {
                return Err(Failure::new("TimeoutExpired", "fixture process timed out"));
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
    #[cfg(unix)]
    fn output(&mut self) -> Result<Value> {
        let mut output = Vec::new();
        for reader in self.readers.drain(..) {
            output.push(String::from_utf8_lossy(&reader.join().unwrap()?).into_owned());
        }
        Ok(json!(output))
    }
}
impl Runtime for Install {
    fn call(&mut self, method: &str, p: Value) -> Result<Value> {
        let a = |i: usize| p[i].as_str().unwrap();
        match method {
            "install_path" => {
                let args = p[1].as_array().unwrap();
                let first = Path::new(args[0].as_str().unwrap());
                Ok(match a(0) {
                    "join" => json!(args.iter().fold(PathBuf::new(), |path, item| {
                        path.join(item.as_str().unwrap())
                    })),
                    "exists" => json!(first.exists()),
                    "isdir" => json!(first.is_dir()),
                    "isfile" => json!(first.is_file()),
                    "listdir" => json!(
                        fs::read_dir(first)?
                            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
                            .collect::<Vec<_>>()
                    ),
                    _ => panic!("unexpected path operation {p}"),
                })
            }
            "install_read" => Ok(json!(fs::read_to_string(a(0))?)),
            "install_write" => {
                fs::write(a(0), a(1))?;
                Ok(Value::Null)
            }
            "install_mkdtemp" => {
                let path = self.root.join("probe");
                fs::create_dir(&path)?;
                Ok(json!(path))
            }
            "install_function" if a(0) == "_rmtree_quietly" => {
                self.run("_rmtree_quietly", json!({"path":p[1][0]}))
            }
            "install_rmtree" => {
                fs::remove_dir_all(a(0))?;
                Ok(Value::Null)
            }
            #[cfg(unix)]
            "install_function" if a(0) == "codex_bin" => Self::installed()
                .map(|value| json!(value))
                .ok_or_else(|| Failure::new("RuntimeError", "codex-cli not installed")),
            #[cfg(unix)]
            "install_environment" => {
                self.environment =
                    json!({"HOME":self.root,"PATH":std::env::var("PATH").unwrap_or_default()})
                        .as_object()
                        .unwrap()
                        .clone();
                self.environment.extend(p.as_object().unwrap().clone());
                self.environment.extend(self.extra_env.clone());
                Ok(Value::Null)
            }
            #[cfg(unix)]
            "install_spawn" => {
                let args: Vec<_> = p[0]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|arg| arg.as_str().unwrap())
                    .collect();
                let mut command = Command::new(args[0]);
                let capture = p[1].as_bool().unwrap();
                command
                    .args(&args[1..])
                    .env_clear()
                    .envs(
                        self.environment
                            .iter()
                            .map(|(key, value)| (key, value.as_str().unwrap())),
                    )
                    .current_dir(&self.root)
                    .stdin(Stdio::null())
                    .stdout(if capture {
                        Stdio::piped()
                    } else {
                        Stdio::null()
                    })
                    .stderr(if capture {
                        Stdio::piped()
                    } else {
                        Stdio::null()
                    });
                // The SDK creates a session so native cleanup can kill the CLI and its children.
                unsafe {
                    command.pre_exec(|| {
                        if libc::setsid() < 0 {
                            Err(io::Error::last_os_error())
                        } else {
                            Ok(())
                        }
                    });
                }
                let mut child = command.spawn()?;
                if capture {
                    let stdout = child.stdout.take().unwrap();
                    let stderr = child.stderr.take().unwrap();
                    self.readers = vec![
                        thread::spawn(move || {
                            let mut bytes = Vec::new();
                            let mut stream = stdout;
                            stream.read_to_end(&mut bytes)?;
                            Ok(bytes)
                        }),
                        thread::spawn(move || {
                            let mut bytes = Vec::new();
                            let mut stream = stderr;
                            stream.read_to_end(&mut bytes)?;
                            Ok(bytes)
                        }),
                    ];
                }
                self.child = Some(child);
                Ok(Value::Null)
            }
            #[cfg(unix)]
            "install_poll" => Ok(self
                .child
                .as_mut()
                .unwrap()
                .try_wait()?
                .map(|status| json!(status.code().unwrap_or(-9)))
                .unwrap_or(Value::Null)),
            #[cfg(unix)]
            "install_clock" => Ok(json!(self.clock.elapsed().as_secs_f64())),
            #[cfg(unix)]
            "install_sleep" => {
                thread::sleep(Duration::from_secs_f64(p[0].as_f64().unwrap()));
                Ok(Value::Null)
            }
            #[cfg(unix)]
            "install_communicate" => {
                self.wait(p[0].as_f64().unwrap())?;
                self.output()
            }
            #[cfg(unix)]
            "install_cleanup_process" => {
                self.run("_kill_process_group", json!({}))?;
                self.output()?;
                Ok(Value::Null)
            }
            #[cfg(unix)]
            "install_killpg" => {
                let pid = self.child.as_ref().unwrap().id() as i32;
                // This PID identifies the session created by install_spawn.
                if unsafe { libc::kill(-pid, libc::SIGKILL) } < 0 {
                    let error = io::Error::last_os_error();
                    return Err(Failure::new(
                        match error.raw_os_error() {
                            Some(libc::ESRCH) => "ProcessLookupError",
                            Some(libc::EPERM) => "PermissionError",
                            _ => "OSError",
                        },
                        error.to_string(),
                    ));
                }
                Ok(Value::Null)
            }
            #[cfg(unix)]
            "install_wait" => self.wait(p[0].as_f64().unwrap()),
            _ => panic!("unexpected installation callback {method}: {p}"),
        }
    }
}
#[cfg(unix)]
impl Drop for Install {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            // Failed assertions must also remove the fixture's descendant processes.
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            let _ = child.wait();
        }
        let _ = self.output();
    }
}
