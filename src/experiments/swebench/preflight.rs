use super::{
    Failure, Result, Runtime,
    providers::{boolean, capture, equal, literal, trim},
};
use crate::value::scalar_text;
use serde_json::{Value, json};

fn formatted(rt: &mut dyn Runtime, slot: &str, spec: &str) -> Result<String> {
    rt.call("preflight_format", json!([slot, spec]))
        .map(|v| scalar_text(&v))
}
fn log(rt: &mut dyn Runtime, text: String) -> Result<()> {
    rt.call("preflight_log", json!([text])).map(|_| ())
}
fn endpoint(rt: &mut dyn Runtime, codex: bool) -> Result<String> {
    if codex {
        rt.call("provider_routing_item", json!(["base_url", "base_url"]))?;
    }
    formatted(rt, "base_url", "")
}
pub fn run(codex: bool, rt: &mut dyn Runtime) -> Result<Value> {
    capture(rt, "gate", "TE_OPTIMIZER_PREFLIGHT", Some("1"))?;
    if equal(rt, "gate", "0")? {
        // The source captures the logger before formatting can replace it.
        rt.call("preflight_capture_log", json!([]))?;
        let label = if codex {
            "codex".into()
        } else {
            formatted(rt, "label", "")?
        };
        log(
            rt,
            format!(
                "{label}: no mechanism on this arm makes a paid call -- skipping the endpoint preflight"
            ),
        )?;
        return Ok(Value::Null);
    }
    let kwargs = if codex {
        json!({"configured_model":["ref","configured_model"],"routing":["ref","routing"],"max_tokens":["value",16]})
    } else {
        json!({"model":["value","agent"],"configured_model":["ref","configured_model"],"max_tokens":["value",16],"api_key":["ref","api_key"],"bearer":["ref","bearer"],"base_url":["ref","base_url"]})
    };
    rt.call(
        "preflight_invoke",
        json!([
            if codex {
                "optimizer_call"
            } else {
                "anthropic_optimizer_call"
            },
            "Reply with the single word: ok",
            kwargs
        ]),
    )?;
    if !boolean(rt, "probe")? {
        literal(rt, "probe", json!(""))?;
    }
    trim(rt, "probe", false)?;
    if !boolean(rt, "probe")? {
        let base = endpoint(rt, codex)?;
        let remedy = if codex {
            "Check the endpoint and the model id, then re-launch.".into()
        } else {
            formatted(rt, "remedy", "")?
        };
        return Err(Failure::new(
            "RuntimeError",
            format!(
                "the mechanism-call endpoint {base} did not answer a 16-token probe. Every mechanism would read that as 'no verdict' and quietly fall back to its LLM-free path, so an economized arm would run as a baseline wearing its name. {remedy}"
            ),
        ));
    }
    if codex {
        rt.call("preflight_capture_log", json!([]))?;
        let base = endpoint(rt, true)?;
        let model = formatted(rt, "configured_model", "")?;
        let effective = formatted(rt, "effective", ".0f")?;
        log(
            rt,
            format!(
                "codex: mechanism endpoint answered the 16-token preflight probe ({base}, model {model}, {effective} eff tokens)"
            ),
        )?;
    }
    Ok(Value::Null)
}
