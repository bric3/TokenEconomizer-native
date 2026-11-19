// Converted from reference/upstream/tests/test_contribution_finite_config.py.
use serde_json::json;
use tokeneconomizer::plugin::config::LiveConfig;

#[test]
fn nonfinite_config_is_rejected() {
    for (key, value) in [
        ("loopguard.judge_confidence", json!("nan")),
        ("loopguard.envelope_margin", json!("inf")),
        ("loopguard.min_turns", json!(f64::INFINITY)),
        ("repomap_tokens", json!(f64::INFINITY)),
    ] {
        let mut cfg = LiveConfig::default();
        let before = cfg.values.clone();
        assert!(!cfg.set(key, value.clone(), "test"), "{key}={value}");
        assert_eq!(cfg.values, before, "{key}={value}");
        assert!(!cfg.warnings.is_empty(), "{key}={value}");
    }
}
