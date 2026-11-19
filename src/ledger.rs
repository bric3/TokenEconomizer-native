use serde_json::Value;

use crate::{
    pricing,
    value::{number, sum_floats},
};

pub const OVERHEAD_KEYS: [&str; 6] = [
    "optimizer_tokens",
    "untool_classify_tokens",
    "untool_judge_tokens",
    "codex_untool_adapt_tokens",
    "spiral_judge_tokens",
    "spiral_nudge_writer_tokens",
];

pub fn overhead(row: &Value) -> f64 {
    // shortcut: mixed wide integers are rounded before summing; retain integer prefixes in the numeric parity audit.
    sum_floats(
        OVERHEAD_KEYS
            .iter()
            .map(|key| number(&row[*key]).unwrap_or(0.0)),
    )
}

pub fn effective_exec(row: &Value) -> Result<f64, pricing::PricingError> {
    pricing::effective_tokens(row, None)
}
