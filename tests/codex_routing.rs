// Converted from reference/upstream/experiments/tests/test_codex_routing.py.
pub mod fixtures;
#[path = "support/codex_install_fixture.rs"]
pub mod install_fixture;
#[path = "support/codex_argv.rs"]
pub mod invocation_fixture;
#[path = "support/provider_fixture.rs"]
pub mod provider_fixture;
mod support;

use install_fixture::Install;
use invocation_fixture::codex_argv;
use provider_fixture::Provider;
use serde_json::{Value, json};
use std::{collections::BTreeSet, fs, path::Path};
use support::TempDir;
use tokeneconomizer::{
    config::{SpiralConfig, ToolingConfig},
    experiments::{
        arms,
        launch::Context,
        net_guard,
        spiral_stage::SpiralStage,
        swebench::{self, codex_config as cc, codex_trace, providers, setup},
        usage::codex,
    },
    toolprune::{
        codex::CONFIG_CAPABILITIES,
        stage::{ToolingContext, ToolingStage},
    },
};

const KEY: &str = "sk-test-key-abcd";
fn direct() -> Value {
    json!({"base_url":"https://api.openai.com/v1","api_key":KEY})
}
fn config(extra: Value) -> String {
    let mut args = json!({"routing":direct(),"model":"gpt-5.6-terra","effort":"high"});
    args.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    cc::config_toml(&args, &Default::default()).unwrap()
}
fn stream(events: &[Value]) -> Value {
    codex::row_usage(
        &events
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .unwrap()
}
fn launch(name: &str, args: &[&str]) -> String {
    Context::new(
        Default::default(),
        Path::new(env!("CARGO_MANIFEST_DIR")).into(),
    )
    .function(name, &args.iter().map(|s| (*s).into()).collect::<Vec<_>>())
    .unwrap()
    .stdout
    .trim()
    .into()
}
fn base(version: &str) -> Value {
    let root = TempDir::new("codex-base-prompt");
    Install::new(root.path())
        .run("base_prompt_for", json!({"cli_version":version}))
        .unwrap()
}
fn spec() -> Value {
    json!({"serena":{"command":"/tmp/agent-home/.local/bin/serena","args":["start-mcp-server","--project","/testbed","--context","codex"],"env":{"PATH":"/tmp/agent-home/.local/bin:/usr/bin"}}})
}

#[test]
fn the_lane_resolves_to_the_openai_api() {
    for (env, base) in [
        (json!({"OPENAI_API_KEY":KEY}), "https://api.openai.com/v1"),
        (
            json!({"OPENAI_API_KEY":KEY,"OPENAI_BASE_URL":"https://us.api.openai.com/v1/"}),
            "https://us.api.openai.com/v1",
        ),
        (
            json!({"OPENAI_API_KEY":KEY,"ANTHROPIC_BASE_URL":"https://example.invalid"}),
            "https://api.openai.com/v1",
        ),
    ] {
        assert_eq!(
            Provider::new("codex", env)
                .run("resolve_codex_routing")
                .unwrap(),
            json!({"base_url":base,"api_key":KEY})
        );
    }
}

#[test]
fn without_the_openai_key_the_lane_is_refused_at_construction() {
    for env in [
        json!({}),
        json!({"OPENAI_BASE_URL":"https://us.api.openai.com/v1","ANTHROPIC_API_KEY":"sk-ant-x"}),
    ] {
        let error = Provider::new("codex", env)
            .run("resolve_codex_routing")
            .unwrap_err();
        assert_eq!(error.kind, "RuntimeError");
        assert!(error.message.contains("OPENAI_API_KEY"));
    }
}

#[test]
fn validate_priceable_refuses_an_unknown_id() {
    let mut provider = Provider::new("codex", json!({}));
    provider
        .run_with(
            "validate_priceable",
            json!({"model":"gpt-5.6-sol","agent":"codex"}),
        )
        .unwrap();
    assert_eq!(
        provider
            .run_with(
                "validate_priceable",
                json!({"model":"not-a-model","agent":"codex"})
            )
            .unwrap_err()
            .kind,
        "ValueError"
    );
}

#[test]
fn config_toggles_cover_exactly_the_mechanism_s_config_capabilities() {
    let actual: BTreeSet<_> = serde_json::from_value::<Vec<String>>(
        setup::constants()["codex"]["CONFIG_CAPABILITIES"].clone(),
    )
    .unwrap()
    .into_iter()
    .collect();
    assert_eq!(
        actual,
        CONFIG_CAPABILITIES.map(String::from).into_iter().collect()
    );
}

#[test]
fn config_toggles_refuse_a_protected_capability() {
    let error = cc::config_toggles(
        &json!(["collaboration", "apply_patch"]),
        &Default::default(),
    )
    .unwrap_err();
    assert_eq!(error.kind, "RuntimeError");
    assert!(error.message.contains("protected"));
}

#[test]
fn web_is_blocked_on_every_arm_including_an_empty_mask() {
    let (top, tables) = cc::config_toggles(&json!([]), &Default::default()).unwrap();
    assert!(top.contains(&"web_search = \"disabled\"".into()));
    assert!(!tables.iter().any(
        |(table, line)| matches!(table.as_str(), "features" | "tools")
            && line.contains("web_search")
    ));
    let (top2, tables2) = cc::config_toggles(&json!(["web"]), &Default::default()).unwrap();
    assert_eq!(top2.len(), top2.iter().collect::<BTreeSet<_>>().len());
    assert_eq!(tables2.len(), tables2.iter().collect::<BTreeSet<_>>().len());
    assert_eq!(top.iter().collect::<BTreeSet<_>>(), top2.iter().collect());
}

#[test]
fn network_block_can_be_lifted() {
    assert_eq!(
        cc::config_toggles(
            &json!([]),
            json!({"TE_ALLOW_NETWORK_TOOLS":"1"}).as_object().unwrap()
        )
        .unwrap(),
        (Vec::new(), Vec::new())
    );
}

#[test]
fn config_toml_names_the_openai_api_and_the_key_by_name_only() {
    let text = config(json!({"model":"gpt-5.6-sol","capabilities":[]}));
    for expected in [
        "model_provider = \"openai_direct\"",
        "[model_providers.openai_direct]",
        "base_url = \"https://api.openai.com/v1\"",
        "env_key = \"OPENAI_API_KEY\"",
        "wire_api = \"responses\"",
        "trust_level = \"trusted\"",
    ] {
        assert!(text.contains(expected), "{expected}");
    }
    assert_eq!(text.matches("[model_providers.").count(), 1);
    assert_eq!(providers::constants()["codex"]["KEY_ENV"], "OPENAI_API_KEY");
    assert!(!text.contains("requires_openai_auth"));
    assert!(!text.contains(KEY));
}

#[test]
fn a_regional_endpoint_is_written_as_given() {
    let routing = Provider::new(
        "codex",
        json!({"OPENAI_API_KEY":KEY,"OPENAI_BASE_URL":"https://eu.api.openai.com/v1"}),
    )
    .run("resolve_codex_routing")
    .unwrap();
    assert!(
        config(json!({"routing":routing,"model":"gpt-5.6-sol"}))
            .contains("base_url = \"https://eu.api.openai.com/v1\"")
    );
}

#[test]
fn the_key_reaches_the_container_by_name_only() {
    let env = codex_argv("env", json!({"extra":null}));
    assert!(env.get("OPENAI_API_KEY").is_none());
    assert!(!env.as_object().unwrap().values().any(|v| v == KEY));
}

#[test]
fn row_usage_decomposes_the_grand_input_total() {
    let row = stream(&[
        json!({"type":"thread.started","thread_id":"th-1"}),
        json!({"type":"item.completed","item":{"type":"command_execution","aggregated_output":"ok"}}),
        json!({"type":"item.completed","item":{"type":"file_change"}}),
        json!({"type":"item.completed","item":{"type":"agent_message"}}),
        json!({"type":"turn.completed","usage":{"input_tokens":10000,"cached_input_tokens":9000,"cache_write_input_tokens":400,"output_tokens":300,"reasoning_output_tokens":120}}),
    ]);
    for (key, value) in [
        ("input_tokens", 600),
        ("cache_read_input_tokens", 9000),
        ("cache_creation_input_tokens", 400),
        ("output_tokens", 420),
        ("tokens_total", 10420),
        ("num_turns", 3),
    ] {
        assert_eq!(row[key], value, "{key}");
    }
    assert_eq!(row["turns_basis"], "codex_steps");
    assert_eq!(row["thread_id"], "th-1");
    assert_eq!(row["tokens_source"], "stream_usage");
}

#[test]
fn row_usage_accumulates_across_turns() {
    let row = stream(&[
        json!({"type":"turn.completed","usage":{"input_tokens":100,"cached_input_tokens":10,"output_tokens":5}}),
        json!({"type":"turn.completed","usage":{"input_tokens":200,"cached_input_tokens":20,"output_tokens":7}}),
    ]);
    assert_eq!(row["input_tokens"], 270);
    assert_eq!(row["cache_read_input_tokens"], 30);
    assert_eq!(row["output_tokens"], 12);
}

#[test]
fn a_killed_stream_records_unknown_not_zero() {
    let row = stream(&[json!({"type":"item.completed","item":{"type":"command_execution"}})]);
    assert!(row["input_tokens"].is_null());
    assert!(row["tokens_total"].is_null());
    assert_eq!(row["tokens_source"], "no_turn_event");
}

#[test]
fn merge_usage_propagates_unknown() {
    let good = stream(&[
        json!({"type":"turn.completed","usage":{"input_tokens":100,"cached_input_tokens":10,"output_tokens":5}}),
    ]);
    let dead = stream(&[]);
    let merged = codex::merge_usage(&good, &dead).unwrap();
    assert!(merged["input_tokens"].is_null());
    assert_ne!(merged["tokens_source"], "stream_usage");
    let both = codex::merge_usage(&good, &good).unwrap();
    assert_eq!(both["input_tokens"], 180);
    assert_eq!(both["tokens_source"], "stream_usage");
}

#[test]
fn merge_keeps_the_first_thread_id() {
    let row = |id| {
        stream(&[
            json!({"type":"thread.started","thread_id":id}),
            json!({"type":"turn.completed","usage":{"input_tokens":1}}),
        ])
    };
    assert_eq!(
        codex::merge_usage(&row("th-A"), &row("th-B")).unwrap()["thread_id"],
        "th-A"
    );
}

#[test]
fn guard_reads_every_payload_spelling() {
    for payload in [
        json!({"tool_name":"Bash","tool_input":{"command":"curl https://github.com/x"}}),
        json!({"toolName":"bash","toolArgs":{"command":"curl https://github.com/x"}}),
        json!({"tool":{"name":"shell","input":{"command":["bash","-lc","curl https://github.com/x"]}}}),
        json!({"item":{"type":"command_execution","command":"curl https://github.com/x"}}),
    ] {
        let command = net_guard::hook_command(&payload);
        assert!(!command.is_empty());
        assert!(net_guard::decide(&json!(command)).unwrap().0);
    }
}

#[test]
fn guard_still_returns_empty_for_a_payload_with_no_command() {
    assert_eq!(net_guard::hook_command(&json!({"nothing":1})), "");
    assert!(!net_guard::decide(&json!("")).unwrap().0);
}

#[test]
fn the_codex_label_gets_the_codex_mechanisms() {
    let root = TempDir::new("codex-stage-gates");
    let tooling = ToolingStage::new(
        ToolingConfig {
            codex_untool: true,
            ..Default::default()
        },
        "codex",
        "",
        Some(("acme/widget", root.path())),
        true,
        &mut ToolingContext::default(),
    )
    .unwrap();
    assert!(tooling.codex_profile.is_some());
    let spiral = SpiralStage::new(
        SpiralConfig {
            codex_spiral: true,
            ..Default::default()
        },
        None,
        "",
        Some("codex".into()),
    );
    assert!(spiral.codex_tracker.is_some());
    for source in [
        include_str!("../src/experiments/sdk/te_runtime/swebench.py"),
        include_str!("../src/experiments/sdk/te_runtime/webbench.py"),
    ] {
        let registry = source
            .lines()
            .find_map(|line| line.trim().strip_prefix("module.RUNNERS=dict("))
            .unwrap()
            .trim_end_matches(')');
        let labels: Vec<_> = registry
            .split(',')
            .map(|entry| entry.split_once('=').unwrap().0)
            .filter(|name| name.starts_with("codex"))
            .collect();
        assert_eq!(labels, ["codex"]);
    }
}

#[test]
fn the_codex_tree_is_named_by_agent_domain_and_model() {
    assert_eq!(
        launch(
            "te_tree_name",
            &[
                "codex",
                "swebench",
                &launch("te_model_tag", &["gpt-5.6-sol"])
            ]
        ),
        "codex_swebench_gpt56sol"
    );
}

#[test]
fn a_model_tag_never_contains_a_path_separator() {
    for model in ["vendor/gpt-5.6-sol", "gpt-5.6-sol"] {
        assert!(!launch("te_model_tag", &[model]).contains('/'));
    }
}

#[test]
fn claude_model_tags_are_unchanged() {
    for (model, expected) in [
        ("claude-opus-4-8", "opus48"),
        ("claude-opus-4.8", "opus48"),
        ("claude-sonnet-5", "sonnet5"),
        ("auto", ""),
        ("", ""),
    ] {
        assert_eq!(launch("te_model_tag", &[model]), expected);
    }
}

#[test]
fn the_skills_package_is_pruned_by_disabling_every_bundled_system_skill() {
    let text = config(json!({"capabilities":["skills"],"system_skills":["alpha","beta"]}));
    let blocks: Vec<_> = text.split("[[skills.config]]").collect();
    assert_eq!(blocks.len(), 3);
    assert!(blocks[1].contains(&format!(
        "path = \"{}/skills/.system/alpha/SKILL.md\"",
        cc::HOME
    )));
    assert!(blocks[1].contains("enabled = false") && blocks[2].contains("enabled = false"));
    assert!(text.find("[projects.").unwrap() < text.find("[[skills.config]]").unwrap());
    assert!(!config(json!({"capabilities":[]})).contains("skills.config"));
    assert_eq!(cc::skills_entries(&Value::Null).unwrap().len(), 6);
    assert_eq!(
        setup::constants()["codex"]["DEFAULT_SYSTEM_SKILLS"]
            .as_array()
            .unwrap()
            .len(),
        6
    );
    assert!(CONFIG_CAPABILITIES.contains(&"skills"));
}

const FAKE_BASE: &str = "You are Codex.\n\n# Personality\n\nBe charming.\n\n## Writing style\n\nShort.\n\n# Working with the user\n\nChannels.\n\nYou end your turn by sending a final message.\n\nWhen you run out of context, continue.\n\n# Rules for getting work done\n\n- use rg\n\n## Autonomy and persistence\n\nAct.\n\n# Using skills\n\nSkills intro.\n\n### How to use skills\n\nRead SKILL.md.\n";

#[test]
fn lean_instructions_drops_only_the_pruned_sections_and_keeps_the_rest_verbatim() {
    let lean = cc::lean_instructions(&json!(FAKE_BASE), &json!(["interactive_guidance"])).unwrap();
    let lean = lean.as_str().unwrap();
    for removed in [
        "# Personality",
        "## Writing style",
        "# Working with the user",
        "Channels.",
    ] {
        assert!(!lean.contains(removed));
    }
    for kept in [
        "When you run out of context, continue.",
        "You end your turn by sending a final message.",
        "You are Codex.",
        "# Rules for getting work done\n\n- use rg",
        "## Autonomy and persistence\n\nAct.",
        "# Using skills",
        "### How to use skills",
    ] {
        assert!(lean.contains(kept), "{kept}");
    }
    let both = cc::lean_instructions(
        &json!(FAKE_BASE),
        &json!(["interactive_guidance", "skills"]),
    )
    .unwrap();
    let both = both.as_str().unwrap();
    assert!(!both.contains("# Using skills") && !both.contains("Read SKILL.md."));
    assert!(both.contains("# Rules for getting work done"));
    assert!(
        cc::lean_instructions(&json!(FAKE_BASE), &json!(["collaboration", "goals"]))
            .unwrap()
            .is_null()
    );
    assert!(
        cc::lean_instructions(&Value::Null, &json!(["interactive_guidance"]))
            .unwrap()
            .is_null()
    );
}

#[test]
fn the_captured_base_prompt_exists_for_the_pinned_cli_and_for_no_other() {
    let captured = base("codex-cli 0.153.1");
    let captured = captured.as_str().unwrap();
    assert!(captured.starts_with("You are Codex") && captured.chars().count() / 4 > 4000);
    assert!(base("codex-cli 9.9.9").is_null());
    assert!(base("").is_null());
    let lean = cc::lean_instructions(&json!(captured), &json!(["interactive_guidance", "skills"]))
        .unwrap();
    let lean = lean.as_str().unwrap();
    assert!((lean.chars().count() as f64) < 0.45 * captured.chars().count() as f64);
    for kept in [
        "# Rules for getting work done",
        "## File editing constraints",
        "## Autonomy and persistence",
        "# Destructive Actions",
    ] {
        assert!(lean.contains(kept));
    }
    for removed in [
        "# Personality",
        "## Writing style",
        "## Technical communication",
        "# Working with the user",
        "## Intermediate commentary",
        "## Final answer",
        "### Formatting rules",
        "### Visualizations",
        "# Using skills",
        "### How to use skills",
    ] {
        assert!(!lean.contains(&format!("\n{removed}\n")));
    }
}

#[test]
fn config_toml_names_the_instructions_file_only_when_a_lean_prompt_exists() {
    assert!(
        config(json!({"capabilities":["interactive_guidance"],"instructions":"lean text"}))
            .contains(&format!(
                "model_instructions_file = \"{}\"",
                cc::INSTRUCTIONS
            ))
    );
    assert!(
        !config(json!({"capabilities":["interactive_guidance"],"instructions":null}))
            .contains("model_instructions_file")
    );
    assert!(CONFIG_CAPABILITIES.contains(&"interactive_guidance"));
}

#[test]
fn codex_trimech_names_no_endpoint() {
    let flat = serde_json::to_string(&arms::get("codex-trimech").unwrap().config)
        .unwrap()
        .to_lowercase();
    for token in ["openai", "copilot", "http", "base_url", "api_key"] {
        assert!(!flat.contains(token), "{token}");
    }
}

#[test]
fn rtk_compliance_is_read_off_the_trace() {
    let root = TempDir::new("codex-rtk-compliance");
    let path = fixtures::write_codex_transcript(
        &root.path().join("t.jsonl"),
        &[
            json!({"type":"command_execution","command":"/bin/bash -lc \"rtk git status\"","aggregated_output":"ok","exit_code":0}),
            json!({"type":"command_execution","command":"/bin/bash -lc \"pytest -q\"","aggregated_output":"1 passed","exit_code":0}),
            json!({"type":"command_execution","command":"/bin/bash -lc '/usr/local/bin/rtk proxy make'","aggregated_output":"","exit_code":0}),
            json!({"type":"file_change","changes":[{"path":"a.py","kind":"update"}]}),
        ],
    );
    assert_eq!(
        codex_trace::rtk_compliance(&json!(path), root.path()).unwrap(),
        json!({"rtk_shell_commands":3,"rtk_prefixed_commands":2,"rtk_compliance":0.667})
    );
    assert_eq!(
        codex_trace::rtk_compliance(&json!(root.path().join("missing.jsonl")), root.path())
            .unwrap(),
        json!({"rtk_shell_commands":null,"rtk_prefixed_commands":null,"rtk_compliance":null})
    );
    for cmd in ["/bin/bash -lc \"rtk ls\"", "sh -c 'rtk ls'", "rtk ls"] {
        assert_eq!(codex_trace::unwrap_shell(cmd), "rtk ls");
    }
}

#[test]
fn config_toml_writes_an_mcp_server_table_the_cli_validates() {
    let text = config(json!({"capabilities":["skills"],"mcp_servers":spec()}));
    let index = text.find("[mcp_servers.serena]").unwrap();
    assert!(text.find("[[skills.config]]").unwrap() > index);
    for expected in [
        "command = \"/tmp/agent-home/.local/bin/serena\"",
        "args = [\"start-mcp-server\", \"--project\", \"/testbed\", \"--context\", \"codex\"]",
        "startup_timeout_sec = 90",
        "[mcp_servers.serena.env]",
        "PATH = \"/tmp/agent-home/.local/bin:/usr/bin\"",
    ] {
        assert!(text[index..].contains(expected), "{expected}");
    }
    assert!(!config(json!({})).contains("mcp_servers"));
    #[cfg(unix)]
    if let Some(binary) = Install::installed() {
        let root = TempDir::new("codex-mcp-validator");
        let error = Install::new(root.path())
            .run(
                "validate_config",
                json!({"text":text,"binary":binary,"home":root.path(),"timeout":20}),
            )
            .unwrap();
        assert!(error.as_str().unwrap_or("").trim().is_empty(), "{error}");
    }
}

#[test]
fn mcp_tool_calls_counts_completed_items_and_keeps_a_sample_of_what_it_cannot_read() {
    let root = TempDir::new("codex-mcp-counts");
    let path = root.path().join("t.jsonl");
    let live = json!({"id":"item_2","type":"mcp_tool_call","server":"codex","tool":"list_mcp_resources","arguments":{},"result":{"content":[{"type":"text","text":"{\"resources\":[]}"}],"structured_content":null},"error":null,"status":"completed"});
    let rows = [
        json!({"type":"item.started","item":{"type":"mcp_tool_call","server":"serena","tool":"find_symbol"}}),
        json!({"type":"item.completed","item":{"type":"mcp_tool_call","server":"serena","tool":"find_symbol","status":"completed"}}),
        json!({"type":"item.completed","item":live}),
        json!({"type":"item.completed","item":{"type":"mcp_tool_call","invocation":{"server":"serena","tool":"get_symbols_overview"}}}),
        json!({"type":"item.completed","item":{"type":"mcp_tool_call","weird":1}}),
        json!({"type":"item.completed","item":{"type":"command_execution","command":"ls"}}),
    ];
    fs::write(
        &path,
        rows.iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n"),
    )
    .unwrap();
    let got = codex_trace::mcp_tool_calls(&json!(path), &json!("serena"), root.path()).unwrap();
    assert_eq!(got["mcp_tool_calls"], 3);
    assert_eq!(got["serena_tool_calls"], 2);
    assert_eq!(
        got["mcp_tools_used"],
        json!({"serena/find_symbol":1,"serena/get_symbols_overview":1,"codex/list_mcp_resources":1})
    );
    assert_eq!(got["mcp_unrecognised_items"], 1);
    assert!(
        got["mcp_item_sample"]
            .as_str()
            .unwrap()
            .contains("\"weird\"")
    );
    let none = codex_trace::mcp_tool_calls(
        &json!(root.path().join("missing.jsonl")),
        &json!("serena"),
        root.path(),
    )
    .unwrap();
    assert!(none["mcp_tool_calls"].is_null() && none["serena_tool_calls"].is_null());
}

#[test]
fn the_codex_lane_hosts_every_scaffold_and_wires_an_mcp_one_through_the_config() {
    struct MissingKey<'a>(&'a str);
    impl swebench::Runtime for MissingKey<'_> {
        fn call(&mut self, method: &str, p: Value) -> swebench::Result<Value> {
            match method {
                "setup_assign" => {
                    assert_eq!(p, json!(["agent", ["arg", "agent"]]));
                    Ok(Value::Null)
                }
                "setup_bool" => {
                    assert_eq!(p, json!(["arg", "scaffold"]));
                    Ok(json!(!self.0.is_empty()))
                }
                "setup_call" => {
                    assert_eq!(p, json!(["codex.resolve_codex_routing", [], {}]));
                    Provider::new("codex", json!({})).run("resolve_codex_routing")
                }
                _ => panic!("unexpected constructor callback {method}: {p}"),
            }
        }
    }
    for scaffold in ["rtk", "repomap", "serena"] {
        let error = setup::construct("codex", &mut MissingKey(scaffold)).unwrap_err();
        assert_eq!(error.kind, "RuntimeError");
        assert!(error.message.contains("nothing to bill"));
    }
    struct Wiring {
        notes: Value,
        config: Value,
        popped: Value,
        servers: Value,
        writes: Vec<Value>,
    }
    impl swebench::Runtime for Wiring {
        fn call(&mut self, method: &str, p: Value) -> swebench::Result<Value> {
            match method {
                "config_set" => {
                    assert_eq!(p[0], "_mcp_servers");
                    self.servers = p[1].clone();
                }
                "base_scaffold" => {
                    assert_eq!(p, json!(["c1",{"instance_id":"x"},"run"]));
                    return Ok(json!({"is_dict":true}));
                }
                "scaffold_notes_pop" => {
                    assert_eq!(p, json!(["mcp_servers", null]));
                    self.popped = self
                        .notes
                        .as_object_mut()
                        .unwrap()
                        .remove("mcp_servers")
                        .unwrap_or(Value::Null);
                    return Ok(self.popped.clone());
                }
                "scaffold_use_popped" => {
                    assert_eq!(p, json!(["_mcp_servers"]));
                    self.servers = self.popped.clone();
                }
                "_write_config" => self.writes.push(json!([p[0], self.servers])),
                "scaffold_notes_update" => self
                    .notes
                    .as_object_mut()
                    .unwrap()
                    .extend(p[0].as_object().unwrap().clone()),
                _ => panic!("unexpected scaffold callback {method}: {p}"),
            }
            Ok(Value::Null)
        }
    }
    let mut r = Wiring {
        notes: json!({"arm_setup":"serena","mcp_servers":spec(),"mcp_config":"/tmp/agent-home/scaffold.mcp.json"}),
        config: json!("/tmp/agent-home/scaffold.mcp.json"),
        popped: Value::Null,
        servers: Value::Null,
        writes: Vec::new(),
    };
    let output = cc::apply_codex_scaffold(
        &json!("c1"),
        &json!({"instance_id":"x"}),
        &json!("run"),
        &mut r,
    )
    .unwrap();
    assert_eq!(output, json!({"_scaffold_result":true}));
    assert_eq!(r.writes, vec![json!(["c1", spec()])]);
    assert!(r.notes.get("mcp_servers").is_none());
    assert_eq!(r.notes["mcp_wiring"], "config.toml");
    assert_eq!(r.notes["mcp_servers_configured"], json!(["serena"]));
    assert_eq!(r.config, "/tmp/agent-home/scaffold.mcp.json");
    r.notes = json!({"arm_setup":"rtk"});
    r.config = Value::Null;
    r.writes.clear();
    let output = cc::apply_codex_scaffold(
        &json!("c1"),
        &json!({"instance_id":"x"}),
        &json!("run"),
        &mut r,
    )
    .unwrap();
    assert_eq!(output, json!({"_scaffold_result":true}));
    assert!(r.writes.is_empty());
    assert!(r.servers.is_null());
}
