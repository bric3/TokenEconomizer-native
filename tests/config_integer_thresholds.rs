// Converted from reference/upstream/tests/test_config_integer_thresholds.py.
mod support;

use serde_json::{Map, json};
use std::fs;
use support::TempDir;
use tokeneconomizer::plugin::config::{LOOPGUARD_KEYS, LiveConfig, load_config, write_config};

#[test]
fn fractional_numeric_count_is_rejected_without_changing_the_previous_value() {
    for spec in LOOPGUARD_KEYS.iter().filter(|spec| spec.integer) {
        for prefix in ["loopguard", "spiral"] {
            let mut cfg = LiveConfig::default();
            let key = format!("{prefix}.{}", spec.name);
            assert!(cfg.set(&key, json!(8), "test"), "{key}");
            assert!(!cfg.set(&key, json!(12.5), "test"), "{key}");
            assert_eq!(cfg.values["loopguard_thresholds"][spec.name], 8, "{key}");
            assert!(!cfg.warnings.is_empty(), "{key}");
        }
    }
}

#[test]
fn whole_number_representations_remain_supported() {
    for value in [json!(12), json!(12.0), json!("12"), json!("12.0")] {
        let mut cfg = LiveConfig::default();
        assert!(
            cfg.set("loopguard.min_turns", value.clone(), "test"),
            "{value}"
        );
        assert_eq!(
            cfg.values["loopguard_thresholds"]["min_turns"], 12,
            "{value}"
        );
        assert!(
            cfg.values["loopguard_thresholds"]["min_turns"]
                .as_i64()
                .is_some(),
            "{value}"
        );
    }
}

#[test]
fn fractional_count_from_json_does_not_become_a_pinned_threshold() {
    for source in ["file", "environment"] {
        let root = TempDir::new("fractional-threshold");
        let thresholds = json!({"min_turns": 12.5, "stagnation": 0.5});
        let mut env = Map::new();
        if source == "file" {
            fs::write(
                root.path().join("config.json"),
                json!({"loopguard_thresholds": thresholds}).to_string(),
            )
            .unwrap();
        } else {
            env.insert(
                "TE_LOOPGUARD_THRESHOLDS".into(),
                json!(thresholds.to_string()),
            );
        }
        let cfg = load_config(root.path(), None, &env);
        assert!(
            cfg.values["loopguard_thresholds"]
                .get("min_turns")
                .is_none(),
            "{source}"
        );
        assert_eq!(
            cfg.values["loopguard_thresholds"]["stagnation"], 0.5,
            "{source}"
        );
        assert!(!cfg.warnings.is_empty(), "{source}");
    }
}

#[test]
fn fractional_update_leaves_the_saved_config_unchanged() {
    for updates in [
        json!({"loopguard.min_turns": 12.5}),
        json!({"loopguard_thresholds": {"min_turns": 12.5}}),
    ] {
        let root = TempDir::new("fractional-update");
        let path = root.path().join("config.json");
        write_config(
            &path,
            json!({"loopguard.min_turns": 8}).as_object().unwrap(),
        )
        .unwrap();
        let before = fs::read(&path).unwrap();
        assert!(
            write_config(&path, updates.as_object().unwrap()).is_err(),
            "{updates}"
        );
        assert_eq!(fs::read(&path).unwrap(), before, "{updates}");
    }
}

#[test]
fn fractional_or_boolean_integer_setting_is_rejected_without_changing_it() {
    for (key, _) in LiveConfig::default()
        .values
        .iter()
        .filter(|(_, value)| value.as_number().is_some_and(|n| !n.is_f64()))
    {
        for value in [json!(12.5), json!(true)] {
            let mut cfg = LiveConfig::default();
            assert!(cfg.set(key, json!(8), "test"), "{key}");
            assert!(!cfg.set(key, value.clone(), "test"), "{key}={value}");
            assert_eq!(cfg.values[key], 8, "{key}={value}");
            assert!(!cfg.warnings.is_empty(), "{key}={value}");
        }
    }
}

#[test]
fn whole_number_integer_settings_remain_supported() {
    for value in [json!(4096), json!(4096.0), json!("4096")] {
        let mut cfg = LiveConfig::default();
        assert!(cfg.set("repomap_tokens", value.clone(), "test"), "{value}");
        assert_eq!(cfg.values["repomap_tokens"], 4096, "{value}");
        assert!(cfg.values["repomap_tokens"].as_i64().is_some(), "{value}");
    }
}

#[test]
fn fractional_or_boolean_integer_settings_from_a_config_file_keep_the_defaults() {
    let root = TempDir::new("fractional-file");
    fs::write(
        root.path().join("config.json"),
        json!({"repomap_tokens": 4096.5, "microskills_propose_every": true}).to_string(),
    )
    .unwrap();
    let cfg = load_config(root.path(), None, &Map::new());
    let defaults = LiveConfig::default();
    assert_eq!(
        cfg.values["repomap_tokens"],
        defaults.values["repomap_tokens"]
    );
    assert_eq!(
        cfg.values["microskills_propose_every"],
        defaults.values["microskills_propose_every"]
    );
    assert_eq!(cfg.warnings.len(), 2);
}
