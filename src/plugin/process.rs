use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{LazyLock, mpsc},
    thread,
    time::{Duration, Instant},
};

use super::paths::executable;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Serialize, Deserialize)]
pub struct Launch {
    pub cmd: Vec<String>,
    pub cwd: PathBuf,
    pub env: Map<String, Value>,
    pub log: PathBuf,
}

pub struct Output {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub fn run(argv: &[String], cwd: &Path, timeout: f64, sidecall: bool) -> io::Result<Output> {
    let mut command = command(argv, cwd, None, 0)?;
    if sidecall {
        command.env("TE_SIDECALL", "1");
    }
    capture(command, timeout)
}

fn command(
    argv: &[String],
    cwd: &Path,
    env: Option<&Map<String, Value>>,
    flags: u32,
) -> io::Result<Command> {
    let mut command = Command::new(
        argv.first()
            .ok_or_else(|| io::Error::other("empty command"))?,
    );
    command.args(&argv[1..]).current_dir(cwd);
    if let Some(env) = env {
        command.env_clear();
        for (key, value) in env {
            command.env(
                key,
                value.as_str().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "process environment values must be strings",
                    )
                })?,
            );
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(flags);
    }
    #[cfg(not(windows))]
    let _ = flags;
    Ok(command)
}

pub fn run_configured(argv: &[String], cwd: &Path, timeout: f64, flags: u32) -> io::Result<Output> {
    capture(command(argv, cwd, None, flags)?, timeout)
}
pub fn run_environment(
    argv: &[String],
    cwd: &Path,
    env: &Map<String, Value>,
    timeout: f64,
) -> io::Result<Output> {
    capture(command(argv, cwd, Some(env), 0)?, timeout)
}

pub fn spawn_logged(
    argv: &[String],
    cwd: &Path,
    env: &Map<String, Value>,
    log: &Path,
    flags: u32,
    new_session: bool,
) -> io::Result<()> {
    let mut command = logged(argv, cwd, env, log, flags)?;
    #[cfg(unix)]
    if new_session {
        use std::os::unix::process::CommandExt;
        // Start a session before exec so the child leaves its hook's session.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
    }
    #[cfg(not(unix))]
    let _ = new_session;
    let mut child = command.spawn()?;
    thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}
pub fn call_logged(
    argv: &[String],
    cwd: &Path,
    env: &Map<String, Value>,
    log: &Path,
    flags: u32,
) -> io::Result<i32> {
    Ok(exit_code(logged(argv, cwd, env, log, flags)?.status()?))
}
fn logged(
    argv: &[String],
    cwd: &Path,
    env: &Map<String, Value>,
    log: &Path,
    flags: u32,
) -> io::Result<Command> {
    let file = fs::OpenOptions::new().append(true).create(true).open(log)?;
    let mut command = command(argv, cwd, Some(env), flags)?;
    command
        .stdin(Stdio::null())
        .stdout(file.try_clone()?)
        .stderr(file);
    Ok(command)
}
fn exit_code(status: std::process::ExitStatus) -> i32 {
    status.code().unwrap_or_else(|| {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            -status.signal().unwrap_or(1)
        }
        #[cfg(not(unix))]
        {
            -1
        }
    })
}
fn capture(mut command: Command, timeout: f64) -> io::Result<Output> {
    let timeout = Duration::try_from_secs_f64(timeout).map_err(io::Error::other)?;
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let (send, receive) = mpsc::channel();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let out_send = send.clone();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout.read_to_end(&mut bytes).map(|_| bytes);
        let _ = out_send.send((true, result));
    });
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stderr.read_to_end(&mut bytes).map(|_| bytes);
        let _ = send.send((false, result));
    });
    let started = Instant::now();
    let mut status = None;
    let (mut out, mut err): (Option<Vec<u8>>, Option<Vec<u8>>) = (None, None);
    let result = (|| -> io::Result<Output> {
        loop {
            if status.is_none() {
                status = child.try_wait()?;
            }
            if let Some(status) = status
                && let (Some(out), Some(err)) = (&out, &err)
            {
                let text = |bytes: &[u8]| {
                    String::from_utf8_lossy(bytes)
                        .replace("\r\n", "\n")
                        .replace('\r', "\n")
                };
                return Ok(Output {
                    code: exit_code(status),
                    stdout: text(out),
                    stderr: text(err),
                });
            }
            let Some(remaining) = timeout.checked_sub(started.elapsed()) else {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "command timed out"));
            };
            match receive.recv_timeout(remaining.min(Duration::from_millis(10))) {
                Ok((true, result)) => out = Some(result?),
                Ok((false, result)) => err = Some(result?),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    thread::sleep(remaining.min(Duration::from_millis(10)))
                }
            }
        }
    })();
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    result
}

pub fn which(cli: &str) -> Option<PathBuf> {
    let path = Path::new(cli);
    let valid = |p: &Path| p.is_file() && executable(p).unwrap_or(false);
    #[cfg(not(windows))]
    {
        if path.components().count() > 1 || path.is_absolute() {
            return valid(path).then(|| path.into());
        }
        std::env::split_paths(&std::env::var_os("PATH")?)
            .map(|p| p.join(cli))
            .find(|p| valid(p))
    }
    #[cfg(windows)]
    {
        let extensions = std::env::var("PATHEXT")
            .unwrap_or(".COM;.EXE;.BAT;.CMD".into())
            .split(';')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect::<Vec<_>>();
        let mut paths = if path.components().count() > 1 || path.is_absolute() {
            vec![PathBuf::new()]
        } else {
            std::env::split_paths(&std::env::var_os("PATH")?).collect()
        };
        if !paths.iter().any(|p| p == Path::new(".")) {
            paths.insert(0, PathBuf::from("."));
        }
        for dir in paths {
            let base = dir.join(cli);
            if valid(&base) {
                return Some(base);
            }
            for ext in &extensions {
                let p = PathBuf::from(format!("{}{ext}", base.display()));
                if valid(&p) {
                    return Some(p);
                }
            }
        }
        None
    }
}

pub fn shim_argv(path: &Path) -> Option<Vec<String>> {
    static TARGET: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#""%~?dp0%?\\([^"]+)"\s*%\*"#).unwrap());
    static PROGRAM: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#"SET "_prog=([^"%]+)""#).unwrap());
    let text = fs::read(path).ok()?;
    let text = String::from_utf8_lossy(&text);
    let matched = TARGET.captures_iter(&text).last()?;
    let base = path.parent()?;
    let target = base.join(matched[1].replace('\\', std::path::MAIN_SEPARATOR_STR));
    let target = super::super::store::Store::new(&target).ok()?.base;
    if !target.is_file() {
        return None;
    }
    let target = target.to_string_lossy().into_owned();
    if target.to_lowercase().ends_with(".exe") {
        return Some(vec![target]);
    }
    let name = PROGRAM
        .captures(&text)
        .map(|m| m[1].to_string())
        .unwrap_or("node".into());
    let beside = base.join(format!("{name}.exe"));
    let program = if beside.is_file() {
        beside
    } else {
        which(&name)?
    };
    Some(vec![program.to_string_lossy().into_owned(), target])
}

pub fn program(cli: &str) -> io::Result<Vec<String>> {
    if !cfg!(windows) {
        return Ok(vec![cli.into()]);
    }
    let found = which(cli);
    let Some(found) = found else {
        return Ok(vec![cli.into()]);
    };
    if !found
        .extension()
        .and_then(|s| s.to_str())
        .is_some_and(|s| ["cmd", "bat"].contains(&s.to_lowercase().as_str()))
    {
        return Ok(vec![found.to_string_lossy().into_owned()]);
    }
    shim_argv(&found).ok_or_else(||io::Error::new(io::ErrorKind::NotFound,format!("{}: a batch file that is not an npm shim, whose arguments cmd.exe would parse again",found.display())))
}
