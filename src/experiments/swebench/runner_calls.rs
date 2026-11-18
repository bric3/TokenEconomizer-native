use super::{
    Result, Runtime,
    optimizer::code_points,
    providers::{boolean, capture, copy, literal},
};
use crate::{python_json::Text, value::truthy};
use serde_json::{Value, json};

fn function(rt: &mut dyn Runtime, name: &str) -> Result<()> {
    rt.call("runner_function", json!([name])).map(|_| ())
}
fn member(rt: &mut dyn Runtime, name: &str, target: &str) -> Result<()> {
    rt.call("runner_member", json!([name, target])).map(|_| ())
}
fn invoke(rt: &mut dyn Runtime, args: &[&str], kwargs: &[(&str, &str)]) -> Result<()> {
    rt.call("runner_invoke", json!([args, kwargs])).map(|_| ())
}
fn label(rt: &mut dyn Runtime, labels: &[&str]) -> Result<bool> {
    rt.call("runner_label", json!([labels])).map(|v| truthy(&v))
}
fn endpoint(rt: &mut dyn Runtime) -> Result<()> {
    member(rt, "optimizer_endpoint", "endpoint")?;
    rt.call("runner_endpoint", json!([])).map(|_| ())
}
pub fn optimizer(agent: &str, web: bool, rt: &mut dyn Runtime) -> Result<Value> {
    let api = if agent == "copilot" {
        member(rt, "optimizer_endpoint", "endpoint")?;
        boolean(rt, "endpoint")?
    } else {
        agent == "claude"
    };
    if api {
        if agent == "copilot" {
            endpoint(rt)?;
        }
        function(rt, "anthropic_optimizer_call")?;
        member(rt, "optimizer_model", "configured_model")?;
        member(rt, "optimizer_max_tokens", "max_tokens")?;
        if agent == "claude" {
            member(rt, "api_key", "api_key")?;
        }
        let mut kwargs = vec![
            ("model", "model"),
            ("configured_model", "configured_model"),
            ("max_tokens", "max_tokens"),
            ("api_key", "api_key"),
        ];
        if agent == "copilot" {
            kwargs.extend([("bearer", "bearer"), ("base_url", "base")]);
        }
        invoke(rt, &["prompt"], &kwargs)?;
    } else if agent == "copilot" {
        function(rt, "copilot.optimizer_call")?;
        member(rt, "copilot", "binary")?;
        if label(rt, &["agent", "copilot"])? {
            member(rt, "model", "selected")?;
        } else {
            copy(rt, "selected", "model")?;
        }
        invoke(
            rt,
            &["prompt"],
            &[("binary", "binary"), ("model", "selected")],
        )?;
    } else {
        function(rt, "codex.optimizer_call")?;
        if web || label(rt, &["agent", "codex"])? {
            member(rt, "model", "configured_model")?;
        } else {
            copy(rt, "configured_model", "model")?;
        }
        member(rt, "routing", "routing")?;
        member(rt, "optimizer_max_tokens", "max_tokens")?;
        invoke(
            rt,
            &["prompt"],
            &[
                ("configured_model", "configured_model"),
                ("routing", "routing"),
                ("max_tokens", "max_tokens"),
            ],
        )?;
    }
    Ok(json!({"provider_ref":"called"}))
}
pub fn preflight(agent: &str, rt: &mut dyn Runtime) -> Result<Value> {
    if agent == "codex" {
        function(rt, "codex.preflight_optimizer")?;
        member(rt, "routing", "routing")?;
        member(rt, "model", "configured_model")?;
        invoke(
            rt,
            &[],
            &[
                ("routing", "routing"),
                ("configured_model", "configured_model"),
            ],
        )?;
    } else {
        if agent == "copilot" {
            endpoint(rt)?;
        }
        function(rt, "preflight_optimizer_endpoint")?;
        if agent == "claude" {
            member(rt, "api_key", "api_key")?;
        }
        member(rt, "optimizer_model", "configured_model")?;
        literal(rt, "label", json!(agent))?;
        literal(
            rt,
            "remedy",
            json!(if agent == "claude" {
                "Check that ANTHROPIC_API_KEY is valid and can call MODEL."
            } else {
                "Check COPILOT_PROVIDER_BASE_URL and the credential that goes with it."
            }),
        )?;
        let mut kwargs = if agent == "copilot" {
            vec![
                ("base_url", "base"),
                ("api_key", "api_key"),
                ("bearer", "bearer"),
            ]
        } else {
            vec![("api_key", "api_key")]
        };
        kwargs.extend([
            ("configured_model", "configured_model"),
            ("label", "label"),
            ("remedy", "remedy"),
        ]);
        invoke(rt, &[], &kwargs)?;
    }
    Ok(Value::Null)
}
fn available(rt: &mut dyn Runtime, collection: &str) -> Result<Text> {
    let keys = rt.call("runner_keys", json!([collection]))?;
    let keys = keys.as_array().unwrap();
    if keys.iter().any(Value::is_null) {
        return code_points(rt.call("runner_sorted_keys", json!([]))?);
    }
    let mut names = keys
        .iter()
        .map(|v| code_points(v.clone()))
        .collect::<Result<Vec<_>>>()?;
    names.sort();
    let mut text = Text::default();
    for (index, name) in names.into_iter().enumerate() {
        if index > 0 {
            text.0.extend([44, 32]);
        }
        text.0.extend(name.0);
    }
    Ok(text)
}
fn repr(rt: &mut dyn Runtime, slot: &str) -> Result<Text> {
    code_points(rt.call("runner_repr", json!([slot]))?)
}
fn append(text: &mut Text, suffix: &str) {
    text.0.extend(suffix.chars().map(u32::from));
}
pub fn factory(web: bool, rt: &mut dyn Runtime) -> Result<Value> {
    if !boolean(rt, "agent")? {
        capture(rt, "agent", "TE_AGENT", None)?;
        if !boolean(rt, "agent")? {
            literal(rt, "agent", json!("claude"))?;
        }
    }
    rt.call("provider_method", json!(["agent", "lower", [], "agent"]))?;
    if !truthy(&rt.call("runner_registered", json!([]))?) {
        let mut message = Text::from(if web {
            "no Web-Bench runner for --agent "
        } else {
            "no SWE-bench runner for --agent "
        });
        message.0.extend(repr(rt, "agent")?.0);
        append(&mut message, "; available: ");
        message.0.extend(available(rt, "RUNNERS")?.0);
        return rt.call("runner_raise", json!([message.0]));
    }
    function(rt, "_install_signal_cleanup")?;
    invoke(rt, &[], &[])?;
    if !boolean(rt, "scaffold")? {
        capture(rt, "scaffold", "TE_SCAFFOLD", None)?;
        if !boolean(rt, "scaffold")? {
            literal(rt, "scaffold", Value::Null)?;
        }
    }
    if web
        && !truthy(&rt.call("provider_is_none", json!(["scaffold"]))?)
        && !truthy(&rt.call("runner_supported", json!([]))?)
    {
        let mut message = Text::from("the ");
        message.0.extend(repr(rt, "scaffold")?.0);
        append(
            &mut message,
            " baseline is not wired for Web-Bench (supported: ",
        );
        message.0.extend(available(rt, "WEBBENCH_SCAFFOLDS")?.0);
        append(
            &mut message,
            "). Web-Bench runs the agent on the HOST with no per-task repo container, and the repo map generates a PYTHON symbol map that a Node/TypeScript project does not have.",
        );
        return rt.call("runner_raise", json!([message.0]));
    }
    rt.call("runner_construct", json!([]))?;
    Ok(json!({"provider_ref":"called"}))
}
