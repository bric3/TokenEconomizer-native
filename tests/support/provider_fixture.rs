use serde_json::{Map, Value, json};
use std::{collections::BTreeMap, path::Path};
use tokeneconomizer::experiments::swebench::{
    Failure, Result, Runtime, optimizer, providers, setup,
};

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
    pub http_response: Value,
    pub http_error: bool,
    pub requests: Vec<Value>,
    cache: BTreeMap<(String, String), Value>,
}
impl Provider {
    pub fn new(agent: &'static str, env: Value) -> Self {
        Self {
            agent,
            env: env.as_object().unwrap().clone(),
            slots: Value::Null,
            logs: Vec::new(),
            http_response: Value::Null,
            http_error: false,
            requests: Vec::new(),
            cache: BTreeMap::new(),
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
        if let Some(encoded) = result.get("provider_encoded").and_then(Value::as_str) {
            return Ok(serde_json::from_str(encoded).unwrap());
        }
        Ok(match result.get("provider_ref").and_then(Value::as_str) {
            Some(slot) => self.slots[slot].clone(),
            None => result,
        })
    }
    fn cache_key(&self) -> (String, String) {
        (
            self.slots["base_url"].as_str().unwrap().into(),
            self.slots["api_key"].as_str().unwrap().into(),
        )
    }
    fn nested(&mut self, name: &str, args: Value) -> Result<Value> {
        let arguments = if name == "fetch_catalog" {
            assert_eq!(args.as_array().unwrap().len(), 2);
            json!({"base_url":self.slots[args[0].as_str().unwrap()],"api_key":self.slots[args[1].as_str().unwrap()],"timeout":10})
        } else {
            assert_eq!(args, json!([]));
            json!({})
        };
        let mut child = Provider::new(self.agent, json!(self.env));
        child.http_response = self.http_response.clone();
        child.http_error = self.http_error;
        child.cache = std::mem::take(&mut self.cache);
        let result = child.run_with(name, arguments);
        self.cache = child.cache;
        self.requests.extend(child.requests);
        self.logs.extend(child.logs);
        result
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
            "provider_data" | "provider_string" => return Ok(self.slots[a(0)].clone()),
            "provider_collect" => {
                let value = self.slots[a(1)].clone();
                self.slots["output"][a(0)] = value;
            }
            "provider_log" => self.logs.push(a(0).into()),
            "provider_pricing" => {
                tokeneconomizer::pricing::weights_for(
                    &json!({"model":self.slots["model"],"agent":self.slots["agent"]}),
                )
                .map_err(|error| Failure::new("ValueError", error.to_string()))?;
            }
            "provider_function" => {
                self.slots["called"] = self.nested(a(0), p[1].clone())?;
            }
            "host_function" => self.slots[a(1)] = self.nested(a(0), json!([]))?,
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
            "web_abort" => self.slots["abort"] = self.nested("webbench_undici_abort", json!([]))?,
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
                let text = self.slots[a(0)].as_str().unwrap();
                self.slots[a(3)] = json!(match a(1) {
                    "strip" => {
                        assert_eq!(p[2], json!([]));
                        text.trim()
                    }
                    "rstrip" => {
                        assert_eq!(p[2], json!(["/"]));
                        text.trim_end_matches('/')
                    }
                    _ => panic!("unexpected method {p}"),
                });
            }
            "provider_routing_item" => self.slots[a(0)] = self.slots["routing"][a(1)].clone(),
            "provider_format" => return Ok(json!(self.slots[a(0)].as_str().unwrap())),
            "provider_repr" => {
                let text = self.slots[a(0)].as_str().unwrap();
                assert!(!text.contains(['\'', '\\']));
                return Ok(json!(format!("'{text}'")));
            }
            "provider_is_none" => return Ok(json!(self.slots[a(0)].is_null())),
            "provider_contains" => {
                return Ok(json!(
                    self.slots[a(1)]
                        .as_array()
                        .unwrap()
                        .contains(&self.slots[a(0)])
                ));
            }
            "provider_items" => return Ok(json!(self.slots[a(0)].to_string())),
            "provider_cache_begin" => return Ok(json!(self.cache.contains_key(&self.cache_key()))),
            "provider_cache_get" => self.slots["catalogue"] = self.cache[&self.cache_key()].clone(),
            "provider_cache_put" => {
                self.cache
                    .insert(self.cache_key(), self.slots["catalogue"].clone());
            }
            "provider_catalogue_value" => {
                self.slots["catalogue"] = if p[0].is_null() {
                    Value::Null
                } else {
                    json!(
                        p[0].as_array()
                            .unwrap()
                            .iter()
                            .map(
                                |encoded| serde_json::from_str::<Value>(encoded.as_str().unwrap())
                                    .unwrap()
                            )
                            .collect::<Vec<_>>()
                    )
                }
            }
            "provider_request" => self.requests.push(json!({"url":p[0],"headers":p[1]})),
            "provider_http_enter" => {
                if self.http_error {
                    return Err(Failure::new("OSError", "connection refused"));
                }
            }
            "provider_http_read" => {
                return Ok(json!({"text":self.http_response.to_string(),"int_max_digits":4300}));
            }
            "provider_http_exit" => return Ok(json!(false)),
            "optimizer_json" => return Ok(json!(self.slots[a(0)].to_string())),
            "optimizer_format" => return Ok(json!(self.slots[a(0)].to_string())),
            "optimizer_capture_request" | "optimizer_record" => {}
            "optimizer_request" => {
                let mut headers = Map::new();
                for header in p[2].as_array().unwrap() {
                    headers.insert(
                        header[0].as_str().unwrap().into(),
                        match header[1].as_str().unwrap() {
                            "value" => header[2].clone(),
                            "ref" => self.slots[header[2].as_str().unwrap()].clone(),
                            "encoded" => serde_json::from_str(header[2].as_str().unwrap()).unwrap(),
                            _ => panic!("unexpected request header {header}"),
                        },
                    );
                }
                self.requests.push(json!({"url":serde_json::from_str::<Value>(a(0)).unwrap(),"body":serde_json::from_str::<Value>(a(1)).unwrap(),"headers":headers}));
            }
            "optimizer_response_text" => {
                self.slots["data"] = serde_json::from_str(a(0)).unwrap();
                let result = optimizer::response_text_call(self)?;
                self.slots["text"] =
                    serde_json::from_str(result["provider_encoded"].as_str().unwrap()).unwrap();
                return Ok(json!({"data":p[0],"text":result["provider_encoded"]}));
            }
            "optimizer_usage_from_response" => {
                self.slots["usage"] = self.slots["data"]["usage"].clone()
            }
            "optimizer_usage_field" => return Ok(json!(self.slots["usage"][a(0)].to_string())),
            "optimizer_pricing" => {
                let mut record = json!({"model":self.slots["mid"]});
                for (key, reference) in p.as_object().unwrap() {
                    record[key] = match reference[0].as_str().unwrap() {
                        "integer" => json!(reference[1].as_str().unwrap().parse::<i64>().unwrap()),
                        "value" => reference[1].clone(),
                        "ref" => self.slots[reference[1].as_str().unwrap()].clone(),
                        _ => panic!("unexpected price input {reference}"),
                    };
                }
                self.slots["effective"] = json!(
                    tokeneconomizer::pricing::effective_tokens(&record, None)
                        .map_err(|error| Failure::new("ValueError", error.to_string()))?
                );
            }
            "optimizer_finish" => {
                self.slots["optimizer_result"] = json!([
                    serde_json::from_str::<Value>(a(0)).unwrap(),
                    self.slots["effective"]
                ])
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
