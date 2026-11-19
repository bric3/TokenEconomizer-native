use super::{Failure, Result, Runtime, codex_config::integer_with_limit};
use crate::value::{python_repr, scalar_text, truthy};
use serde_json::{Value, json};

mod web;

pub fn constants() -> Value {
    let mut codex = super::codex_config::constants();
    codex["EFFORT_LEVELS"] = json!(["minimal", "low", "medium", "high", "xhigh", "max"]);
    codex["PROTECTED_TOOLS"] = json!(["shell", "apply_patch"]);
    json!({
        "claude":{"EFFORT_LEVELS":["low","medium","high","xhigh","max"],"PROTECTED_TOOLS":["Bash","Edit","Write","Read"]},
        "copilot":{"EFFORT_LEVELS":["none","low","medium","high","xhigh","max"],
            "CONTAINER_LOG_DIR":"/home/agent/.copilot-runlog"},
        "codex":codex,
        "web":{"WEBBENCH_SCAFFOLDS":["rtk"],"WEBBENCH_UNDICI_ABORT_DEFAULT_S":300,
            "WEBBENCH_TASK_TIMEOUT_HEADROOM_S":40,"WEBBENCH_TASK_TIMEOUT_DEFAULT":240}
    })
}

const OPTIMIZER_MODEL_ERROR: &str = "TE_OPTIMIZER_MODEL is no longer honoured: the between-task optimizer runs on the agent's own model so mechanism overhead is priced on the same basis as the task rows. Unset it.";

fn field(name: &str) -> Value {
    json!(["field", name])
}
fn arg(name: &str) -> Value {
    json!(["arg", name])
}
fn module(name: &str) -> Value {
    json!(["module", name])
}
fn value(value: Value) -> Value {
    json!(["value", value])
}
fn read(rt: &mut dyn Runtime, reference: Value) -> Result<Value> {
    rt.call("setup_read", reference)
}
fn boolean(rt: &mut dyn Runtime, reference: Value) -> Result<bool> {
    rt.call("setup_bool", reference).map(|v| truthy(&v))
}
fn assign(rt: &mut dyn Runtime, name: &str, reference: Value) -> Result<()> {
    rt.call("setup_assign", json!([name, reference]))
        .map(|_| ())
}
fn literal(rt: &mut dyn Runtime, name: &str, data: Value) -> Result<()> {
    assign(rt, name, value(data))
}
fn result(rt: &mut dyn Runtime, name: &str) -> Result<()> {
    assign(rt, name, json!(["result"]))
}
fn call(rt: &mut dyn Runtime, name: &str, args: Value, kwargs: Value) -> Result<()> {
    rt.call("setup_call", json!([name, args, kwargs]))
        .map(|_| ())
}
fn apply(rt: &mut dyn Runtime, name: &str, args: Value) -> Result<()> {
    call(rt, name, args, json!({}))
}
fn env(rt: &mut dyn Runtime, name: &str, default: Option<&str>) -> Result<Value> {
    let args = if let Some(default) = default {
        json!([name, default])
    } else {
        json!([name])
    };
    rt.call("setup_env", args)
}
fn log(rt: &mut dyn Runtime, text: String) -> Result<()> {
    apply(rt, "_log", json!([value(json!(text))]))
}
fn text(rt: &mut dyn Runtime, name: &str) -> Result<String> {
    read(rt, field(name)).map(|v| scalar_text(&v))
}
fn reject_optimizer_override(rt: &mut dyn Runtime) -> Result<()> {
    if truthy(&env(rt, "TE_OPTIMIZER_MODEL", None)?) {
        return Err(Failure::new("RuntimeError", OPTIMIZER_MODEL_ERROR));
    }
    Ok(())
}
fn effort(rt: &mut dyn Runtime, agent: &str, target: &str) -> Result<()> {
    let mut selected = env(rt, "TE_EFFORT_LEVEL", None)?;
    if !truthy(&selected) {
        selected = env(
            rt,
            &format!(
                "{}_EFFORT_LEVEL",
                if agent == "claude" {
                    "CLAUDE_CODE"
                } else if agent == "copilot" {
                    "COPILOT"
                } else {
                    "CODEX"
                }
            ),
            None,
        )?;
    }
    if !truthy(&selected) {
        selected = json!("high");
    }
    literal(rt, target, selected)?;
    let owner = if agent == "claude" {
        "EFFORT_LEVELS".into()
    } else {
        format!("{agent}.EFFORT_LEVELS")
    };
    let mut levels = read(rt, module(&owner))?
        .as_array()
        .cloned()
        .unwrap_or_default();
    let selected = read(rt, field(target))?;
    if !levels.contains(&selected) {
        levels.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
        return Err(Failure::new(
            "RuntimeError",
            format!(
                "TE_EFFORT_LEVEL={} is not one of {}",
                python_repr(&selected),
                python_repr(&json!(levels))
            ),
        ));
    }
    Ok(())
}
fn scaffold(rt: &mut dyn Runtime) -> Result<()> {
    if boolean(rt, arg("scaffold"))? {
        assign(rt, "scaffold_name", arg("scaffold"))?;
    } else {
        literal(rt, "scaffold_name", Value::Null)?;
    }
    apply(rt, "scaffolds_pkg.get", json!([field("scaffold_name")]))?;
    result(rt, "scaffold")?;
    if truthy(&rt.call("setup_present", field("scaffold"))?) {
        apply(
            rt,
            "self.scaffold.preflight",
            json!([field("scaffold_profile")]),
        )?;
        let name = read(rt, json!(["nested_field", "scaffold", "name"]))?;
        log(
            rt,
            format!("scaffold: {} (external baseline)", scalar_text(&name)),
        )?;
    }
    Ok(())
}
fn integer_env(rt: &mut dyn Runtime, target: &str, variable: &str, default: &str) -> Result<()> {
    let input = env(rt, variable, Some(default))?;
    let limit = rt
        .call("setup_integer_limit", json!([]))?
        .as_u64()
        .unwrap_or(0) as usize;
    let number = integer_with_limit(&input, limit)?;
    assign(rt, target, json!(["integer", number]))
}
fn timeouts(rt: &mut dyn Runtime) -> Result<()> {
    integer_env(rt, "task_timeout", "TE_TASK_TIMEOUT", "1800")?;
    integer_env(rt, "eval_timeout", "TE_EVAL_TIMEOUT", "1800")?;
    let keep = env(rt, "TE_KEEP_CONTAINERS", None)? == "1";
    literal(rt, "keep", json!(keep))
}
fn grader(rt: &mut dyn Runtime) -> Result<()> {
    apply(rt, "_Grader", json!([arg("grading_path")]))?;
    result(rt, "grader")
}
fn model(rt: &mut dyn Runtime, agent: &str) -> Result<()> {
    if boolean(rt, arg("model"))? {
        assign(rt, "model", arg("model"))?;
    } else {
        let variable = if agent == "claude" {
            "ANTHROPIC_MODEL"
        } else {
            "CODEX_MODEL"
        };
        let default = if agent == "claude" {
            Some("claude-sonnet-5")
        } else {
            None
        };
        let mut selected = env(rt, variable, default)?;
        if agent == "codex" && !truthy(&selected) {
            selected = json!("gpt-5.6-sol");
        }
        literal(rt, "model", selected)?;
    }
    Ok(())
}
fn claude(rt: &mut dyn Runtime) -> Result<()> {
    model(rt, "claude")?;
    apply(rt, "resolve_anthropic_key", json!([]))?;
    result(rt, "api_key")?;
    reject_optimizer_override(rt)?;
    assign(rt, "optimizer_model", field("model"))?;
    effort(rt, "claude", "effort")?;
    scaffold(rt)?;
    apply(rt, "claude_bin", json!([]))?;
    result(rt, "claude")?;
    apply(
        rt,
        "pricing.weights_for",
        json!([["mapping",{"model":field("model"),"agent":field("agent")}]]),
    )?;
    assign(rt, "agent_bin", field("claude"))?;
    assign(rt, "protected_tools", module("PROTECTED_TOOLS"))?;
    assign(rt, "routing_base", module("_ANTHROPIC_DIRECT"))?;
    apply(rt, "cli_version", json!([field("claude")]))?;
    result(rt, "cli_version")?;
    grader(rt)?;
    integer_env(
        rt,
        "optimizer_max_tokens",
        "TE_OPTIMIZER_MAX_TOKENS",
        "16384",
    )?;
    timeouts(rt)?;
    literal(rt, "optimizer_transport", json!("api"))?;
    apply(rt, "self._preflight_optimizer_endpoint", json!([]))
}
fn copilot(rt: &mut dyn Runtime) -> Result<()> {
    // Normalisation precedes assigning self.model in the source constructor.
    let input = if boolean(rt, arg("model"))? {
        arg("model")
    } else {
        value(env(rt, "COPILOT_MODEL", None)?)
    };
    apply(rt, "copilot.normalize_model", json!([input]))?;
    result(rt, "model")?;
    apply(
        rt,
        "copilot.validate_priceable",
        json!([field("model"), field("agent")]),
    )?;
    reject_optimizer_override(rt)?;
    if boolean(rt, field("model"))? {
        assign(rt, "optimizer_model", field("model"))?;
    } else {
        literal(rt, "optimizer_model", json!("auto"))?;
    }
    effort(rt, "copilot", "effort_requested")?;
    apply(rt, "copilot.effort_applies", json!([field("model")]))?;
    if boolean(rt, json!(["result"]))? {
        assign(rt, "effort", field("effort_requested"))?;
    } else {
        literal(rt, "effort", Value::Null)?;
    }
    if !truthy(&rt.call("setup_present", field("effort"))?) {
        let why = if env(rt, "TE_EFFORT_APPLIES", None)? == "0" {
            "the launcher probed this model and the CLI rejected --effort"
        } else {
            "the 'auto' router takes no effort level; pin COPILOT_MODEL to a served model id to control it"
        };
        let effort = text(rt, "effort_requested")?;
        let model = read(rt, field("model"))?;
        log(
            rt,
            format!(
                "copilot: TE_EFFORT_LEVEL={effort} NOT applied (model={}, recorded as null) -- {why}.",
                if truthy(&model) {
                    scalar_text(&model)
                } else {
                    "auto".into()
                }
            ),
        )?;
    }
    scaffold(rt)?;
    call(
        rt,
        "copilot.require_install",
        json!([]),
        json!({"need_container_seed":value(json!(true))}),
    )?;
    result(rt, "copilot")?;
    assign(rt, "agent_bin", field("copilot"))?;
    assign(rt, "protected_tools", module("copilot.PROTECTED_TOOLS"))?;
    apply(rt, "copilot.provider_base", json!([]))?;
    result(rt, "routing_base")?;
    apply(rt, "copilot.anthropic_optimizer_endpoint", json!([]))?;
    result(rt, "optimizer_endpoint")?;
    let endpoint = boolean(rt, field("optimizer_endpoint"))?;
    literal(
        rt,
        "optimizer_transport",
        json!(if endpoint { "api" } else { "cli" }),
    )?;
    if boolean(rt, field("optimizer_endpoint"))? {
        let endpoint = read(rt, json!(["item", "optimizer_endpoint", 0]))?;
        log(
            rt,
            format!(
                "copilot: mechanism calls -> {} (the BYOK provider, direct Messages API)",
                scalar_text(&endpoint)
            ),
        )?;
        apply(rt, "self._preflight_optimizer_endpoint", json!([]))?;
    }
    apply(rt, "copilot.cli_version", json!([field("copilot")]))?;
    result(rt, "cli_version")?;
    grader(rt)?;
    integer_env(
        rt,
        "optimizer_max_tokens",
        "TE_OPTIMIZER_MAX_TOKENS",
        "16384",
    )?;
    timeouts(rt)
}
fn codex(rt: &mut dyn Runtime) -> Result<()> {
    apply(rt, "codex.resolve_codex_routing", json!([]))?;
    result(rt, "routing")?;
    assign(rt, "routing_base", json!(["item", "routing", "base_url"]))?;
    assign(rt, "api_key", json!(["item", "routing", "api_key"]))?;
    model(rt, "codex")?;
    apply(
        rt,
        "codex.validate_priceable",
        json!([field("model"), field("agent")]),
    )?;
    let model = text(rt, "model")?;
    let base = text(rt, "routing_base")?;
    log(rt, format!("codex: model={model} endpoint={base}"))?;
    reject_optimizer_override(rt)?;
    assign(rt, "optimizer_model", field("model"))?;
    literal(rt, "optimizer_transport", json!("api"))?;
    integer_env(
        rt,
        "optimizer_max_tokens",
        "TE_OPTIMIZER_MAX_TOKENS",
        "16384",
    )?;
    effort(rt, "codex", "effort")?;
    scaffold(rt)?;
    apply(rt, "codex.require_install", json!([]))?;
    rt.call(
        "setup_pair",
        json!([["field", "codex_shim"], ["field", "codex_vendor"]]),
    )?;
    assign(rt, "agent_bin", module("codex.C_BIN"))?;
    assign(rt, "protected_tools", module("codex.PROTECTED_TOOLS"))?;
    apply(rt, "codex.cli_version", json!([field("codex_shim")]))?;
    result(rt, "cli_version")?;
    call(
        rt,
        "codex.bundled_system_skills",
        json!([]),
        json!({"binary":field("codex_shim")}),
    )?;
    assign(rt, "system_skills", json!(["tuple"]))?;
    if !boolean(rt, field("system_skills"))? {
        assign(rt, "system_skills", module("codex.DEFAULT_SYSTEM_SKILLS"))?;
        let skills = read(rt, field("system_skills"))?;
        log(
            rt,
            format!(
                "codex: could not discover the CLI's bundled system skills -- using the 0.153.1 set {}",
                python_repr(&skills)
            ),
        )?;
    } else {
        let skills = read(rt, field("system_skills"))?;
        log(
            rt,
            format!(
                "codex: bundled system skills on this CLI: {}",
                python_repr(&skills)
            ),
        )?;
    }
    apply(rt, "codex.base_prompt_for", json!([field("cli_version")]))?;
    result(rt, "base_prompt")?;
    if boolean(rt, field("base_prompt"))? {
        let version = read(rt, field("cli_version"))?;
        let size = rt
            .call("setup_len", field("base_prompt"))?
            .as_u64()
            .unwrap_or(0)
            / 4;
        log(
            rt,
            format!(
                "codex: base prompt capture found for {} ({size} tokens); the interactive_guidance and skills how-to sections can be pruned through model_instructions_file",
                python_repr(&version)
            ),
        )?;
    } else {
        let version = read(rt, field("cli_version"))?;
        log(
            rt,
            format!(
                "codex: no base prompt capture for {} -- the interactive_guidance package cannot be applied on this lane and will stay loaded; capture this CLI's base prompt in the runtime bundle at assets/codex_base_prompt/<version>.md to enable it",
                python_repr(&version)
            ),
        )?;
    }
    // Capture the callable and earlier arguments before lean_instructions can replace them.
    rt.call(
        "setup_capture",
        json!([
            ["probe_config_function", module("codex.config_toml")],
            ["probe_routing", field("routing")],
            ["probe_model", field("model")],
            ["probe_effort", field("effort")]
        ]),
    )?;
    let mut capabilities = read(rt, module("codex.CONFIG_CAPABILITIES"))?
        .as_array()
        .cloned()
        .unwrap_or_default();
    capabilities.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
    rt.call(
        "setup_capture",
        json!([
            ["probe_skills", field("system_skills")],
            ["probe_lean_function", module("codex.lean_instructions")],
            ["probe_prompt", field("base_prompt")]
        ]),
    )?;
    let mut lean_capabilities = read(rt, module("codex.CONFIG_CAPABILITIES"))?
        .as_array()
        .cloned()
        .unwrap_or_default();
    lean_capabilities.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
    rt.call(
        "setup_call",
        json!([
            ["local", "probe_lean_function"],
            [["local", "probe_prompt"], value(json!(lean_capabilities))],
            {}
        ]),
    )?;
    rt.call(
        "setup_call",
        json!([
            ["local", "probe_config_function"],
            [],
            {
                "routing": ["local", "probe_routing"],
                "model": ["local", "probe_model"],
                "effort": ["local", "probe_effort"],
                "capabilities": value(json!(capabilities)),
                "system_skills": ["local", "probe_skills"],
                "instructions": ["result"]
            }
        ]),
    )?;
    call(
        rt,
        "codex.validate_config",
        json!([["result"]]),
        json!({"binary":field("codex_shim")}),
    )?;
    if boolean(rt, json!(["result"]))? {
        let bad = scalar_text(&read(rt, json!(["result"]))?);
        return Err(Failure::new(
            "RuntimeError",
            format!(
                "codex refused the generated config.toml, so the request contract this lane would send is not the one it describes:\n{bad}\n  A renamed toggle is IGNORED without --strict-config, which is why this is checked here: the untooling arm would otherwise run as a baseline under its own name. Update codex._TOP_LEVEL_TOGGLES / _TABLE_TOGGLES for this CLI build."
            ),
        ));
    }
    apply(
        rt,
        "codex.verify_model",
        json!([field("model"), field("routing")]),
    )?;
    rt.call("setup_pair", json!([["local", "served"], ["local", "why"]]))?;
    let served = read(rt, json!(["local", "served"]))?;
    if served == false {
        let why = scalar_text(&read(rt, json!(["local", "why"]))?);
        return Err(Failure::new(
            "RuntimeError",
            format!("codex model check failed: {why}"),
        ));
    }
    if served.is_null() {
        let why = scalar_text(&read(rt, json!(["local", "why"]))?);
        let base = text(rt, "routing_base")?;
        log(
            rt,
            format!(
                "codex: {why} -- an unserved id would fail per task, so check `curl {base}/models` by hand before a long run"
            ),
        )?;
    }
    grader(rt)?;
    timeouts(rt)?;
    assign(rt, "_capabilities", json!(["empty_tuple"]))?;
    literal(rt, "_thread_id", json!(""))?;
    call(
        rt,
        "codex.preflight_optimizer",
        json!([]),
        json!({"routing":field("routing"),"configured_model":field("model")}),
    )
}
pub fn construct(agent: &str, rt: &mut dyn Runtime) -> Result<Value> {
    if let Some(agent) = agent.strip_prefix("web.") {
        return web::construct(agent, rt);
    }
    assign(rt, "agent", arg("agent"))?;
    match agent {
        "claude" => claude(rt)?,
        "copilot" => copilot(rt)?,
        "codex" => codex(rt)?,
        _ => {
            return Err(Failure::new(
                "ValueError",
                format!("unknown constructor agent: {agent}"),
            ));
        }
    }
    Ok(Value::Null)
}
