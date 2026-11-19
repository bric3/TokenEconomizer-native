use serde_json::json;
use tokeneconomizer::plugin::config::LiveConfig;

#[test]
fn setting_names_are_independent_of_instance_fields() {
    let mut config = LiveConfig::default();
    config.values.insert("custom".into(), json!(true));
    config.values.remove("mode");

    assert!(!config.set("custom", json!(false), "test"));
    assert_eq!(config.values["custom"], true);
    assert!(config.set("mode", json!("headless"), "test"));
    assert_eq!(config.values["mode"], "headless");
}

#[test]
fn configuration_instances_own_their_default_values() {
    let mut first = LiveConfig::default();
    first.values["local_roles"]
        .as_array_mut()
        .unwrap()
        .push(json!("proposer"));
    assert!(first.set("mode", json!("headless"), "test"));

    let second = LiveConfig::default();
    assert_eq!(second.values["local_roles"], json!(["judge"]));
    assert_eq!(second.values["mode"], "auto");
}
