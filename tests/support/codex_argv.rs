use serde_json::{Map, Value, json};
use tokeneconomizer::experiments::swebench::{self, invocation, providers};

pub fn codex_argv(operation: &str, arguments: Value) -> Value {
    struct Slots(Map<String, Value>);
    impl swebench::Runtime for Slots {
        fn call(&mut self, method: &str, args: Value) -> swebench::Result<Value> {
            let slot = |index: usize| args[index].as_str().unwrap();
            match method {
                "runner_function" => {
                    let value = if slot(0) == "json.dumps" {
                        json!("json.dumps")
                    } else {
                        providers::optimizer_constants()["codex"][slot(0)].clone()
                    };
                    self.0.insert("function".into(), value);
                }
                "provider_copy" => {
                    self.0.insert(slot(0).into(), self.0[slot(1)].clone());
                }
                "provider_literal" => {
                    self.0.insert(slot(0).into(), args[1].clone());
                }
                "provider_bool" => {
                    let value = &self.0[slot(0)];
                    return Ok(json!(match value {
                        Value::Null => false,
                        Value::Bool(value) => *value,
                        Value::String(value) => !value.is_empty(),
                        _ => panic!("unexpected boolean input: {value}"),
                    }));
                }
                "eval_list" => {
                    let values: Vec<_> = args[1]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|name| self.0[name.as_str().unwrap()].clone())
                        .collect();
                    self.0.insert(slot(0).into(), json!(values));
                }
                "eval_append" => {
                    let value = self.0[slot(1)].clone();
                    self.0[slot(0)].as_array_mut().unwrap().push(value);
                }
                "web_binary" => {
                    assert!(matches!(slot(0), "add" | "iadd"));
                    let mut values = self.0[slot(1)].as_array().unwrap().clone();
                    values.extend(self.0[slot(2)].as_array().unwrap().iter().cloned());
                    self.0.insert(slot(3).into(), json!(values));
                }
                "eval_builtin" => {
                    assert_eq!(args, json!(["str"]));
                    self.0.insert("function".into(), json!("str"));
                }
                "runner_invoke" => {
                    let argument = &self.0[args[0][0].as_str().unwrap()];
                    let text = match self.0["function"].as_str().unwrap() {
                        "str" => argument.as_str().unwrap().to_owned(),
                        "json.dumps" => serde_json::to_string(argument).unwrap(),
                        name => panic!("unexpected function: {name}"),
                    };
                    self.0.insert("called".into(), json!(text));
                }
                "task_format" => {
                    assert_eq!(args[1], "");
                    return Ok(json!(
                        self.0[slot(0)]
                            .as_str()
                            .unwrap()
                            .chars()
                            .map(u32::from)
                            .collect::<Vec<_>>()
                    ));
                }
                "logger_literal" => {
                    let text: String = args[1]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|c| char::from_u32(c.as_u64().unwrap() as u32).unwrap())
                        .collect();
                    self.0.insert(slot(0).into(), json!(text));
                }
                _ => panic!("unexpected callback: {method} {args}"),
            }
            Ok(Value::Null)
        }
    }
    let mut slots = json!({"model":null,"effort":null})
        .as_object()
        .unwrap()
        .clone();
    slots.extend(arguments.as_object().unwrap().clone());
    let mut runtime = Slots(slots);
    let result = invocation::dispatch(operation, "codex", &mut runtime).unwrap();
    runtime
        .0
        .remove(result["provider_ref"].as_str().unwrap())
        .unwrap()
}
