use super::{hosts::session::Environment, paths, process};
use crate::{
    accounting,
    value::{scalar_text, truthy},
};
use chrono::Local;
use regex::Regex;
use serde_json::{Map, Value, json};
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        LazyLock,
        atomic::{AtomicU64, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

pub const TIMEOUT_S: u64 = 180;
pub const FM_CLI: &str = "/usr/bin/fm";
pub const FM_TIMEOUT_S: u64 = 60;
pub const NO_SIDE_CALLS: [&str; 1] = ["antigravity"];

pub trait Runtime {
    fn temporary_directory(
        &mut self,
        _prefix: &str,
        _ignore_cleanup_errors: bool,
    ) -> Result<Option<PathBuf>, String> {
        Ok(None)
    }
    fn remove_temporary_directory(&mut self, _path: &Path) -> Result<(), String> {
        Ok(())
    }
    fn which(&mut self, cli: &str) -> Result<Option<String>, String> {
        Ok(process::which(cli).map(|p| p.to_string_lossy().into_owned()))
    }
    fn claude_cli(&mut self, environment: &Environment, windows: bool) -> Result<String, String> {
        installed_cli("claude", environment, windows, self)
    }
    fn run(
        &mut self,
        argv: &[String],
        cwd: &Path,
        env: &Map<String, Value>,
        input: Option<&str>,
        timeout: f64,
    ) -> io::Result<process::Output> {
        match input {
            Some(input) => process::run_input(argv, cwd, env, input, timeout),
            None => process::run_environment(argv, cwd, env, timeout),
        }
    }
}
static SHIM_TARGET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""%~?dp0%?\\([^\"]+)"\s*%\*"#).unwrap());
static SHIM_PROG: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"SET "_prog=([^"%]+)""#).unwrap());
static SIGNED_OUT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"To authenticate|/login\b|\b[Nn]ot (?:logged|signed) in").unwrap()
});

pub fn shim_argv(
    path: &Path,
    environment: &Environment,
    runtime: &mut dyn Runtime,
) -> Result<Option<Vec<String>>, String> {
    let text = match fs::read(environment.path(path)) {
        Ok(text) => String::from_utf8_lossy(&text).into_owned(),
        Err(_) => return Ok(None),
    };
    let Some(target) = SHIM_TARGET.captures_iter(&text).last() else {
        return Ok(None);
    };
    let target = paths::normalize(
        &path
            .parent()
            .unwrap_or(Path::new(""))
            .join(target[1].replace('\\', std::path::MAIN_SEPARATOR_STR)),
    );
    if !environment.path(&target).is_file() {
        return Ok(None);
    }
    let target_text = target.to_string_lossy().into_owned();
    if target_text.to_lowercase().ends_with(".exe") {
        return Ok(Some(vec![target_text]));
    }
    let prog = SHIM_PROG.captures(&text);
    let name = prog.as_ref().map(|m| &m[1]).unwrap_or("node");
    let beside = path
        .parent()
        .unwrap_or(Path::new(""))
        .join(format!("{name}.exe"));
    let program = if environment.path(&beside).is_file() {
        Some(beside.to_string_lossy().into_owned())
    } else {
        runtime.which(name)?
    };
    Ok(program.map(|p| vec![p, target_text]))
}
pub fn program(
    cli: &str,
    windows: bool,
    environment: &Environment,
    runtime: &mut dyn Runtime,
) -> Result<Vec<String>, String> {
    if !windows {
        return Ok(vec![cli.into()]);
    }
    let Some(found) = runtime.which(cli)?.filter(|s| !s.is_empty()) else {
        return Ok(vec![cli.into()]);
    };
    let batch = Path::new(&found).extension().is_some_and(|s| {
        matches!(
            s.to_str().unwrap_or("").to_lowercase().as_str(),
            "cmd" | "bat"
        )
    });
    if !batch {
        return Ok(vec![found]);
    }
    shim_argv(Path::new(&found), environment, runtime)?.ok_or_else(|| {
        format!(
            "{found}: a batch file that is not an npm shim, whose arguments cmd.exe would parse again"
        )
    })
}
pub fn installed_cli<R: Runtime + ?Sized>(
    name: &str,
    environment: &Environment,
    windows: bool,
    runtime: &mut R,
) -> Result<String, String> {
    if let Some(found) = runtime.which(name)? {
        return Ok(found);
    }
    let filename = if name == "claude" && windows {
        "claude.exe"
    } else {
        name
    };
    let path = environment.home.join(".local/bin").join(filename);
    Ok(if environment.path(&path).is_file() {
        path.to_string_lossy().into_owned()
    } else {
        name.into()
    })
}
pub fn pause_path(host: &str, root: &Path) -> PathBuf {
    root.join("hosts").join(host).join("sidecall.json")
}
pub fn paused(host: &str, root: &Path) -> Value {
    fs::read(pause_path(host, root))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .filter(|v| v.is_object() && truthy(&v["at"]))
        .unwrap_or(Value::Null)
}
pub fn resume(host: &str, root: &Path) -> bool {
    fs::remove_file(pause_path(host, root)).is_ok()
}
pub fn log(root: &Path, text: &str) {
    let _ = (|| -> io::Result<()> {
        fs::create_dir_all(root)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(root.join("errors.log"))?;
        write!(
            file,
            "--- {} {text}",
            Local::now().format("%Y-%m-%dT%H:%M:%S")
        )
    })();
}
pub fn no_reply(host: &str, root: &Path, stdout: &str, stderr: &str, code: &Value, now: f64) {
    let combined = format!("{stderr} {stdout}")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let tail = combined
        .chars()
        .skip(combined.chars().count().saturating_sub(300))
        .collect::<String>();
    log(
        root,
        &format!(
            "sidecall {host}: no reply (exit {}) {tail}\n",
            scalar_text(code)
        ),
    );
    if SIGNED_OUT.is_match(&format!("{stderr}{stdout}")) {
        let _ = (|| -> io::Result<()> {
            let path = pause_path(host, root);
            fs::create_dir_all(path.parent().unwrap())?;
            let reason = tail
                .chars()
                .skip(tail.chars().count().saturating_sub(200))
                .collect::<String>();
            fs::write(
                path,
                serde_json::to_vec(&json!({"at":now,"reason":reason}))?,
            )
        })();
    }
}
pub fn environment(env: &Map<String, Value>, hermes: bool) -> Map<String, Value> {
    let mut env = env.clone();
    if hermes {
        for key in ["TE_SIDECALL", "TE_HERMES_MODE", "PYTHONPATH"] {
            env.remove(key);
        }
    } else {
        env.insert("TE_SIDECALL".into(), json!("1"));
    }
    env
}
pub fn claude_json(stdout: &str) -> Value {
    let Ok(data) = serde_json::from_str::<Value>(stdout) else {
        return json!({});
    };
    match data {
        Value::Object(_) => data,
        Value::Array(events) => events
            .into_iter()
            .rev()
            .find(|ev| ev.is_object() && ev["type"] == "result")
            .unwrap_or(json!({})),
        _ => json!({}),
    }
}
fn setting<'a>(env: &'a Map<String, Value>, key: &str) -> &'a str {
    env.get(key).and_then(Value::as_str).unwrap_or("")
}
pub fn copilot_model(paid_model: &str, env: &Map<String, Value>) -> String {
    if !paid_model.is_empty() {
        return paid_model.into();
    }
    for key in ["COPILOT_PROVIDER_MODEL_ID", "COPILOT_MODEL"] {
        let value = setting(env, key).trim();
        if !value.is_empty() && value.to_lowercase() != "auto" {
            return value.into();
        }
    }
    let base = setting(env, "COPILOT_PROVIDER_BASE_URL");
    if base.contains("openrouter.ai") {
        "anthropic/claude-haiku-4.5".into()
    } else if !base.is_empty()
        && setting(env, "COPILOT_PROVIDER_TYPE").to_lowercase() == "anthropic"
    {
        "claude-haiku-4-5".into()
    } else {
        String::new()
    }
}
pub fn droid_enabled(env: &Map<String, Value>) -> bool {
    let v = env
        .get("TE_DROID_SIDECALL")
        .filter(|v| truthy(v))
        .map(scalar_text)
        .unwrap_or_default();
    matches!(
        v.trim().to_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}
pub fn droid_usage(data: &Value) -> Value {
    if !data.is_object() {
        return Value::Null;
    }
    let mut usage = json!({"model":data["model"]});
    for (camel, snake) in [
        ("inputTokens", "input_tokens"),
        ("cacheReadTokens", "cache_read_input_tokens"),
        ("cacheCreationTokens", "cache_creation_input_tokens"),
        ("outputTokens", "output_tokens"),
    ] {
        usage[snake] = data.get(snake).unwrap_or(&data[camel]).clone();
    }
    if usage["input_tokens"].is_null() {
        Value::Null
    } else {
        usage
    }
}
pub fn local_routes(role: &Value, config: &Value, macos: bool) -> bool {
    macos
        && config["local_model"] == "fm"
        && config["local_roles"]
            .as_array()
            .is_some_and(|roles| roles.contains(role))
}
pub fn call_name(host: &str) -> String {
    match host {
        "codex" => "codex exec".into(),
        "droid" => "droid exec".into(),
        "opencode" => "opencode run".into(),
        "cursor" => "cursor-agent -p".into(),
        "hermes" => "hermes chat".into(),
        _ => format!("{host} -p"),
    }
}

pub fn claude_help(
    cli: &str,
    cache: &mut Map<String, Value>,
    environment: &Environment,
    runtime: &mut dyn Runtime,
) -> String {
    if !cache.contains_key(cli) {
        let help = runtime
            .run(
                &[cli.into(), "--help".into()],
                &environment.cwd,
                &environment.env,
                None,
                30.0,
            )
            .map(|out| out.stdout)
            .unwrap_or_default();
        cache.insert(cli.into(), json!(help));
    }
    cache[cli].as_str().unwrap_or("").into()
}
pub fn claude_flags(help: &str) -> Vec<String> {
    if help.contains("--no-session-persistence") {
        vec!["--no-session-persistence".into()]
    } else {
        Vec::new()
    }
}
pub fn claude_no_mcp(help: &str, cwd: &Path) -> io::Result<Vec<String>> {
    if !help.contains("--strict-mcp-config") {
        return Ok(Vec::new());
    }
    let path = cwd.join("mcp.json");
    fs::write(&path, "{\"mcpServers\": {}}")?;
    Ok(vec![
        "--strict-mcp-config".into(),
        "--mcp-config".into(),
        path.to_string_lossy().into_owned(),
    ])
}
struct WorkDir(PathBuf);
impl WorkDir {
    fn new(prefix: &str) -> io::Result<Self> {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = std::env::temp_dir().join(format!(
                "{prefix}{}-{}-{}",
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
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn with_work_dir<T>(
    prefix: &str,
    runtime: &mut dyn Runtime,
    call: impl FnOnce(&Path, &mut dyn Runtime) -> Result<T, String>,
) -> Result<T, String> {
    let external = runtime.temporary_directory(prefix, true)?;
    let owned = if external.is_none() {
        Some(WorkDir::new(prefix).map_err(|e| e.to_string())?)
    } else {
        None
    };
    let path = external
        .as_deref()
        .unwrap_or_else(|| &owned.as_ref().unwrap().0);
    let result = call(path, runtime);
    if external.is_some() {
        runtime.remove_temporary_directory(path)?;
    }
    result
}
pub struct ClaudeCall<'a> {
    pub prompt: &'a str,
    pub model: &'a str,
    pub cli: &'a str,
    pub timeout: f64,
    pub effort: &'a str,
    pub windows: bool,
}
pub fn claude_call(
    request: &ClaudeCall<'_>,
    environment: &Environment,
    root: &Path,
    cache: &mut Map<String, Value>,
    runtime: &mut dyn Runtime,
) -> Result<(Value, Option<f64>), String> {
    let cli = if request.cli.is_empty() {
        runtime.claude_cli(environment, request.windows)?
    } else {
        request.cli.into()
    };
    let out = with_work_dir("te-side-", runtime, |cwd, runtime| {
        let help = claude_help(&cli, cache, environment, runtime);
        let mut cmd: Vec<String> = [
            &cli,
            "-p",
            "--output-format",
            "json",
            "--max-turns",
            "1",
            "--tools",
            "",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        cmd.extend(claude_no_mcp(&help, cwd).map_err(|e| e.to_string())?);
        cmd.extend([
            "--model".into(),
            if request.model.is_empty() {
                "haiku".into()
            } else {
                request.model.into()
            },
        ]);
        cmd.extend(claude_flags(&help));
        if !request.effort.is_empty() && help.contains("--effort") {
            cmd.extend(["--effort".into(), request.effort.into()]);
        }
        runtime
            .run(
                &cmd,
                cwd,
                &self::environment(&environment.env, false),
                Some(request.prompt),
                request.timeout,
            )
            .map_err(|e| e.to_string())
    })?;
    let data = claude_json(&out.stdout);
    let failed = truthy(&data["is_error"]) || !truthy(&data["result"]);
    if failed {
        no_reply(
            "claude",
            root,
            &out.stdout,
            &out.stderr,
            &json!(out.code),
            super::history::now(),
        );
    }
    let mut usage = data["usage"].as_object().cloned().unwrap_or_default();
    let model = data["modelUsage"]
        .as_object()
        .and_then(|models| models.keys().next())
        .filter(|m| !m.is_empty())
        .map(String::as_str)
        .unwrap_or(if request.model.is_empty() {
            "claude"
        } else {
            request.model
        });
    usage.insert("model".into(), json!(model));
    let (eff, _) = accounting::effective(&json!(usage)).map_err(|e| e.to_string())?;
    Ok((
        if failed {
            json!("")
        } else {
            data["result"].clone()
        },
        eff,
    ))
}
