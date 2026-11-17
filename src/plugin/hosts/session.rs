use super::session_id;
use crate::{
    plugin::{paths, process},
    trace::{
        antigravity, calls, claude, codex, copilot, cursor, droid, grok, hermes, opencode, pi,
    },
    value::{float_or_zero, scalar_text, truthy},
};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    fs::{self, File},
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::LazyLock,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Invalid(String),
    Later(String),
    Unimplemented,
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => e.fmt(f),
            Self::Invalid(e) | Self::Later(e) => e.fmt(f),
            Self::Unimplemented => f.write_str("host does not provide session usage"),
        }
    }
}
impl std::error::Error for Error {}
impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<String> for Error {
    fn from(e: String) -> Self {
        Self::Invalid(e)
    }
}
pub type Result<T> = std::result::Result<T, Error>;

pub struct Environment {
    pub env: Map<String, Value>,
    pub home: PathBuf,
    pub cwd: PathBuf,
}
impl Environment {
    pub fn path(&self, path: &Path) -> PathBuf {
        if path.is_absolute() || path.as_os_str().is_empty() {
            path.into()
        } else {
            self.cwd.join(path)
        }
    }
}
pub struct Limits {
    pub export_tries: i64,
    pub export_wait: f64,
    pub fold_retry: f64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            export_tries: 3,
            export_wait: 2.0,
            fold_retry: 7.0 * 86400.0,
        }
    }
}
pub trait Runtime {
    fn program(&mut self, cli: &str) -> Result<Vec<String>>;
    fn run(
        &mut self,
        argv: &[String],
        cwd: &Path,
        env: Option<&Map<String, Value>>,
        timeout: f64,
    ) -> io::Result<process::Output> {
        if let Some(env) = env {
            process::run_environment(argv, cwd, env, timeout)
        } else {
            process::run(argv, cwd, timeout, false)
        }
    }
    fn sleep(&mut self, seconds: f64) -> Result<()> {
        thread::sleep(
            Duration::try_from_secs_f64(seconds).map_err(|e| Error::Invalid(e.to_string()))?,
        );
        Ok(())
    }
    fn ancestors(&mut self, limit: i64, pid: u32, windows: bool, cwd: &Path)
    -> Result<Vec<String>>;
    fn now(&mut self) -> Result<f64> {
        Ok(SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64())
    }
}
pub fn copilot_home(env: &Map<String, Value>, home: &Path) -> PathBuf {
    env.get("COPILOT_HOME")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".copilot"))
}
pub fn hermes_home(env: &Map<String, Value>, home: &Path) -> PathBuf {
    env.get("HERMES_HOME")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".hermes"))
}
pub fn cwd(host: &str, p: &Value, env: &Value, current: &Path) -> Value {
    let roots = match host {
        "cursor" => &p["workspace_roots"],
        "antigravity" => &p["workspacePaths"],
        _ => &Value::Null,
    };
    let root = roots
        .as_array()
        .and_then(|a| a.first())
        .filter(|v| truthy(v));
    let key = match host {
        "cursor" => "CURSOR_PROJECT_DIR",
        "copilot" => "COPILOT_PROJECT_DIR",
        _ => "CLAUDE_PROJECT_DIR",
    };
    p.get("cwd")
        .filter(|v| truthy(v))
        .or_else(|| {
            if host == "antigravity" {
                root
            } else {
                env.get(key).filter(|v| truthy(v)).or(root)
            }
        })
        .cloned()
        .unwrap_or_else(|| json!(current))
}
pub fn transcript(host: &str, p: &Value, environment: &Environment) -> Result<Value> {
    if truthy(&p["transcript_path"]) {
        return Ok(p["transcript_path"].clone());
    }
    if host == "antigravity" {
        return Ok(p
            .get("transcriptPath")
            .filter(|v| truthy(v))
            .cloned()
            .unwrap_or(json!("")));
    }
    let sid = session_id(host, p);
    Ok(match host {
        "cursor" if !sid.is_empty() => {
            let safe = basename(&sid);
            json!(
                paths::state_root(&environment.env, &environment.home)?
                    .join("cursor-sessions")
                    .join(format!("{safe}.jsonl"))
            )
        }
        "copilot" if !sid.is_empty() => json!(
            copilot_home(&environment.env, &environment.home)
                .join("session-state")
                .join(sid)
                .join("events.jsonl")
        ),
        _ => json!(""),
    })
}
pub(super) fn basename(path: &str) -> &str {
    if cfg!(windows) {
        path.rsplit(['\\', '/']).next().unwrap_or("")
    } else {
        path.rsplit('/').next().unwrap_or("")
    }
}
pub fn headless(
    host: &str,
    env: &Value,
    transcript: Option<&str>,
    environment: &Environment,
    parent: u32,
    windows: bool,
    runtime: &mut dyn Runtime,
) -> Result<bool> {
    let text = |key| env.get(key).map(scalar_text).unwrap_or_default();
    Ok(match host {
        "claude" => match text("CLAUDE_CODE_SESSION_ATTENDED").trim() {
            "0" => true,
            "1" => false,
            _ => text("CLAUDE_CODE_ENTRYPOINT").starts_with("sdk"),
        },
        "pi" => ["print", "json", "rpc"].contains(&text("TE_PI_MODE").trim()),
        "opencode" => text("TE_OPENCODE_MODE").trim() == "run",
        "hermes" => text("TE_HERMES_MODE").trim() == "oneshot",
        "codex" => {
            let record = transcript
                .filter(|s| !s.is_empty())
                .and_then(|p| File::open(environment.path(Path::new(p))).ok())
                .and_then(|file| BufReader::new(file).lines().next())
                .and_then(|line| line.ok())
                .and_then(|line| serde_json::from_str::<Value>(&line).ok());
            record.is_some_and(|r| {
                r["payload"]["source"] == "exec" || r["payload"]["originator"] == "codex_exec"
            })
        }
        "copilot" => {
            static PROMPT_FLAG: LazyLock<Regex> =
                LazyLock::new(|| Regex::new(r"\s(?:-p|--prompt)(?:[\s=]|$)").unwrap());
            runtime
                .ancestors(4, parent, windows, &environment.cwd)?
                .iter()
                .any(|line| {
                    line.split(' ').next().unwrap_or("").contains("copilot")
                        && PROMPT_FLAG.is_match(line)
                })
        }
        _ => false,
    })
}
pub fn ancestor_args(
    limit: i64,
    mut pid: u32,
    windows: bool,
    cwd: &Path,
    runtime: &mut dyn Runtime,
) -> Result<Vec<String>> {
    let mut out = Vec::new();
    if windows {
        return Ok(out);
    }
    for _ in 0..limit {
        if pid <= 1 {
            break;
        }
        let output = match runtime.run(
            &[
                "ps".into(),
                "-o".into(),
                "ppid=,args=".into(),
                "-p".into(),
                pid.to_string(),
            ],
            cwd,
            None,
            2.0,
        ) {
            Ok(out) => out,
            Err(_) => break,
        };
        let line = output.stdout.trim();
        let Some((parent, args)) = line.split_once(' ').filter(|(_, args)| !args.is_empty()) else {
            break;
        };
        out.push(args.trim().into());
        pid = parent.parse().unwrap_or(0);
    }
    Ok(out)
}
pub fn mechanism_trace(
    host: &str,
    transcript: Option<&str>,
    workdir: &Path,
    environment: &Environment,
) -> Result<Option<PathBuf>> {
    let input = environment.path(Path::new(transcript.unwrap_or("")));
    let output = |filename: &str| environment.path(&workdir.join(filename));
    let (filename, n) = match host {
        "codex" => (
            "codex_exec_trace.jsonl",
            codex::rollout_to_exec(&input, &output("codex_exec_trace.jsonl"))?,
        ),
        "pi" => (
            "pi_trace.jsonl",
            pi::session_to_claude(&input, &output("pi_trace.jsonl"))?,
        ),
        "opencode" => (
            "opencode_trace.jsonl",
            opencode::session_to_claude(&input, &output("opencode_trace.jsonl"))?,
        ),
        "cursor" => (
            "cursor_trace.jsonl",
            cursor::events_to_claude(&input, &output("cursor_trace.jsonl")),
        ),
        "hermes" => (
            "hermes_trace.jsonl",
            hermes::session_to_claude(&input, &output("hermes_trace.jsonl"))?,
        ),
        "antigravity" => (
            "antigravity_trace.jsonl",
            antigravity::session_to_claude(&input, &output("antigravity_trace.jsonl")),
        ),
        "grok" => (
            "grok_trace.jsonl",
            if transcript.is_some_and(|s| !s.is_empty()) {
                grok::session_to_claude(&input, &output("grok_trace.jsonl"))?
            } else {
                None
            },
        ),
        "droid" => (
            "droid_trace.jsonl",
            droid::session_to_claude(&input, &output("droid_trace.jsonl"))?,
        ),
        _ => {
            return Ok(transcript
                .filter(|s| !s.is_empty() && input.exists())
                .map(PathBuf::from));
        }
    };
    Ok(n.map(|_| workdir.join(filename)))
}
pub fn prompt_count(host: &str, path: &Path) -> Option<usize> {
    Some(match host {
        "claude" => claude::prompt_count(path),
        "codex" => codex::prompt_count(path),
        "copilot" => copilot::prompt_count(path),
        "pi" => pi::prompt_count(path),
        "opencode" => opencode::prompt_count(path),
        "cursor" => cursor::prompt_count(path),
        "hermes" => hermes::prompt_count(path),
        "antigravity" => antigravity::prompt_count(path),
        "grok" => grok::prompt_count(path),
        "droid" => droid::prompt_count(path),
        _ => return None,
    })
}
pub fn session_usage(host: &str, path: &Path) -> Result<Value> {
    Ok(match host {
        "claude" => claude::session_usage(path, true)?,
        "codex" => codex::session_usage(path)?,
        "copilot" => copilot::session_usage(path)?,
        "pi" => pi::session_usage(path)?,
        "opencode" => opencode::session_usage(path)?,
        "cursor" => json!({}),
        "hermes" => hermes::session_usage(path)?,
        "antigravity" => antigravity::session_usage(path),
        "grok" => grok::session_usage(path)?,
        "droid" => droid::session_usage(path),
        _ => return Err(Error::Unimplemented),
    })
}
pub fn tool_usage(host: &str, path: &Path) -> Result<BTreeMap<String, usize>> {
    Ok(if host == "claude" {
        claude::tool_usage(&paths::realpath(path)?, true)
    } else {
        calls::tool_usage(path)
    })
}
pub fn modified(path: &Path) -> io::Result<f64> {
    Ok(fs::metadata(path)?
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or_else(|e| -e.duration().as_secs_f64()))
}
pub fn last_active(host: &str, path: &Path, session: &Value) -> Result<f64> {
    if host == "hermes" {
        return float_or_zero(
            session
                .get("updated")
                .filter(|v| truthy(v))
                .unwrap_or(&session["started"]),
        )
        .map_err(Error::Invalid);
    }
    Ok(modified(path)?)
}
pub fn session_state(host: &str, env: &Map<String, Value>, home: &Path) -> Value {
    if host == "hermes" {
        json!({"hermes_home":hermes_home(env,home)})
    } else {
        json!({})
    }
}
pub fn settle(
    host: &str,
    path: Option<&Path>,
    wait: f64,
    fresh: f64,
    runtime: &mut dyn Runtime,
) -> Result<()> {
    let Some(path) = path.filter(|p| !p.as_os_str().is_empty()) else {
        return Ok(());
    };
    if host == "cursor" {
        let real = paths::realpath(path)?;
        if real
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|name| name == "cursor-sessions")
        {
            let _ = fs::remove_file(path);
        }
    } else if host == "copilot" {
        let deadline = runtime.now()? + wait;
        while runtime.now()? < deadline {
            let check = (|| -> Result<bool> {
                if runtime.now()? - modified(path)? > fresh {
                    return Ok(true);
                }
                let mut file = File::open(path)?;
                let len = file.metadata()?.len();
                file.seek(SeekFrom::Start(len.saturating_sub(65536)))?;
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes)?;
                Ok(bytes
                    .windows(b"\"session.shutdown\"".len())
                    .any(|part| part == b"\"session.shutdown\""))
            })();
            match check {
                Ok(true) | Err(Error::Io(_)) => return Ok(()),
                Err(e) => return Err(e),
                _ => {}
            }
            runtime.sleep(0.25)?;
        }
    }
    Ok(())
}
pub fn history(
    host: &str,
    env: &Map<String, Value>,
    extra: &[String],
    environment: &Environment,
) -> Result<Vec<Value>> {
    if host == "droid" {
        let root = env
            .get("DROID_SESSIONS_DIR")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| environment.home.join(".factory/sessions"));
        return Ok(droid::sessions(&environment.path(&root)).into_iter().filter(|r|!truthy(&r["parent"])).map(|r|json!({"session_id":r["id"],"folder":r["folder"],"path":r["path"],"mtime":r["mtime"],"size":r["size"]})).collect());
    }
    if host != "claude" {
        return Ok(Vec::new());
    }
    let mut dirs = vec![claude::projects_dir(&json!(env), &environment.home)];
    for dir in extra {
        // shortcut: expand the current user's tilde, cover named users and Windows forms in the path parity audit.
        let path = if dir == "~" {
            environment.home.clone()
        } else if let Some(rest) = dir.strip_prefix("~/") {
            environment.home.join(rest)
        } else {
            PathBuf::from(dir)
        };
        dirs.push(if environment.path(&path.join("projects")).is_dir() {
            path.join("projects")
        } else {
            path
        });
    }
    let mut seen = BTreeSet::new();
    let mut seen_dirs = BTreeSet::new();
    let mut out = Vec::new();
    for dir in dirs {
        let dir = environment.path(&dir);
        if !seen_dirs.insert(paths::realpath(&dir)?) {
            continue;
        }
        for (folder, path, mtime, size) in claude::history_logs(&dir) {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            let sid = name.strip_suffix(".jsonl").unwrap_or(&name);
            if seen.insert(sid.to_string()) {
                out.push(
                    json!({"session_id":sid,"folder":folder,"path":path,"mtime":mtime,"size":size}),
                );
            }
        }
    }
    Ok(out)
}
pub fn folder_matches(host: &str, root: &Path, folder: &Value) -> Result<bool> {
    let absolute = std::path::absolute(root)?;
    Ok(if host == "droid" {
        folder == &json!(absolute.to_string_lossy().replace(['\\', '/'], "-"))
    } else {
        folder == &json!(claude::project_folder(&root.to_string_lossy()))
            || folder
                == &json!(claude::project_folder(
                    &paths::realpath(root)?.to_string_lossy()
                ))
    })
}
pub fn log_head(host: &str, path: &Path) -> Value {
    if host == "droid" {
        droid::log_head(path)
    } else {
        claude::log_head(path)
    }
}
pub fn safe_session_id<'a>(host: &str, sid: &'a Value) -> Option<&'a str> {
    if host == "droid" {
        droid::safe_session_id(sid)
    } else {
        claude::safe_session_id(sid)
    }
}
pub fn prompt_turns(host: &str, path: &Path) -> Vec<Vec<String>> {
    if host == "droid" {
        droid::prompt_turns(path).collect()
    } else {
        claude::prompt_turns(path).collect()
    }
}
pub fn turn_calls(lines: &[String]) -> Vec<claude::Call> {
    claude::calls(lines)
}
pub fn turn_interrupted(lines: &[String]) -> bool {
    claude::turn_interrupted(lines)
}
pub fn turn_answered(lines: &[String]) -> bool {
    claude::turn_answered(lines)
}

#[allow(clippy::too_many_arguments)]
pub fn complete(
    host: &str,
    transcript: Option<&str>,
    sid: &str,
    cwd: Option<&Path>,
    session: &Value,
    environment: &Environment,
    limits: &Limits,
    runtime: &mut dyn Runtime,
) -> Result<Value> {
    let original = transcript.map(|s| json!(s)).unwrap_or(Value::Null);
    let safe = {
        static SAFE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\A[\w-]+\z").unwrap());
        SAFE.is_match(sid)
    };
    if host == "hermes" {
        if !safe {
            return Ok(original);
        }
        let path = if let Some(tp) = transcript.filter(|s| !s.is_empty()) {
            PathBuf::from(tp)
        } else {
            paths::state_root(&environment.env, &environment.home)?
                .join("hermes-sessions")
                .join(format!("{sid}.json"))
        };
        let home = session["hermes_home"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| hermes_home(&environment.env, &environment.home));
        let output = environment.path(&path);
        if hermes::export(&environment.path(&home.join("state.db")), sid, &output)
            || output.exists()
        {
            return Ok(json!(path));
        }
        if runtime.now()? - last_active(host, &output, session)? < limits.fold_retry {
            return Err(Error::Later(sid.into()));
        }
        return Ok(original);
    }
    if host != "opencode"
        || opencode::prompt_count(&environment.path(Path::new(transcript.unwrap_or("")))) > 0
        || !safe
    {
        return Ok(original);
    }
    let path = if let Some(path) = transcript.filter(|s| !s.is_empty()) {
        PathBuf::from(path)
    } else {
        paths::state_root(&environment.env, &environment.home)?
            .join("opencode-sessions")
            .join(format!("{sid}.json"))
    };
    let output_path = environment.path(&path);
    let tmp = PathBuf::from(format!(
        "{}.{}.tmp",
        output_path.to_string_lossy(),
        std::process::id()
    ));
    let mut env = environment.env.clone();
    env.insert("TE_SIDECALL".into(), json!("1"));
    let cwd = cwd
        .map(|p| environment.path(p))
        .filter(|p| p.is_dir())
        .unwrap_or_else(|| environment.cwd.clone());
    for attempt in 0..limits.export_tries {
        if attempt > 0 {
            runtime.sleep(limits.export_wait)?;
        }
        let mut result = None;
        for args in [&["session", "export"][..], &["export"][..]] {
            let mut cmd = runtime.program("opencode")?;
            cmd.extend(args.iter().map(|s| s.to_string()));
            cmd.push(sid.into());
            let out = match runtime.run(&cmd, &cwd, Some(&env), 60.0) {
                Ok(out) => out,
                Err(_) => return Ok(original),
            };
            if out.code == 0 && out.stdout.trim_start().starts_with('{') {
                result = Some(out);
                break;
            }
        }
        let Some(out) = result else {
            continue;
        };
        let write = (|| -> io::Result<bool> {
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&tmp, out.stdout)?;
            if opencode::prompt_count(&tmp) > 0 {
                fs::rename(&tmp, &output_path)?;
                return Ok(true);
            }
            Ok(false)
        })();
        let _ = fs::remove_file(&tmp);
        match write {
            Ok(true) => return Ok(json!(path)),
            Err(_) => return Ok(original),
            _ => {}
        }
    }
    Ok(original)
}
