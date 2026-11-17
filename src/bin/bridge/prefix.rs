use std::{cell::RefCell, path::Path};

use serde_json::{Value, json};
use tokeneconomizer::{
    config::InjectionConfig,
    microskills::{
        distiller::{PROPOSE_EVERY, ProposeContext},
        observe::Vocabulary,
        stage::{self, FactLayers, PrefixStage, SavedFactError},
    },
    runner::Runner,
};

use super::{
    callback::{CallbackTape, RunnerTransport},
    proposer_dispatch,
};

fn text<'a>(args: &'a Value, key: &str) -> &'a str {
    args[key].as_str().unwrap_or("")
}

fn vocabulary(args: &Value) -> Result<Vocabulary, String> {
    if args["vocab"].is_null() {
        Ok(Vocabulary::default())
    } else {
        serde_json::from_value(args["vocab"].clone()).map_err(|e| e.to_string())
    }
}

fn layers_data(layers: &FactLayers) -> Value {
    json!({"env":layers.env,"dist":layers.dist.as_ref().map(proposer_dispatch::data)})
}

pub(super) fn prefix_data(prefix: &PrefixStage) -> Value {
    json!({"results_root":prefix.results_root,"reason":prefix.reason,"distiller":layers_data(&prefix.distiller)})
}

pub fn dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
    if op.starts_with("stage.") {
        let repo = text(args, "repo");
        let root = Path::new(text(args, "root"));
        let value = match op {
            "stage._saved_layers" => json!(
                stage::saved_layers(repo, root, &vocabulary(args)?)?
                    .iter()
                    .map(layers_data)
                    .collect::<Vec<_>>()
            ),
            "stage.forgotten_facts" => {
                json!(stage::forgotten_facts(repo, root, &vocabulary(args)?)?)
            }
            "stage.forget_facts" => {
                let ids: Vec<String> =
                    serde_json::from_value(args["ids"].clone()).map_err(|e| e.to_string())?;
                match stage::forget_facts(
                    repo,
                    root,
                    &ids,
                    &vocabulary(args)?,
                    args["undo"] == true,
                ) {
                    Ok(value) => json!(value),
                    Err(SavedFactError::UnknownIds(ids)) => {
                        json!({"raised":"KeyError","message":ids})
                    }
                    Err(SavedFactError::State(error)) => return Err(error),
                }
            }
            _ => return Err(format!("unknown saved-layer operation: {op}")),
        };
        return Ok(Some(value));
    }
    if !op.starts_with("prefix.") && !op.starts_with("layers.") {
        return Ok(None);
    }
    let tape = RefCell::new(CallbackTape::new(args));
    let mut runner = RunnerTransport(&tape);
    let mut external = || {
        let reply = tape.borrow_mut().call(json!({"callback":"external"}))?;
        serde_json::from_value::<Vec<String>>(reply["value"].clone()).map_err(|e| e.to_string())
    };
    let mut context = ProposeContext {
        runner: if args["runner"] == true {
            Some(&mut runner as &mut dyn Runner)
        } else {
            None
        },
        external: if args["external"] == true {
            Some(&mut external)
        } else {
            None
        },
        propose_every: args["propose_every"].as_i64().unwrap_or(PROPOSE_EVERY),
        home: text(args, "home"),
    };
    let result = (|| {
        if op == "prefix.new" {
            let cfg: InjectionConfig =
                serde_json::from_value(args["cfg"].clone()).map_err(|e| e.to_string())?;
            let prefix = PrefixStage::new(
                &cfg,
                text(args, "repo"),
                Path::new(text(args, "results_root")),
                args["fresh"] == true,
                args["optimizer_model"].as_str().unwrap_or("agent"),
                vocabulary(args)?,
                &mut context,
            )?;
            return Ok(json!({"self":prefix_data(&prefix),"propose_every":context.propose_every}));
        }
        if op.starts_with("prefix.") {
            let mut prefix: PrefixStage =
                serde_json::from_value(args["self"].clone()).map_err(|e| e.to_string())?;
            let mut srow = args["srow"].clone();
            let value = match op {
                "prefix.render" => json!(prefix.render()),
                "prefix.injected_ids" => json!(prefix.injected_ids()),
                "prefix.after_task" => json!(prefix.after_task(
                    &args["task"],
                    Path::new(text(args, "run_dir_path")),
                    &mut srow,
                    &mut context
                )?),
                _ => return Err(format!("unknown prefix operation: {op}")),
            };
            return Ok(json!({"self":prefix_data(&prefix),"value":value,"srow":srow}));
        }
        let mut layers: FactLayers =
            serde_json::from_value(args["self"].clone()).map_err(|e| e.to_string())?;
        let value = match op {
            "layers.render" => json!(layers.render()),
            "layers.injected_ids" => json!(layers.injected_ids()),
            "layers.opt_tokens" => json!(layers.opt_tokens()),
            "layers.now" => json!(layers.now()),
            "layers.seed_injected" => {
                let ids = serde_json::from_value::<Vec<String>>(args["ids"].clone())
                    .map_err(|e| e.to_string())?;
                layers.seed_injected(&ids);
                Value::Null
            }
            "layers.update" => {
                let edited = if args["edited"].is_null() {
                    None
                } else {
                    Some(
                        serde_json::from_value(args["edited"].clone())
                            .map_err(|e| e.to_string())?,
                    )
                };
                layers.update(
                    Path::new(text(args, "transcript_path")),
                    text(args, "iid"),
                    args["resolved"].clone(),
                    edited,
                    &mut context,
                    args["environment_claims"] == true,
                )?;
                Value::Null
            }
            "layers.save" => {
                layers.save(Path::new(text(args, "root")))?;
                Value::Null
            }
            "layers.debug_row" => layers.debug_row(),
            _ => return Err(format!("unknown composition operation: {op}")),
        };
        Ok(json!({"self":layers_data(&layers),"value":value}))
    })();
    Ok(Some(tape.into_inner().finish_result(result)?))
}
