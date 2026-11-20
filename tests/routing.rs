// Converted from reference/upstream/experiments/tests/test_routing.py.
use serde_json::{Map, Value, json};
use tokeneconomizer::experiments::swebench::{Result, Runtime, providers, setup};

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::String(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
        _ => panic!("unexpected boolean input: {value}"),
    }
}

struct Provider {
    agent: &'static str,
    env: Map<String, Value>,
    slots: Value,
    logs: Vec<String>,
}
impl Provider {
    fn new(agent: &'static str, env: Value) -> Self {
        Self {
            agent,
            env: env.as_object().unwrap().clone(),
            slots: Value::Null,
            logs: Vec::new(),
        }
    }
    fn constant(&self, name: &str) -> Value {
        for constants in [
            providers::constants(),
            providers::optimizer_constants(),
            setup::constants(),
        ] {
            if let Some(value) = constants[self.agent].get(name) {
                return value.clone();
            }
        }
        panic!("unknown constant {name}")
    }
    fn run(&mut self, name: &str) -> Result<Value> {
        self.slots = json!({"output":{}, "extra":null});
        let result = providers::dispatch(&format!("{}.{name}", self.agent), self)?;
        Ok(match result.get("provider_ref").and_then(Value::as_str) {
            Some(slot) => self.slots[slot].clone(),
            None => result,
        })
    }
    fn nested(&self, name: &str) -> Result<Value> {
        Provider::new(self.agent, json!(self.env)).run(name)
    }
}
impl Runtime for Provider {
    fn call(&mut self, method: &str, p: Value) -> Result<Value> {
        let a = |i: usize| p[i].as_str().unwrap();
        match method {
            "provider_env_get" => {
                self.slots[a(0)] = self
                    .env
                    .get(a(1))
                    .cloned()
                    .unwrap_or_else(|| p.get(2).cloned().unwrap_or(Value::Null))
            }
            "provider_env_item" => self.slots[a(0)] = self.env[a(1)].clone(),
            "provider_bool" => return Ok(json!(truthy(&self.slots[a(0)]))),
            "provider_literal" => self.slots[a(0)] = p[1].clone(),
            "provider_copy" => self.slots[a(0)] = self.slots[a(1)].clone(),
            "provider_names" => return Ok(self.constant(a(0))),
            "provider_collect" => {
                let value = self.slots[a(1)].clone();
                self.slots["output"][a(0)] = value;
            }
            "provider_log" => self.logs.push(a(0).into()),
            "provider_function" => {
                assert_eq!(p[1], json!([]));
                self.slots["called"] = self.nested(a(0))?;
            }
            "host_function" => self.slots[a(1)] = self.nested(a(0))?,
            "host_update" => {
                let additions = self.slots[a(1)].as_object().unwrap().clone();
                self.slots[a(0)].as_object_mut().unwrap().extend(additions);
            }
            _ => panic!("unexpected provider callback {method}: {p}"),
        }
        Ok(Value::Null)
    }
}

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

struct Constructor {
    provider: Provider,
    fields: Value,
    locals: Value,
    result: Value,
    resolutions: usize,
}
impl Constructor {
    fn read(&self, reference: &Value) -> Value {
        let name = reference[1].as_str().unwrap_or("");
        match reference[0].as_str().unwrap() {
            "arg" => {
                if name == "agent" {
                    json!("claude")
                } else {
                    Value::Null
                }
            }
            "field" => self.fields[name].clone(),
            "local" => self.locals[name].clone(),
            "value" => reference[1].clone(),
            "integer" => json!(name.parse::<i64>().unwrap()),
            "module" => self.provider.constant(name),
            "result" => self.result.clone(),
            _ => panic!("unexpected setup reference {reference}"),
        }
    }
    fn env(&self, key: &str, default: Option<&Value>) -> Value {
        self.provider
            .env
            .get(key)
            .or(default)
            .cloned()
            .unwrap_or(Value::Null)
    }
}
impl Runtime for Constructor {
    fn call(&mut self, method: &str, p: Value) -> Result<Value> {
        let a = |i: usize| p[i].as_str().unwrap();
        match method {
            "setup_env" => return Ok(self.env(a(0), p.get(1))),
            "setup_env_capture" => self.locals[a(0)] = self.env(a(1), p.get(2)),
            "setup_bool" => return Ok(json!(truthy(&self.read(&p)))),
            "setup_present" => return Ok(json!(!self.read(&p).is_null())),
            "setup_read" => return Ok(self.read(&p)),
            "setup_assign" => self.fields[a(0)] = self.read(&p[1]),
            "setup_capture" => {
                for pair in p.as_array().unwrap() {
                    self.locals[pair[0].as_str().unwrap()] = self.read(&pair[1]);
                }
            }
            "setup_contains" => {
                return Ok(json!(
                    self.read(&p[1])
                        .as_array()
                        .unwrap()
                        .contains(&self.read(&p[0]))
                ));
            }
            "setup_compare" => {
                self.locals[a(3)] = json!(match a(0) {
                    "eq" => self.read(&p[1]) == self.read(&p[2]),
                    "le" =>
                        self.read(&p[1]).as_i64().unwrap() <= self.read(&p[2]).as_i64().unwrap(),
                    _ => panic!("unexpected setup comparison {p}"),
                })
            }
            "setup_integer" => {
                self.locals[a(1)] =
                    json!(self.read(&p[0]).as_str().unwrap().parse::<i64>().unwrap())
            }
            "setup_integer_limit" => return Ok(json!(4300)),
            "setup_call" => {
                self.result = match a(0) {
                    "resolve_anthropic_key" => {
                        self.resolutions += 1;
                        self.provider.run("resolve_anthropic_key")?
                    }
                    "claude_bin" => json!("/fixture/claude"),
                    "cli_version" => json!("fixture-cli"),
                    "_Grader" => json!("fixture-grader"),
                    "webbench_repo" => json!("/fixture/WebBench"),
                    "node_bin" => json!("/fixture/node"),
                    "webbench_task_timeout" => json!(240),
                    "scaffolds_pkg.get"
                    | "pricing.weights_for"
                    | "self._init_stream_state"
                    | "self._preflight_optimizer_endpoint" => Value::Null,
                    _ => panic!("unexpected setup dependency {p}"),
                }
            }
            _ => panic!("unexpected setup callback {method}: {p}"),
        }
        Ok(Value::Null)
    }
}

#[test]
fn the_claude_runners_record_the_anthropic_api_as_their_endpoint() {
    assert_eq!(
        providers::optimizer_constants()["claude"]["_ANTHROPIC_DIRECT"],
        "https://api.anthropic.com"
    );
    for domain in ["claude", "web.claude"] {
        let mut r = Constructor {
            provider: Provider::new("claude", json!({"ANTHROPIC_API_KEY":"sk-ant-x"})),
            fields: json!({}),
            locals: json!({}),
            result: Value::Null,
            resolutions: 0,
        };
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
