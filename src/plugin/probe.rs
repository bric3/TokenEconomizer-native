use super::{
    cli::{Error, Result},
    hosts::session::{self, Environment},
};
use crate::value::python_space;
use crate::{
    store::Store,
    trace::copilot,
    value::{scalar_text, truthy},
};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::LazyLock,
};

pub const PROMPT: &str = "Run this repository's test suite and report how many tests pass. Do not install anything. Keep the answer to one line.";
pub trait Runtime {
    fn call(&mut self, method: &str, args: Value) -> Result<Value>;
    fn say(&mut self, text: &str) -> Result<()>;
}
fn text(value: &Value) -> Result<&str> {
    value
        .as_str()
        .ok_or_else(|| Error::new("TypeError", "expected text"))
}
fn lines(text: &str) -> impl DoubleEndedIterator<Item = &str> {
    text.split([
        '\n', '\r', '\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2028}',
        '\u{2029}',
    ])
}
pub fn test_commands(commands: &[String]) -> (Vec<String>, usize) {
    static RUNNER: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?:^|[\s\x1c-\x1f;&|(])(?:py\.?test|python[\d.]*[\s\x1c-\x1f]+-m[\s\x1c-\x1f]+(?:pytest|unittest))(?:$|[^\p{L}\p{N}_])",
        )
        .unwrap()
    });
    static TRAP: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?:^|[\s\x1c-\x1f;&|(])(?:py\.?test|python[\d.]*[\s\x1c-\x1f]+-m[\s\x1c-\x1f]+pytest)(?:$|[^\p{L}\p{N}_])").unwrap()
    });
    let runs: Vec<_> = commands
        .iter()
        .filter(|c| RUNNER.is_match(c))
        .cloned()
        .collect();
    let traps = runs.iter().filter(|c| TRAP.is_match(c)).count();
    (runs, traps)
}
pub fn write(path: &Path, content: &str, mode: u32) -> Result<()> {
    fs::create_dir_all(path.parent().unwrap_or(Path::new(".")))?;
    fs::write(path, content)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    }
    #[cfg(not(unix))]
    let _ = mode;
    Ok(())
}
fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}
pub fn make_repo(
    root: &Path,
    python: &str,
    runtime: &mut dyn Runtime,
) -> Result<(PathBuf, PathBuf)> {
    let repo = root.join("repo");
    let trap = root.join("trap");
    for (path, content) in [
        (repo.join("calc.py"), "def add(a, b):\n    return a + b\n"),
        (repo.join("tests/__init__.py"), ""),
        (
            repo.join("tests/test_calc.py"),
            "import unittest\n\nfrom calc import add\n\n\nclass TestAdd(unittest.TestCase):\n    def test_add(self):\n        self.assertEqual(add(2, 2), 4)\n",
        ),
        (
            repo.join("pyproject.toml"),
            "[project]\nname = \"calc\"\n\n[tool.pytest.ini_options]\ntestpaths = [\"tests\"]\n",
        ),
        (repo.join(".gitignore"), "__pycache__/\n"),
        (
            trap.join("pymods/pytest/__init__.py"),
            "raise ModuleNotFoundError(\"No module named 'pytest'\", name=\"pytest\")\n",
        ),
    ] {
        write(&path, content, 0o644)?;
    }
    write(
        &trap.join("pytest"),
        "#!/bin/sh\necho \"bash: pytest: command not found\" >&2\nexit 127\n",
        0o755,
    )?;
    let wrapper = format!(
        "#!/bin/sh\nPYTHONPATH={}\"${{PYTHONPATH:+:$PYTHONPATH}}\" exec {} \"$@\"\n",
        shell_quote(&trap.join("pymods").to_string_lossy()),
        shell_quote(python)
    );
    for name in ["python", "python3"] {
        write(&trap.join(name), &wrapper, 0o755)?;
    }
    runtime.call("git", json!(["git", "init", "-q", repo]))?;
    runtime.call("git", json!(["git", "-C", repo, "add", "-A"]))?;
    // Use the configured signing key for temporary fixture commits too.
    runtime.call(
        "git",
        json!([
            "git",
            "-C",
            repo,
            "commit",
            "-S",
            "-qm",
            "test: initialise probe fixture"
        ]),
    )?;
    Ok((repo, trap))
}
pub fn plugin_root(environment: &Environment, module: &Path) -> Result<PathBuf> {
    let here = module
        .parent()
        .and_then(Path::parent)
        .unwrap_or(Path::new("."));
    plugin_root_candidates(
        environment,
        &[
            here.join("plugin"),
            here.parent().unwrap_or(Path::new(".")).into(),
        ],
    )
}
pub fn plugin_root_candidates(
    environment: &Environment,
    candidates: &[PathBuf],
) -> Result<PathBuf> {
    let configured = environment
        .env
        .get("TE_PLUGIN_ROOT")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty());
    let candidates = configured
        .map(PathBuf::from)
        .into_iter()
        .chain(candidates.iter().cloned());
    for path in candidates {
        if environment
            .path(&path)
            .join("hooks/copilot-hooks.json")
            .is_file()
        {
            return Ok(path);
        }
    }
    Err("cannot find the plugin directory (set TE_PLUGIN_ROOT)".into())
}
pub fn session_env(state: &str, trap: &str, env: &Map<String, Value>) -> Map<String, Value> {
    let mut out = env.clone();
    for key in [
        "CLAUDECODE",
        "CLAUDE_CODE_SESSION_ID",
        "CLAUDE_PLUGIN_ROOT",
        "TE_SIDECALL",
        "TE_DISABLE",
        "TERM_PROGRAM",
    ] {
        out.remove(key);
    }
    let separator = if cfg!(windows) { ';' } else { ':' };
    out.insert(
        "PATH".into(),
        json!(format!(
            "{trap}{separator}{}",
            env.get("PATH").and_then(Value::as_str).unwrap_or("")
        )),
    );
    for (key, value) in [
        ("TOKENECONOMIZER_HOME", state),
        ("TE_TOOLPRUNE", "propose"),
        ("TE_MICROSKILLS", "envfacts"),
    ] {
        out.insert(key.into(), json!(value));
    }
    out
}
pub fn run_session(
    repo: &str,
    env: &Value,
    plugin: &str,
    model: &Value,
    cli: &str,
    timeout: &Value,
    runtime: &mut dyn Runtime,
) -> Result<Value> {
    let mut command = super::cli::words(&runtime.call("program", json!([cli]))?)?;
    command.extend(
        [
            "-p",
            PROMPT,
            "--plugin-dir",
            plugin,
            "--allow-all-tools",
            "--no-auto-update",
            "--output-format",
            "json",
        ]
        .map(String::from),
    );
    if truthy(model) {
        command.extend(["--model".into(), text(model)?.into()]);
    }
    let output = runtime.call("process", json!([command, repo, env, timeout]))?;
    let stdout = output["stdout"].as_str().unwrap_or("");
    let stderr = output["stderr"].as_str().unwrap_or("");
    let error = lines(stderr.trim_matches(python_space))
        .next_back()
        .unwrap_or("");
    let mut sid = Value::Null;
    let mut answer = Value::String(String::new());
    for line in lines(stdout) {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if event["type"] == "result" {
            sid = event["sessionId"].clone();
        } else if event["type"] == "assistant.message" && truthy(&event["data"]["content"]) {
            answer = event["data"]["content"].clone();
        }
    }
    Ok(json!([
        sid,
        text(&answer)?.trim_matches(python_space),
        error
    ]))
}
pub fn inspect_session(
    sid: &str,
    environment: &Environment,
    runtime: &mut dyn Runtime,
) -> Result<Value> {
    static CD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^cd [^\s\x1c-\x1f]+ && ").unwrap());
    let log = environment.path(
        &session::copilot_home(&environment.env, &environment.home)
            .join("session-state")
            .join(sid)
            .join("events.jsonl"),
    );
    runtime.call("settle", json!([log]))?;
    let mut commands = Vec::new();
    for (name, args, _, _) in copilot::turns(&log).into_iter().flatten() {
        if name == "bash" {
            commands.push(
                text(
                    args.get("command")
                        .filter(|v| truthy(v))
                        .unwrap_or(&json!("")),
                )?
                .to_string(),
            );
        }
    }
    let (tests, tried_pytest) = test_commands(&commands);
    let mut injected = false;
    for line in String::from_utf8_lossy(&fs::read(&log)?).lines() {
        if line.contains("\"hook.end\"") && line.contains("\"sessionStart\"") {
            let Ok(event) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let context = &event["data"]["output"]["additionalContext"];
            if truthy(context) {
                injected |= text(context)?.contains("fails here");
            }
        }
    }
    let usage = copilot::session_usage(&log)?;
    let first = tests
        .first()
        .map(|s| CD.replace(s, "").chars().take(60).collect::<String>());
    let input = [
        "input_tokens",
        "cache_read_input_tokens",
        "cache_creation_input_tokens",
    ]
    .iter()
    .map(|k| usage[k].as_f64().unwrap_or(0.0))
    .sum::<f64>();
    let requests = usage["requests"].as_f64().unwrap_or(0.0);
    let input = if requests != 0.0 {
        let n = (input / requests).round_ties_even();
        if n >= i64::MIN as f64 && n < i64::MAX as f64 {
            json!(n as i64)
        } else {
            json!(n)
        }
    } else {
        Value::Null
    };
    let mut tool_tokens = usage["tool_definitions_tokens"].clone();
    // shortcut: trace counters use f64; audit values above 2^53 with the trace numeric contract.
    if let Some(n) = tool_tokens
        .as_f64()
        .filter(|n| *n >= i64::MIN as f64 && *n < i64::MAX as f64)
    {
        tool_tokens = json!(n as i64);
    }
    Ok(
        json!({"session":sid,"injected":injected,"tests":tests,"first_test":first,"tried_pytest":tried_pytest,"tool_definition_tokens":tool_tokens,"input_per_request":input}),
    )
}
pub fn wait_folded(
    store: &Store,
    sid: &str,
    timeout: f64,
    runtime: &mut dyn Runtime,
) -> Result<bool> {
    let now = |r: &mut dyn Runtime| {
        r.call("time", json!([]))?
            .as_f64()
            .ok_or_else(|| Error::from("expected clock value"))
    };
    let deadline = now(runtime)? + timeout;
    while now(runtime)? < deadline {
        let folded = store.read("folded.json", json!({}))?;
        if folded.as_object().is_some_and(|m| m.contains_key(sid))
            || folded.as_array().is_some_and(|a| a.contains(&json!(sid)))
        {
            return Ok(true);
        }
        runtime.call("sleep", json!([1]))?;
    }
    Ok(false)
}
pub fn line(report: &Value) -> Result<String> {
    let label = text(&report["label"])?;
    let padding = " ".repeat(10usize.saturating_sub(label.chars().count()));
    let first = report
        .get("first_test")
        .filter(|v| truthy(v))
        .unwrap_or(&Value::Null);
    Ok(format!(
        "  {padding}{label}: fact in context={}  first test command: {}  pytest attempts: {}  tool definitions: {} tokens",
        if truthy(&report["injected"]) {
            "yes"
        } else {
            "no "
        },
        if first.is_null() {
            "-".into()
        } else {
            scalar_text(first)
        },
        scalar_text(&report["tried_pytest"]),
        scalar_text(&report["tool_definition_tokens"])
    ))
}
pub fn run(
    sessions: i64,
    model: &Value,
    keep: bool,
    cli: &str,
    environment: &Environment,
    runtime: &mut dyn Runtime,
) -> Result<i32> {
    if !truthy(&runtime.call("which", json!([cli]))?) {
        return Err(format!("`{cli}` is not on PATH").into());
    }
    let model = runtime.call(
        "copilot_model",
        json!([if truthy(model) {
            model.clone()
        } else {
            json!("")
        }]),
    )?;
    let plugin = runtime.call("plugin_root", json!([]))?;
    let trusted = runtime.call("trusted_folders", json!([]))?;
    let mut base = Value::Null;
    if let Some(folders) = trusted.as_array() {
        for folder in folders {
            if environment.path(Path::new(text(folder)?)).is_dir()
                && truthy(&runtime.call("writable", json!([folder]))?)
            {
                base = folder.clone();
                break;
            }
        }
    }
    let root = runtime.call("mkdtemp", json!([base]))?;
    let root_path = environment.path(Path::new(text(&root)?));
    let state = root_path.join("state");
    let result = (|| {
        let repo_trap = runtime.call("make_repo", json!([root]))?;
        let repo = &repo_trap[0];
        let env = session_env(
            &state.to_string_lossy(),
            text(&repo_trap[1])?,
            &environment.env,
        );
        let route = if environment
            .env
            .get("COPILOT_PROVIDER_BASE_URL")
            .is_some_and(truthy)
        {
            format!(
                "BYOK {}",
                environment
                    .env
                    .get("COPILOT_PROVIDER_TYPE")
                    .filter(|v| truthy(v))
                    .map(scalar_text)
                    .unwrap_or("openai".into())
            )
        } else {
            "Copilot subscription".into()
        };
        runtime.say(&format!(
            "TokenEconomizer probe -- Copilot ({route}), model {}, plugin {}",
            if truthy(&model) {
                scalar_text(&model)
            } else {
                "(Copilot default)".into()
            },
            scalar_text(&plugin)
        ))?;
        if base.is_null() {
            runtime.say("  note: Copilot trusts no folder here, so the deny list cannot take effect in the probe's repository (Copilot ignores an untrusted folder's settings)")?;
        }
        let catalog = runtime.call("refresh_catalog", json!([state]))?;
        let tools = catalog[1]
            .as_array()
            .map(Vec::len)
            .or_else(|| catalog[1].as_object().map(Map::len))
            .ok_or("expected tool catalog")?;
        runtime.say(&format!(
            "  catalog: {tools} tools recorded (local capture, no model call)"
        ))?;
        let ctx = runtime.call("context", json!([repo, env]))?;
        let store = Store::new(Path::new(text(&ctx["base"])?))?;
        let mut results = Vec::new();
        let session =
            |label: String, runtime: &mut dyn Runtime| -> Result<(Option<Value>, Value)> {
                let output = runtime.call("run_session", json!([repo, env, plugin, model, cli]))?;
                if !truthy(&output[0]) {
                    return Ok((None, output[2].clone()));
                }
                runtime.call("wait_folded", json!([ctx, output[0]]))?;
                let mut report = runtime.call("inspect_session", json!([output[0]]))?;
                report["label"] = json!(label);
                report["answer"] = output[1].clone();
                runtime.say(&format!(
                    "{}  answer: {}",
                    line(&report)?,
                    text(&output[1])?.chars().take(40).collect::<String>()
                ))?;
                Ok((Some(report), Value::Null))
            };
        for i in 1..=sessions {
            let (report, error) = session(format!("session {i}"), runtime)?;
            let Some(report) = report else {
                runtime.say(&format!(
                    "  session {i}: no session ran ({})",
                    if truthy(&error) {
                        scalar_text(&error)
                    } else {
                        "no output".into()
                    }
                ))?;
                return Ok(1);
            };
            results.push(report);
        }
        let untool = store.read("untool.json", json!({}))?;
        let proposal = untool
            .get("proposal")
            .filter(|v| truthy(v))
            .cloned()
            .unwrap_or(json!([]));
        let lean = if truthy(&proposal) {
            runtime.call("apply", json!([repo, proposal, ctx]))?;
            let report = session("deny list".into(), runtime);
            let restored = runtime.call("restore", json!([repo, ctx]));
            restored?;
            report?.0
        } else {
            None
        };
        let mut with_fact = (0, 0);
        let mut without = (0, 0);
        for report in results
            .iter()
            .chain(lean.iter())
            .filter(|r| truthy(&r["tests"]))
        {
            let counts = if truthy(&report["injected"]) {
                &mut with_fact
            } else {
                &mut without
            };
            counts.1 += 1;
            counts.0 += usize::from(truthy(&report["tried_pytest"]));
        }
        runtime.say("")?;
        runtime.say(&format!("  without the fact: {}/{} sessions tried pytest, which is not installed; with it: {}/{}{}",without.0,without.1,with_fact.0,with_fact.1,if with_fact.1==0{"  (the fact never cleared its firing gate; run more --sessions)"}else{""}))?;
        if let Some(last) = results
            .iter()
            .rfind(|r| truthy(&r["tool_definition_tokens"]))
            && let Some(lean) = lean
                .as_ref()
                .filter(|r| truthy(&r["tool_definition_tokens"]))
        {
            runtime.say(&format!(
                "  deny list {}: tool definitions {} -> {} tokens per request",
                super::cli::words(&proposal)?.join(", "),
                scalar_text(&last["tool_definition_tokens"]),
                scalar_text(&lean["tool_definition_tokens"])
            ))?;
        } else if !truthy(&proposal) {
            runtime.say("  deny list: nothing proposed (a server is proposed once none of its tools was used in `toolprune_evidence` sessions)")?;
        }
        Ok(0)
    })();
    if keep {
        runtime.say(&format!("  kept: {}", root_path.display()))?;
    } else {
        let _ = super::temporary::remove_tree(&root_path);
    }
    result
}

pub fn command(args: &Value, runtime: &mut dyn super::cli::Runtime) -> Result<i32> {
    if args["host"] != "copilot" {
        runtime.say("the probe runs on Copilot only so far: `te --host copilot probe`")?;
        return Ok(2);
    }
    runtime
        .global(
            "probe_run",
            json!([args["sessions"], args["model"], args["keep"]]),
        )?
        .as_i64()
        .map(|v| v as i32)
        .ok_or_else(|| "expected probe exit status".into())
}
