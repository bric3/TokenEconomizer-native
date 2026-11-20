// Converted from reference/upstream/experiments/tests/test_codex_routing.py.
#[path = "support/provider_fixture.rs"]
pub mod provider_fixture;

use provider_fixture::Provider;
use serde_json::json;

const KEY: &str = "sk-test-key-abcd";

#[test]
fn the_mechanism_call_is_a_tool_free_responses_request_to_the_openai_api() {
    let mut provider = Provider::new("codex", json!({}));
    provider.http_response = json!({"output":[{"type":"message","content":[{"type":"output_text","text":"ok"}]}],"usage":{"input_tokens":3,"output_tokens":1}});
    let result = provider.run_with("optimizer_call",json!({"prompt":"Reply with the single word: ok","configured_model":"gpt-5.6-terra","routing":{"base_url":"https://api.openai.com/v1","api_key":KEY},"max_tokens":16,"timeout":180})).unwrap();
    assert_eq!(result[0], "ok");
    assert!(result[1].as_f64().unwrap() > 0.0);
    assert_eq!(provider.requests.len(), 1);
    let request = &provider.requests[0];
    assert_eq!(request["url"], "https://api.openai.com/v1/responses");
    assert_eq!(request["headers"]["authorization"], format!("Bearer {KEY}"));
    assert!(request["headers"].get("user-agent").is_none());
    let body = &request["body"];
    assert_eq!(body["model"], "gpt-5.6-terra");
    assert!(body.get("tools").is_none() && body.get("tool_choice").is_none());
    assert_eq!(body["max_output_tokens"], 16);
    assert_eq!(body["store"], false);
}

#[test]
fn the_model_check_reads_the_openai_catalog() {
    let mut provider = Provider::new("codex", json!({}));
    provider.http_response =
        json!({"object":"list","data":[{"id":"gpt-5.6-sol"},{"id":"gpt-5.6-terra"}]});
    let args = |model| json!({"model":model,"routing":{"base_url":"https://api.openai.com/v1","api_key":KEY}});
    assert_eq!(
        provider
            .run_with("verify_model", args("gpt-5.6-sol"))
            .unwrap(),
        json!([true, ""])
    );
    let result = provider.run_with("verify_model", args("gpt-9")).unwrap();
    assert_eq!(result[0], false);
    assert!(result[1].as_str().unwrap().contains("gpt-5.6-terra"));
    assert_eq!(
        provider.requests,
        vec![
            json!({"url":"https://api.openai.com/v1/models","headers":{"authorization":format!("Bearer {KEY}")}})
        ]
    );
}

#[test]
fn an_unreadable_catalog_is_inconclusive_not_a_refusal() {
    let mut provider = Provider::new("codex", json!({}));
    provider.http_error = true;
    let result = provider.run_with("verify_model",json!({"model":"gpt-5.6-sol","routing":{"base_url":"https://api.openai.com/v1","api_key":KEY}})).unwrap();
    assert!(result[0].is_null());
    assert!(result[1].as_str().unwrap().contains("unreadable"));
}
