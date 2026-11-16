use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use super::{
    codex::{self, CodexUntoolProfile},
    tools::judge_tools_for_task,
};
use crate::{config::ToolingConfig, runner::Runner, value::truthy};

pub trait ToolingAdapter {
    fn protected_tools(&mut self) -> Result<BTreeSet<String>, String>;
    fn tool_usage(&mut self, transcript: &Path) -> Result<BTreeSet<String>, String>;
    fn parse_catalog_with_source(
        &mut self,
        run_dir: &Path,
        agent: &str,
    ) -> Result<(Map<String, Value>, Value), String>;
    fn problem_statements(&mut self) -> Result<Map<String, Value>, String>;
}

#[derive(Default)]
pub struct ToolingContext<'a> {
    pub adapter: Option<&'a mut dyn ToolingAdapter>,
    pub judge: Option<&'a mut dyn Runner>,
    pub messages: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct ToolingStage {
    pub cfg: ToolingConfig,
    pub agent: String,
    pub optimizer_model: String,
    pub used_tools: BTreeSet<String>,
    pub tool_unused_streaks: BTreeMap<String, i64>,
    pub catalog_tools: BTreeSet<String>,
    #[serde(rename = "_warned_no_catalog")]
    pub warned_no_catalog: bool,
    pub catalog_desc: Map<String, Value>,
    pub codex_profile: Option<CodexUntoolProfile>,
    #[serde(rename = "_codex_selection")]
    pub codex_selection: Option<Value>,
    pub problem_by_iid: Map<String, Value>,
    #[serde(rename = "_ev_reaudit")]
    pub ev_reaudit: bool,
    #[serde(rename = "_exclude_tools")]
    pub exclude_tools: Option<Vec<String>>,
    #[serde(rename = "_task_judge_tokens")]
    pub task_judge_tokens: f64,
    #[serde(rename = "_task_judge_dropped")]
    pub task_judge_dropped: Vec<String>,
}

impl ToolingStage {
    pub const NAME: &str = "tooling";

    pub fn new(
        cfg: ToolingConfig,
        agent: &str,
        model: &str,
        repository: Option<(&str, &Path)>,
        fresh: bool,
        context: &mut ToolingContext<'_>,
    ) -> Result<Self, String> {
        let codex_profile = if cfg.codex_untool && agent == "codex" {
            let (repo, root) = repository
                .filter(|(repo, root)| !repo.is_empty() && !root.as_os_str().is_empty())
                .ok_or("Codex capability pruning requires repo and results_root")?;
            Some(CodexUntoolProfile::new(repo, root, fresh)?)
        } else {
            None
        };
        let mut problem_by_iid = Map::new();
        if cfg.judge
            && let Some(adapter) = context.adapter.as_mut()
        {
            match adapter.problem_statements() {
                Ok(problems) => problem_by_iid = problems,
                Err(error) => context.messages.push(format!(
                    "[untool-judge] could not load problem statements ({error}); judge disabled"
                )),
            }
        }
        Ok(Self {
            cfg,
            agent: agent.into(),
            optimizer_model: model.into(),
            used_tools: BTreeSet::new(),
            tool_unused_streaks: BTreeMap::new(),
            catalog_tools: BTreeSet::new(),
            warned_no_catalog: false,
            catalog_desc: Map::new(),
            codex_profile,
            codex_selection: None,
            problem_by_iid,
            ev_reaudit: false,
            exclude_tools: None,
            task_judge_tokens: 0.0,
            task_judge_dropped: Vec::new(),
        })
    }

    pub fn build_tools(
        &mut self,
        task: &Value,
        index: i64,
        context: &mut ToolingContext<'_>,
    ) -> Result<Option<Vec<String>>, String> {
        let iid = task["instance_id"].as_str().ok_or("expected instance_id")?;
        self.task_judge_tokens = 0.0;
        self.task_judge_dropped.clear();
        self.ev_reaudit = false;
        if let Some(profile) = self.codex_profile.as_mut() {
            let selection = profile.begin_task(
                &task["instance_id"],
                task.get("problem_statement").unwrap_or(&json!("")),
            )?;
            self.exclude_tools = Some(
                serde_json::from_value(selection["relay_capabilities"].clone())
                    .map_err(|e| e.to_string())?,
            );
            self.codex_selection = Some(selection);
            return Ok(self.exclude_tools.clone());
        }
        if self.cfg.evidence != 0 {
            self.ev_reaudit = self.cfg.evidence_reaudit > 0
                && index > 0
                && index % self.cfg.evidence_reaudit == 0;
            let mut excluded = BTreeSet::new();
            if !self.ev_reaudit {
                let protected = match context.adapter.as_mut() {
                    Some(adapter) => adapter.protected_tools()?,
                    None => BTreeSet::new(),
                };
                let candidates: BTreeSet<_> = self
                    .catalog_tools
                    .iter()
                    .filter(|t| !protected.contains(*t) && !self.used_tools.contains(*t))
                    .cloned()
                    .collect();
                excluded = candidates
                    .iter()
                    .filter(|t| {
                        self.tool_unused_streaks.get(*t).copied().unwrap_or(0) >= self.cfg.evidence
                    })
                    .cloned()
                    .collect();
                if self.cfg.judge
                    && !self.catalog_tools.is_empty()
                    && self.problem_by_iid.get(iid).is_some_and(truthy)
                {
                    let candidates: Map<_, _> = candidates
                        .difference(&excluded)
                        .map(|t| {
                            (
                                t.clone(),
                                self.catalog_desc.get(t).cloned().unwrap_or(json!("")),
                            )
                        })
                        .collect();
                    if !candidates.is_empty() {
                        let (loadmap, tokens) = judge_tools_for_task(
                            self.problem_by_iid[iid]
                                .as_str()
                                .ok_or("expected problem statement")?,
                            &candidates,
                            context.judge.as_mut().map(|r| &mut **r as &mut dyn Runner),
                            &self.optimizer_model,
                            None,
                            4000,
                        );
                        self.task_judge_tokens = tokens;
                        self.task_judge_dropped = loadmap
                            .iter()
                            .filter(|(t, load)| !truthy(load) && candidates.contains_key(*t))
                            .map(|(t, _)| t.clone())
                            .collect();
                        excluded.extend(self.task_judge_dropped.iter().cloned());
                    }
                }
            }
            self.exclude_tools = Some(excluded.into_iter().collect());
        } else {
            self.exclude_tools = None;
        }
        Ok(self.exclude_tools.clone())
    }

    pub fn run_kwargs(&self) -> Value {
        if self.codex_profile.is_none() {
            return json!({});
        }
        json!({"codex_config_capabilities":self.codex_selection.as_ref().map(|s| &s["config_capabilities"]).filter(|v| truthy(v)).cloned().unwrap_or(json!([]))})
    }

    pub fn after_task(
        &mut self,
        row: &Value,
        run_dir: &Path,
        srow: &mut Value,
        context: &mut ToolingContext<'_>,
    ) -> Result<i64, String> {
        let transcript = run_dir.join("transcript.jsonl");
        let task_used = match context.adapter.as_mut() {
            Some(adapter) => adapter.tool_usage(&transcript)?,
            None => BTreeSet::new(),
        };
        self.used_tools.extend(task_used.iter().cloned());
        if let Some(profile) = self.codex_profile.as_mut() {
            let selection = self.codex_selection.clone().unwrap_or(json!({}));
            let record =
                profile.observe(&codex::inspect_usage(&transcript), srow["resolved"].clone())?;
            srow["codex_untool_active"] = json!(true);
            srow["codex_untool_profile"] = json!(codex::PROFILE_VERSION);
            srow["codex_untool_reason"] = selection["reason"].clone();
            for key in [
                "config_capabilities",
                "relay_capabilities",
                "core_capabilities",
            ] {
                srow[format!("codex_untool_{key}")] = selection
                    .get(key)
                    .filter(|v| truthy(v))
                    .cloned()
                    .unwrap_or(json!([]));
            }
            for (out, key) in [
                ("newly_pruned", "newly_pruned"),
                ("mask_after", "mask_after"),
                ("usage_attempted", "attempted"),
                ("usage_completed", "completed"),
            ] {
                srow[format!("codex_untool_{out}")] = record[key].clone();
            }
            srow["codex_untool_adapt_tokens"] = json!(0);
            return Ok(0);
        }
        if self.cfg.evidence != 0 {
            srow["untooled_excluded"] = row.get("untooled_excluded").cloned().unwrap_or(json!([]));
            srow["untool_evidence_excluded"] =
                json!(self.exclude_tools.clone().unwrap_or_default());
            srow["untool_evidence_reaudit"] = json!(self.ev_reaudit);
            if self.exclude_tools.as_ref().is_none_or(Vec::is_empty)
                && let Some(adapter) = context.adapter.as_mut()
            {
                let (catalog, source) = adapter.parse_catalog_with_source(run_dir, &self.agent)?;
                srow["untool_catalog_source"] = source;
                if !catalog.is_empty() {
                    self.catalog_tools.extend(catalog.keys().cloned());
                    self.catalog_desc.extend(catalog);
                } else if self.catalog_tools.is_empty() && !self.warned_no_catalog {
                    self.warned_no_catalog = true;
                    context.messages.push(format!("[tooling] !! no tool catalog parsed for agent={} from {} -- evidence pruning can drop NOTHING and this arm will behave as its baseline. Likely an agent-CLI upgrade that moved the catalog; see trace.catalog_sources.", self.agent, run_dir.display()));
                }
            }
            for tool in &self.catalog_tools {
                let streak = self.tool_unused_streaks.entry(tool.clone()).or_default();
                *streak = if task_used.contains(tool) {
                    0
                } else {
                    *streak + 1
                };
            }
            srow["untool_catalog_size"] = json!(self.catalog_tools.len());
            if self.cfg.judge {
                srow["untool_judge_tokens"] = json!(self.task_judge_tokens);
                srow["untool_judge_dropped"] = json!(self.task_judge_dropped);
            }
        }
        Ok(0)
    }
}
