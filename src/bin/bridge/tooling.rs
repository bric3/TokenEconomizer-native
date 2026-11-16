use std::{cell::RefCell, collections::BTreeSet, path::Path};

use serde_json::{Map, Value, json};
use tokeneconomizer::{
    config::ToolingConfig,
    runner::Runner,
    toolprune::{
        codex::{self, CodexUntoolProfile},
        stage::{ToolingAdapter, ToolingContext, ToolingStage},
        tools,
    },
};

use super::callback::{CallbackTape, RunnerTransport};

fn text<'a>(args: &'a Value, key: &str) -> &'a str {
    args[key].as_str().unwrap_or("")
}
fn strings(args: &Value, key: &str) -> Result<Vec<String>, String> {
    serde_json::from_value(args[key].clone()).map_err(|e| e.to_string())
}

fn codex_dispatch(op: &str, args: &Value) -> Result<Value, String> {
    Ok(match op {
        "toolprune.codex._item_capabilities" => json!(codex::item_capabilities(&args["item"])),
        "toolprune.codex._audit_capabilities" => json!(codex::audit_capabilities(&args["item"])),
        "toolprune.codex._wire_text" => json!(codex::wire_text(&args["item"])),
        "toolprune.codex.task_hints" => json!(codex::task_hints(&args["problem_statement"])),
        "toolprune.codex.inspect_usage" => {
            codex::inspect_usage(Path::new(text(args, "transcript_path")))
        }
        "toolprune.codex.relay_capabilities" => json!(codex::relay_capabilities(
            &if args["logical_capabilities"].is_null() {
                Vec::new()
            } else {
                strings(args, "logical_capabilities")?
            }
        )),
        "toolprune.codex._contract_fingerprint" => json!(codex::contract_fingerprint()),
        "toolprune.codex.new" => json!(CodexUntoolProfile::new(
            text(args, "repo"),
            Path::new(text(args, "results_root")),
            args["fresh"] == true
        )?),
        _ => {
            let mut profile: CodexUntoolProfile =
                serde_json::from_value(args["self"].clone()).map_err(|e| e.to_string())?;
            let value = match op {
                "toolprune.codex._new_state" => CodexUntoolProfile::new_state(&profile.repo),
                "toolprune.codex._load" => {
                    profile.load()?;
                    Value::Null
                }
                "toolprune.codex.save" => {
                    profile.save()?;
                    Value::Null
                }
                "toolprune.codex._choose_probe" => json!(profile.choose_probe(
                    &strings(args, "applied_pruned")?.into_iter().collect(),
                    &strings(args, "hints")?.into_iter().collect()
                )?),
                "toolprune.codex._eligible_demotions" => json!(profile.eligible_demotions(
                    &strings(args, "pruned")?.into_iter().collect(),
                    &strings(args, "protected")?.into_iter().collect()
                )),
                "toolprune.codex.begin_task" => {
                    profile.begin_task(&args["task_id"], &args["problem_statement"])?
                }
                "toolprune.codex._transition" => {
                    profile.transition(
                        text(args, "kind"),
                        &strings(args, "capabilities")?,
                        text(args, "reason"),
                        args["task_id"].clone(),
                    )?;
                    Value::Null
                }
                "toolprune.codex.observe" => {
                    profile.observe(&args["usage"], args["resolved"].clone())?
                }
                _ => return Err(format!("unknown Codex pruning operation: {op}")),
            };
            json!({"self":profile,"value":value})
        }
    })
}

struct AdapterTransport<'a>(&'a RefCell<CallbackTape>);
impl AdapterTransport<'_> {
    fn call(&self, method: &str, args: Value) -> Result<Value, String> {
        self.0
            .borrow_mut()
            .call(json!({"callback":"adapter","method":method,"args":args}))
            .map(|reply| reply["value"].clone())
    }
}
impl ToolingAdapter for AdapterTransport<'_> {
    fn protected_tools(&mut self) -> Result<BTreeSet<String>, String> {
        serde_json::from_value(self.call("protected_tools", json!([]))?).map_err(|e| e.to_string())
    }
    fn tool_usage(&mut self, path: &Path) -> Result<BTreeSet<String>, String> {
        let value = self.call("tool_usage", json!([path]))?;
        Ok(value
            .as_object()
            .ok_or("expected tool usage object")?
            .keys()
            .cloned()
            .collect())
    }
    fn parse_catalog_with_source(
        &mut self,
        root: &Path,
        agent: &str,
    ) -> Result<(Map<String, Value>, Value), String> {
        serde_json::from_value(self.call("parse_catalog_with_source", json!([root, agent]))?)
            .map_err(|e| e.to_string())
    }
    fn problem_statements(&mut self) -> Result<Map<String, Value>, String> {
        serde_json::from_value(self.call("problem_statements", json!([]))?)
            .map_err(|e| e.to_string())
    }
}

pub fn dispatch(op: &str, args: &Value) -> Result<Option<Value>, String> {
    if op.starts_with("toolprune.codex.") {
        return codex_dispatch(op, args).map(Some);
    }
    if op == "toolprune.tools.constants" {
        return Ok(Some(
            json!({"PROTECTED_TOOLS":tools::PROTECTED_TOOLS,"INAPPLICABLE_TOOLS":tools::INAPPLICABLE_TOOLS,"TOOL_JUDGE_DOMAIN":tools::TOOL_JUDGE_DOMAIN,"CONTRACT_FINGERPRINT":codex::contract_fingerprint()}),
        ));
    }
    if op != "toolprune.tools.judge_tools_for_task" && !op.starts_with("tooling.") {
        return Ok(None);
    }
    let tape = RefCell::new(CallbackTape::new(args));
    let mut runner = RunnerTransport(&tape);
    let mut adapter = AdapterTransport(&tape);
    let mut context = ToolingContext {
        adapter: if args["adapter"] == true {
            Some(&mut adapter)
        } else {
            None
        },
        judge: if args["runner"] == true {
            Some(&mut runner as &mut dyn Runner)
        } else {
            None
        },
        messages: Vec::new(),
    };
    let result = (|| {
        if op == "toolprune.tools.judge_tools_for_task" {
            let catalog = args["catalog"].as_object().cloned().unwrap_or_default();
            return Ok(json!(tools::judge_tools_for_task(
                text(args, "problem_statement"),
                &catalog,
                context.judge.as_mut().map(|r| &mut **r as &mut dyn Runner),
                args["model"].as_str().unwrap_or("agent"),
                args["domain"].as_str(),
                args["max_issue_chars"].as_i64().unwrap_or(4000)
            )));
        }
        if op == "tooling.new" {
            let cfg: ToolingConfig =
                serde_json::from_value(args["cfg"].clone()).map_err(|e| e.to_string())?;
            let repository = args["repo"]
                .as_str()
                .zip(args["results_root"].as_str())
                .map(|(repo, root)| (repo, Path::new(root)));
            let stage = ToolingStage::new(
                cfg,
                text(args, "agent"),
                text(args, "optimizer_model"),
                repository,
                args["fresh"] == true,
                &mut context,
            )?;
            return Ok(json!({"self":stage,"messages":context.messages}));
        }
        let mut stage: ToolingStage =
            serde_json::from_value(args["self"].clone()).map_err(|e| e.to_string())?;
        let mut srow = args["srow"].clone();
        let value = match op {
            "tooling.build_tools" => json!(stage.build_tools(
                &args["task"],
                args["i"].as_i64().unwrap_or(0),
                &mut context
            )?),
            "tooling.run_kwargs" => stage.run_kwargs(),
            "tooling.after_task" => json!(stage.after_task(
                &args["row"],
                Path::new(text(args, "run_dir_path")),
                &mut srow,
                &mut context
            )?),
            _ => return Err(format!("unknown tooling stage operation: {op}")),
        };
        Ok(json!({"self":stage,"value":value,"srow":srow,"messages":context.messages}))
    })();
    Ok(Some(tape.into_inner().finish_result(result)?))
}
