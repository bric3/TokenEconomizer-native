// Converted from reference/upstream/experiments/tests/test_model_spelling.py.
mod support;

use serde_json::{Value, json};
use std::{fs, path::Path};
use support::TempDir;
use tokeneconomizer::experiments::launch::{Context, ShellOutput};

fn run(name: &str, args: &[&str], env: Value) -> ShellOutput {
    Context::new(
        env.as_object().unwrap().clone(),
        Path::new(env!("CARGO_MANIFEST_DIR")).into(),
    )
    .function(
        name,
        &args.iter().map(|value| (*value).into()).collect::<Vec<_>>(),
    )
    .unwrap()
}

#[test]
fn alt_spelling_swaps_the_version_separator() {
    for (model, alt) in [
        ("claude-opus-4-8", "claude-opus-4.8"),
        ("claude-opus-4.8", "claude-opus-4-8"),
        ("claude-haiku-4-5", "claude-haiku-4.5"),
        ("claude-haiku-4.5", "claude-haiku-4-5"),
        ("claude-opus-5", ""),
        ("claude-sonnet-5", ""),
        ("auto", ""),
    ] {
        let out = run("te_model_alt_spelling", &[model], json!({}));
        assert_eq!(out.code, 0, "{}", out.stderr);
        assert_eq!(out.stdout.trim(), alt, "{model}");
    }
}

#[test]
fn model_for_agent_picks_the_served_spelling() {
    for (agent, byok, model, expected) in [
        ("copilot", "", "claude-opus-4-8", "claude-opus-4.8"),
        ("copilot", "", "claude-opus-4.8", "claude-opus-4.8"),
        (
            "copilot",
            "https://api.anthropic.com",
            "claude-opus-4.8",
            "claude-opus-4-8",
        ),
        (
            "copilot",
            "https://api.anthropic.com",
            "claude-opus-4-8",
            "claude-opus-4-8",
        ),
        ("claude", "", "claude-opus-4.8", "claude-opus-4-8"),
        ("claude", "", "claude-opus-4-8", "claude-opus-4-8"),
        ("claude", "", "claude-haiku-4.5", "claude-haiku-4-5"),
        ("copilot", "", "claude-opus-5", "claude-opus-5"),
        ("claude", "", "claude-sonnet-5", "claude-sonnet-5"),
        ("codex", "", "gpt-5.6-sol", "gpt-5.6-sol"),
        ("copilot", "", "auto", "auto"),
    ] {
        let out = run(
            "te_model_for_agent",
            &[agent, model],
            json!({"COPILOT_PROVIDER_BASE_URL":byok}),
        );
        assert_eq!(out.code, 0, "{}", out.stderr);
        assert_eq!(out.stdout.trim(), expected);
        assert_eq!(out.stderr.contains("model id:"), expected != model);
    }
}

#[test]
fn model_for_agent_is_verbatim_under_strict() {
    for (agent, model) in [
        ("copilot", "claude-opus-4-8"),
        ("claude", "claude-opus-4.8"),
    ] {
        let out = run(
            "te_model_for_agent",
            &[agent, model],
            json!({"TE_STRICT_MODEL_ID":"1"}),
        );
        assert_eq!(out.stdout.trim(), model);
        assert!(!out.stderr.contains("model id:"));
    }
}

#[test]
fn model_for_agent_never_changes_the_tree_tag() {
    for (agent, model) in [
        ("copilot", "claude-opus-4-8"),
        ("claude", "claude-opus-4.8"),
    ] {
        let spelled = run("te_model_for_agent", &[agent, model], json!({}));
        let tag = run("te_model_tag", &[spelled.stdout.trim()], json!({}));
        assert_eq!(tag.stdout.trim(), "opus48");
    }
}

fn check(root: &Path, model: &str, extra: Value) -> (ShellOutput, Vec<String>) {
    let log = root.join("asked.log");
    fs::write(&log, "").unwrap();
    let mut env = json!({"PATH":"", "HOME":root, "COPILOT_HOST_BIN":env!("CARGO_BIN_EXE_te-model-probe"), "STUB_LOG":log, "MODEL":model});
    env.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    let out = run("te_check_copilot_model", &[model, "high"], env);
    let asked = fs::read_to_string(log)
        .unwrap()
        .split_whitespace()
        .map(String::from)
        .collect();
    (out, asked)
}

#[test]
fn copilot_model_check_switches_to_the_cli_spelling() {
    let root = TempDir::new("model-alternate");
    let (out, asked) = check(root.path(), "claude-opus-4-8", json!({}));
    assert_eq!(out.code, 0, "{}", out.stderr);
    assert_eq!(out.changed["MODEL"], "claude-opus-4.8");
    assert!(
        out.stdout
            .contains("copilot model 'claude-opus-4.8' serves; effort=high accepted")
    );
    assert!(
        out.stderr
            .contains("does not serve 'claude-opus-4-8' but DOES serve 'claude-opus-4.8'")
    );
    assert_eq!(asked, ["claude-opus-4-8", "claude-opus-4.8"]);
}

#[test]
fn copilot_model_check_is_silent_on_a_served_id() {
    let root = TempDir::new("model-served");
    let (out, asked) = check(root.path(), "claude-sonnet-5", json!({}));
    assert_eq!(out.code, 0);
    assert!(!out.stderr.contains("!!"));
    assert_eq!(asked, ["claude-sonnet-5"]);
}

#[test]
fn copilot_model_check_refuses_a_byok_404() {
    let root = TempDir::new("model-byok-404");
    let (out, _) = check(
        root.path(),
        "claude-sonnet-9",
        json!({"COPILOT_PROVIDER_BASE_URL":"https://api.anthropic.com"}),
    );
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("not served by the BYOK provider"));
    assert!(!out.stdout.contains("serves"));
}

#[test]
fn copilot_model_check_refuses_when_neither_spelling_serves() {
    let root = TempDir::new("model-neither");
    let (out, asked) = check(root.path(), "claude-haiku-4.5", json!({}));
    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("is not available on this subscription"));
    assert_eq!(asked, ["claude-haiku-4.5", "claude-haiku-4-5"]);
}

#[test]
fn copilot_model_check_can_be_made_strict() {
    let root = TempDir::new("model-strict");
    let (out, asked) = check(
        root.path(),
        "claude-opus-4-8",
        json!({"TE_STRICT_MODEL_ID":"1"}),
    );
    assert_eq!(out.code, 2);
    assert_eq!(asked, ["claude-opus-4-8"]);
}
