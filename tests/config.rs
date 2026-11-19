// Converted from reference/upstream/tests/test_config.py.
use serde_json::json;
use tokeneconomizer::config::{EconomizerConfig, InjectionConfig};

#[test]
fn defaults_all_off() {
    let cfg = EconomizerConfig::default();
    assert!(!cfg.injection.enabled);
    assert!(!cfg.tooling.enabled());
    assert!(!cfg.spiral.enabled());
}

#[test]
fn from_args_generic_trimech() {
    let cfg = EconomizerConfig::from_args(&json!({
        "inject": true, "propose_every": 5,
        "untool_evidence": 3, "untool_evidence_reaudit": 12, "untool_judge": true,
        "spiral_guard": true, "spiral_judge": true, "spiral_nudge": true
    }))
    .unwrap();
    assert!(cfg.injection.enabled && cfg.injection.propose_every == 5);
    assert_eq!(cfg.tooling.evidence, 3);
    assert!(cfg.tooling.judge);
    assert!(cfg.tooling.enabled());
    assert!(cfg.spiral.guard && cfg.spiral.judge && cfg.spiral.nudge);
    assert!(cfg.spiral.enabled());
}

#[test]
fn from_args_codex() {
    let cfg =
        EconomizerConfig::from_args(&json!({"codex_untool": true, "codex_spiral": true})).unwrap();
    assert!(cfg.tooling.codex_untool && cfg.tooling.enabled());
    assert!(cfg.spiral.codex_spiral && cfg.spiral.enabled());
    assert!(!cfg.injection.enabled);
}

#[test]
fn propose_every_defaults_when_zero() {
    let cfg = EconomizerConfig::from_args(&json!({"inject": true, "propose_every": 0})).unwrap();
    assert_eq!(
        cfg.injection.propose_every,
        InjectionConfig::default().propose_every
    );
}

#[test]
fn partial_args_default_off() {
    let cfg = EconomizerConfig::from_args(&json!({})).unwrap();
    assert!(!cfg.injection.enabled && !cfg.tooling.enabled() && !cfg.spiral.enabled());
}
