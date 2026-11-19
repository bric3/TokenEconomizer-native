// Converted from reference/upstream/tests/test_pricing_new_models.py.
use serde_json::{Value, json};
use tokeneconomizer::{accounting, pricing};

// Vendor prices are the pinned upstream fixture, dated 2026-11-13.
const PRICES: [(&str, f64, f64, Option<f64>, f64); 11] = [
    ("gpt-6-astra", 10.0, 1.0, Some(12.5), 50.0),
    ("gpt-6-luna", 0.1, 0.01, Some(0.125), 0.5),
    ("gpt-6-sol", 2.0, 0.2, Some(2.5), 10.0),
    ("gpt-6.1-sol", 2.0, 0.1, Some(2.5), 10.0),
    ("gpt-6.1-terra", 1.0, 0.05, Some(1.25), 5.0),
    ("gemini-3.1-flash-lite", 0.25, 0.025, None, 1.5),
    ("gemini-3.5-flash-lite", 0.30, 0.03, None, 2.50),
    ("gemini-3.8-flash", 0.75, 0.075, None, 3.75),
    ("gemini-4-argon", 2.0, 0.1, None, 10.0),
    ("gemini-4-carbon", 2.0, 0.1, None, 10.0),
    ("gemini-4-hydrogen", 1.5, 0.075, None, 7.5),
];

fn usage(model: &str) -> Value {
    json!({"input_tokens":800, "cache_read_input_tokens":2000,
        "cache_creation_input_tokens":200, "output_tokens":120, "model":model})
}

fn assert_approx(actual: f64, expected: f64, relative: f64) {
    assert!(
        (actual - expected).abs() <= (expected.abs() * relative).max(1e-12),
        "{actual} != {expected}"
    );
}

#[test]
fn a_models_weights_are_its_prices_over_its_input_price() {
    for (model, price_in, cached, write, price_out) in PRICES {
        let weights = pricing::weights_for(&json!({"model":model})).unwrap();
        assert_eq!(weights.input, 1.0, "{model}");
        assert_approx(weights.cache_read, cached / price_in, 1e-3);
        assert_approx(
            weights.cache_write,
            write.map_or(0.0, |price| price / price_in),
            1e-3,
        );
        assert_approx(weights.output, price_out / price_in, 1e-3);
    }
}

#[test]
fn a_session_on_one_of_these_models_is_priced_by_the_model_not_the_fallback() {
    for (model, ..) in PRICES {
        assert_eq!(
            accounting::effective(&usage(model)).unwrap().1,
            "model",
            "{model}"
        );
    }
}

#[test]
fn a_cache_hit_on_gpt_6_1_terra_is_billed_at_half_the_usual_fraction() {
    let (effective, priced_by) = accounting::effective(&usage("gpt-6.1-terra")).unwrap();
    assert_eq!(priced_by, "model");
    let effective = effective.unwrap();
    assert_approx(
        effective,
        800.0 + 2000.0 * 0.05 + 200.0 * 1.25 + 120.0 * 5.0,
        1e-6,
    );
    let (fallback, _) = accounting::effective(&usage("some-new-model")).unwrap();
    assert!(effective < fallback.unwrap());
}

#[test]
fn an_unlisted_model_is_still_priced_on_the_labelled_fallback() {
    assert_eq!(
        accounting::effective(&usage("some-new-model")).unwrap().1,
        "fallback"
    );
}
