use crate::pricing::approx_tokens;
use serde_json::Value;

pub trait Runner {
    fn run(&mut self, prompt: &str, model: &str) -> Result<(String, Option<f64>), String>;

    fn estimate_effective(&mut self, prompt: &str, raw: &str) -> Result<f64, String> {
        Ok((approx_tokens(prompt) + 5 * approx_tokens(raw)) as f64)
    }

    fn local_answer(&self) -> Option<Value> {
        None
    }
}
