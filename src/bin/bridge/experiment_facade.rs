use super::{callback::CallbackTape, spiral_dispatch::failure, tooling_dispatch::AdapterTransport};
use serde_json::{Value, json};
use std::{cell::RefCell, path::Path, rc::Rc};
use tokeneconomizer::{
    experiments::{
        adapters::Adapter,
        facade::{DomainAdapter, Economizer, StreamOptions},
    },
    microskills::{distiller::ProposeContext, observe::Vocabulary},
    runner::{Runner, optimizer_method},
};

struct FacadeRunner {
    tape: Rc<RefCell<CallbackTape>>,
    slot: String,
}
impl Runner for FacadeRunner {
    fn run(&mut self, prompt: &str, model: &str) -> Result<(String, Option<f64>), String> {
        let reply = self
            .tape
            .borrow_mut()
            .call(json!({"callback":"runner","slot":self.slot,"prompt":prompt,"model":model}))?;
        Ok((
            reply["value"][0].as_str().unwrap_or("").into(),
            reply["value"][1].as_f64(),
        ))
    }
    fn estimate_effective(&mut self, prompt: &str, raw: &str) -> Result<f64, String> {
        let reply = self
            .tape
            .borrow_mut()
            .call(json!({"callback":"estimate","slot":self.slot,"prompt":prompt,"raw":raw}))?;
        reply["value"]
            .as_f64()
            .ok_or_else(|| "expected token estimate".into())
    }
}
impl DomainAdapter for AdapterTransport<Rc<RefCell<CallbackTape>>> {
    fn env_vocabulary(&mut self) -> Result<Vocabulary, String> {
        let value = self.call("env_vocabulary", json!([]))?;
        if value.is_null() {
            Ok(Vocabulary::default())
        } else {
            serde_json::from_value(value).map_err(|e| e.to_string())
        }
    }
    fn spiral_snapshot(&mut self, path: &Path) -> Result<Value, String> {
        self.call("spiral_snapshot", json!([path]))
    }
}
fn decode<T: serde::de::DeserializeOwned>(value: &Value) -> Result<T, String> {
    serde_json::from_value(value.clone()).map_err(|e| e.to_string())
}

pub fn dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
    if op == "experiment.runner.optimizer_method" {
        let result = optimizer_method(
            args["present"] == true,
            args["has_optimizer"] == true,
            args["callable"] == true,
            args["representation"].as_str().unwrap_or(""),
        );
        return Ok(Some(
            if result.is_err() && args.get("representation").is_none() {
                json!({"callback":"representation"})
            } else {
                result.map(|v| json!(v)).unwrap_or_else(failure)
            },
        ));
    }
    let Some(method) = op.strip_prefix("experiment.facade.") else {
        return Ok(None);
    };
    let tape = Rc::new(RefCell::new(CallbackTape::new(args)));
    let mut propose = ProposeContext {
        propose_every: args["propose_every"].as_i64().unwrap_or(8),
        home: args["home"].as_str().unwrap_or(""),
        ..Default::default()
    };
    let mut messages = Vec::new();
    let result = (|| {
        let adapter: Box<dyn DomainAdapter> = if let Some(value) = args.get("native_adapter") {
            Box::new(decode::<Adapter>(value)?)
        } else {
            Box::new(AdapterTransport(tape.clone()))
        };
        let mut econ = if matches!(method, "new" | "from_args") {
            if args["adapter"] != true && args.get("native_adapter").is_none() {
                tape.borrow_mut()
                    .call(json!({"callback":"default_adapter"}))?;
            }
            let mut resolve = |slot: &str| -> Result<Option<Box<dyn Runner>>, String> {
                let source = if args["runner"] == true {
                    "runner"
                } else {
                    let reply = tape
                        .borrow_mut()
                        .call(json!({"callback":"factory_available"}))?;
                    if reply["value"] != true {
                        return Ok(None);
                    }
                    "factory"
                };
                let reply = tape
                    .borrow_mut()
                    .call(json!({"callback":"resolve","source":source,"slot":slot}))?;
                Ok((reply["value"] == true).then(|| {
                    Box::new(FacadeRunner {
                        tape: tape.clone(),
                        slot: slot.into(),
                    }) as Box<dyn Runner>
                }))
            };
            let adapter = Some(adapter);
            let options: StreamOptions = decode(&args["options"])?;
            if method == "new" {
                Economizer::new(
                    decode(&args["config"])?,
                    &options,
                    adapter,
                    &mut resolve,
                    &mut propose,
                    &mut messages,
                )?
            } else {
                Economizer::from_args(
                    &args["arguments"],
                    options,
                    adapter,
                    &mut resolve,
                    &mut propose,
                    &mut messages,
                )?
            }
        } else {
            let mut econ: Economizer = decode(&args["self"])?;
            econ.adapter = adapter;
            for (slot, runner) in [
                ("injection", &mut econ.injection_runner),
                ("tooling", &mut econ.tooling_runner),
                ("spiral", &mut econ.spiral_runner),
            ] {
                *runner = (args["runners"][slot] == true).then(|| {
                    Box::new(FacadeRunner {
                        tape: tape.clone(),
                        slot: slot.into(),
                    }) as Box<dyn Runner>
                });
            }
            econ
        };
        let mut srow = args["srow"].clone();
        let result = match method {
            "new" | "from_args" => Ok(Value::Null),
            "injection_prefix" => Ok(json!(econ.injection_prefix())),
            "build_tools" => econ
                .build_tools(
                    &args["task"],
                    args["i"].as_i64().unwrap_or(0),
                    &mut messages,
                )
                .map(|v| json!(v)),
            "run_kwargs" => econ.run_kwargs(),
            "after_task" => econ
                .after_task(
                    &args["task"],
                    &args["row"],
                    Path::new(args["run_dir_path"].as_str().unwrap_or("")),
                    &mut srow,
                    &propose,
                    &mut messages,
                )
                .map(|v| json!(v)),
            _ => return Err(format!("unknown facade operation: {method}")),
        };
        let mut data = json!(econ);
        data["injection"] = super::prefix_dispatch::prefix_data(&econ.injection);
        Ok(
            json!({"self":data,"value":result.unwrap_or_else(failure),"srow":srow,"messages":messages}),
        )
    })();
    let result = result.unwrap_or_else(|e| json!({"value":failure(e),"messages":messages}));
    let mut result = Rc::try_unwrap(tape)
        .map_err(|_| "facade retained callback transport")?
        .into_inner()
        .finish(result);
    result["propose_every"] = json!(propose.propose_every);
    Ok(Some(result))
}
