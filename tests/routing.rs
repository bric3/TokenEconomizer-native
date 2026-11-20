// Converted from reference/upstream/experiments/tests/test_routing.py.
#[path = "support/provider_fixture.rs"]
pub mod provider_fixture;

use provider_fixture::{Constructor, Provider};
use serde_json::json;
use tokeneconomizer::experiments::swebench::{providers, setup};

#[test]
fn the_key_is_the_one_credential() {
    assert_eq!(
        Provider::new("claude", json!({"ANTHROPIC_API_KEY":"sk-ant-x"}))
            .run("resolve_anthropic_key")
            .unwrap(),
        "sk-ant-x"
    );
}

#[test]
fn nothing_but_the_api_key_satisfies_the_claude_lane() {
    for env in [
        json!({}),
        json!({"ANTHROPIC_BASE_URL":"https://example.invalid", "ANTHROPIC_AUTH_TOKEN":"tok"}),
        json!({"ANTHROPIC_AUTH_TOKEN":"tok"}),
        json!({"OPENAI_API_KEY":"sk-test"}),
    ] {
        let error = Provider::new("claude", env)
            .run("resolve_anthropic_key")
            .unwrap_err();
        assert_eq!(error.kind, "RuntimeError");
        assert!(error.message.contains("ANTHROPIC_API_KEY"));
    }
}

#[test]
fn a_set_base_url_is_reported_and_ignored() {
    let env = json!({"TE_VERBOSE":"1", "ANTHROPIC_API_KEY":"sk-ant-x", "ANTHROPIC_BASE_URL":"https://example.invalid", "ANTHROPIC_AUTH_TOKEN":"tok"});
    let mut p = Provider::new("claude", env.clone());
    assert_eq!(p.run("resolve_anthropic_key").unwrap(), "sk-ant-x");
    let out = p.logs.join("\n");
    assert_eq!(
        out.matches("ANTHROPIC_BASE_URL is set and ignored").count(),
        1
    );
    assert!(out.contains("api.anthropic.com"));
    assert_eq!(json!(p.env), env);
}

#[test]
fn no_notice_when_no_base_url_is_set() {
    let mut p = Provider::new(
        "claude",
        json!({"TE_VERBOSE":"1", "ANTHROPIC_API_KEY":"sk-ant-x"}),
    );
    p.run("resolve_anthropic_key").unwrap();
    assert!(!p.logs.join("\n").contains("ignored"));
}

#[test]
fn the_ignored_set_covers_every_steering_variable() {
    let constants = providers::constants();
    let ignored = constants["claude"]["IGNORED_ANTHROPIC_VARS"]
        .as_array()
        .unwrap();
    assert!(ignored.contains(&json!("ANTHROPIC_BASE_URL")));
    assert!(ignored.contains(&json!("ANTHROPIC_AUTH_TOKEN")));
    assert!(!ignored.contains(&json!("ANTHROPIC_API_KEY")));
}

#[test]
fn the_claude_runners_record_the_anthropic_api_as_their_endpoint() {
    assert_eq!(
        providers::optimizer_constants()["claude"]["_ANTHROPIC_DIRECT"],
        "https://api.anthropic.com"
    );
    for domain in ["claude", "web.claude"] {
        let mut r = Constructor::new(json!({"ANTHROPIC_API_KEY":"sk-ant-x"}));
        setup::construct(domain, &mut r).unwrap();
        assert_eq!(r.fields["routing_base"], "https://api.anthropic.com");
        assert_eq!(r.fields["api_key"], "sk-ant-x");
        assert_eq!(r.resolutions, 1);
    }
}

#[test]
fn the_subscription_is_the_default() {
    let mut p = Provider::new("copilot", json!({}));
    assert_eq!(p.run("byok_active").unwrap(), false);
    assert_eq!(p.run("provider_base").unwrap(), "github-copilot");
    assert_eq!(p.run("provider_env").unwrap(), json!({}));
    assert_eq!(p.run("provider_secret_names").unwrap(), json!([]));
}

#[test]
fn byok_is_forwarded_to_the_cli_as_configured() {
    let mut p = Provider::new(
        "copilot",
        json!({"COPILOT_PROVIDER_BASE_URL":"https://api.anthropic.com", "COPILOT_PROVIDER_TYPE":"anthropic", "COPILOT_PROVIDER_API_KEY":"sk-ant-x", "COPILOT_PROVIDER_MODEL_ID":"claude-sonnet-5"}),
    );
    assert_eq!(p.run("byok_active").unwrap(), true);
    assert_eq!(p.run("provider_base").unwrap(), "https://api.anthropic.com");
    assert_eq!(
        p.run("provider_env").unwrap(),
        json!({"COPILOT_PROVIDER_BASE_URL":"https://api.anthropic.com", "COPILOT_PROVIDER_TYPE":"anthropic", "COPILOT_PROVIDER_MODEL_ID":"claude-sonnet-5"})
    );
    assert_eq!(
        p.run("provider_secret_names").unwrap(),
        json!(["COPILOT_PROVIDER_API_KEY"])
    );
    assert!(
        !p.run("run_env")
            .unwrap()
            .as_object()
            .unwrap()
            .values()
            .any(|value| value == "sk-ant-x")
    );
}

#[test]
fn an_unrelated_key_never_fills_a_byok_slot() {
    let mut p = Provider::new("copilot", json!({"ANTHROPIC_API_KEY":"sk-ant-x"}));
    assert_eq!(p.run("byok_active").unwrap(), false);
    p.env.insert(
        "COPILOT_PROVIDER_BASE_URL".into(),
        json!("https://example.invalid"),
    );
    assert!(
        p.run("provider_env")
            .unwrap()
            .get("COPILOT_PROVIDER_TYPE")
            .is_none()
    );
    assert_eq!(p.run("provider_secret_names").unwrap(), json!([]));
}
