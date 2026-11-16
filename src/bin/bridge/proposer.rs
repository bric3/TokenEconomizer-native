use super::callback::{CallbackTape, RunnerTransport};
use serde_json::{Value, json};
use std::{cell::RefCell, path::Path};
use tokeneconomizer::{
    lifecycle::{self, FactRecord, ForgetError},
    microskills::distiller::{self, Detector, Distiller, Extractor, Fact, ProposeContext},
    runner::Runner,
};

fn text<'a>(args: &'a Value, key: &str) -> &'a str {
    args[key].as_str().unwrap_or("")
}
fn strings(args: &Value, key: &str) -> Result<Vec<String>, String> {
    serde_json::from_value(args[key].clone()).map_err(|e| e.to_string())
}
fn commands(args: &Value, key: &str) -> Result<Vec<distiller::Command>, String> {
    serde_json::from_value(args[key].clone()).map_err(|e| e.to_string())
}
fn data(distiller: &Distiller) -> Value {
    let mut data = serde_json::to_value(distiller).unwrap();
    for (id, fact) in &distiller.facts {
        data["facts"][id] = fact.data();
    }
    for (id, extractor) in &distiller.extractors {
        data["extractors"][id] = extractor.data();
    }
    data
}
fn forgetting(result: Result<Value, ForgetError>) -> Result<Value, String> {
    match result {
        Ok(value) => Ok(value),
        Err(ForgetError::UnknownId(id)) => Ok(json!({"raised":"KeyError","message":id})),
        Err(error) => Err(error.to_string()),
    }
}
fn detector(args: &Value) -> Detector {
    let mut detector = Detector::new(
        &args["self"]["raw"],
        args["self"]
            .get("features")
            .filter(|v| !v.is_null())
            .cloned(),
    );
    detector.scope = args["self"]["scope"].clone();
    detector.fired = args["self"]["fired"].clone();
    detector.confirm = distiller::rx(&args["self"]["confirm"]);
    detector.contra = distiller::rx(&args["self"]["contra"]);
    detector
}
fn distiller_method(op: &str, args: &Value) -> Result<Value, String> {
    let mut distiller: Distiller =
        serde_json::from_value(args["self"].clone()).map_err(|e| e.to_string())?;
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
        propose_every: args["propose_every"]
            .as_i64()
            .unwrap_or(distiller::PROPOSE_EVERY),
        home: text(args, "home"),
    };
    let mut input_spec = args["spec"].clone();
    let result = (|| {
        let value = match op {
            "distiller.render" => json!(distiller.render()),
            "distiller.update" => {
                distiller.update(
                    Path::new(text(args, "transcript_path")),
                    text(args, "iid"),
                    args["resolved"].clone(),
                    &mut context,
                )?;
                Value::Null
            }
            "distiller._maybe_propose" => {
                distiller.maybe_propose(&mut context)?;
                Value::Null
            }
            "distiller._featurize" => {
                let mut dg = args["dg"].clone();
                distiller.featurize(&mut dg, &commands(args, "cmds")?);
                dg
            }
            "distiller._sensed" => json!(distiller.sensed(text(args, "cmd"))),
            "distiller._coverage" => json!(distiller.coverage(&commands(args, "cmds")?)?),
            "distiller._abstaining_excerpts" => json!(distiller.abstaining_excerpts()?),
            "distiller._admit_extractor" => json!(distiller.admit_extractor(&input_spec)?),
            "distiller._confirms" => json!(distiller.confirms(&args["detector"])?),
            "distiller._evidence" => {
                let fact: Fact =
                    serde_json::from_value(args["fact"].clone()).map_err(|e| e.to_string())?;
                json!(distiller.evidence(&fact)?)
            }
            "distiller._restates_forgotten" => json!(distiller.restates_forgotten(&input_spec)?),
            "distiller._ground" => json!(distiller.ground(&input_spec)?),
            "distiller._build_fact" => distiller.build_fact(&input_spec)?.data(),
            "distiller._frozen_view" => Distiller::frozen_view(&input_spec),
            "distiller._admit" => json!(distiller.admit(&mut input_spec)?),
            "distiller.save" => {
                distiller.save(Path::new(text(args, "root")))?;
                Value::Null
            }
            "distiller.debug_row" => distiller.debug_row(),
            "distiller.known_ids" => {
                json!(lifecycle::known_ids(&distiller.facts, &distiller.forgotten))
            }
            "distiller.forget" => forgetting(
                distiller
                    .forget(text(args, "fid"))
                    .map(|value| json!(value)),
            )?,
            "distiller.unforget" => forgetting(distiller.unforget(text(args, "fid")))?,
            "distiller.drop_forgotten" => {
                lifecycle::drop_forgotten(&mut distiller.facts, &distiller.forgotten);
                Value::Null
            }
            "distiller._observe_fact" => {
                let mut fact: Fact =
                    serde_json::from_value(args["fact"].clone()).map_err(|e| e.to_string())?;
                let value = fact.observe(
                    args["now"].as_i64().unwrap_or(0),
                    &args["dg"],
                    strings(args, "files")?,
                )?;
                json!({"fact":fact.data(),"value":value})
            }
            _ => return Err(format!("unknown proposer operation: {op}")),
        };
        Ok(json!({"self":data(&distiller),"value":value,"spec":input_spec}))
    })();
    tape.into_inner().finish_result(result)
}

pub fn dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
    let value = match op {
        "distiller.prompts" => {
            json!({"paper":distiller::PROPOSER_SYSTEM,"generic":distiller::PROPOSER_SYSTEM_GENERIC})
        }
        "distiller.load_calls" => json!(distiller::load_calls(Path::new(text(args, "fp")))),
        "distiller.bash_cmds" => {
            let calls: Vec<distiller::Call> =
                serde_json::from_value(args["calls"].clone()).map_err(|e| e.to_string())?;
            json!(distiller::bash_cmds(&calls))
        }
        "distiller._templates" => json!(distiller::templates(
            strings(args, "cmds")?.iter().map(String::as_str),
            args["cap"].as_i64().unwrap_or(12)
        )),
        "distiller._relative" => json!(distiller::relative(
            text(args, "text"),
            &strings(args, "roots")?,
            text(args, "home")
        )),
        "distiller._command_failed" => json!(distiller::command_failed(text(args, "out"))),
        "distiller.error_signature" => json!(distiller::error_signature(
            text(args, "out"),
            &strings(args, "roots")?,
            text(args, "home")
        )?),
        "distiller.build_digest" => distiller::build_digest(
            text(args, "repo"),
            Path::new(text(args, "transcript_path")),
            text(args, "iid"),
            args["resolved"].clone(),
            args["generic"].as_bool().unwrap_or(false),
            text(args, "home"),
        )?,
        "distiller.build_generic_digest" => distiller::build_generic_digest(
            Path::new(text(args, "transcript_path")),
            text(args, "iid"),
            args["resolved"].clone(),
            text(args, "home"),
        )?,
        "distiller.proposer_view" => distiller::proposer_view(&args["dg"]),
        "distiller._rx" => json!(
            distiller::rx(&args["pat"])
                .as_ref()
                .map(fancy_regex::Regex::as_str)
        ),
        "distiller.regex_search" => json!(distiller::search(
            &distiller::rx(&args["pattern"]),
            text(args, "text")
        )?),
        "distiller.predicate" => json!(distiller::predicate(text(args, "name"), &args["dg"])),
        "distiller.build_proposer_prompt" => json!(distiller::build_proposer_prompt(
            text(args, "repo"),
            args["window"].as_array().ok_or("expected digest window")?,
            &args["existing"],
            &strings(args, "existing_feats")?.into_iter().collect(),
            &strings(args, "raw_excerpts")?,
            &strings(args, "external")?,
            args["generic"].as_bool().unwrap_or(false)
        )),
        "distiller.parse_payload" => json!(distiller::parse_payload(text(args, "raw"))),
        "distiller._detector_fields_ok" => json!(distiller::detector_fields_ok(&args["det"])),
        "distiller._reparse_cmds" => json!(distiller::reparse_cmds(&args["dg"])),
        "distiller._forgotten_evidence" => json!(distiller::forgotten_evidence(&args["rec"])),
        "detector.new" => Detector::new(&args["spec"], args.get("features").cloned()).data(),
        "detector._lookup" => json!(detector(args).lookup(&args["name"], &args["dg"])?),
        "detector._exercised" => json!(detector(args).exercised(&args["dg"])?),
        "detector._fired" => json!(detector(args).fired(&args["dg"])?),
        "detector.verdict" => json!(detector(args).verdict(&args["dg"])?),
        "extractor.new" => Extractor::new(&args["spec"])?.data(),
        "extractor.compute" => {
            let extractor = Extractor::new(&args["self"]["spec"])?;
            json!(extractor.compute(&commands(args, "cmds")?)?)
        }
        "extractor.to_dict" => args["self"]["spec"].clone(),
        "proposed_fact.new" => Fact::new(&args["spec"])?.data(),
        "proposed_fact.from_dict" => Fact::from_record(&args["d"])?.data(),
        "proposed_fact.state"
        | "proposed_fact.injectable"
        | "proposed_fact.firing_rate"
        | "proposed_fact.to_dict"
        | "proposed_fact.observe" => {
            let mut fact: Fact =
                serde_json::from_value(args["self"].clone()).map_err(|e| e.to_string())?;
            let value = match op {
                "proposed_fact.state" => json!(fact.life.state),
                "proposed_fact.injectable" => json!(fact.life.injectable()),
                "proposed_fact.firing_rate" => json!(fact.life.firing_rate()),
                "proposed_fact.to_dict" => json!(fact),
                _ => json!(fact.observe(
                    args["now"].as_i64().unwrap_or(0),
                    &args["dg"],
                    strings(args, "files")?
                )?),
            };
            json!({"self":fact.data(),"value":value})
        }
        "distiller.new" => data(&Distiller::new(
            text(args, "repo"),
            args["model"].as_str().unwrap_or("agent"),
            args["generic"].as_bool().unwrap_or(false),
        )),
        "distiller.load" => data(&Distiller::load(
            text(args, "repo"),
            Path::new(text(args, "root")),
            args["model"].as_str().unwrap_or("agent"),
            args["generic"].as_bool().unwrap_or(false),
        )?),
        "distiller._path" => json!(Distiller::path(
            Path::new(text(args, "root")),
            text(args, "repo")
        )?),
        op if op.starts_with("distiller.") => distiller_method(op, args)?,
        _ => return Ok(None),
    };
    Ok(Some(value))
}
