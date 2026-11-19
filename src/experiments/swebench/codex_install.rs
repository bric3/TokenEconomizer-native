use super::{Failure, Result, Runtime};
use crate::value::python_space;
use crate::value::{python_repr, scalar_text, truthy};
use regex::Regex;
use serde_json::{Value, json};
use std::sync::LazyLock;

const HINT: &str = "\n  install:  npm i -g @openai/codex   (needs Node on the host)\n  override: TE_CODEX_BIN (the node shim) / TE_CODEX_VENDOR (the vendor triple dir)";
const DEAD_CONFIG: &str = "model = \"gpt-5.6-terra\"\nmodel_provider = \"dead\"\n[model_providers.dead]\nname = \"OpenAI\"\nbase_url = \"http://127.0.0.1:9/v1\"\nenv_key = \"TE_DEAD_KEY\"\nwire_api = \"responses\"\nrequest_max_retries = 0\n";

fn path(rt: &mut dyn Runtime, method: &str, args: Value) -> Result<Value> {
    rt.call("install_path", json!([method, args]))
}
fn function(rt: &mut dyn Runtime, name: &str, args: Value) -> Result<Value> {
    rt.call("install_function", json!([name, args]))
}
fn names(rt: &mut dyn Runtime, root: &Value) -> Result<Vec<String>> {
    let value = path(rt, "listdir", json!([root]))?;
    let mut values: Vec<String> =
        serde_json::from_value(value).map_err(|e| Failure::new("TypeError", e.to_string()))?;
    values.sort();
    Ok(values)
}
fn binary(rt: &mut dyn Runtime) -> Result<Value> {
    let mut binary = rt.call("install_env", json!(["TE_CODEX_BIN"]))?;
    if !truthy(&binary) {
        binary = rt.call("install_which", json!(["codex"]))?;
    }
    if !truthy(&binary) {
        return Err(Failure::new(
            "RuntimeError",
            format!("no codex binary found on PATH{HINT}"),
        ));
    }
    Ok(binary)
}
fn vendor(input: &Value, rt: &mut dyn Runtime) -> Result<Value> {
    let mut vendor = rt.call("install_env", json!(["TE_CODEX_VENDOR"]))?;
    if !truthy(&vendor) {
        let binary = if truthy(&input["binary"]) {
            input["binary"].clone()
        } else {
            function(rt, "codex_bin", json!([]))?
        };
        let shim = path(rt, "realpath", json!([binary]))?;
        let parent = path(rt, "dirname", json!([shim]))?;
        let package = path(rt, "dirname", json!([parent]))?;
        let base = path(rt, "join", json!([package, "node_modules", "@openai"]))?;
        let platforms = match names(rt, &base) {
            Ok(names) => names,
            Err(e) if e.is_a("builtins.OSError") => Vec::new(),
            Err(e) => return Err(e),
        };
        for platform in platforms.into_iter().filter(|n| n.starts_with("codex-")) {
            let root = path(rt, "join", json!([base, platform, "vendor"]))?;
            let triples = match names(rt, &root) {
                Ok(names) => names,
                Err(e) if e.is_a("builtins.OSError") => continue,
                Err(e) => return Err(e),
            };
            for triple in triples {
                let executable = path(rt, "join", json!([root, triple, "bin", "codex"]))?;
                if truthy(&path(rt, "isfile", json!([executable]))?) {
                    vendor = path(rt, "join", json!([root, triple]))?;
                    break;
                }
            }
            if truthy(&vendor) {
                break;
            }
        }
    }
    let present = if truthy(&vendor) {
        let executable = path(rt, "join", json!([vendor, "bin", "codex"]))?;
        truthy(&path(rt, "isfile", json!([executable]))?)
    } else {
        false
    };
    if !present {
        return Err(Failure::new(
            "RuntimeError",
            format!(
                "no codex native binary under vendor dir {} -- a bind mount of a missing path becomes an EMPTY dir in the container, so every task would fail with 'codex: not found' behind a green launch banner.{HINT}",
                python_repr(&vendor)
            ),
        ));
    }
    Ok(vendor)
}
fn kill(rt: &mut dyn Runtime) -> Result<Value> {
    match rt.call("install_killpg", json!([])) {
        Err(e)
            if e.is_a("builtins.ProcessLookupError")
                || e.is_a("builtins.PermissionError")
                || e.is_a("builtins.AttributeError") => {}
        Err(e) => return Err(e),
        Ok(_) => {}
    }
    match rt.call("install_wait", json!([10])) {
        Err(e) if e.is_a("subprocess.TimeoutExpired") => {}
        Err(e) => return Err(e),
        Ok(_) => {}
    }
    Ok(Value::Null)
}
fn remove(root: &Value, rt: &mut dyn Runtime) -> Result<Value> {
    for _ in 0..3 {
        match rt.call("install_rmtree", json!([root, false])) {
            Ok(_) => return Ok(Value::Null),
            Err(e) if e.is_a("builtins.OSError") => {
                rt.call("install_sleep", json!([0.2]))?;
            }
            Err(e) => return Err(e),
        }
    }
    rt.call("install_rmtree", json!([root, true]))?;
    Ok(Value::Null)
}
fn base_prompt(input: &Value, rt: &mut dyn Runtime) -> Result<Value> {
    static VERSION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d+\.\d+\.\d+").unwrap());
    let text = if truthy(&input["cli_version"]) {
        scalar_text(&input["cli_version"])
    } else {
        String::new()
    };
    let Some(version) = VERSION.find(&text) else {
        return Ok(Value::Null);
    };
    let Some(root) = crate::experiments::assets::installed()? else {
        return Ok(json!(crate::experiments::assets::capture(version.as_str())));
    };
    let file = json!(
        root.join("codex_base_prompt")
            .join(format!("{}.md", version.as_str()))
    );
    if !truthy(&path(rt, "exists", json!([file]))?) {
        return Ok(Value::Null);
    }
    rt.call("install_read", json!([file]))
}
fn probe(input: &Value, skills: bool, rt: &mut dyn Runtime) -> Result<Value> {
    let binary = if truthy(&input["binary"]) {
        input["binary"].clone()
    } else {
        function(rt, "codex_bin", json!([]))?
    };
    let tmp = rt.call("install_mkdtemp", json!([input["home"]]))?;
    // Cleanup also runs when writing, spawning, polling or collecting output fails.
    let result = (|| {
        let config = path(rt, "join", json!([tmp, "config.toml"]))?;
        rt.call(
            "install_write",
            json!([
                config,
                if skills {
                    json!(DEAD_CONFIG)
                } else {
                    input["text"].clone()
                }
            ]),
        )?;
        let environment = if skills {
            json!({"CODEX_HOME":tmp,"TE_DEAD_KEY":"dead"})
        } else {
            json!({"CODEX_HOME":tmp})
        };
        rt.call("install_environment", environment)?;
        let root = if skills {
            path(rt, "join", json!([tmp, "skills", ".system"]))?
        } else {
            Value::Null
        };
        let marker = if skills {
            path(rt, "join", json!([root, ".codex-system-skills.marker"]))?
        } else {
            Value::Null
        };
        let mut argv = vec![binary, json!("exec")];
        if !skills {
            argv.push(json!("--strict-config"));
        }
        argv.extend([
            json!("--skip-git-repo-check"),
            json!("-C"),
            tmp.clone(),
            json!("probe"),
        ]);
        match rt.call("install_spawn", json!([argv, !skills])) {
            Ok(_) => {}
            Err(e) if e.is_a("builtins.OSError") => {
                return Ok(if skills {
                    json!([])
                } else {
                    json!(format!("could not run the config validator: {}", e.message))
                });
            }
            Err(e) => return Err(e),
        }
        let output = if skills {
            let observed: Result<Value> = (|| {
                let start = rt.call("install_clock", json!([]))?;
                // shortcut: audit foreign numeric protocols and non-finite timeouts before release.
                let timeout = match &input["timeout"] {
                    Value::Number(n) => n.as_f64().unwrap_or(f64::INFINITY),
                    Value::Bool(b) => {
                        if *b {
                            1.0
                        } else {
                            0.0
                        }
                    }
                    other => {
                        return Err(Failure::new(
                            "TypeError",
                            format!(
                                "unsupported operand type(s) for +: 'float' and '{}'",
                                crate::value::python_type(other)
                            ),
                        ));
                    }
                };
                let deadline = start.as_f64().unwrap_or(0.0) + timeout;
                loop {
                    let now = rt.call("install_clock", json!([]))?;
                    if now.as_f64().unwrap_or(0.0) >= deadline
                        || !rt.call("install_poll", json!([]))?.is_null()
                    {
                        break;
                    }
                    if truthy(&path(rt, "exists", json!([marker]))?) {
                        break;
                    }
                    rt.call("install_sleep", json!([0.2]))?;
                }
                Ok(Value::Null)
            })();
            rt.call("install_cleanup_process", json!([]))?;
            observed?
        } else {
            let output = rt.call("install_communicate", json!([input["timeout"]]));
            rt.call("install_cleanup_process", json!([]))?;
            match output {
                Err(e) if e.is_a("subprocess.TimeoutExpired") => {
                    return Ok(json!(format!(
                        "could not run the config validator: {}",
                        e.message
                    )));
                }
                result => result?,
            }
        };
        if skills {
            if !truthy(&path(rt, "isdir", json!([root]))?) {
                return Ok(json!([]));
            }
            let value = path(rt, "listdir", json!([root]))?;
            let entries = value
                .as_array()
                .ok_or_else(|| Failure::new("TypeError", "expected directory entries"))?;
            let mut found = Vec::new();
            for name in entries {
                let file = path(rt, "join", json!([root, name, "SKILL.md"]))?;
                if truthy(&path(rt, "isfile", json!([file]))?) {
                    found.push(name.clone());
                }
            }
            found.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
            Ok(json!(found))
        } else {
            let blob = format!(
                "{}{}",
                output[0].as_str().unwrap_or(""),
                output[1].as_str().unwrap_or("")
            );
            if [
                "unknown configuration field",
                "invalid type",
                "unknown variant",
                "Error loading config.toml",
            ]
            .iter()
            .any(|pattern| blob.contains(pattern))
            {
                Ok(json!(
                    blob.trim_matches(python_space)
                        .chars()
                        .take(800)
                        .collect::<String>()
                ))
            } else {
                Ok(json!(""))
            }
        }
    })();
    function(rt, "_rmtree_quietly", json!([tmp]))?;
    result
}
pub fn dispatch(method: &str, input: &Value, rt: &mut dyn Runtime) -> Result<Value> {
    match method {
        "codex_bin" => binary(rt),
        "codex_vendor" => vendor(input, rt),
        "require_install" => {
            let binary = function(rt, "codex_bin", json!([]))?;
            let vendor = function(rt, "codex_vendor", json!([binary]))?;
            Ok(json!([binary, vendor]))
        }
        "cli_version" => {
            let result: Result<Value> = (|| {
                let output = rt.call("install_version", json!([input["binary"]]))?;
                let text = output[0]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .or_else(|| output[1].as_str())
                    .unwrap_or("");
                Ok(json!(
                    text.trim_matches(python_space)
                        .split([
                            '\n', '\r', '\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}', '\u{85}',
                            '\u{2028}', '\u{2029}'
                        ])
                        .next()
                        .unwrap_or("")
                        .chars()
                        .take(80)
                        .collect::<String>()
                ))
            })();
            match result {
                Err(e) if e.is_a("builtins.Exception") => Ok(json!("")),
                other => other,
            }
        }
        "base_prompt_for" => base_prompt(input, rt),
        "bundled_system_skills" => probe(input, true, rt),
        "validate_config" => probe(input, false, rt),
        "_kill_process_group" => kill(rt),
        "_rmtree_quietly" => remove(&input["path"], rt),
        _ => Err(Failure::new(
            "ValueError",
            format!("unknown Codex installation operation: {method}"),
        )),
    }
}
