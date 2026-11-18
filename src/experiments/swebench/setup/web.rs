use super::super::optimizer::code_points;
use super::{
    Failure, OPTIMIZER_MODEL_ERROR, Result, Runtime, apply, arg, assign, boolean, field, literal,
    module, result, value,
};
use crate::{python_json::Text, value::truthy};
use serde_json::{Value, json};

fn local(name: &str) -> Value {
    json!(["local", name])
}
fn capture(rt: &mut dyn Runtime, name: &str, reference: Value) -> Result<()> {
    rt.call("setup_capture", json!([[name, reference]]))
        .map(|_| ())
}
fn env(rt: &mut dyn Runtime, slot: &str, name: &str, default: Option<&str>) -> Result<()> {
    rt.call(
        "setup_env_capture",
        if let Some(default) = default {
            json!([slot, name, default])
        } else {
            json!([slot, name])
        },
    )
    .map(|_| ())
}
fn compare(rt: &mut dyn Runtime, operation: &str, left: Value, right: Value) -> Result<()> {
    rt.call(
        "setup_compare",
        json!([operation, left, right, "comparison"]),
    )
    .map(|_| ())
}
fn equal(rt: &mut dyn Runtime, reference: Value, expected: &str) -> Result<bool> {
    compare(rt, "eq", reference, value(json!(expected)))?;
    boolean(rt, local("comparison"))
}
fn invoke(rt: &mut dyn Runtime, function: Value, args: Value) -> Result<()> {
    rt.call("setup_call", json!([function, args, {}]))
        .map(|_| ())
}
fn formatted(rt: &mut dyn Runtime, reference: Value, repr: bool) -> Result<Text> {
    code_points(rt.call("setup_format", json!([reference, repr]))?)
}
fn sorted_repr(rt: &mut dyn Runtime, reference: Value) -> Result<Text> {
    code_points(rt.call("setup_sorted_repr", reference)?)
}
fn append(text: &mut Text, suffix: &str) {
    text.0.extend(suffix.chars().map(u32::from));
}
fn reject(rt: &mut dyn Runtime, message: Text) -> Result<()> {
    rt.call("setup_raise", json!([message.0])).map(|_| ())
}
fn begin_log(rt: &mut dyn Runtime) -> Result<()> {
    capture(rt, "logger", module("_log"))
}
fn log(rt: &mut dyn Runtime, message: Text) -> Result<()> {
    invoke(rt, local("logger"), json!([["points", message.0]]))
}
fn override_model(rt: &mut dyn Runtime) -> Result<()> {
    env(rt, "override", "TE_OPTIMIZER_MODEL", None)?;
    if boolean(rt, local("override"))? {
        return Err(Failure::new("RuntimeError", OPTIMIZER_MODEL_ERROR));
    }
    Ok(())
}
fn model(rt: &mut dyn Runtime, variable: &str, default: &str) -> Result<()> {
    if boolean(rt, arg("model"))? {
        assign(rt, "model", arg("model"))
    } else {
        env(rt, "model", variable, Some(default))?;
        assign(rt, "model", local("model"))
    }
}
fn effort(rt: &mut dyn Runtime, copilot: bool) -> Result<()> {
    let target = if copilot {
        "effort_requested"
    } else {
        "effort"
    };
    let levels = if copilot {
        "copilot.EFFORT_LEVELS"
    } else {
        "EFFORT_LEVELS"
    };
    env(rt, "effort", "TE_EFFORT_LEVEL", None)?;
    if !boolean(rt, local("effort"))? {
        env(
            rt,
            "effort",
            if copilot {
                "COPILOT_EFFORT_LEVEL"
            } else {
                "CLAUDE_CODE_EFFORT_LEVEL"
            },
            None,
        )?;
        if !boolean(rt, local("effort"))? {
            capture(rt, "effort", value(json!("high")))?;
        }
    }
    assign(rt, target, local("effort"))?;
    validate_effort(rt, target, levels)
}
fn validate_effort(rt: &mut dyn Runtime, target: &str, levels: &str) -> Result<()> {
    if !truthy(&rt.call("setup_contains", json!([field(target), module(levels)]))?) {
        let mut message = Text::from("TE_EFFORT_LEVEL=");
        message.0.extend(formatted(rt, field(target), true)?.0);
        append(&mut message, " is not one of ");
        message.0.extend(sorted_repr(rt, module(levels))?.0);
        reject(rt, message)?;
    }
    Ok(())
}
fn integer(
    rt: &mut dyn Runtime,
    target: &str,
    variable: &str,
    default: &str,
    floor: bool,
) -> Result<()> {
    env(rt, "integer_input", variable, Some(default))?;
    rt.call("setup_integer", json!([local("integer_input"), "integer"]))?;
    if floor {
        compare(rt, "le", local("integer"), value(json!(1)))?;
        if boolean(rt, local("comparison"))? {
            return literal(rt, target, json!(1));
        }
    }
    assign(rt, target, local("integer"))
}
fn keep(rt: &mut dyn Runtime) -> Result<()> {
    env(rt, "keep", "TE_KEEP_WORKSPACES", None)?;
    compare(rt, "eq", local("keep"), value(json!("1")))?;
    assign(rt, "keep_workspaces", local("comparison"))
}
fn stream(rt: &mut dyn Runtime) -> Result<()> {
    integer(
        rt,
        "start_timeout",
        "TE_WEBBENCH_START_TIMEOUT",
        "1800",
        false,
    )?;
    keep(rt)?;
    for (name, data) in [
        ("_session", Value::Null),
        ("_session_project", Value::Null),
        ("_stream", json!([])),
        ("_stream_broken", json!(false)),
        ("_current_task", Value::Null),
    ] {
        literal(rt, name, data)?;
    }
    Ok(())
}
fn timeouts(rt: &mut dyn Runtime) -> Result<()> {
    apply(rt, "webbench_task_timeout", json!([]))?;
    result(rt, "task_timeout")?;
    integer(rt, "retry", "TE_WEBBENCH_RETRY", "2", true)?;
    integer(rt, "max_workers", "TE_WEBBENCH_MAX_WORKERS", "4", false)?;
    integer(
        rt,
        "verdict_timeout",
        "TE_WEBBENCH_VERDICT_TIMEOUT",
        "1800",
        false,
    )
}
fn repo(rt: &mut dyn Runtime) -> Result<()> {
    if boolean(rt, arg("repo"))? {
        apply(rt, "os.path.realpath", json!([arg("repo")]))?;
    } else {
        apply(rt, "webbench_repo", json!([]))?;
    }
    result(rt, "repo")
}
fn scaffold(rt: &mut dyn Runtime, instruction: bool) -> Result<()> {
    if boolean(rt, arg("scaffold"))? {
        assign(rt, "scaffold_name", arg("scaffold"))?;
    } else {
        literal(rt, "scaffold_name", Value::Null)?;
    }
    apply(rt, "scaffolds_pkg.get", json!([field("scaffold_name")]))?;
    result(rt, "scaffold")?;
    if truthy(&rt.call("setup_present", field("scaffold"))?) {
        capture(rt, "scaffold_preflight", module("self.scaffold.preflight"))?;
        apply(
            rt,
            "self.scaffold_profile_for",
            json!([value(json!("/nonexistent"))]),
        )?;
        invoke(rt, local("scaffold_preflight"), json!([["result"]]))?;
        begin_log(rt)?;
        let mut message = Text::from("scaffold: ");
        message
            .0
            .extend(formatted(rt, json!(["nested_field", "scaffold", "name"]), false)?.0);
        append(
            &mut message,
            if instruction {
                " (external baseline, instruction mode)"
            } else {
                " (external baseline)"
            },
        );
        log(rt, message)?;
    }
    Ok(())
}
fn claude(rt: &mut dyn Runtime) -> Result<()> {
    model(rt, "ANTHROPIC_MODEL", "claude-sonnet-5")?;
    apply(rt, "resolve_anthropic_key", json!([]))?;
    result(rt, "api_key")?;
    override_model(rt)?;
    assign(rt, "optimizer_model", field("model"))?;
    effort(rt, false)?;
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
    repo(rt)?;
    scaffold(rt, false)?;
    apply(rt, "node_bin", json!([]))?;
    result(rt, "node")?;
    integer(
        rt,
        "optimizer_max_tokens",
        "TE_OPTIMIZER_MAX_TOKENS",
        "16384",
        false,
    )?;
    literal(rt, "optimizer_transport", json!("api"))?;
    timeouts(rt)?;
    apply(rt, "self._init_stream_state", json!([]))?;
    apply(rt, "self._preflight_optimizer_endpoint", json!([]))
}
fn copilot(rt: &mut dyn Runtime) -> Result<()> {
    capture(rt, "normalize", module("copilot.normalize_model"))?;
    let input = if boolean(rt, arg("model"))? {
        arg("model")
    } else {
        env(rt, "model", "COPILOT_MODEL", None)?;
        local("model")
    };
    invoke(rt, local("normalize"), json!([input]))?;
    result(rt, "model")?;
    apply(
        rt,
        "copilot.validate_priceable",
        json!([field("model"), field("agent")]),
    )?;
    override_model(rt)?;
    capture(rt, "optimizer_model", field("model"))?;
    if boolean(rt, local("optimizer_model"))? {
        assign(rt, "optimizer_model", local("optimizer_model"))?;
    } else {
        literal(rt, "optimizer_model", json!("auto"))?;
    }
    effort(rt, true)?;
    apply(rt, "copilot.effort_applies", json!([field("model")]))?;
    if boolean(rt, json!(["result"]))? {
        assign(rt, "effort", field("effort_requested"))?;
    } else {
        literal(rt, "effort", Value::Null)?;
    }
    if !truthy(&rt.call("setup_present", field("effort"))?) {
        env(rt, "effort_applies", "TE_EFFORT_APPLIES", None)?;
        let why = if equal(rt, local("effort_applies"), "0")? {
            "the launcher probed this model and the CLI rejected --effort"
        } else {
            "the 'auto' router takes no effort level; pin COPILOT_MODEL to a served model id to control it"
        };
        begin_log(rt)?;
        let mut message = Text::from("copilot: TE_EFFORT_LEVEL=");
        message
            .0
            .extend(formatted(rt, field("effort_requested"), false)?.0);
        append(&mut message, " NOT applied (model=");
        capture(rt, "log_model", field("model"))?;
        if boolean(rt, local("log_model"))? {
            message
                .0
                .extend(formatted(rt, local("log_model"), false)?.0);
        } else {
            append(&mut message, "auto");
        }
        append(&mut message, ", recorded as null) -- ");
        append(&mut message, why);
        append(&mut message, ".");
        log(rt, message)?;
    }
    apply(rt, "copilot.require_install", json!([]))?;
    result(rt, "copilot")?;
    assign(rt, "agent_bin", field("copilot"))?;
    assign(rt, "protected_tools", module("copilot.PROTECTED_TOOLS"))?;
    apply(rt, "copilot.provider_base", json!([]))?;
    result(rt, "routing_base")?;
    apply(rt, "copilot.anthropic_optimizer_endpoint", json!([]))?;
    result(rt, "optimizer_endpoint")?;
    let api = boolean(rt, field("optimizer_endpoint"))?;
    literal(
        rt,
        "optimizer_transport",
        json!(if api { "api" } else { "cli" }),
    )?;
    if boolean(rt, field("optimizer_endpoint"))? {
        begin_log(rt)?;
        let mut message = Text::from("copilot: mechanism calls -> ");
        message
            .0
            .extend(formatted(rt, json!(["item", "optimizer_endpoint", 0]), false)?.0);
        append(&mut message, " (the BYOK provider, direct Messages API)");
        log(rt, message)?;
        apply(rt, "self._preflight_optimizer_endpoint", json!([]))?;
    }
    apply(rt, "copilot.cli_version", json!([field("copilot")]))?;
    result(rt, "cli_version")?;
    repo(rt)?;
    apply(rt, "node_bin", json!([]))?;
    result(rt, "node")?;
    scaffold(rt, false)?;
    integer(
        rt,
        "optimizer_max_tokens",
        "TE_OPTIMIZER_MAX_TOKENS",
        "16384",
        false,
    )?;
    timeouts(rt)?;
    apply(rt, "self._init_stream_state", json!([]))
}
fn codex(rt: &mut dyn Runtime) -> Result<()> {
    if boolean(rt, arg("scaffold"))? {
        compare(rt, "ne", arg("scaffold"), value(json!("rtk")))?;
        if boolean(rt, local("comparison"))? {
            let mut message = Text::from("the ");
            message.0.extend(formatted(rt, arg("scaffold"), true)?.0);
            append(
                &mut message,
                " baseline is not wired for the Codex Web-Bench lane (Web-Bench hosts ",
            );
            message
                .0
                .extend(sorted_repr(rt, module("WEBBENCH_SCAFFOLDS"))?.0);
            append(
                &mut message,
                "; on Codex that is the instruction-mode rtk). A scaffold that installs and never takes effect would produce native rows under the scaffold's name.",
            );
            reject(rt, message)?;
        }
    }
    apply(rt, "codex.resolve_codex_routing", json!([]))?;
    result(rt, "routing")?;
    assign(rt, "api_key", json!(["item", "routing", "api_key"]))?;
    model(rt, "CODEX_MODEL", "gpt-5.6-sol")?;
    apply(
        rt,
        "codex.validate_priceable",
        json!([field("model"), field("agent")]),
    )?;
    apply(
        rt,
        "codex.verify_model",
        json!([field("model"), field("routing")]),
    )?;
    override_model(rt)?;
    assign(rt, "optimizer_model", field("model"))?;
    env(rt, "effort", "TE_EFFORT_LEVEL", Some("high"))?;
    assign(rt, "effort", local("effort"))?;
    validate_effort(rt, "effort", "EFFORT_LEVELS")?;
    apply(rt, "codex.require_install", json!([]))?;
    apply(rt, "codex.codex_vendor", json!([]))?;
    result(rt, "codex_vendor")?;
    apply(rt, "codex.codex_bin", json!([]))?;
    result(rt, "agent_bin")?;
    apply(rt, "codex.cli_version", json!([field("agent_bin")]))?;
    result(rt, "cli_version")?;
    assign(rt, "protected_tools", module("PROTECTED_TOOLS"))?;
    apply(rt, "codex.base_prompt_for", json!([field("cli_version")]))?;
    result(rt, "base_prompt")?;
    apply(
        rt,
        "codex.bundled_system_skills",
        json!([field("agent_bin")]),
    )?;
    if boolean(rt, json!(["result"]))? {
        result(rt, "system_skills")?;
    } else {
        literal(rt, "system_skills", Value::Null)?;
    }
    scaffold(rt, true)?;
    integer(
        rt,
        "optimizer_max_tokens",
        "TE_OPTIMIZER_MAX_TOKENS",
        "16384",
        false,
    )?;
    repo(rt)?;
    apply(rt, "node_bin", json!([]))?;
    result(rt, "node")?;
    timeouts(rt)?;
    assign(rt, "routing_base", json!(["item", "routing", "base_url"]))?;
    literal(rt, "optimizer_transport", json!("api"))?;
    literal(rt, "claude", Value::Null)?;
    keep(rt)?;
    integer(
        rt,
        "start_timeout",
        "TE_WEBBENCH_START_TIMEOUT",
        "1800",
        false,
    )?;
    assign(rt, "_capabilities", json!(["empty_tuple"]))?;
    literal(rt, "_lean_instructions_tokens", json!(0))?;
    begin_log(rt)?;
    let mut message = Text::from("codex: model=");
    message.0.extend(formatted(rt, field("model"), false)?.0);
    append(&mut message, " endpoint=");
    message
        .0
        .extend(formatted(rt, field("routing_base"), false)?.0);
    log(rt, message)?;
    apply(rt, "self._probe_namespace_guard", json!([]))?;
    result(rt, "answer_key_ns")?;
    apply(rt, "self._preflight_optimizer_endpoint", json!([]))?;
    apply(rt, "self._init_stream_state", json!([]))
}
pub(super) fn construct(agent: &str, rt: &mut dyn Runtime) -> Result<Value> {
    if agent == "stream" {
        stream(rt)?;
    } else {
        assign(rt, "agent", arg("agent"))?;
        match agent {
            "claude" => claude(rt)?,
            "copilot" => copilot(rt)?,
            "codex" => codex(rt)?,
            _ => {
                return Err(Failure::new(
                    "ValueError",
                    format!("unknown Web-Bench constructor agent: {agent}"),
                ));
            }
        }
    }
    Ok(Value::Null)
}
