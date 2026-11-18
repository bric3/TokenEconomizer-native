use super::{Failure, Result, Runtime};
use crate::{
    python_json::{self, Node, Text},
    value::{scalar_text, truthy},
};
use serde_json::{Value, json};
use std::collections::HashSet;

pub(super) fn capture(
    rt: &mut dyn Runtime,
    slot: &str,
    key: &str,
    default: Option<&str>,
) -> Result<()> {
    let args = if let Some(default) = default {
        json!([slot, key, default])
    } else {
        json!([slot, key])
    };
    rt.call("provider_env_get", args).map(|_| ())
}
pub(super) fn boolean(rt: &mut dyn Runtime, slot: &str) -> Result<bool> {
    rt.call("provider_bool", json!([slot])).map(|v| truthy(&v))
}
pub(super) fn literal(rt: &mut dyn Runtime, slot: &str, value: Value) -> Result<()> {
    rt.call("provider_literal", json!([slot, value]))
        .map(|_| ())
}
pub(super) fn copy(rt: &mut dyn Runtime, target: &str, source: &str) -> Result<()> {
    rt.call("provider_copy", json!([target, source]))
        .map(|_| ())
}
fn returned(slot: &str) -> Value {
    json!({"provider_ref":slot})
}
pub(super) fn equal(rt: &mut dyn Runtime, slot: &str, value: &str) -> Result<bool> {
    rt.call("provider_equal", json!([slot, value]))
        .map(|v| truthy(&v))
}
fn string(rt: &mut dyn Runtime, slot: &str) -> Result<Option<String>> {
    rt.call("provider_string", json!([slot]))
        .map(|v| v.as_str().map(str::to_owned))
}
pub(super) fn trim(rt: &mut dyn Runtime, slot: &str, slash: bool) -> Result<()> {
    if let Some(text) = string(rt, slot)? {
        let trimmed = if slash {
            text.trim_end_matches('/')
        } else {
            text.trim_matches(|c: char| c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}'))
        };
        if trimmed != text {
            literal(rt, slot, json!(trimmed))?;
        }
    } else {
        rt.call(
            "provider_method",
            json!([
                slot,
                if slash { "rstrip" } else { "strip" },
                if slash { json!(["/"]) } else { json!([]) },
                slot
            ]),
        )?;
    }
    Ok(())
}
fn lower_equal(rt: &mut dyn Runtime, slot: &str, expected: &str) -> Result<bool> {
    if let Some(text) = string(rt, slot)? {
        return Ok(text.to_lowercase() == expected);
    }
    rt.call("provider_method", json!([slot, "lower", [], "lowered"]))?;
    equal(rt, "lowered", expected)
}
fn names(rt: &mut dyn Runtime, name: &str) -> Result<Vec<String>> {
    let values = rt.call("provider_names", json!([name]))?;
    serde_json::from_value(values).map_err(|e| Failure::new("TypeError", e.to_string()))
}
fn collect(rt: &mut dyn Runtime, name: &str, slot: &str) -> Result<()> {
    rt.call("provider_collect", json!([name, slot])).map(|_| ())
}
fn routed_key(rt: &mut dyn Runtime, claude: bool) -> Result<Value> {
    if claude {
        capture(rt, "base", "ANTHROPIC_BASE_URL", None)?;
        if boolean(rt, "base")? {
            rt.call("provider_log",json!(["ANTHROPIC_BASE_URL is set and ignored: this harness bills api.anthropic.com only"]))?;
        }
    }
    let key = if claude {
        "ANTHROPIC_API_KEY".into()
    } else {
        rt.call("provider_constant", json!(["key_name", "KEY_ENV"]))?;
        rt.call("provider_data", json!(["key_name"]))?
            .as_str()
            .unwrap_or("")
            .to_owned()
    };
    capture(rt, "key", &key, Some(""))?;
    if !boolean(rt, "key")? {
        return Err(Failure::new(
            "RuntimeError",
            if claude {
                "ANTHROPIC_API_KEY is unset -- the Claude lane bills the Anthropic API directly and has no other credential (see the repo's .env)"
            } else {
                "OPENAI_API_KEY is unset, so this Codex lane has nothing to bill -- see the repo's .env. (A region-scoped key also needs OPENAI_BASE_URL: it 401s on the global endpoint.)"
            },
        ));
    }
    if claude {
        return Ok(returned("key"));
    }
    capture(rt, "base", "OPENAI_BASE_URL", None)?;
    if !boolean(rt, "base")? {
        rt.call("provider_constant", json!(["base", "OPENAI_DIRECT"]))?;
    }
    trim(rt, "base", false)?;
    trim(rt, "base", true)?;
    collect(rt, "base_url", "base")?;
    collect(rt, "api_key", "key")?;
    Ok(returned("output"))
}
fn forward(rt: &mut dyn Runtime, kind: &str) -> Result<Value> {
    if kind != "auth_env" {
        rt.call("provider_function", json!(["byok_active", []]))?;
        if !boolean(rt, "called")? {
            return Ok(if kind == "provider_env" {
                json!({})
            } else {
                json!([])
            });
        }
    }
    let keys = match kind {
        "auth_env" => vec![
            "COPILOT_GITHUB_TOKEN".into(),
            "GH_TOKEN".into(),
            "GITHUB_TOKEN".into(),
        ],
        "provider_env" => names(rt, "PROVIDER_VARS")?,
        _ => names(rt, "PROVIDER_SECRET_VARS")?,
    };
    let mut secrets = Vec::new();
    for key in keys {
        capture(rt, "item", &key, None)?;
        if !boolean(rt, "item")? {
            continue;
        }
        if kind == "provider_secret_names" {
            secrets.push(key);
        } else {
            if kind == "provider_env" {
                rt.call("provider_env_item", json!(["item", key]))?;
            }
            collect(rt, &key, "item")?;
        }
    }
    Ok(if kind == "provider_secret_names" {
        json!(secrets)
    } else {
        returned("output")
    })
}
fn endpoint(rt: &mut dyn Runtime) -> Result<Value> {
    capture(rt, "base", "COPILOT_PROVIDER_BASE_URL", None)?;
    if !boolean(rt, "base")? {
        return Ok(Value::Null);
    }
    capture(rt, "kind", "COPILOT_PROVIDER_TYPE", None)?;
    if !boolean(rt, "kind")? {
        literal(rt, "kind", json!(""))?;
    }
    if !lower_equal(rt, "kind", "anthropic")? {
        return Ok(Value::Null);
    }
    capture(rt, "key", "COPILOT_PROVIDER_API_KEY", None)?;
    if !boolean(rt, "key")? {
        literal(rt, "key", json!(""))?;
    }
    capture(rt, "bearer", "COPILOT_PROVIDER_BEARER_TOKEN", None)?;
    if !boolean(rt, "bearer")? {
        literal(rt, "bearer", json!(""))?;
    }
    if !boolean(rt, "key")? && !boolean(rt, "bearer")? {
        return Ok(Value::Null);
    }
    rt.call("provider_tuple", json!(["base", "key", "bearer"]))?;
    Ok(returned("tuple"))
}
fn normalize(rt: &mut dyn Runtime) -> Result<Value> {
    if boolean(rt, "model")? {
        copy(rt, "normal", "model")?;
    } else {
        literal(rt, "normal", json!(""))?;
    }
    trim(rt, "normal", false)?;
    if !boolean(rt, "normal")? || lower_equal(rt, "normal", "auto")? {
        return Ok(Value::Null);
    }
    Ok(returned("normal"))
}
pub(super) fn read_json(rt: &mut dyn Runtime) -> Result<Node> {
    let document = rt.call("provider_http_read", json!([]))?;
    let text = document["text"].as_str().unwrap_or("");
    python_json::parse_result(
        text.as_bytes(),
        document["int_max_digits"].as_u64().unwrap_or(0) as usize,
    )
    .map_err(|error| match error {
        python_json::ParseError::Syntax { message, position } => {
            let mut failure = Failure::new("JSONDecodeError", error.message(text));
            failure.details = Some(Box::new(json!({"msg":message,"doc":text,"pos":position})));
            failure
        }
        python_json::ParseError::IntegerLimit { .. } => {
            Failure::new("ValueError", error.message(""))
        }
    })
}
fn catalogue(rt: &mut dyn Runtime) -> Result<Value> {
    if truthy(&rt.call("provider_cache_begin", json!([]))?) {
        rt.call("provider_cache_get", json!([]))?;
        return Ok(returned("catalogue"));
    }
    let result = (|| -> Result<Option<Vec<String>>> {
        copy(rt, "url", "base_url")?;
        trim(rt, "url", true)?;
        let base = rt.call("provider_data", json!(["url"]))?;
        let base = base
            .as_str()
            .ok_or_else(|| Failure::new("TypeError", "catalogue URL must be a string"))?;
        let key = rt.call("provider_format", json!(["api_key"]))?;
        rt.call("provider_request",json!([format!("{base}/models"),{"authorization":format!("Bearer {}",scalar_text(&key))}]))?;
        rt.call("provider_http_enter", json!([]))?;
        let parsed = read_json(rt);
        let suppressed = rt.call("provider_http_exit", json!([parsed.as_ref().err()]))?;
        let payload = match parsed {
            Err(_) if truthy(&suppressed) => {
                return Err(Failure::new(
                    "UnboundLocalError",
                    "cannot access local variable 'payload' where it is not associated with a value",
                ));
            }
            other => other?,
        };
        let mut ids = HashSet::<Text>::new();
        if let Some(Node::List(data)) = payload.field("data") {
            for model in data {
                if let Some(Node::String(id)) = model.field("id") {
                    ids.insert(id.clone());
                }
            }
        }
        Ok(if ids.is_empty() {
            None
        } else {
            Some(ids.iter().map(Text::json).collect())
        })
    })();
    let ids = match result {
        Ok(ids) => ids,
        Err(e)
            if e.is_a("urllib.error.URLError")
                || e.is_a("builtins.OSError")
                || e.is_a("builtins.ValueError")
                || e.is_a("builtins.TypeError")
                || e.kind == "JSONDecodeError" =>
        {
            None
        }
        Err(e) => return Err(e),
    };
    rt.call("provider_catalogue_value", json!([ids]))?;
    rt.call("provider_cache_put", json!([]))?;
    Ok(returned("catalogue"))
}
fn verify(rt: &mut dyn Runtime) -> Result<Value> {
    rt.call("provider_routing_item", json!(["base", "base_url"]))?;
    rt.call("provider_routing_item", json!(["key", "api_key"]))?;
    rt.call(
        "provider_function",
        json!(["fetch_catalog", ["base", "key"]]),
    )?;
    if truthy(&rt.call("provider_is_none", json!(["called"]))?) {
        return Ok(json!([null, "catalog unreadable -- model not verified"]));
    }
    if truthy(&rt.call("provider_contains", json!(["model", "called"]))?) {
        return Ok(json!([true, ""]));
    }
    let sample = rt.call("provider_items", json!(["called"]))?;
    let sample = python_json::parse(sample.as_str().unwrap_or("").as_bytes(), 0)
        .ok_or_else(|| Failure::new("TypeError", "expected catalogue ids"))?;
    let Node::List(ref sample) = sample else {
        return Err(Failure::new("TypeError", "expected catalogue ids"));
    };
    let mut sample = sample
        .iter()
        .map(|v| {
            if let Node::String(text) = v {
                Ok(text)
            } else {
                Err(Failure::new("TypeError", "catalogue id must be a string"))
            }
        })
        .collect::<Result<Vec<_>>>()?;
    sample.sort_unstable();
    rt.call("provider_routing_item", json!(["base", "base_url"]))?;
    let base = scalar_text(&rt.call("provider_format", json!(["base"]))?);
    let model = scalar_text(&rt.call("provider_repr", json!(["model"]))?);
    let prefix = format!(
        "{base} does not serve {model}. An unserved id fails per task rather than at startup, so this lane is refused. Served (first 12): "
    );
    let mut message: Vec<u32> = prefix.chars().map(u32::from).collect();
    for (index, id) in sample.into_iter().take(12).enumerate() {
        if index > 0 {
            message.push(32);
        }
        message.extend(&id.0);
    }
    Ok(json!({"provider_encoded":format!("[false,{}]",Text(message).json())}))
}
pub fn dispatch(method: &str, rt: &mut dyn Runtime) -> Result<Value> {
    let (agent, name) = method.split_once('.').unwrap_or(("", method));
    match name {
        "begin_stream" | "end_stream" | "session_for" | "close_session" if agent == "webbench" => {
            super::web_stream::run(name, rt)
        }
        "webbench_repo"
        | "node_bin"
        | "webbench_undici_abort"
        | "webbench_task_timeout"
        | "webbench_undici_env"
        | "_probe_namespace_guard"
            if agent == "webbench" =>
        {
            super::web_host::run(name, rt)
        }
        "resolve_anthropic_key" => routed_key(rt, true),
        "resolve_codex_routing" => routed_key(rt, false),
        "preflight_optimizer_endpoint" => super::preflight::run(false, rt),
        "preflight_optimizer" if agent == "codex" => super::preflight::run(true, rt),
        "anthropic_optimizer_call" => super::optimizer::run(false, rt),
        "optimizer_call" if agent == "codex" => super::optimizer::run(true, rt),
        "optimizer_call" if agent == "copilot" => super::optimizer::copilot(rt),
        "_responses_text" if agent == "codex" => super::optimizer::response_text_call(rt),
        "copilot_bin" | "cli_version" | "require_install" | "run_env" if agent == "copilot" => {
            super::copilot_install::run(name, rt)
        }
        "optimizer_log_path" | "_cell_of" | "set_optimizer_context" | "optimizer_call_kind" => {
            super::optimizer_context::run(name, rt)
        }
        "_log_optimizer_call" => super::optimizer_log::record(rt),
        "_log" => super::optimizer_log::verbose(rt),
        "runner_optimizer_swebench" | "runner_optimizer_webbench" => {
            super::runner_calls::optimizer(agent, name.ends_with("webbench"), rt)
        }
        "runner_preflight" => super::runner_calls::preflight(agent, rt),
        "runner_factory_swebench" | "runner_factory_webbench" => {
            super::runner_calls::factory(name.ends_with("webbench"), rt)
        }
        "agent_credential_env" => {
            for key in names(rt, "IGNORED_ANTHROPIC_VARS")? {
                rt.call("provider_env_pop", json!([key]))?;
            }
            rt.call("provider_env_set", json!(["ANTHROPIC_API_KEY", "api_key"]))?;
            Ok(returned("env"))
        }
        "byok_active" => {
            capture(rt, "base", "COPILOT_PROVIDER_BASE_URL", None)?;
            Ok(json!(boolean(rt, "base")?))
        }
        "provider_base" => {
            capture(rt, "base", "COPILOT_PROVIDER_BASE_URL", None)?;
            if !boolean(rt, "base")? {
                literal(rt, "base", json!("github-copilot"))?;
            }
            Ok(returned("base"))
        }
        "auth_env" | "provider_env" | "provider_secret_names" => forward(rt, name),
        "anthropic_optimizer_endpoint" => endpoint(rt),
        "normalize_model" => normalize(rt),
        "effort_applies" => {
            capture(rt, "effort", "TE_EFFORT_APPLIES", None)?;
            if equal(rt, "effort", "0")? || !boolean(rt, "model")? {
                return Ok(json!(false));
            }
            let text = rt.call("provider_stringify", json!(["model"]))?;
            Ok(json!(text.as_str().unwrap_or("").to_lowercase() != "auto"))
        }
        "validate_priceable" => {
            rt.call("provider_pricing", json!([]))?;
            Ok(Value::Null)
        }
        "fetch_catalog" if agent == "codex" => catalogue(rt),
        "verify_model" if agent == "codex" => verify(rt),
        _ => Err(Failure::new(
            "ValueError",
            format!("unknown provider operation: {method}"),
        )),
    }
}
