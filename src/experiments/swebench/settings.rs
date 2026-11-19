use super::{Failure, Result, Runtime, basename, dirname};
use crate::value::python_space;
use crate::{
    experiments::adapters::NETWORK_TOOLS,
    value::{decimal_integer, json_pretty, python_type, truthy},
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::PathBuf};

fn setting<'a>(env: &'a Map<String, Value>, key: &str, default: &'a str) -> &'a str {
    env.get(key).and_then(Value::as_str).unwrap_or(default)
}
fn strip(text: &str) -> &str {
    text.trim_matches(python_space)
}
fn sequence(value: &Value) -> Result<Vec<Value>> {
    match value {
        Value::Array(values) => Ok(values.clone()),
        Value::Object(values) => Ok(values.keys().map(|key| json!(key)).collect()),
        Value::String(text) => Ok(text.chars().map(|c| json!(c.to_string())).collect()),
        _ => Err(Failure::new(
            "TypeError",
            format!("'{}' object is not iterable", python_type(value)),
        )),
    }
}
fn length(value: &Value) -> Result<usize> {
    match value {
        Value::String(text) => Ok(text.chars().count()),
        Value::Array(values) => Ok(values.len()),
        Value::Object(values) => Ok(values.len()),
        _ => Err(Failure::new(
            "TypeError",
            format!("object of type '{}' has no len()", python_type(value)),
        )),
    }
}
fn path(value: &Value) -> Result<&str> {
    value.as_str().ok_or_else(|| {
        Failure::new(
            "TypeError",
            format!(
                "expected str, bytes or os.PathLike object, not {}",
                python_type(value)
            ),
        )
    })
}
pub fn env_activation_enabled(env: &Map<String, Value>) -> bool {
    setting(env, "TE_ENV_ACTIVATION", "1") != "0"
}
pub fn network_tool_block(env: &Map<String, Value>) -> Vec<&'static str> {
    if ["1", "true", "yes"].contains(&strip(setting(env, "TE_ALLOW_NETWORK_TOOLS", ""))) {
        Vec::new()
    } else {
        NETWORK_TOOLS.to_vec()
    }
}
// shortcut: environment paths use POSIX rules; complete Windows path parity before release.
pub fn choose_project_env(env_bins: &Value) -> Result<Value> {
    let bins: Vec<_> = sequence(env_bins)?.into_iter().filter(truthy).collect();
    for bin in &bins {
        if basename(dirname(path(bin)?)) == "testbed" {
            return Ok(bin.clone());
        }
    }
    Ok(if bins.len() == 1 {
        bins[0].clone()
    } else {
        Value::Null
    })
}
pub fn activation_env(env_bin: &Value, container_path: &Value) -> Result<Value> {
    let bin = path(env_bin)?;
    let prefix = dirname(bin);
    let mut path = bin.to_owned();
    if truthy(container_path) {
        let inherited = container_path.as_str().ok_or_else(|| {
            Failure::new(
                "TypeError",
                format!(
                    "can only concatenate str (not \"{}\") to str",
                    python_type(container_path)
                ),
            )
        })?;
        path.push(':');
        path.push_str(inherited);
    }
    Ok(json!({"PATH":path,"CONDA_PREFIX":prefix,"CONDA_DEFAULT_ENV":basename(prefix)}))
}
pub fn image_for(instance_id: &Value) -> Result<String> {
    let id = instance_id.as_str().ok_or_else(|| {
        Failure::new(
            "AttributeError",
            format!(
                "'{}' object has no attribute 'replace'",
                python_type(instance_id)
            ),
        )
    })?;
    Ok(format!(
        "swebench/sweb.eval.x86_64.{}:latest",
        id.replace("__", "_1776_")
    ))
}

fn cpu_number(text: &str) -> Option<f64> {
    let text: String = strip(text)
        .chars()
        .map(|c| {
            if c.is_ascii() {
                c.to_string()
            } else {
                decimal_integer(c.encode_utf8(&mut [0; 4])).unwrap_or_else(|| c.to_string())
            }
        })
        .collect();
    let bytes = text.as_bytes();
    for (i, byte) in bytes.iter().enumerate() {
        if *byte == b'_'
            && (i == 0
                || i + 1 == bytes.len()
                || !bytes[i - 1].is_ascii_digit()
                || !bytes[i + 1].is_ascii_digit())
        {
            return None;
        }
    }
    text.replace('_', "").parse().ok()
}
pub fn cpu_limit_args(env: &Map<String, Value>) -> Result<Vec<String>> {
    let value = cpu_number(setting(env, "TE_SWEBENCH_MAX_CPUS", "16"))
        .filter(|n| !n.is_nan())
        .unwrap_or(16.0);
    if value.is_infinite() {
        return Err(Failure::new(
            "OverflowError",
            "cannot convert float infinity to integer",
        ));
    }
    let value = value.trunc();
    if value <= 0.0 {
        return Ok(Vec::new());
    }
    let limit = format!("{value:.0}");
    let mut args = vec!["--cpus".into(), limit.clone()];
    for key in [
        "OMP_NUM_THREADS",
        "MKL_NUM_THREADS",
        "OPENBLAS_NUM_THREADS",
        "NUMEXPR_NUM_THREADS",
        "LOKY_MAX_CPU_COUNT",
    ] {
        args.extend(["-e".into(), format!("{key}={limit}")]);
    }
    Ok(args)
}

#[derive(Deserialize)]
pub struct RunConfig {
    pub task: Value,
    pub arm: Value,
    pub injected: Value,
    pub exclude_tools: Value,
    pub max_turns: Value,
    pub run_dir: PathBuf,
    pub process_cwd: PathBuf,
}
pub(super) fn field(
    runtime: &mut dyn Runtime,
    owner: &str,
    name: &str,
    format: &str,
    default: Option<Value>,
) -> Result<Value> {
    let mut args = vec![json!(owner), json!(name), json!(format)];
    if let Some(default) = default {
        args.push(default);
    }
    runtime.call("config_attr", json!(args))
}
fn function(runtime: &mut dyn Runtime, name: &str, args: Value) -> Result<Value> {
    runtime.call("config_function", json!([name, args]))
}
fn kwarg(runtime: &mut dyn Runtime, name: &str, format: &str) -> Result<Value> {
    runtime.call("config_kwarg", json!([name, format]))
}

pub fn patch_excludes(runtime: &mut dyn Runtime) -> Result<Value> {
    if !truthy(&field(runtime, "runner", "scaffold", "present", None)?) {
        return Ok(json!([]));
    }
    let profile = field(runtime, "scaffold_profile", "patch_excludes", "list", None)?;
    let scaffold = field(runtime, "scaffold", "patch_excludes", "list", None)?;
    // shortcut: scaffold paths follow the string schema; audit other element types before release.
    let mut excludes: BTreeSet<String> =
        serde_json::from_value(profile).map_err(|e| Failure::new("TypeError", e.to_string()))?;
    let scaffold: Vec<String> =
        serde_json::from_value(scaffold).map_err(|e| Failure::new("TypeError", e.to_string()))?;
    excludes.extend(scaffold);
    Ok(json!(excludes))
}

pub fn write_run_config(input: &RunConfig, runtime: &mut dyn Runtime) -> Result<Value> {
    let excluded = if truthy(&input.exclude_tools) {
        sequence(&input.exclude_tools)?
    } else {
        Vec::new()
    };
    let mut drop = Vec::new();
    for tool in &excluded {
        let protected = field(runtime, "runner", "protected_tools", "list", None)?;
        if !sequence(&protected)?.contains(tool) {
            drop.push(tool.clone());
        }
    }
    for tool in sequence(&function(runtime, "network_tool_block", json!([]))?)? {
        if !drop.contains(&tool) {
            drop.push(tool);
        }
    }
    let task = input.task.as_object().ok_or_else(|| {
        Failure::new(
            "AttributeError",
            format!(
                "'{}' object has no attribute 'get'",
                python_type(&input.task)
            ),
        )
    })?;
    let mut config =
        json!({"instance_id":task.get("instance_id"),"repo":task.get("repo"),"arm":input.arm});
    for name in ["agent", "model", "effort"] {
        config[name] = field(runtime, "runner", name, "json", None)?;
    }
    config["max_turns"] = input.max_turns.clone();
    for name in ["cli_version", "agent_bin"] {
        config[name] = field(runtime, "runner", name, "json", None)?;
    }
    let iid = task
        .get("instance_id")
        .ok_or_else(|| Failure::new("KeyError", "instance_id"))?;
    config["image"] = function(runtime, "image_for", json!([iid]))?;
    let cpu = function(runtime, "_cpu_limit_args", json!([]))?;
    config["cpu_limit"] = if truthy(&cpu) {
        sequence(&function(runtime, "_cpu_limit_args", json!([]))?)?
            .get(1)
            .cloned()
            .ok_or_else(|| Failure::new("IndexError", "list index out of range"))?
    } else {
        Value::Null
    };
    let base = field(runtime, "runner", "routing_base", "json", None)?;
    let model = field(runtime, "runner", "optimizer_model", "json", None)?;
    let transport = field(
        runtime,
        "runner",
        "optimizer_transport",
        "json",
        Some(json!("api")),
    )?;
    config["routing"] =
        json!({"base_url":base,"optimizer_model":model,"optimizer_transport":transport});
    config["env_activation"] =
        json!({"requested":function(runtime,"env_activation_enabled",json!([]))?,"mode":null});
    let protected = field(runtime, "runner", "protected_tools", "list", None)?;
    // shortcut: protected tool names follow the string schema; audit other element types before release.
    let mut protected: Vec<String> =
        serde_json::from_value(protected).map_err(|e| Failure::new("TypeError", e.to_string()))?;
    protected.sort();
    config["tools"] =
        json!({"excluded_requested":excluded,"excluded_applied":drop,"protected":protected});
    let chars = if truthy(&input.injected) {
        length(&input.injected)?
    } else {
        0
    };
    let manual = if truthy(&input.injected) {
        input.injected.as_str().ok_or_else(|| {
            Failure::new(
                "AttributeError",
                format!(
                    "'{}' object has no attribute 'encode'",
                    python_type(&input.injected)
                ),
            )
        })?
    } else {
        ""
    };
    let fingerprint = if truthy(&input.injected) {
        json!(format!("{:x}", Sha256::digest(manual.as_bytes()))[..16])
    } else {
        Value::Null
    };
    config["injection"] =
        json!({"present":truthy(&input.injected),"chars":chars,"sha256":fingerprint});
    let guard = kwarg(runtime, "spiral_guard", "bool")?;
    let envelope = kwarg(runtime, "abort_envelope", "json")?;
    let thresholds = kwarg(runtime, "spiral_thr", "json")?;
    let judge = kwarg(runtime, "spiral_judge", "present")?;
    let writer = kwarg(runtime, "spiral_nudge_writer", "present")?;
    let budget = field(runtime, "guard", "nudge_budget", "json", Some(json!(0)))?;
    let never_abort = field(runtime, "guard", "never_abort", "json", Some(json!(false)))?;
    config["spiral"] = json!({"guard":guard,"envelope":envelope,"thresholds":thresholds,"judge":judge,"nudge_writer_llm":writer,"nudge_budget":budget,"never_abort":never_abort});
    let scaffold = field(runtime, "runner", "scaffold", "bool", None)?;
    let scaffold = if truthy(&scaffold) {
        field(runtime, "scaffold", "name", "json", None)?
    } else {
        Value::Null
    };
    let excludes = runtime.call("config_patch_excludes", json!([]))?;
    config["scaffold"] = json!({"name":scaffold,"patch_excludes":excludes});
    let task_timeout = field(runtime, "runner", "task_timeout", "json", None)?;
    let eval_timeout = field(runtime, "runner", "eval_timeout", "json", None)?;
    config["timeouts"] = json!({"task_s":task_timeout,"eval_s":eval_timeout});
    config["grading_source"] = field(runtime, "grader", "path", "json", None)?;
    let path = input.run_dir.join("run_config.json");
    if path.as_os_str().as_encoded_bytes().contains(&0) {
        return Err(Failure::new("ValueError", "embedded null byte"));
    }
    fs::write(input.process_cwd.join(&path), json_pretty(&config, false))
        .map_err(|error| Failure::for_path(error, &path))?;
    Ok(Value::Null)
}
