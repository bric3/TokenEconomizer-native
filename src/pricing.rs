use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::value::{integer, number};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Weights {
    pub input: f64,
    pub cache_read: f64,
    pub cache_write: f64,
    pub output: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_write_1h: Option<f64>,
}

pub const WEIGHTS_CLAUDE: Weights = Weights {
    input: 1.0,
    cache_read: 0.10,
    cache_write: 1.25,
    output: 5.0,
    cache_write_1h: Some(2.0),
};

// Ratios are the pinned upstream snapshot, including its November additions.
pub fn model_weights(model: &str) -> Option<Weights> {
    let (cache_read, cache_write, output) = match model {
        "claude-haiku-4-5" | "claude-opus-4-6" | "claude-opus-4-7" | "claude-opus-4-8"
        | "claude-opus-5" | "claude-sonnet-4-5" | "claude-sonnet-4-6" | "claude-sonnet-5" => {
            return Some(WEIGHTS_CLAUDE);
        }
        "gemini-2.5-pro" => (0.096, 0.0, 8.0),
        "gemini-3-flash-preview"
        | "gemini-3.1-pro-preview"
        | "gemini-3.5-flash"
        | "gemini-3.1-flash-lite" => (0.10, 0.0, 6.0),
        "gemini-3.6-flash" | "gemini-3.8-flash" => (0.10, 0.0, 5.0),
        "gemini-3.5-flash-lite" => (0.10, 0.0, 8.333),
        "gemini-4-argon" | "gemini-4-carbon" | "gemini-4-hydrogen" => (0.05, 0.0, 5.0),
        "gpt-5-mini" => (0.08, 0.0, 8.0),
        "gpt-5.3-codex" => (0.0971, 0.0, 8.0),
        "gpt-5.4" | "gpt-5.5" => (0.10, 0.0, 6.0),
        "gpt-5.4-mini" | "mai-code-1-flash-picker" => (0.0933, 0.0, 6.0),
        "gpt-5.6-luna" | "gpt-5.6-sol" => (0.10, 1.25, 6.0),
        "gpt-5.6-terra" => (0.10, 1.248, 6.0),
        "gpt-5" | "gpt-5.1" | "gpt-5-codex" | "gpt-5.1-codex" | "gpt-5.1-codex-max"
        | "gpt-5.2-codex" => (0.10, 1.0, 8.0),
        "codex-mini-latest" => (0.25, 1.0, 4.0),
        "gpt-6-astra" | "gpt-6-luna" | "gpt-6-sol" => (0.10, 1.25, 5.0),
        "gpt-6.1-sol" | "gpt-6.1-terra" => (0.05, 1.25, 5.0),
        _ => return None,
    };
    Some(Weights {
        input: 1.0,
        cache_read,
        cache_write,
        output,
        cache_write_1h: None,
    })
}

pub fn resolve_model(record: &Value) -> Option<&str> {
    if let Some(model) = record["model"].as_str().filter(|s| !s.is_empty()) {
        return model.rsplit('/').next();
    }
    record["agent"]
        .as_str()
        .filter(|agent| agent.eq_ignore_ascii_case("codex"))
        .map(|_| "gpt-5.6-sol")
}

pub fn weights_for(record: &Value) -> Result<Weights, String> {
    let Some(model) = resolve_model(record).filter(|s| !s.is_empty()) else {
        return Ok(WEIGHTS_CLAUDE);
    };
    let key = model.to_lowercase();
    if let Some(weights) = model_weights(&key) {
        return Ok(weights);
    }
    if key.contains("claude") {
        static CHEAPER: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"claude-(opus|sonnet)-5-5\b").unwrap());
        static CHEAPEST: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"claude-(fable|mythos)-5-[12]\b").unwrap());
        let name = key.replace('.', "-");
        let cache_read = if CHEAPER.is_match(&name) {
            0.05
        } else if CHEAPEST.is_match(&name) {
            0.025
        } else {
            0.10
        };
        return Ok(Weights {
            cache_read,
            ..WEIGHTS_CLAUDE
        });
    }
    Err(format!(
        "pricing: no weight entry for billed model {model:?} (agent={:?}). Add it to pricing.MODEL_WEIGHTS (from the vendor's published price list).",
        record["agent"]
    ))
}

pub fn has_breakdown(record: &Value) -> bool {
    record.is_object() && !record["input_tokens"].is_null()
}

pub fn cache_write_1h_tokens(usage: &Value) -> f64 {
    let n = if usage["cache_creation_1h_input_tokens"].is_null() {
        &usage["cache_creation"]["ephemeral_1h_input_tokens"]
    } else {
        &usage["cache_creation_1h_input_tokens"]
    };
    match (integer(n), integer(&usage["cache_creation_input_tokens"])) {
        (Some(n), Some(total)) => n.min(total).max(0.0),
        _ => 0.0,
    }
}

pub fn effective_tokens(record: &Value, weights: Option<Weights>) -> Result<f64, String> {
    let w = match weights {
        Some(w) => w,
        None => weights_for(record)?,
    };
    if has_breakdown(record) {
        let count = |key: &str| number(&record[key]).unwrap_or(0.0);
        let one_hour = cache_write_1h_tokens(record);
        Ok(count("input_tokens") * w.input
            + count("cache_read_input_tokens") * w.cache_read
            + (count("cache_creation_input_tokens") - one_hour) * w.cache_write
            + one_hour * w.cache_write_1h.unwrap_or(w.cache_write)
            + count("output_tokens") * w.output)
    } else {
        Ok(number(&record["tokens_total"]).unwrap_or(0.0))
    }
}

pub fn approx_tokens(text: &str) -> usize {
    (text.chars().count() / 4).max(1)
}

pub fn runner_effective_or_approx<E>(
    prompt: &str,
    raw: &str,
    real: Option<f64>,
    estimator: Option<E>,
) -> f64
where
    E: FnOnce(&str, &str) -> Result<f64, String>,
{
    if let Some(real) = real {
        return real;
    }
    if let Some(Ok(estimated)) = estimator.map(|estimate| estimate(prompt, raw)) {
        return estimated;
    }
    (approx_tokens(prompt) + 5 * approx_tokens(raw)) as f64
}
