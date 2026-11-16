use std::path::Path;

use serde_json::{Value, json};
use tokeneconomizer::{
    lifecycle::{self, FactRecord, ForgetError, Lifecycle},
    microskills::{
        envfacts::{self, EnvFact, EnvFactsDistiller},
        observe::{self, Observation, Vocabulary},
    },
};

fn vocabulary(args: &Value) -> Result<Vocabulary, String> {
    if args["vocab"].is_null() {
        Ok(Vocabulary::default())
    } else {
        serde_json::from_value(args["vocab"].clone()).map_err(|e| e.to_string())
    }
}

fn cap(args: &Value, key: &str) -> Result<Option<i64>, String> {
    match &args[key] {
        Value::Null => Ok(None),
        Value::String(s) if s.is_empty() => Ok(None),
        Value::String(s) => s.trim().parse::<i64>().map(Some).map_err(|e| e.to_string()),
        Value::Bool(b) => Ok(Some(i64::from(*b))),
        value => value
            .as_i64()
            .or_else(|| value.as_f64().map(|n| n as i64))
            .map(Some)
            .ok_or_else(|| format!("expected {key} integer")),
    }
}

fn fact_method(op: &str, args: &Value) -> Result<Value, String> {
    let fact: EnvFact = serde_json::from_value(args["self"].clone()).map_err(|e| e.to_string())?;
    let value = match op {
        "envfact.claim" => json!(fact.claim()),
        "envfact._best" => json!(fact.best(text(args, "key"))),
        "envfact.working_form" => json!(fact.working_form()),
        "envfact.state" => json!(fact.life.state),
        "envfact.injectable" => json!(fact.life.injectable()),
        "envfact.firing_rate" => json!(fact.life.firing_rate()),
        "envfact.to_dict" => json!(fact),
        _ => return Err(format!("unknown fact operation: {op}")),
    };
    Ok(json!({"self":fact,"value":value}))
}

fn forgetting(result: Result<Value, ForgetError>) -> Result<Value, String> {
    match result {
        Ok(value) => Ok(value),
        Err(ForgetError::UnknownId(id)) => Ok(json!({"raised":"KeyError","message":id})),
        Err(error) => Err(error.to_string()),
    }
}

fn distiller_method(op: &str, args: &Value) -> Result<Value, String> {
    let mut distiller: EnvFactsDistiller =
        serde_json::from_value(args["self"].clone()).map_err(|e| e.to_string())?;
    let value = match op {
        "envfacts.render" => json!(distiller.render()),
        "envfacts.injected_ids" => json!(distiller.injected_ids()),
        "envfacts.seed_injected" => {
            distiller.seed_injected(&if args["ids"].is_null() {
                Vec::new()
            } else {
                strings(args, "ids")?
            });
            Value::Null
        }
        "envfacts.injectable_claims" => json!(distiller.injectable_claims()),
        "envfacts._select" => json!(distiller.select().iter().map(|f| &f.id).collect::<Vec<_>>()),
        "envfacts._merged_lines" => {
            let facts: Vec<EnvFact> =
                serde_json::from_value(args["facts"].clone()).map_err(|e| e.to_string())?;
            json!(distiller.merged_lines(&facts.iter().collect::<Vec<_>>()))
        }
        "envfacts.header" => json!(distiller.header()),
        "envfacts.save" => {
            distiller.save(Path::new(text(args, "root")))?;
            Value::Null
        }
        "envfacts.debug_row" => distiller.debug_row(),
        "envfacts.update" => {
            let edited = if args["edited"].is_null() {
                None
            } else {
                Some(strings(args, "edited")?.into_iter().collect())
            };
            distiller.update(
                Path::new(text(args, "transcript_path")),
                text(args, "iid"),
                args["resolved"].clone(),
                edited,
            )?;
            Value::Null
        }
        "envfacts.known_ids" => json!(lifecycle::known_ids(&distiller.facts, &distiller.forgotten)),
        "envfacts.forget" => forgetting(
            lifecycle::forget(
                &mut distiller.facts,
                &mut distiller.forgotten,
                text(args, "fid"),
                None,
            )
            .map(|changed| json!(changed)),
        )?,
        "envfacts.unforget" => forgetting(lifecycle::unforget(
            &mut distiller.facts,
            &mut distiller.forgotten,
            text(args, "fid"),
        ))?,
        "envfacts.drop_forgotten" => {
            lifecycle::drop_forgotten(&mut distiller.facts, &distiller.forgotten);
            Value::Null
        }
        _ => return Err(format!("unknown distiller operation: {op}")),
    };
    Ok(json!({"self":distiller,"value":value}))
}

fn text<'a>(args: &'a Value, key: &str) -> &'a str {
    args[key].as_str().unwrap_or("")
}
fn optional<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args[key].as_str()
}
fn strings(args: &Value, key: &str) -> Result<Vec<String>, String> {
    serde_json::from_value(args[key].clone()).map_err(|e| e.to_string())
}
fn observation(args: &Value) -> Result<Observation, String> {
    serde_json::from_value(args["o"].clone()).map_err(|e| e.to_string())
}

pub fn dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
    let vocab = || vocabulary(args);
    let value = match op {
        "lifecycle.new" => json!(Lifecycle::new(
            text(args, "key").into(),
            args["high"].as_bool().unwrap_or(false),
            args["drift"].as_bool().unwrap_or(true)
        )),
        "lifecycle.from_dict" => json!(Lifecycle::from_record(&args["d"])?),
        "lifecycle.to_dict" => json!(
            serde_json::from_value::<Lifecycle>(args["self"].clone()).map_err(|e| e.to_string())?
        ),
        "envfact.new" => json!(EnvFact::new(
            text(args, "fid"),
            text(args, "family"),
            args["high"].as_bool().unwrap_or(false),
            args["params"].as_object().cloned().unwrap_or_default()
        )),
        "envfact.from_dict" => json!(EnvFact::from_record(&args["d"])?),
        op if op.starts_with("envfact.") => fact_method(op, args)?,
        "envfacts.new" => json!(EnvFactsDistiller::new(
            text(args, "repo"),
            vocab()?,
            cap(args, "max_facts")?,
            cap(args, "max_tokens")?
        )),
        "envfacts.load" => json!(EnvFactsDistiller::load(
            text(args, "repo"),
            Path::new(text(args, "root")),
            vocab()?,
            cap(args, "max_facts")?,
            cap(args, "max_tokens")?
        )?),
        "envfacts._path" => json!(EnvFactsDistiller::path(
            Path::new(text(args, "root")),
            text(args, "repo")
        )?),
        "envfacts._repairs" => json!(envfacts::repairs(
            text(args, "form"),
            text(args, "a_form"),
            text(args, "intent")
        )),
        "envfacts._tool_of" => json!(envfacts::tool_of(text(args, "form"))),
        "envfacts._moves_shell" => json!(envfacts::moves_shell(text(args, "form"))),
        "envfacts._usage" => json!(envfacts::usage(
            text(args, "form"),
            optional(args, "args").unwrap_or(" …")
        )),
        "envfacts._root" => json!(envfacts::root(text(args, "form"))),
        "envfacts._join" => json!(envfacts::join(&strings(args, "items")?)),
        "envfacts.lint_ok" => json!(envfacts::lint_ok(text(args, "text"))),
        "envfacts.typed_ok" => json!(envfacts::typed_ok(&observation(args)?)),
        "envfacts.fold" => {
            let mut fact: EnvFact =
                serde_json::from_value(args["fact"].clone()).map_err(|e| e.to_string())?;
            envfacts::fold(
                &mut fact,
                args["now"].as_i64().unwrap_or(0),
                text(args, "verdict"),
                args["fired"].as_bool().unwrap_or(false),
                args["exercised"].as_bool().unwrap_or(false),
                strings(args, "files")?,
            );
            json!(fact)
        }
        op if op.starts_with("envfacts.") => distiller_method(op, args)?,
        "lifecycle.edited_files" | "lifecycle.edited_pyfiles" => {
            let calls: Vec<(String, Value, Value)> =
                serde_json::from_value(args["calls"].clone()).map_err(|e| e.to_string())?;
            json!(lifecycle::edited_files(
                calls.iter().map(|(tool, args, _)| (tool.as_str(), args)),
                op == "lifecycle.edited_pyfiles"
            ))
        }
        "observe.unwrap" => json!(observe::unwrap(text(args, "cmd"))),
        "observe.segments" => json!(observe::segments(text(args, "cmd"))?),
        "observe._tokens" => json!(text(args, "seg").split_whitespace().collect::<Vec<_>>()),
        "observe.repository_top" => json!(observe::repository_top(text(args, "start"))),
        "observe._cd_target" => json!(observe::cd_target(
            text(args, "cur"),
            text(args, "target"),
            optional(args, "root"),
            optional(args, "top")
        )),
        "observe._shell_path" => json!(observe::shell_path(text(args, "p"))),
        "observe._shell_isabs" => json!(observe::shell_isabs(text(args, "p"))),
        "observe.classify_command" => json!(observe::classify_command(
            text(args, "cmd"),
            args["call_index"].as_u64().unwrap_or(0) as usize,
            &vocab()?,
            text(args, "cwd"),
            optional(args, "root"),
            optional(args, "top")
        )?),
        "observe._runner_script" => {
            let rest = strings(args, "rest")?;
            json!(observe::runner_script(
                &rest.iter().map(String::as_str).collect::<Vec<_>>(),
                &vocab()?
            ))
        }
        "observe._probe_kind" => {
            let rest = strings(args, "args")?;
            json!(observe::probe_kind(
                text(args, "sub"),
                &rest.iter().map(String::as_str).collect::<Vec<_>>()
            ))
        }
        "observe.error_class" => json!(observe::error_class(text(args, "output"))),
        "observe._exit_code_hint" => json!(observe::exit_code_hint(text(args, "output"))),
        "observe._label_failed" => json!(observe::label_failed(
            &strings(args, "labels")?,
            text(args, "out")
        )),
        "observe._test_labels" => json!(observe::test_labels(&observation(args)?)),
        "observe.label_rule" => json!(observe::label_rule(text(args, "a"), text(args, "b"))),
        "observe._env_calls" => json!(observe::env_calls(
            text(args, "cmd"),
            text(args, "out"),
            args["success"].as_bool().unwrap_or(false)
        )?),
        "observe.form_failed" => json!(observe::form_failed(&observation(args)?)),
        "observe.form_worked" => json!(observe::form_worked(&observation(args)?)),
        "observe.observe_task" => json!(observe::observe_task(
            Path::new(text(args, "transcript_path")),
            &vocab()?
        )?),
        "observe._error_line" => json!(observe::error_line(text(args, "out"), text(args, "err"))),
        "observe._base" => json!(observe::base(text(args, "path"))),
        "observe._not_found_name" => json!(observe::not_found_name(text(args, "line"))?),
        "observe.is_bare" => json!(observe::is_bare(text(args, "form"))),
        _ => return Ok(None),
    };
    Ok(Some(value))
}
