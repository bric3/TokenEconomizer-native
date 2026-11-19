//! Capture the tool schemas a real Claude task loads, then retain the longest descriptions.
pub mod recorder;

use super::{Failure, Result, Runtime, codex_config::integer};
use crate::python_json::{self, Node};
use crate::value::python_space;
use crate::value::{json_text, python_repr, python_type, scalar_text, truthy};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;

pub const INSTANCE: &str = "pydata__xarray-6461";
pub const RECORDER: &str = include_str!("catalog/recorder_io.py");
const NAME: &str = "te-catalog-capture";

pub fn report(catalog: &Value) -> Result<Value> {
    let catalog = catalog
        .as_object()
        .ok_or_else(|| Failure::new("TypeError", "catalogue must be a mapping"))?;
    let mut stdout = String::new();
    for (name, description) in catalog {
        let description = text(description, "__getitem__")?;
        stdout.push_str(&format!(
            "  {name:<22} {}\n",
            description.chars().take(66).collect::<String>()
        ));
    }
    Ok(json!({"stdout":stdout,"stderr":format!("\n{} tools captured\n", catalog.len())}))
}

pub fn install(args: &Value, rt: &mut dyn Runtime) -> Result<Value> {
    let root = super::container_helper::root(args["helper_root"].as_str())?;
    let path = super::container_helper::copy(&root, "/tmp", &args["name"], rt)?;
    let result = rt.call("_exec", json!([args["name"], format!("chmod 755 {}", super::container_helper::quote(&path)), {"timeout":60}]))?;
    if result["returncode"] != 0 {
        return Err(Failure::new(
            "RuntimeError",
            "could not make the catalogue helper executable",
        ));
    }
    Ok(json!(path))
}

fn text(value: &Value, method: &str) -> Result<String> {
    value.as_str().map(str::to_owned).ok_or_else(|| {
        Failure::new(
            "AttributeError",
            format!(
                "'{}' object has no attribute '{method}'",
                python_type(value)
            ),
        )
    })
}
pub fn system_tools(value: &Value) -> Result<Value> {
    let input = if truthy(value) {
        text(value, "splitlines")?
    } else {
        String::new()
    };
    let system = Node::String("system".into());
    for line in input.split([
        '\n', '\r', '\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}', '\u{2028}',
        '\u{2029}',
    ]) {
        let Some(event) = python_json::parse(line.as_bytes(), 4300) else {
            continue;
        };
        if !matches!(event, Node::Dict(_)) {
            return Err(Failure::new(
                "AttributeError",
                format!("'{}' object has no attribute 'get'", event.kind()),
            ));
        }
        if event.field("type") == Some(&system)
            && matches!(event.field("tools"), Some(Node::List(_)))
        {
            return serde_json::from_str(&event.field("tools").unwrap().json(false))
                .map_err(|error| Failure::new("ValueError", error.to_string()));
        }
    }
    Ok(json!([]))
}
fn run(rt: &mut dyn Runtime, argv: Value, timeout: i64) -> Result<Value> {
    rt.call(
        "run",
        json!([
            argv,
            if timeout == 120 {
                json!({})
            } else {
                json!({"timeout":timeout})
            }
        ]),
    )
}
fn log(rt: &mut dyn Runtime, message: impl Into<String>) -> Result<()> {
    rt.call("log", json!(message.into())).map(|_| ())
}
fn tail(value: &Value, length: usize) -> Result<String> {
    let value = text(value, "__getitem__")?;
    Ok(value
        .chars()
        .skip(value.chars().count().saturating_sub(length))
        .collect())
}
fn captured(rt: &mut dyn Runtime) -> Result<Value> {
    let value = run(
        rt,
        json!([
            "docker",
            "exec",
            NAME,
            "bash",
            "-lc",
            "cat /tmp/tools.json 2>/dev/null || true"
        ]),
        120,
    )?;
    let raw = text(&value["stdout"], "strip")?;
    let raw = raw.trim_matches(python_space);
    if raw.is_empty() {
        Ok(json!({}))
    } else {
        rt.call("loads", json!(raw))
    }
}
fn names(value: &Value) -> Result<BTreeSet<String>> {
    let values = match value {
        Value::Array(values) => values.clone(),
        Value::Object(values) => values.keys().map(|key| json!(key)).collect(),
        _ => {
            return Err(Failure::new(
                "TypeError",
                format!("'{}' object is not iterable", python_type(value)),
            ));
        }
    };
    values.iter().map(|value| text(value, "__hash__")).collect()
}
fn missing(loaded: &Value, captured: &Value) -> Result<Vec<String>> {
    Ok(names(loaded)?
        .difference(&names(captured)?)
        .cloned()
        .collect())
}
struct Agent<'a> {
    port: &'a str,
    model: &'a Value,
    effort: &'a Value,
    turns: &'a str,
}
impl Agent<'_> {
    fn run(&self, rt: &mut dyn Runtime, prompt: &str, stream: bool, direct: bool) -> Result<Value> {
        let Self {
            port,
            model,
            effort,
            turns,
        } = *self;
        let mut argv = vec![
            json!("docker"),
            json!("exec"),
            json!("-i"),
            json!("-w"),
            json!("/testbed"),
        ];
        if direct {
            argv.extend([json!("-e"), json!("ANTHROPIC_API_KEY")]);
        } else {
            argv.extend([
                json!("-e"),
                json!(format!("ANTHROPIC_BASE_URL=http://127.0.0.1:{port}")),
                json!("-e"),
                json!("ANTHROPIC_API_KEY=dummy"),
            ]);
        }
        argv.extend([
            json!("-e"),
            json!("HOME=/tmp/agent-home"),
            json!("-e"),
            json!("IS_SANDBOX=1"),
            json!(NAME),
            json!("claude"),
            json!("-p"),
            json!("--dangerously-skip-permissions"),
            json!("--model"),
            model.clone(),
            json!("--effort"),
            effort.clone(),
            json!("--max-turns"),
            json!(turns),
            json!("--output-format"),
            json!(if stream { "stream-json" } else { "json" }),
        ]);
        if stream {
            argv.push(json!("--verbose"));
        }
        rt.call("agent", json!([argv, prompt]))
    }
}
fn select(
    args: &Value,
    env: &Map<String, Value>,
    argument: &str,
    key: &str,
    default: Value,
) -> Value {
    args.get(argument)
        .filter(|value| truthy(value))
        .or_else(|| env.get(key))
        .cloned()
        .unwrap_or(default)
}

pub fn capture(args: &Value, rt: &mut dyn Runtime) -> Result<Value> {
    let env = args["env"]
        .as_object()
        .ok_or_else(|| Failure::new("TypeError", "missing capture environment"))?;
    let port = integer(&select(args, env, "port", "TE_CATALOG_PORT", json!("8770")))?;
    let out = select(
        args,
        env,
        "out_path",
        "TE_CATALOG_OUT",
        args["default_out"].clone(),
    );
    let chars = integer(&select(
        args,
        env,
        "desc_chars",
        "TE_CATALOG_DESC_CHARS",
        json!("300"),
    ))?;
    let negative = chars.starts_with('-');
    let chars = chars
        .trim_start_matches('-')
        .parse::<usize>()
        .unwrap_or(usize::MAX);
    let model = select(
        args,
        env,
        "model",
        "ANTHROPIC_MODEL",
        json!("claude-sonnet-5"),
    );
    let effort = select(args, env, "effort", "TE_EFFORT_LEVEL", json!("high"));
    let turns = integer(env.get("TE_MAX_TURNS").unwrap_or(&json!("80")))?;
    let agent = Agent {
        port: &port,
        model: &model,
        effort: &effort,
        turns: &turns,
    };
    let key = env.get("ANTHROPIC_API_KEY").cloned().unwrap_or(json!(""));
    if !truthy(&key) {
        return Err(Failure::new(
            "SystemExit",
            "ANTHROPIC_API_KEY is unset (see the repo's .env)",
        ));
    }

    run(rt, json!(["docker", "rm", "-f", NAME]), 120)?;
    rt.call("live_add", json!(NAME))?;
    let result = (|| {
        let binary = rt.call("claude_bin", Value::Null)?;
        let image = rt.call("image_for", args["instance_id"].clone())?;
        let process = run(
            rt,
            json!([
                "docker",
                "run",
                "-d",
                "--name",
                NAME,
                "--entrypoint",
                "bash",
                "-v",
                format!("{}:/usr/local/bin/claude:ro", scalar_text(&binary)),
                "-e",
                "HOME=/tmp/agent-home",
                "-w",
                "/testbed",
                image,
                "-lc",
                "sleep infinity"
            ]),
            300,
        )?;
        if process["returncode"] != 0 {
            return Err(Failure::new(
                "SystemExit",
                format!("docker run failed: {}", tail(&process["stderr"], 400)?),
            ));
        }
        run(
            rt,
            json!(["docker", "exec", NAME, "mkdir", "-p", "/tmp/agent-home"]),
            120,
        )?;
        // The shared static helper owns recorder policy; Python supplies HTTP and TLS I/O.
        rt.call("install_recorder", json!(NAME))?;
        let recorder = args["recorder_source"].as_str().unwrap_or(RECORDER);
        run(
            rt,
            json!([
                "docker",
                "exec",
                NAME,
                "bash",
                "-lc",
                format!("cat > /tmp/capture_recorder.py <<'TE_EOF'\n{recorder}\nTE_EOF")
            ]),
            120,
        )?;
        rt.call("popen", json!(["docker", "exec", "-e", format!("REAL_ANTHROPIC_KEY={}", scalar_text(&key)), NAME, "bash", "-lc", format!("python /tmp/capture_recorder.py {port} /tmp/tools.json https://api.anthropic.com >/tmp/recorder.log 2>&1")]))?;
        for _ in 0..30 {
            let status = run(
                rt,
                json!([
                    "docker",
                    "exec",
                    NAME,
                    "bash",
                    "-lc",
                    format!("curl -s -o /dev/null -m 2 http://127.0.0.1:{port}/ ; echo $?")
                ]),
                120,
            )?;
            if text(&status["stdout"], "strip")?.trim_matches(python_space) != "7" {
                break;
            }
            rt.call("sleep", json!(1))?;
        }
        let firstparty = agent.run(rt, "Reply with exactly: probe", true, true)?;
        let firstparty = system_tools(&firstparty["stdout"])?;
        log(
            rt,
            format!(
                "first-party run loads {} tools",
                firstparty.as_array().unwrap().len()
            ),
        )?;
        let process = agent.run(rt, "Reply with exactly: catalog-capture", true, false)?;
        let loaded = system_tools(&process["stdout"])?;
        let absent = missing(&loaded, &captured(rt)?)?;
        if !absent.is_empty() {
            log(
                rt,
                format!(
                    "{} deferred tool(s) not yet on the wire: {}",
                    absent.len(),
                    python_repr(&json!(absent))
                ),
            )?;
            agent.run(
                rt,
                &format!(
                    "Call ToolSearch once with query \"select:{}\" and then reply DONE. Do nothing else.",
                    absent.join(",")
                ),
                false,
                false,
            )?;
            let still = missing(&loaded, &captured(rt)?)?;
            log(
                rt,
                format!(
                    "after ToolSearch fetch, still missing: {}",
                    if still.is_empty() {
                        "none".into()
                    } else {
                        python_repr(&json!(still))
                    }
                ),
            )?;
        }
        let merged = captured(rt)?;
        let gap = missing(&firstparty, &merged)?;
        if !gap.is_empty() {
            log(
                rt,
                format!(
                    "NOT captured (first-party-only, no readable source): {}",
                    python_repr(&json!(gap))
                ),
            )?;
        }
        if json_text(&merged) == "{}" {
            log(
                rt,
                format!(
                    "agent exit={} stdout={}",
                    scalar_text(&process["returncode"]),
                    python_repr(&json!(tail(&process["stdout"], 300)?))
                ),
            )?;
            let recorder_log = run(
                rt,
                json!([
                    "docker",
                    "exec",
                    NAME,
                    "bash",
                    "-lc",
                    "tail -5 /tmp/recorder.log"
                ]),
                120,
            )?;
            log(
                rt,
                format!("recorder log: {}", tail(&recorder_log["stdout"], 300)?),
            )?;
            return Err(Failure::new("SystemExit", "captured no tool schemas"));
        }
        Ok(merged)
    })();
    run(rt, json!(["docker", "rm", "-f", NAME]), 120)?;
    rt.call("live_discard", json!(NAME))?;
    let tools = result?;
    let prior = if truthy(&rt.call("exists", out.clone())?) {
        match rt.call("read_json", out.clone()) {
            Ok(value) => value,
            Err(error)
                if !matches!(
                    error.kind.as_str(),
                    "KeyboardInterrupt" | "SystemExit" | "GeneratorExit" | "BaseException"
                ) =>
            {
                json!({})
            }
            Err(error) => return Err(error),
        }
    } else {
        json!({})
    };
    let mut merged = prior
        .as_object()
        .cloned()
        .ok_or_else(|| Failure::new("TypeError", "catalog snapshot must be a mapping"))?;
    merge(&mut merged, &tools)?;
    let added: Vec<_> = merged
        .keys()
        .filter(|key| prior.get(key).is_none())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    if truthy(&prior) {
        log(
            rt,
            format!(
                "merged with existing snapshot ({} tools); new: {}",
                prior.as_object().unwrap().len(),
                if added.is_empty() {
                    "none".into()
                } else {
                    python_repr(&json!(added))
                }
            ),
        )?;
    }
    merged.sort_keys();
    for value in merged.values_mut() {
        let description = text(value, "__getitem__")?;
        let end = if negative {
            description.chars().count().saturating_sub(chars)
        } else {
            chars
        };
        *value = json!(description.chars().take(end).collect::<String>());
    }
    rt.call("write_json", json!([out, merged]))?;
    log(
        rt,
        format!(
            "wrote {} tool descriptions -> {}",
            merged.len(),
            scalar_text(&out)
        ),
    )?;
    Ok(json!(merged))
}

pub(super) fn merge(target: &mut Map<String, Value>, offered: &Value) -> Result<bool> {
    let offered = offered.as_object().ok_or_else(|| {
        Failure::new(
            "AttributeError",
            format!("'{}' object has no attribute 'items'", python_type(offered)),
        )
    })?;
    let mut changed = false;
    for (name, description) in offered {
        let description = text(description, "__len__")?;
        let previous = target
            .get(name)
            .map(|value| text(value, "__len__"))
            .transpose()?
            .unwrap_or_default();
        if description.chars().count() > previous.chars().count() {
            target.insert(name.clone(), json!(description));
            changed = true;
        }
    }
    Ok(changed)
}
