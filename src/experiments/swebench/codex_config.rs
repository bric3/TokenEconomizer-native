use super::{Failure, Result, Runtime, settings::field};
use crate::value::python_space;
use crate::{
    toolprune::codex::CONFIG_CAPABILITIES,
    value::{base64, decimal_integer, json_text, python_repr, scalar_text, truthy},
};
use regex::Regex;
use serde_json::{Map, Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
    sync::LazyLock,
};

type Toggles = (Vec<String>, Vec<(String, String)>);

pub fn dispatch(method: &str, args: &Value, rt: &mut dyn Runtime) -> Result<Value> {
    let empty = Map::new();
    let env = args
        .get("environment")
        .and_then(Value::as_object)
        .unwrap_or(&empty);
    match method {
        "rtk_compliance" => super::codex_trace::rtk_compliance(
            &args["transcript_path"],
            Path::new(args["process_cwd"].as_str().unwrap_or(".")),
        ),
        "mcp_tool_calls" => super::codex_trace::mcp_tool_calls(
            &args["transcript_path"],
            &args["server"],
            Path::new(args["process_cwd"].as_str().unwrap_or(".")),
        ),
        "_unwrap_shell" => {
            let command = &args["cmd"];
            if !truthy(command) {
                return Ok(json!(""));
            }
            let command = command.as_str().ok_or_else(|| {
                Failure::new(
                    "AttributeError",
                    format!("'{}' object has no attribute 'strip'", kind(command)),
                )
            })?;
            Ok(json!(super::codex_trace::unwrap_shell(command)))
        }
        "network_block_toggles" => Ok(json!(network_toggles(env))),
        "config_toggles" => config_toggles(&args["capabilities"], env).map(|v| json!(v)),
        "lean_instructions" => lean_instructions(&args["base_text"], &args["capabilities"]),
        "skills_config_entries" => skills_entries(&args["system_skills"]).map(|v| json!(v)),
        "config_toml" => config_toml(args, env).map(|v| json!(v)),
        "write_config" => write_config(&args["name"], rt),
        "apply_scaffold" => apply_scaffold(&args["name"], &args["task"], &args["run_dir_path"], rt),
        "apply_codex_scaffold" => {
            apply_codex_scaffold(&args["name"], &args["task"], &args["run_dir_path"], rt)
        }
        _ => Err(Failure::new(
            "ValueError",
            format!("unknown Codex operation: {method}"),
        )),
    }
}

pub const HOME: &str = "/opt/codex-home";
pub const INSTRUCTIONS: &str = "/opt/codex-home/instructions.md";
const PROTECTED: &[&str] = &[
    "deferred_wait",
    "apply_patch",
    "exec_command",
    "planning",
    "write_stdin",
];
const SKILLS: &[&str] = &[
    "imagegen",
    "openai-docs",
    "plugin-creator",
    "review-agent",
    "skill-creator",
    "skill-installer",
];
const INTERACTIVE: &[&str] = &[
    "# Personality",
    "## Writing style",
    "## Technical communication",
    "# Working with the user",
    "## Intermediate commentary",
    "## Final answer",
    "### Formatting rules",
    "### Visualizations",
];

pub(super) fn constants() -> Value {
    json!({"CONFIG_CAPABILITIES":CONFIG_CAPABILITIES,"PROTECTED_CAPABILITIES":PROTECTED,
        "DEFAULT_SYSTEM_SKILLS":SKILLS})
}

pub(super) use crate::value::python_type as kind;
pub(super) fn sequence(value: &Value) -> Result<Vec<Value>> {
    match value {
        Value::Array(v) => Ok(v.clone()),
        Value::String(v) => Ok(v.chars().map(|c| json!(c.to_string())).collect()),
        Value::Object(v) => Ok(v.keys().map(|k| json!(k)).collect()),
        _ => Err(Failure::new(
            "TypeError",
            format!("'{}' object is not iterable", kind(value)),
        )),
    }
}
fn capabilities(value: &Value) -> Result<Vec<String>> {
    if !truthy(value) {
        return Ok(Vec::new());
    }
    // shortcut: capability identifiers follow the string schema; audit other element types before release.
    serde_json::from_value(json!(sequence(value)?))
        .map_err(|e| Failure::new("TypeError", e.to_string()))
}
fn mapping<'a>(value: &'a Value, method: &str) -> Result<&'a Map<String, Value>> {
    value.as_object().ok_or_else(|| {
        Failure::new(
            "AttributeError",
            format!("'{}' object has no attribute '{method}'", kind(value)),
        )
    })
}
fn required<'a>(value: &'a Value, key: &str) -> Result<&'a Value> {
    value.get(key).ok_or_else(|| Failure::new("KeyError", key))
}
fn text(value: &Value) -> Result<&str> {
    value
        .as_str()
        .ok_or_else(|| Failure::new("TypeError", format!("expected str, found {}", kind(value))))
}
pub(super) fn integer_with_limit(value: &Value, limit: usize) -> Result<String> {
    let number = integer(value)?;
    if let Some(text) = value.as_str()
        && limit != 0
    {
        let digits = text.chars().filter(|c| c.is_numeric()).count();
        if digits > limit {
            return Err(Failure::new(
                "ValueError",
                format!(
                    "Exceeds the limit ({limit} digits) for integer string conversion: value has {digits} digits; use sys.set_int_max_str_digits() to increase the limit"
                ),
            ));
        }
    }
    Ok(number)
}
pub(super) fn integer(value: &Value) -> Result<String> {
    if let Some(s) = value.as_str() {
        let normalized = decimal_integer(s).ok_or_else(|| {
            Failure::new(
                "ValueError",
                format!(
                    "invalid literal for int() with base 10: {}",
                    python_repr(value)
                ),
            )
        })?;
        let negative = normalized.starts_with('-');
        let digits = normalized
            .trim_start_matches(['-', '+'])
            .trim_start_matches('0');
        return Ok(if digits.is_empty() {
            "0".into()
        } else if negative {
            format!("-{digits}")
        } else {
            digits.into()
        });
    }
    if let Some(n) = value.as_i64() {
        return Ok(n.to_string());
    }
    if let Some(n) = value.as_u64() {
        return Ok(n.to_string());
    }
    if let Some(n) = value.as_bool() {
        return Ok(u8::from(n).to_string());
    }
    if let Some(n) = value.as_f64() {
        if n.is_nan() {
            return Err(Failure::new(
                "ValueError",
                "cannot convert float NaN to integer",
            ));
        }
        if n.is_infinite() {
            return Err(Failure::new(
                "OverflowError",
                "cannot convert float infinity to integer",
            ));
        }
        return Ok(format!("{:.0}", n.trunc()));
    }
    Err(Failure::new(
        "TypeError",
        format!(
            "int() argument must be a string, a bytes-like object or a real number, not '{}'",
            kind(value)
        ),
    ))
}
fn stable<T: PartialEq>(values: Vec<T>) -> Vec<T> {
    let mut out = Vec::new();
    for value in values {
        if !out.contains(&value) {
            out.push(value);
        }
    }
    out
}

pub fn network_toggles(env: &Map<String, Value>) -> Toggles {
    if env.get("TE_ALLOW_NETWORK_TOOLS").and_then(Value::as_str) == Some("1") {
        (Vec::new(), Vec::new())
    } else {
        (vec!["web_search = \"disabled\"".into()], Vec::new())
    }
}
pub fn config_toggles(caps: &Value, env: &Map<String, Value>) -> Result<Toggles> {
    let mut caps = capabilities(caps)?;
    let bad: BTreeSet<_> = caps
        .iter()
        .filter(|c| PROTECTED.contains(&c.as_str()))
        .cloned()
        .collect();
    if !bad.is_empty() {
        return Err(Failure::new(
            "RuntimeError",
            format!(
                "refusing to prune protected Codex capabilities {}: without an edit path and a shell there is no task, and the resulting 'resolved: false' would be a harness artifact rather than a mechanism result",
                python_repr(&json!(bad))
            ),
        ));
    }
    caps.retain(|cap| CONFIG_CAPABILITIES.contains(&cap.as_str()));
    caps.sort();
    let mut top = Vec::new();
    let mut tables = Vec::new();
    for cap in caps {
        match cap.as_str() {
            "collaboration" => {
                top.push("include_collaboration_mode_instructions = false".into());
                tables.extend([
                    ("agents".into(), "enabled = false".into()),
                    ("features".into(), "multi_agent = false".into()),
                ]);
            }
            "web" => top.push("web_search = \"disabled\"".into()),
            "goals" => tables.push(("features".into(), "goals = false".into())),
            "image_media" => tables.push(("features".into(), "view_image = false".into())),
            _ => (),
        }
    }
    let (net_top, net_tables) = network_toggles(env);
    top.extend(net_top);
    tables.extend(net_tables);
    Ok((stable(top), stable(tables)))
}
pub fn lean_instructions(base: &Value, caps: &Value) -> Result<Value> {
    let mut drop = BTreeSet::new();
    for cap in capabilities(caps)? {
        match cap.as_str() {
            "interactive_guidance" => drop.extend(INTERACTIVE.iter().copied()),
            "skills" => drop.extend(["# Using skills", "### How to use skills"]),
            _ => (),
        }
    }
    if !truthy(base) || drop.is_empty() {
        return Ok(Value::Null);
    }
    let base = base
        .as_str()
        .ok_or_else(|| Failure::new("TypeError", "expected string or bytes-like object"))?;
    static HEADER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^#{1,6} ").unwrap());
    static KEEP: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)When you run out of context|end your turn").unwrap());
    let starts: Vec<_> = std::iter::once(0)
        .chain(HEADER.find_iter(base).map(|m| m.start()))
        .chain(std::iter::once(base.len()))
        .collect();
    let mut out = String::new();
    let mut dropped = false;
    for pair in starts.windows(2) {
        let section = &base[pair[0]..pair[1]];
        let header = section
            .split('\n')
            .next()
            .unwrap_or("")
            .trim_matches(python_space);
        if drop.contains(header) {
            dropped = true;
            let kept: Vec<_> = section
                .split("\n\n")
                .filter(|p| KEEP.is_match(p))
                .map(|p| p.trim_matches(python_space))
                .collect();
            if !kept.is_empty() {
                out.push_str(&kept.join("\n\n"));
                out.push_str("\n\n");
            }
        } else {
            out.push_str(section);
        }
    }
    Ok(if dropped {
        json!(format!("{}\n", out.trim_end_matches(python_space)))
    } else {
        Value::Null
    })
}
pub fn skills_entries(skills: &Value) -> Result<Vec<String>> {
    let skills = if truthy(skills) {
        sequence(skills)?
    } else {
        SKILLS.iter().map(|s| json!(s)).collect()
    };
    Ok(skills
        .iter()
        .map(|name| {
            format!(
                "[[skills.config]]\npath = {}\nenabled = false\n",
                json_text(&json!(format!(
                    "{HOME}/skills/.system/{}/SKILL.md",
                    scalar_text(name)
                )))
            )
        })
        .collect())
}
pub fn config_toml(args: &Value, env: &Map<String, Value>) -> Result<String> {
    let caps = args.get("capabilities").unwrap_or(&Value::Null);
    let (top, tables) = config_toggles(caps, env)?;
    let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (table, line) in tables {
        grouped.entry(table).or_default().push(line);
    }
    let mut out = format!(
        "model = {}\nmodel_provider = \"openai_direct\"\nmodel_reasoning_effort = {}\n",
        json_text(required(args, "model")?),
        json_text(required(args, "effort")?)
    );
    if truthy(&args["instructions"]) {
        let path = args
            .get("instructions_path")
            .filter(|v| truthy(v))
            .cloned()
            .unwrap_or(json!(INSTRUCTIONS));
        out.push_str(&format!("model_instructions_file = {}\n", json_text(&path)));
    }
    for line in top {
        out.push_str(&line);
        out.push('\n');
    }
    let base = required(required(args, "routing")?, "base_url")?;
    out.push_str(&format!("\n[model_providers.openai_direct]\nname = \"OpenAI\"\nbase_url = {}\nenv_key = \"OPENAI_API_KEY\"\nwire_api = \"responses\"\n",json_text(base)));
    out.push_str(&format!(
        "\n[projects.{}]\ntrust_level = \"trusted\"\n",
        json_text(args.get("project").unwrap_or(&json!("/testbed")))
    ));
    let servers = &args["mcp_servers"];
    if truthy(servers) {
        let specs = mapping(servers, "getitem")?;
        let mut names: Vec<_> = specs.keys().collect();
        names.sort();
        for name in names {
            let spec = &specs[name];
            out.push_str(&format!(
                "\n[mcp_servers.{name}]\ncommand = {}\n",
                json_text(required(spec, "command")?)
            ));
            let params = sequence(mapping(spec, "get")?.get("args").unwrap_or(&json!([])))?;
            out.push_str(&format!(
                "args = [{}]\n",
                params.iter().map(json_text).collect::<Vec<_>>().join(", ")
            ));
            if truthy(&spec["cwd"]) {
                out.push_str(&format!("cwd = {}\n", json_text(&spec["cwd"])));
            }
            out.push_str(&format!(
                "startup_timeout_sec = {}\n",
                integer(spec.get("startup_timeout_sec").unwrap_or(&json!(90)))?
            ));
            if truthy(&spec["env"]) {
                out.push_str(&format!("\n[mcp_servers.{name}.env]\n"));
                let vars = mapping(&spec["env"], "getitem")?;
                let mut keys: Vec<_> = vars.keys().collect();
                keys.sort();
                for key in keys {
                    out.push_str(&format!(
                        "{key} = {}\n",
                        json_text(&json!(scalar_text(&vars[key])))
                    ));
                }
            }
        }
    }
    for (table, lines) in grouped {
        out.push_str(&format!("\n[{table}]\n"));
        for line in stable(lines) {
            out.push_str(&line);
            out.push('\n');
        }
    }
    if capabilities(caps)?.iter().any(|c| c == "skills") {
        out.push('\n');
        out.push_str(&skills_entries(&args["system_skills"])?.join("\n"));
    }
    Ok(out)
}

fn codex_function(rt: &mut dyn Runtime, name: &str, args: Value, kwargs: Value) -> Result<Value> {
    rt.call("codex_function", json!([name, args, kwargs]))
}
fn set(rt: &mut dyn Runtime, name: &str, value: Value) -> Result<()> {
    rt.call("config_set", json!([name, value])).map(|_| ())
}
pub fn write_config(name: &Value, rt: &mut dyn Runtime) -> Result<Value> {
    let base = field(rt, "runner", "base_prompt", "json", Some(Value::Null))?;
    let caps = field(rt, "runner", "_capabilities", "list", None)?;
    let lean = codex_function(rt, "lean_instructions", json!([base, caps]), json!({}))?;
    let count = if truthy(&lean) {
        text(&lean)?.chars().count() / 4
    } else {
        0
    };
    set(rt, "_lean_instructions_tokens", json!(count))?;
    let routing = field(rt, "runner", "routing", "json", None)?;
    let model = field(rt, "runner", "model", "json", None)?;
    let effort = field(rt, "runner", "effort", "json", None)?;
    let caps = field(rt, "runner", "_capabilities", "list", None)?;
    let skills = field(rt, "runner", "system_skills", "json", Some(Value::Null))?;
    let servers = field(rt, "runner", "_mcp_servers", "json", Some(Value::Null))?;
    let cfg = codex_function(
        rt,
        "config_toml",
        json!([]),
        json!({"routing":routing,"model":model,"effort":effort,"capabilities":caps,"system_skills":skills,"instructions":lean,"mcp_servers":servers}),
    )?;
    let mut script = format!(
        "mkdir -p {HOME} && echo {} | base64 -d > {HOME}/config.toml",
        base64(text(&cfg)?.as_bytes())
    );
    if truthy(&lean) {
        script.push_str(&format!(
            " && echo {} | base64 -d > {INSTRUCTIONS}",
            base64(text(&lean)?.as_bytes())
        ));
    }
    script.push_str(" && echo wrote");
    let result = rt.call("_exec", json!([name,script,{"timeout":60}]))?;
    let stdout = if truthy(&result["stdout"]) {
        text(&result["stdout"])?
    } else {
        ""
    };
    if !stdout.contains("wrote") {
        let stderr = if truthy(&result["stderr"]) {
            text(&result["stderr"])?
        } else {
            ""
        };
        let combined = format!("{stderr}{stdout}");
        let end: String = combined
            .chars()
            .skip(combined.chars().count().saturating_sub(200))
            .collect();
        return Err(Failure::new(
            "RuntimeError",
            format!("could not install the codex config.toml: {end}"),
        ));
    }
    Ok(cfg)
}
pub fn apply_scaffold(
    name: &Value,
    task: &Value,
    path: &Value,
    rt: &mut dyn Runtime,
) -> Result<Value> {
    if !truthy(&field(rt, "runner", "scaffold", "present", None)?) {
        return Ok(json!([{}, null]));
    }
    let setup = rt.call("scaffold_setup", json!([name, task, path]));
    if let Err(error) = setup {
        if !error.is_a("builtins.Exception") {
            return Err(error);
        }
        let iid = required(task, "instance_id")?;
        let scaffold = field(rt, "scaffold", "name", "json", None)?;
        rt.call(
            "log",
            json!([format!(
                "{}: scaffold {} FAILED ({}: {}) -- task runs native, row flagged",
                scalar_text(iid),
                scalar_text(&scaffold),
                error.kind,
                error.message
            )]),
        )?;
        let scaffold = field(rt, "scaffold", "name", "json", None)?;
        let message = format!("{}: {}", error.kind, error.message)
            .chars()
            .take(300)
            .collect::<String>();
        return Ok(json!([{"arm_setup":scaffold,"scaffold_error":message},null]));
    }
    rt.call("scaffold_resolve_mcp", json!(["mcp_config", null]))?;
    Ok(json!({"_scaffold_result":true}))
}
pub fn apply_codex_scaffold(
    name: &Value,
    task: &Value,
    path: &Value,
    rt: &mut dyn Runtime,
) -> Result<Value> {
    set(rt, "_mcp_servers", Value::Null)?;
    let result = rt.call("base_scaffold", json!([name, task, path]))?;
    if truthy(&result["is_dict"]) {
        let spec = rt.call("scaffold_notes_pop", json!(["mcp_servers", null]))?;
        if truthy(&spec) {
            rt.call("scaffold_use_popped", json!(["_mcp_servers"]))?;
            rt.call("_write_config", json!([name]))?;
            let mut names = sequence(&spec)?;
            names.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
            rt.call(
                "scaffold_notes_update",
                json!([{"mcp_wiring":"config.toml","mcp_servers_configured":names}]),
            )?;
        }
    }
    Ok(json!({"_scaffold_result":true}))
}
