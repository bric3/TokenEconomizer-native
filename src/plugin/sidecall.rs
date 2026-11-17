use super::{hosts::session::Environment, paths, process};
use crate::{
    accounting,
    trace::{grok, opencode, pi},
    value::{int_value, scalar_text, truthy},
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
pub const PI_FLAGS: [&str; 10] = [
    "--print",
    "--mode",
    "json",
    "--no-session",
    "--no-tools",
    "--no-extensions",
    "--no-skills",
    "--no-prompt-templates",
    "--no-context-files",
    "--offline",
];
pub const GROK_TOOLS: &str = "read_file";
pub const GROK_REMOVED: &str = "use_tool,search_tool";
pub const GROK_DENY_RULES: [&str; 6] = ["Bash", "Edit", "Write", "WebFetch", "WebSearch", "Task"];
pub const OPENCODE_CONFIG: &str = r#"{"permission": {"bash": "deny", "edit": "deny", "webfetch": "deny", "websearch": "deny"}, "share": "disabled", "autoupdate": false}"#;

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
    fn program(
        &mut self,
        cli: &str,
        windows: bool,
        environment: &Environment,
    ) -> Result<Vec<String>, String> {
        program(cli, windows, environment, self)
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

pub fn shim_argv<R: Runtime + ?Sized>(
    path: &Path,
    environment: &Environment,
    runtime: &mut R,
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
pub fn program<R: Runtime + ?Sized>(
    cli: &str,
    windows: bool,
    environment: &Environment,
    runtime: &mut R,
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
pub struct Call<'a> {
    pub prompt: &'a str,
    pub model: &'a str,
    pub cli: &'a str,
    pub timeout: f64,
    pub effort: &'a str,
    pub windows: bool,
}
pub fn claude_call(
    request: &Call<'_>,
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

fn usage_count(value: &Value) -> Result<i128, String> {
    if !truthy(value) {
        return Ok(0);
    }
    if let Some(n) = value.as_i64() {
        return Ok(n.into());
    }
    if let Some(n) = value.as_u64() {
        return Ok(n.into());
    }
    if let Some(n) = value.as_str().and_then(|s| s.trim().parse::<i128>().ok()) {
        return Ok(n);
    }
    // shortcut: optimiser usage is bounded by i128, widen it in the numeric parity audit.
    int_value(value)
        .filter(|n| *n >= i128::MIN as f64 && *n < -(i128::MIN as f64))
        .map(|n| n as i128)
        .ok_or_else(|| format!("invalid optimiser usage: {value}"))
}
pub fn codex_call(
    request: &Call<'_>,
    environment: &Environment,
    root: &Path,
    runtime: &mut dyn Runtime,
) -> Result<(Value, Option<f64>), String> {
    let mut cmd = runtime.program(request.cli, request.windows, environment)?;
    cmd.extend(
        [
            "exec",
            "--json",
            "--disable",
            "hooks",
            "--ephemeral",
            "-s",
            "read-only",
            "--skip-git-repo-check",
        ]
        .map(String::from),
    );
    if !request.model.is_empty() {
        cmd.extend(["-m".into(), request.model.into()]);
    }
    if !request.effort.is_empty() {
        cmd.extend([
            "-c".into(),
            format!("model_reasoning_effort=\"{}\"", request.effort),
        ]);
    }
    cmd.push(request.prompt.into());
    let out = with_work_dir("te-side-", runtime, |cwd, runtime| {
        runtime
            .run(
                &cmd,
                cwd,
                &self::environment(&environment.env, false),
                None,
                request.timeout,
            )
            .map_err(|e| e.to_string())
    })?;
    let mut text = json!("");
    let mut usage = Value::Null;
    for line in out.stdout.lines() {
        let Ok(ev) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if ev["type"] == "item.completed" && ev["item"]["type"] == "agent_message" {
            if truthy(&ev["item"]["text"]) {
                text = ev["item"]["text"].clone();
            }
        } else if ev["type"] == "turn.completed" && ev["usage"].is_object() && truthy(&ev["usage"])
        {
            usage = ev["usage"].clone();
        }
    }
    if !truthy(&text) {
        no_reply(
            "codex",
            root,
            &out.stdout,
            &out.stderr,
            &json!(out.code),
            super::history::now(),
        );
    }
    let eff = if truthy(&usage) {
        let inp = usage_count(&usage["input_tokens"])?;
        let cached = usage_count(&usage["cached_input_tokens"])?;
        let cw = usage_count(&usage["cache_write_input_tokens"])?;
        let input = inp
            .checked_sub(cached)
            .and_then(|n| n.checked_sub(cw))
            .ok_or("optimiser usage exceeds i128")?;
        let output = usage_count(&usage["output_tokens"])?
            .checked_add(usage_count(&usage["reasoning_output_tokens"])?)
            .ok_or("optimiser usage exceeds i128")?;
        accounting::effective(&json!({"input_tokens":input.max(0) as f64,"cache_read_input_tokens":cached as f64,"cache_creation_input_tokens":cw as f64,"output_tokens":output as f64,"model":if request.model.is_empty(){"codex"}else{request.model}})).map_err(|e|e.to_string())?.0
    } else {
        None
    };
    Ok((text, eff))
}
pub fn pi_extension_args(
    extensions: &[String],
    environment: &Environment,
) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    for extension in extensions {
        args.push("--extension".into());
        args.push(if extension.starts_with("builtin:") {
            extension.clone()
        } else {
            let expanded = paths::expand_user(extension, &environment.home, &environment.env)?;
            paths::normalize(&if expanded.is_absolute() {
                expanded
            } else {
                environment.cwd.join(expanded)
            })
            .to_string_lossy()
            .into_owned()
        });
    }
    Ok(args)
}
pub fn pi_call(
    request: &Call<'_>,
    extensions: &[String],
    environment: &Environment,
    root: &Path,
    runtime: &mut dyn Runtime,
) -> Result<(Value, Option<f64>), String> {
    let extra = pi_extension_args(extensions, environment)?;
    let exe = runtime.program(request.cli, request.windows, environment)?;
    let out = with_work_dir("te-side-", runtime, |cwd, runtime| {
        let mut output = None;
        for no_mcp in [true, false] {
            let mut cmd = exe.clone();
            cmd.extend(PI_FLAGS.map(String::from));
            if no_mcp {
                cmd.push("--no-mcp".into());
            }
            cmd.extend(extra.clone());
            if !request.model.is_empty() {
                cmd.extend(["--model".into(), request.model.into()]);
            }
            cmd.extend(["--".into(), request.prompt.into()]);
            let out = runtime
                .run(
                    &cmd,
                    cwd,
                    &self::environment(&environment.env, false),
                    None,
                    request.timeout,
                )
                .map_err(|e| e.to_string())?;
            let retry = out.stderr.contains("Unknown option: --no-mcp");
            output = Some(out);
            if !retry {
                break;
            }
        }
        Ok(output.unwrap())
    })?;
    let (text, usage) = pi::json_reply(&out.stdout)?;
    if text.is_empty() {
        no_reply(
            "pi",
            root,
            &out.stdout,
            &out.stderr,
            &json!(out.code),
            super::history::now(),
        );
    }
    let eff = if let Some(mut usage) = usage {
        usage.as_object_mut().unwrap().remove("cost_usd");
        if !truthy(&usage["model"]) {
            usage["model"] = json!(if request.model.is_empty() {
                "pi"
            } else {
                request.model
            });
        }
        accounting::effective(&usage).map_err(|e| e.to_string())?.0
    } else {
        None
    };
    Ok((json!(text), eff))
}
pub fn cursor_call(
    request: &Call<'_>,
    environment: &Environment,
    root: &Path,
    runtime: &mut dyn Runtime,
) -> Result<(Value, Option<f64>), String> {
    let out = with_work_dir("te-side-", runtime, |cwd, runtime| {
        let mut cmd = runtime.program(request.cli, request.windows, environment)?;
        cmd.extend(
            [
                "--print",
                "--output-format",
                "json",
                "--mode",
                "ask",
                "--trust",
            ]
            .map(String::from),
        );
        if !request.model.is_empty() {
            cmd.extend(["--model".into(), request.model.into()]);
        }
        cmd.push(request.prompt.into());
        runtime
            .run(
                &cmd,
                cwd,
                &self::environment(&environment.env, false),
                None,
                request.timeout,
            )
            .map_err(|e| e.to_string())
    })?;
    let data = serde_json::from_str::<Value>(&out.stdout).unwrap_or(Value::Null);
    let text = data
        .get("result")
        .filter(|v| truthy(v))
        .cloned()
        .unwrap_or(json!(""));
    if !truthy(&text) {
        no_reply(
            "cursor",
            root,
            &out.stdout,
            &out.stderr,
            &json!(out.code),
            super::history::now(),
        );
    }
    Ok((text, None))
}
pub fn grok_call(
    request: &Call<'_>,
    environment: &Environment,
    root: &Path,
    runtime: &mut dyn Runtime,
) -> Result<(Value, Option<f64>), String> {
    let mut cmd = runtime.program(request.cli, request.windows, environment)?;
    cmd.extend(
        [
            "-p",
            request.prompt,
            "--output-format",
            "json",
            "--no-auto-update",
            "--tools",
            GROK_TOOLS,
            "--disallowed-tools",
            GROK_REMOVED,
        ]
        .map(String::from),
    );
    for rule in GROK_DENY_RULES {
        cmd.extend(["--deny".into(), rule.into()]);
    }
    if !request.model.is_empty() {
        cmd.extend(["--model".into(), request.model.into()]);
    }
    let out = with_work_dir("te-side-", runtime, |cwd, runtime| {
        runtime
            .run(
                &cmd,
                cwd,
                &self::environment(&environment.env, false),
                None,
                request.timeout,
            )
            .map_err(|e| e.to_string())
    })?;
    let (text, usage) = grok::json_reply(&out.stdout)?;
    if text.is_empty() {
        no_reply(
            "grok",
            root,
            &out.stdout,
            &out.stderr,
            &json!(out.code),
            super::history::now(),
        );
    }
    let eff = if let Some(mut usage) = usage {
        if !truthy(&usage["model"]) {
            usage["model"] = json!(if request.model.is_empty() {
                "grok"
            } else {
                request.model
            });
        }
        accounting::effective(&usage).map_err(|e| e.to_string())?.0
    } else {
        None
    };
    Ok((json!(text), eff))
}
pub fn opencode_flags(
    cli: &str,
    windows: bool,
    cache: &mut Map<String, Value>,
    environment: &Environment,
    runtime: &mut dyn Runtime,
) -> Result<Vec<String>, String> {
    if !cache.contains_key(cli) {
        let help = runtime
            .program(cli, windows, environment)
            .ok()
            .and_then(|mut cmd| {
                cmd.extend(["run".into(), "--help".into()]);
                runtime
                    .run(&cmd, &environment.cwd, &environment.env, None, 30.0)
                    .ok()
                    .map(|out| out.stdout)
            })
            .unwrap_or_default();
        let flags: Vec<_> = ["--pure", "--standalone"]
            .into_iter()
            .filter(|flag| help.contains(flag))
            .collect();
        cache.insert(cli.into(), json!(flags));
    }
    serde_json::from_value(cache[cli].clone()).map_err(|e| e.to_string())
}
pub fn opencode_call(
    request: &Call<'_>,
    cache: &mut Map<String, Value>,
    environment: &Environment,
    root: &Path,
    runtime: &mut dyn Runtime,
) -> Result<(Value, Option<f64>), String> {
    let mut env = self::environment(&environment.env, false);
    env.insert("OPENCODE_CONFIG_CONTENT".into(), json!(OPENCODE_CONFIG));
    let exe = runtime.program(request.cli, request.windows, environment)?;
    let (out, text, usage) = with_work_dir("te-side-", runtime, |cwd, runtime| {
        let mut cmd = exe.clone();
        cmd.push("run".into());
        cmd.extend(opencode_flags(
            request.cli,
            request.windows,
            cache,
            environment,
            runtime,
        )?);
        cmd.extend(["--format".into(), "json".into()]);
        if !request.model.is_empty() {
            cmd.extend(["--model".into(), request.model.into()]);
        }
        cmd.push(request.prompt.into());
        let out = runtime
            .run(&cmd, cwd, &env, None, request.timeout)
            .map_err(|e| e.to_string())?;
        let (text, usage, sid) = opencode::run_reply(&out.stdout)?;
        if !sid.is_empty() {
            let mut delete = exe.clone();
            delete.extend(["session".into(), "delete".into(), sid]);
            let _ = runtime.run(&delete, cwd, &env, None, 30.0);
        }
        Ok((out, text, usage))
    })?;
    if text.is_empty() {
        no_reply(
            "opencode",
            root,
            &out.stdout,
            &out.stderr,
            &json!(out.code),
            super::history::now(),
        );
    }
    let eff = if let Some(mut usage) = usage {
        usage.as_object_mut().unwrap().remove("cost_usd");
        usage["model"] = json!(if request.model.is_empty() {
            "opencode"
        } else {
            request.model
        });
        accounting::effective(&usage).map_err(|e| e.to_string())?.0
    } else {
        None
    };
    Ok((json!(text), eff))
}
