use crate::pricing::approx_tokens;
use serde_json::Value;

pub fn optimizer_method(
    present: bool,
    has_optimizer: bool,
    callable: bool,
    representation: &str,
) -> Result<Option<&'static str>, String> {
    if !present {
        return Ok(None);
    }
    if has_optimizer {
        return Ok(Some("optimizer_call"));
    }
    if callable {
        return Ok(Some("__call__"));
    }
    Err(format!(
        "TypeError: cannot use {representation} as an optimizer runner"
    ))
}

pub trait Runner {
    fn run(&mut self, prompt: &str, model: &str) -> Result<(String, Option<f64>), String>;

    fn estimate_effective(&mut self, prompt: &str, raw: &str) -> Result<f64, String> {
        Ok((approx_tokens(prompt) + 5 * approx_tokens(raw)) as f64)
    }

    fn local_answer(&self) -> Option<Value> {
        None
    }
}
