use serde_json::{Map, Value, json};
use std::path::Path;
use tokeneconomizer::experiments::swebench::{Failure, Result, Runtime, providers, setup};

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::String(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
        Value::Number(value) => value.as_i64().unwrap() != 0,
        _ => panic!("unexpected boolean input: {value}"),
    }
}

pub struct Provider {
    agent: &'static str,
    pub env: Map<String, Value>,
    slots: Value,
    pub logs: Vec<String>,
}
impl Provider {
    pub fn new(agent: &'static str, env: Value) -> Self {
        Self {
            agent,
            env: env.as_object().unwrap().clone(),
            slots: Value::Null,
            logs: Vec::new(),
        }
    }
    pub fn constant(&self, name: &str) -> Value {
        let agent = if self.agent == "webbench" {
            "web"
        } else {
            self.agent
        };
        for constants in [
            providers::constants(),
            providers::optimizer_constants(),
            setup::constants(),
        ] {
            if let Some(value) = constants[agent].get(name) {
                return value.clone();
            }
        }
        panic!("unknown constant {name}")
    }
    pub fn run(&mut self, name: &str) -> Result<Value> {
        self.run_with(name, json!({}))
    }
    pub fn run_with(&mut self, name: &str, arguments: Value) -> Result<Value> {
        self.slots = json!({"output":{}, "extra":null});
        self.slots
            .as_object_mut()
            .unwrap()
            .extend(arguments.as_object().unwrap().clone());
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
            "provider_constant" => self.slots[a(0)] = self.constant(a(1)),
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
            "web_compare" => {
                let left = self.slots[a(1)].as_i64().unwrap();
                let right = self.slots[a(2)].as_i64().unwrap();
                self.slots["comparison"] = json!(match a(0) {
                    "gt" => left > right,
                    "le" => left <= right,
                    _ => panic!("unexpected comparison {p}"),
                });
            }
            "web_integer" => {
                let value = &self.slots[a(0)];
                self.slots[a(1)] = json!(match value {
                    Value::String(text) => text
                        .parse::<i64>()
                        .map_err(|error| Failure::new("ValueError", error.to_string()))?,
                    Value::Number(number) => number.as_i64().unwrap(),
                    _ => panic!("unexpected integer input {value}"),
                });
            }
            "web_decimal" => return Ok(json!(self.slots[a(0)].as_i64().unwrap().to_string())),
            "web_integer_literal" => self.slots[a(0)] = json!(a(1).parse::<i64>().unwrap()),
            "web_abort" => self.slots["abort"] = self.nested("webbench_undici_abort")?,
            "optimizer_capture_log" => {}
            "logger_format" => {
                return Ok(json!(
                    self.slots[a(0)]
                        .to_string()
                        .chars()
                        .map(u32::from)
                        .collect::<Vec<_>>()
                ));
            }
            "logger_report" => self.logs.push(
                p[0].as_array()
                    .unwrap()
                    .iter()
                    .map(|c| char::from_u32(c.as_u64().unwrap() as u32).unwrap())
                    .collect(),
            ),
            "web_uri" => {
                assert!(
                    Path::new(a(0)).is_file(),
                    "missing native preload: {}",
                    a(0)
                );
                // shortcut: URI encoding uses a fixed value; exercise the SDK binding for encoded-path tests.
                self.slots["uri"] = json!("file:///fixture/webbench_undici_preload.mjs");
            }
            "web_stringify" => {
                self.slots[a(1)] = json!(match &self.slots[a(0)] {
                    Value::String(value) => value.clone(),
                    Value::Number(value) => value.to_string(),
                    value => panic!("unexpected string input {value}"),
                })
            }
            "web_binary" => {
                assert_eq!(a(0), "add");
                self.slots[a(3)] = json!(
                    self.slots[a(1)].as_str().unwrap().to_owned()
                        + self.slots[a(2)].as_str().unwrap()
                );
            }
            "provider_method" => {
                assert_eq!(a(1), "strip");
                assert_eq!(p[2], json!([]));
                self.slots[a(3)] = json!(self.slots[a(0)].as_str().unwrap().trim());
            }
            _ => panic!("unexpected provider callback {method}: {p}"),
        }
        Ok(Value::Null)
    }
}

pub struct Constructor {
    provider: Provider,
    pub fields: Value,
    locals: Value,
    result: Value,
    pub resolutions: usize,
}
impl Constructor {
    pub fn new(env: Value) -> Self {
        Self {
            provider: Provider::new("claude", env),
            fields: json!({}),
            locals: json!({}),
            result: Value::Null,
            resolutions: 0,
        }
    }
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
