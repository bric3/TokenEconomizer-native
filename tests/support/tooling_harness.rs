use serde_json::{Map, Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    fs,
    path::{Path, PathBuf},
};
use tokeneconomizer::experiments::swebench::{
    self, Failure, Result, Runtime, Task, guard_install,
    settings::{self, RunConfig},
    web_contract,
};

pub fn task(root: &Path) -> (Value, Vec<String>) {
    struct Host(Vec<String>);
    impl Runtime for Host {
        fn call(&mut self, method: &str, args: Value) -> Result<Value> {
            self.0.push(method.into());
            Ok(match method {
                "clock" => json!(100),
                "_container_name" => json!("fixture-container"),
                "_build_guard" => json!(false),
                "_record_env_activation" => args[2].clone(),
                "_apply_scaffold" => json!([{}, null]),
                "_install_net_guard" | "_install_answer_key_hosts" => json!(true),
                "transcript_open" => {
                    fs::File::create(args[0].as_str().unwrap())?;
                    Value::Null
                }
                "_segment" => {
                    json!({"usage":{"input_tokens":100,"output_tokens":50},"num_turns":2})
                }
                "_graded_diff" => json!("diff --git a/a b/a\n"),
                "_grade" => json!([true,{"resolved":true}]),
                "_net_guard_blocks" => json!(3),
                "_row_from_segment" => swebench::row_from_segment("claude", &args[0])?,
                "_answer_key_attempts"
                | "scaffold_info"
                | "log"
                | "optimizer_context"
                | "_write_run_config"
                | "_start"
                | "_stop"
                | "transcript_close" => Value::Null,
                _ => panic!("unexpected task callback {method}: {args}"),
            })
        }
    }
    let mut host = Host(Vec::new());
    let row=swebench::run_task(&Task {task:json!({"instance_id":"django__django-10097","repo":"django/django","problem_statement":"fix"}),arm:json!("baseline"),injected:Value::Null,exclude_tools:json!([]),run_dir:root.join("run"),kwargs:json!({}),metadata:json!({"agent":"claude","model":"claude-opus-4-8","effort":"high","cli_version":"fixture"}),environment:json!({}),process_cwd:root.into()},&mut host).unwrap();
    (row, host.0)
}

pub fn disabled_guard() -> Value {
    struct Allow;
    impl Runtime for Allow {
        fn call(&mut self, method: &str, p: Value) -> Result<Value> {
            assert_eq!(method, "config_function");
            assert_eq!(p, json!(["network_tool_block", []]));
            Ok(json!(settings::network_tool_block(
                json!({"TE_ALLOW_NETWORK_TOOLS":"1"}).as_object().unwrap()
            )))
        }
    }
    guard_install::dispatch("install", &json!({"name":"no-such-container"}), &mut Allow).unwrap()
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::String(value) => !value.is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
        Value::Number(value) => value.as_i64().unwrap() != 0,
    }
}

struct ConfigHost {
    values: Value,
    iterators: BTreeMap<String, VecDeque<Value>>,
    file: Option<fs::File>,
}
impl ConfigHost {
    fn function(&mut self, function: Value, args: Vec<Value>) -> Result<Value> {
        if let Some(method) = function.as_array() {
            assert_eq!(method[0], "method");
            assert_eq!(method[2], "get");
            return Ok(self.values[method[1].as_str().unwrap()]
                .get(args[0].as_str().unwrap())
                .cloned()
                .unwrap_or_else(|| args.get(1).cloned().unwrap_or(Value::Null)));
        }
        Ok(match function.as_str().unwrap() {
            "network_tool_block" => json!(settings::network_tool_block(&Map::new())),
            "list" => {
                assert!(args[0].is_array());
                args[0].clone()
            }
            "bool" => json!(truthy(&args[0])),
            "len" => json!(
                args[0]
                    .as_str()
                    .map(|s| s.chars().count())
                    .unwrap_or_else(|| args[0].as_array().unwrap().len())
            ),
            "sorted" => {
                let mut list = args[0].as_array().unwrap().clone();
                list.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
                json!(list)
            }
            "getattr" => args[0]
                .get(args[1].as_str().unwrap())
                .cloned()
                .unwrap_or_else(|| args.get(2).cloned().unwrap_or(Value::Null)),
            "guard_accepts" => json!(false),
            "os.path.join" => json!(args.iter().fold(PathBuf::new(), |path, part| {
                path.join(part.as_str().unwrap())
            })),
            "open" => {
                assert_eq!(args[1], "w");
                self.file = Some(fs::File::create(args[0].as_str().unwrap())?);
                json!("file")
            }
            "json.dump" => {
                serde_json::to_writer_pretty(self.file.as_mut().unwrap(), &args[0])
                    .map_err(|error| Failure::new("OSError", error.to_string()))?;
                Value::Null
            }
            _ => panic!("unexpected config function {function}: {args:?}"),
        })
    }
}
impl Runtime for ConfigHost {
    fn call(&mut self, method: &str, p: Value) -> Result<Value> {
        let a = |i: usize| p[i].as_str().unwrap();
        match method {
            "config_function" => {
                return Ok(match a(0) {
                    "network_tool_block" => json!(settings::network_tool_block(&Map::new())),
                    "image_for" => json!(settings::image_for(&p[1][0])?),
                    "_cpu_limit_args" => json!([]),
                    "env_activation_enabled" => {
                        json!(settings::env_activation_enabled(&Map::new()))
                    }
                    _ => panic!("unexpected configuration dependency {p}"),
                });
            }
            "config_attr" => {
                let object = if a(0) == "runner" { "self" } else { a(0) };
                let value = self.values[object]
                    .get(a(1))
                    .cloned()
                    .or_else(|| p.get(3).cloned())
                    .unwrap_or(Value::Null);
                return Ok(match a(2) {
                    "bool" => json!(truthy(&value)),
                    "present" => json!(!value.is_null()),
                    _ => value,
                });
            }
            "config_kwarg" => {
                let value = &self.values["run_kwargs"][a(0)];
                return Ok(match a(1) {
                    "bool" => json!(truthy(value)),
                    "present" => json!(!value.is_null()),
                    _ => value.clone(),
                });
            }
            "config_patch_excludes" => return Ok(json!([])),
            "provider_copy" => self.values[a(0)] = self.values[a(1)].clone(),
            "provider_literal" => self.values[a(0)] = p[1].clone(),
            "provider_bool" => return Ok(json!(truthy(&self.values[a(0)]))),
            "provider_is_none" => return Ok(json!(self.values[a(0)].is_null())),
            "provider_contains" => {
                return Ok(json!(
                    self.values[a(1)]
                        .as_array()
                        .unwrap()
                        .contains(&self.values[a(0)])
                ));
            }
            "provider_method" => {
                assert_eq!(a(1), "get");
                self.values[a(3)] = self.values[a(0)][p[2][0].as_str().unwrap()].clone();
            }
            "workspace_iter" => {
                self.iterators.insert(
                    a(1).into(),
                    self.values[a(0)]
                        .as_array()
                        .unwrap()
                        .iter()
                        .cloned()
                        .collect(),
                );
            }
            "workspace_next" => {
                if let Some(value) = self.iterators.get_mut(a(0)).unwrap().pop_front() {
                    self.values[a(1)] = value;
                    return Ok(json!(true));
                }
                return Ok(json!(false));
            }
            "eval_append" => {
                let value = self.values[a(1)].clone();
                self.values[a(0)].as_array_mut().unwrap().push(value);
            }
            "eval_field" => {
                let value = self.values[a(2)].clone();
                self.values[a(0)][a(1)] = value;
            }
            "web_binary" => {
                assert_eq!(a(0), "add");
                let mut list = self.values[a(1)].as_array().unwrap().clone();
                list.extend(self.values[a(2)].as_array().unwrap().iter().cloned());
                self.values[a(3)] = json!(list);
            }
            "runner_member" => self.values[a(1)] = self.values["self"][a(0)].clone(),
            "runner_function" | "eval_builtin" => self.values["function"] = p[0].clone(),
            "runner_invoke" => {
                let args = p[0]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|name| self.values[name.as_str().unwrap()].clone())
                    .collect();
                self.values["called"] = self.function(self.values["function"].clone(), args)?;
            }
            "stdout_member" => {
                self.values[a(2)] = if a(1) == "get" {
                    json!(["method", a(0), "get"])
                } else {
                    self.values[a(0)][a(1)].clone()
                }
            }
            "task_format" => {
                assert_eq!(a(1), "");
                return Ok(json!(
                    self.values[a(0)]
                        .as_str()
                        .unwrap()
                        .chars()
                        .map(u32::from)
                        .collect::<Vec<_>>()
                ));
            }
            "logger_literal" => {
                self.values[a(0)] = json!(
                    p[1].as_array()
                        .unwrap()
                        .iter()
                        .map(|c| char::from_u32(c.as_u64().unwrap() as u32).unwrap())
                        .collect::<String>()
                )
            }
            "task_enter" => {
                assert_eq!(p, json!(["config"]));
                assert!(self.file.is_some());
                self.values["config_handle"] = json!("file");
            }
            "task_exit" => {
                assert_eq!(p, json!(["config", null]));
                self.file.take();
                self.values["exit_result"] = json!(false);
            }
            _ => panic!("unexpected configuration callback {method}: {p}"),
        }
        Ok(Value::Null)
    }
}

pub fn config(root: &Path, web: bool) -> Value {
    let directory = root.join(if web { "web" } else { "swe" });
    fs::create_dir(&directory).unwrap();
    let task = if web {
        json!({"instance_id":"task-1","repo":"zustand"})
    } else {
        json!({"instance_id":"django__django-10097","repo":"django/django"})
    };
    let mut host = ConfigHost {
        values: json!({
            "self":{"agent":"claude","model":"claude-opus-4-8","effort":"high","cli_version":"fixture","agent_bin":"fixture","routing_base":"fixture","optimizer_model":"claude-opus-4-8","optimizer_transport":"api","protected_tools":["Bash","Edit","Read","Write"],"scaffold":null,"repo":"fixture/WebBench","retry":2,"max_workers":4,"node":"node","task_timeout":240,"verdict_timeout":1800,"start_timeout":1800},
            "guard":{"nudge_budget":0,"never_abort":false},"grader":{"path":"grader-fixture"},
            "task":task,"arm":"baseline","max_turns":80,"exclude_tools":["Bash","Workflow"],"run_kwargs":{},"injected":null,"project":"zustand","sess":{"config_path":"eval.toml","log_path":"eval.log","hash":"fixture"},"run_dir_path":directory,
        }),
        iterators: BTreeMap::new(),
        file: None,
    };
    if web {
        web_contract::config(&mut host).unwrap();
    } else {
        settings::write_run_config(
            &RunConfig {
                task,
                arm: json!("baseline"),
                injected: Value::Null,
                exclude_tools: json!(["Bash", "Workflow"]),
                max_turns: json!(80),
                run_dir: directory.clone(),
                process_cwd: root.into(),
            },
            &mut host,
        )
        .unwrap();
    }
    serde_json::from_slice(&fs::read(directory.join("run_config.json")).unwrap()).unwrap()
}
