use super::{Failure, Result, Runtime, official, settings::field};
use crate::value::python_space;
use crate::value::{
    decimal_integer, json_pretty, number, python_repr, python_type, scalar_text, truthy,
};
use serde_json::{Map, Value, json};
use std::{fs, path::Path};

const DISCOVER_ENV: &str = r#"printf "PATH=%s\n" "$PATH"; for d in /opt/miniconda3/envs/*/bin; do [ -d "$d" ] && printf "ENV=%s\n" "$d"; done; true"#;
pub const COPILOT_SEED: &str = "mkdir -p /home/agent/.cache/copilot /home/agent/.copilot; ln -sfn /seed/copilot-cache/pkg /home/agent/.cache/copilot/pkg; cp -a /seed/copilot-cache/exp-cache.json /home/agent/.cache/copilot/ 2>/dev/null || true; ( cd /seed/copilot-config && tar -cf - --exclude=./logs --exclude=./session-state . ) | ( cd /home/agent/.copilot && tar -xf - ); ";

fn function(rt: &mut dyn Runtime, name: &str, args: Value) -> Result<Value> {
    rt.call("config_function", json!([name, args]))
}
fn run(rt: &mut dyn Runtime, cmd: Value, timeout: Value) -> Result<Value> {
    rt.call("container_run", json!([cmd, timeout]))
}
fn log(rt: &mut dyn Runtime, message: String) -> Result<()> {
    rt.call("log", json!([message])).map(|_| ())
}
fn strip(text: &str) -> &str {
    text.trim_matches(python_space)
}
fn mapping<'a>(value: &'a Value, method: &str) -> Result<&'a Map<String, Value>> {
    value.as_object().ok_or_else(|| {
        Failure::new(
            "AttributeError",
            format!(
                "'{}' object has no attribute '{method}'",
                python_type(value)
            ),
        )
    })
}
fn output<'a>(value: &'a Value, method: &str) -> Result<&'a str> {
    if !truthy(value) {
        return Ok("");
    }
    value.as_str().ok_or_else(|| {
        Failure::new(
            "AttributeError",
            format!(
                "'{}' object has no attribute '{method}'",
                python_type(value)
            ),
        )
    })
}
fn suffix(value: &Value, length: usize) -> Result<String> {
    let text = value.as_str().ok_or_else(|| {
        Failure::new(
            "TypeError",
            format!("'{}' object is not subscriptable", python_type(value)),
        )
    })?;
    Ok(text
        .chars()
        .skip(text.chars().count().saturating_sub(length))
        .collect())
}
fn env(rt: &mut dyn Runtime, key: &str, default: &str) -> Result<Value> {
    rt.call("container_env", json!([key, default]))
}
fn timeout(rt: &mut dyn Runtime, key: &str, default: &str) -> Result<Value> {
    let raw = env(rt, key, default)?;
    let normalized = raw.as_str().and_then(decimal_integer).ok_or_else(|| {
        Failure::new(
            "ValueError",
            format!(
                "invalid literal for int() with base 10: {}",
                python_repr(&raw)
            ),
        )
    })?;
    // shortcut: timeouts fit JSON's integer range; complete numeric parity before release.
    normalized
        .parse::<i64>()
        .map(|n| json!(n))
        .or_else(|_| normalized.parse::<u64>().map(|n| json!(n)))
        .map_err(|_| {
            Failure::new(
                "OverflowError",
                "container timeout exceeds the native JSON range",
            )
        })
}

pub fn run_process(
    cmd: &Value,
    timeout: &Value,
    check: &Value,
    rt: &mut dyn Runtime,
) -> Result<Value> {
    let process = rt.call("subprocess_run", json!([cmd, timeout]))?;
    if truthy(check) && number(&process["returncode"]) != Some(0.0) {
        let command = process.get("command").unwrap_or(cmd);
        let parts: Vec<Value> = if let Some(command) = command.as_str() {
            command
                .chars()
                .take(4)
                .map(|c| json!(c.to_string()))
                .collect()
        } else {
            command
                .as_array()
                .ok_or_else(|| Failure::new("TypeError", "command must be a sequence"))?
                .iter()
                .take(4)
                .cloned()
                .collect()
        };
        let words = parts
            .iter()
            .enumerate()
            .map(|(i, value)| {
                value.as_str().ok_or_else(|| {
                    Failure::new(
                        "TypeError",
                        format!(
                            "sequence item {i}: expected str instance, {} found",
                            python_type(value)
                        ),
                    )
                })
            })
            .collect::<Result<Vec<_>>>()?;
        return Err(Failure::new(
            "RuntimeError",
            format!(
                "{}... -> rc={}\n{}",
                words.join(" "),
                scalar_text(&process["returncode"]),
                suffix(&process["stderr"], 800)?
            ),
        ));
    }
    Ok(process)
}

pub fn reap(name: &Value, rt: &mut dyn Runtime) -> Result<Value> {
    let name_text = name.as_str().ok_or_else(|| {
        Failure::new(
            "AttributeError",
            format!("'{}' object has no attribute 'rsplit'", python_type(name)),
        )
    })?;
    let stem = name_text
        .rsplit_once('-')
        .map_or(name_text, |(stem, _)| stem);
    let attempt = (|| -> Result<()> {
        let p = run(
            rt,
            json!([
                "docker",
                "ps",
                "-a",
                "--filter",
                format!("name=^{stem}-"),
                "--filter",
                "status=exited",
                "--format",
                "{{.Names}}"
            ]),
            json!(60),
        )?;
        let stdout = p["stdout"].as_str().ok_or_else(|| {
            Failure::new(
                "AttributeError",
                format!(
                    "'{}' object has no attribute 'split'",
                    python_type(&p["stdout"])
                ),
            )
        })?;
        for stale in stdout.split(python_space).filter(|s| !s.is_empty()) {
            if stale != name_text {
                run(rt, json!(["docker", "rm", "-f", stale]), json!(60))?;
            }
        }
        Ok(())
    })();
    match attempt {
        Err(error) if !error.is_a("builtins.Exception") => Err(error),
        _ => Ok(Value::Null),
    }
}

pub fn activation_info(state: &Value, name: &str) -> Result<Value> {
    let value = mapping(state, "get")?.get(name).filter(|v| truthy(v));
    Ok(json!({"stored":value.is_some(),"info":value.cloned().unwrap_or(json!({"mode":"none"}))}))
}
pub fn activate(name: &Value, rt: &mut dyn Runtime) -> Result<Value> {
    rt.call("activation_ensure", json!([]))?;
    let info = if !truthy(&function(rt, "env_activation_enabled", json!([]))?) {
        json!({"mode":"none","requested":false})
    } else {
        let discovery = run(
            rt,
            json!(["docker", "exec", name, "bash", "-c", DISCOVER_ENV]),
            json!(120),
        )?;
        let mut path = String::new();
        let mut bins = Vec::new();
        for line in output(&discovery["stdout"], "splitlines")?.split(|c| {
            matches!(
                c,
                '\n' | '\r' | '\u{b}' | '\u{c}' | '\u{1c}'
                    ..='\u{1e}' | '\u{85}' | '\u{2028}' | '\u{2029}'
            )
        }) {
            if let Some(value) = line.strip_prefix("PATH=") {
                path = strip(value).into();
            } else if let Some(value) = line.strip_prefix("ENV=") {
                bins.push(strip(value).to_owned());
            }
        }
        let bin = function(rt, "choose_project_env", json!([bins]))?;
        if !truthy(&bin) {
            log(
                rt,
                format!(
                    "env activation: no project env found under /opt/miniconda3/envs (candidates: {}) -- agent runs with the image's PATH",
                    python_repr(&json!(bins))
                ),
            )?;
            json!({"mode":"none","requested":true,"candidates":bins})
        } else {
            let environment = function(rt, "activation_env", json!([bin, path]))?;
            let check = run(
                rt,
                json!([
                    "docker",
                    "exec",
                    "-e",
                    format!("PATH={}", scalar_text(&environment["PATH"])),
                    name,
                    "bash",
                    "-c",
                    "command -v python; python -c \"import sys; print(sys.prefix)\""
                ]),
                json!(120),
            )?;
            let parts: Vec<_> = output(&check["stdout"], "split")?
                .split(python_space)
                .filter(|s| !s.is_empty())
                .collect();
            let python = parts.first().map(|s| json!(s)).unwrap_or(Value::Null);
            let prefix = parts.get(1).map(|s| json!(s)).unwrap_or(Value::Null);
            let verified = python
                .as_str()
                .zip(bin.as_str())
                .is_some_and(|(python, bin)| python.starts_with(bin));
            let info = json!({"mode":"path-prepend","requested":true,"env_bin":bin,"env_name":environment["CONDA_DEFAULT_ENV"],"python":python,"verified":verified,"env":environment});
            if !verified {
                log(
                    rt,
                    format!(
                        "env activation: PATH prepend did not resolve python to {} (got {}, prefix {})",
                        scalar_text(&bin),
                        python_repr(&python),
                        python_repr(&prefix)
                    ),
                )?;
            }
            info
        }
    };
    rt.call("activation_put", json!([name, info]))?;
    Ok(info)
}
pub fn activation_args(name: &Value, rt: &mut dyn Runtime) -> Result<Value> {
    let info = rt.call("_env_activation_info", json!([name]))?;
    if mapping(&info, "get")?.get("mode") != Some(&json!("path-prepend")) {
        return Ok(json!([]));
    }
    let environment = info
        .get("env")
        .ok_or_else(|| Failure::new("KeyError", "env"))?;
    let mut args = Vec::new();
    for (key, value) in mapping(environment, "items")? {
        args.extend([json!("-e"), json!(format!("{key}={}", scalar_text(value)))]);
    }
    Ok(json!(args))
}
pub fn record_activation(
    path: &Path,
    name: &Value,
    cwd: &Path,
    rt: &mut dyn Runtime,
) -> Result<Value> {
    let info = rt.call("_env_activation_info", json!([name]))?;
    let mut info = info.as_object().cloned().ok_or_else(|| {
        Failure::new(
            "TypeError",
            format!("'{}' object is not iterable", python_type(&info)),
        )
    })?;
    info.remove("env");
    rt.call(
        "activation_row",
        json!(["env_activation", info.get("mode")]),
    )?;
    let path = path.join("run_config.json");
    let record = (|| -> Result<()> {
        let mut config = official::load(&path, cwd)?;
        if !config.is_object() {
            return Err(Failure::new(
                "TypeError",
                if config.is_array() {
                    "list indices must be integers or slices, not str".into()
                } else {
                    format!(
                        "'{}' object does not support item assignment",
                        python_type(&config)
                    )
                },
            ));
        }
        config["env_activation"] = json!(info);
        fs::write(cwd.join(&path), json_pretty(&config, false))
            .map_err(|e| Failure::for_path(e, &path))?;
        Ok(())
    })();
    match record {
        Err(error) if error.is_a("builtins.OSError") => log(
            rt,
            format!(
                "could not record env_activation in run_config.json: {}",
                error.message
            ),
        )?,
        Err(error) => return Err(error),
        _ => (),
    }
    Ok(Value::Null)
}

pub fn stop(name: &Value, rt: &mut dyn Runtime) -> Result<Value> {
    if truthy(&field(rt, "runner", "keep", "bool", None)?) {
        log(
            rt,
            format!(
                "keeping container {} (TE_KEEP_CONTAINERS=1)",
                scalar_text(name)
            ),
        )?;
    } else {
        let removal = (|| -> Result<()> {
            let timeout = timeout(rt, "TE_DOCKER_STOP_TIMEOUT", "600")?;
            run(rt, json!(["docker", "rm", "-f", name]), timeout)?;
            Ok(())
        })();
        if let Err(error) = removal {
            if !error.is_a("builtins.Exception") {
                return Err(error);
            }
            log(
                rt,
                format!(
                    "could not remove container {} ({}: {}) -- leaving it for _reap_exited; the task's row is unaffected",
                    scalar_text(name),
                    error.kind,
                    error.message
                ),
            )?;
        }
    }
    rt.call("live_discard", json!([name]))?;
    Ok(Value::Null)
}
pub fn install_path(which: &str, rt: &mut dyn Runtime) -> Result<Value> {
    let (key, default) = if which == "cache" {
        ("COPILOT_HOST_CACHE", "~/.cache/copilot")
    } else {
        ("COPILOT_HOME", "~/.copilot")
    };
    let value = env(rt, key, "")?;
    if truthy(&value) {
        Ok(value)
    } else {
        rt.call("expanduser", json!([default]))
    }
}
pub fn mounts(agent: &str, binary: &Value, rt: &mut dyn Runtime) -> Result<Value> {
    if agent == "codex" {
        return Ok(json!([
            "-v",
            format!("{}:/opt/codex-vendor:ro", scalar_text(binary))
        ]));
    }
    let cache = rt.call("copilot_function", json!(["copilot_cache", []]))?;
    let config = rt.call("copilot_function", json!(["copilot_config", []]))?;
    Ok(json!([
        "-v",
        format!("{}:/usr/local/bin/copilot:ro", scalar_text(binary)),
        "-v",
        format!("{}:/seed/copilot-cache:ro", scalar_text(&cache)),
        "-v",
        format!("{}:/seed/copilot-config:ro", scalar_text(&config))
    ]))
}
pub fn start(agent: &str, name: &Value, instance: &Value, rt: &mut dyn Runtime) -> Result<Value> {
    if !["claude", "copilot", "codex"].contains(&agent) {
        return Err(Failure::new(
            "RuntimeError",
            format!(
                "no SWE-bench runner for --agent {}; available: claude, codex, copilot",
                python_repr(&json!(agent))
            ),
        ));
    }
    run(rt, json!(["docker", "rm", "-f", name]), json!(120))?;
    rt.call("_reap_exited", json!([name]))?;
    let mut cmd = vec![
        json!("docker"),
        json!("run"),
        json!("-d"),
        json!("--name"),
        name.clone(),
        json!("--entrypoint"),
        json!("bash"),
    ];
    if agent == "claude" {
        let binary = field(rt, "runner", "claude", "json", None)?;
        cmd.extend([
            json!("-v"),
            json!(format!("{}:/usr/local/bin/claude:ro", scalar_text(&binary))),
        ]);
    } else {
        let attribute = if agent == "codex" {
            "codex_vendor"
        } else {
            "copilot"
        };
        let binary = field(rt, "runner", attribute, "json", None)?;
        let mounts = rt.call("agent_mounts", json!([agent, binary]))?;
        cmd.extend(
            mounts
                .as_array()
                .ok_or_else(|| Failure::new("TypeError", "mounts must be a sequence"))?
                .iter()
                .cloned(),
        );
    }
    let home = if agent == "copilot" {
        "/home/agent"
    } else {
        "/tmp/agent-home"
    };
    cmd.extend([
        json!("-e"),
        json!(format!("HOME={home}")),
        json!("-w"),
        json!("/testbed"),
    ]);
    let cpus = function(rt, "_cpu_limit_args", json!([]))?;
    cmd.extend(
        cpus.as_array()
            .ok_or_else(|| Failure::new("TypeError", "CPU arguments must be a sequence"))?
            .iter()
            .cloned(),
    );
    if agent != "copilot" && truthy(&field(rt, "runner", "scaffold", "present", None)?) {
        let specs = rt.call("scaffold_mounts", json!([]))?;
        for spec in specs
            .as_array()
            .ok_or_else(|| Failure::new("TypeError", "scaffold mounts must be a sequence"))?
        {
            cmd.extend([json!("-v"), spec.clone()]);
        }
    }
    rt.call("live_add", json!([name]))?;
    cmd.extend([
        function(rt, "image_for", json!([instance]))?,
        json!("-lc"),
        json!("sleep infinity"),
    ]);
    let timeout = timeout(rt, "TE_DOCKER_START_TIMEOUT", "900")?;
    let p = run(rt, json!(cmd), timeout)?;
    if number(&p["returncode"]) != Some(0.0) {
        return Err(Failure::new(
            "RuntimeError",
            format!(
                "docker run failed for {}: {}",
                scalar_text(instance),
                suffix(&p["stderr"], 500)?
            ),
        ));
    }
    if agent == "copilot" {
        let seed = rt.call("copilot_function", json!(["container_seed_setup", []]))?;
        let setup = format!(
            "mkdir -p /home/agent /home/agent/.copilot-runlog; {}",
            scalar_text(&seed)
        );
        let p = run(
            rt,
            json!(["docker", "exec", name, "bash", "-lc", setup]),
            json!(600),
        )?;
        if number(&p["returncode"]) != Some(0.0) {
            log(
                rt,
                format!(
                    "copilot seed setup warned: {}",
                    suffix(&json!(output(&p["stderr"], "slice")?), 300)?
                ),
            )?;
        }
    } else if agent == "codex" {
        let home = field(rt, "scaffold_profile", "home", "json", None)?;
        run(
            rt,
            json!([
                "docker",
                "exec",
                name,
                "mkdir",
                "-p",
                "/opt/codex-home",
                home
            ]),
            json!(120),
        )?;
        rt.call("_write_config", json!([name]))?;
    } else {
        run(
            rt,
            json!(["docker", "exec", name, "mkdir", "-p", "/tmp/agent-home"]),
            json!(120),
        )?;
    }
    rt.call("_activate_project_env", json!([name]))?;
    Ok(name.clone())
}

pub fn dispatch(method: &str, args: &Value, rt: &mut dyn Runtime) -> Result<Value> {
    let name = &args["name"];
    match method {
        "run" => run_process(&args["cmd"], &args["timeout"], &args["check"], rt),
        "exec" => run(
            rt,
            json!([
                "docker",
                "exec",
                "-w",
                args["workdir"],
                name,
                "bash",
                "-lc",
                args["script"]
            ]),
            args["timeout"].clone(),
        ),
        "start" => start(
            args["agent"]
                .as_str()
                .ok_or_else(|| Failure::new("TypeError", "expected agent label"))?,
            name,
            &args["instance_id"],
            rt,
        ),
        "stop" => stop(name, rt),
        "reap" => reap(name, rt),
        "activate" => activate(name, rt),
        "activation_info" => activation_info(
            &args["state"],
            name.as_str()
                .ok_or_else(|| Failure::new("TypeError", "expected container name"))?,
        ),
        "activation_args" => activation_args(name, rt),
        "record_activation" => record_activation(
            Path::new(
                args["run_dir"]
                    .as_str()
                    .ok_or_else(|| Failure::new("TypeError", "expected run directory"))?,
            ),
            name,
            Path::new(
                args["process_cwd"]
                    .as_str()
                    .ok_or_else(|| Failure::new("TypeError", "expected process cwd"))?,
            ),
            rt,
        ),
        "copilot_cache" => install_path("cache", rt),
        "copilot_config" => install_path("config", rt),
        "copilot_mounts" => mounts("copilot", &args["binary"], rt),
        "codex_mounts" => mounts("codex", &args["vendor"], rt),
        "copilot_seed" => Ok(json!(COPILOT_SEED)),
        _ => Err(Failure::new(
            "ValueError",
            format!("unknown container operation: {method}"),
        )),
    }
}
