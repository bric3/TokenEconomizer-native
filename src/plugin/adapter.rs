use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use super::paths;
use crate::{
    loopguard::state::state_from_trajectory,
    microskills::observe::Vocabulary,
    toolprune::{protected::ProtectedTools, stage::ToolingAdapter, tools::PROTECTED_TOOLS},
    trace::{Call, calls, catalog, claude},
};

pub const INTERACTIVE_PROTECTED: &[&str] = &[
    "Bash",
    "Read",
    "Edit",
    "MultiEdit",
    "Write",
    "Grep",
    "Glob",
    "LS",
    "NotebookRead",
    "BashOutput",
    "KillShell",
    "KillBash",
    "AskUserQuestion",
    "EnterPlanMode",
    "ExitPlanMode",
    "TodoWrite",
    "TaskCreate",
    "TaskUpdate",
    "TaskGet",
    "TaskList",
    "TaskOutput",
    "TaskStop",
    "Agent",
    "Task",
    "Skill",
    "ToolSearch",
    "SlashCommand",
    "Monitor",
    "WebFetch",
    "WebSearch",
];
pub const COPILOT_PROTECTED: &[&str] = &[
    "bash",
    "read_bash",
    "write_bash",
    "stop_bash",
    "list_bash",
    "view",
    "edit",
    "create",
    "str_replace",
    "apply_patch",
    "custom",
    "rg",
    "grep",
    "glob",
    "task",
    "skill",
    "read_agent",
    "list_agents",
    "write_agent",
    "ask_user",
    "web_fetch",
    "web_search",
    "report_intent",
    "update_todo",
    "sql",
];
pub const HIGH_RISK: &[&str] = &["NotebookEdit"];
pub const HIGH_RISK_FACTOR: i64 = 3;

pub fn detect_vocabulary(root: &Path, preference: &str) -> Vocabulary {
    match preference {
        "node-web" => Vocabulary::node_web(),
        "python-git" => Vocabulary::default(),
        _ => {
            let mut vocabulary = Vocabulary::generic();
            if root.join("package.json").exists()
                && ![
                    "pyproject.toml",
                    "setup.py",
                    "setup.cfg",
                    "requirements.txt",
                    "Pipfile",
                    "tox.ini",
                    "environment.yml",
                ]
                .iter()
                .any(|p| root.join(p).exists())
            {
                vocabulary.canonical_interpreter = "node".into();
            }
            vocabulary
        }
    }
}
pub fn catalog_path(root: &Path, host: &str) -> PathBuf {
    root.join("hosts").join(host).join("catalog.json")
}
pub fn load_catalog(root: &Path, host: &str) -> Result<Map<String, Value>, String> {
    let data = fs::read(catalog_path(root, host))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    let Some(data) = data else {
        return Ok(Map::new());
    };
    let object = data.as_object().ok_or("catalog must be an object")?;
    let names = match object.get("tools") {
        None => Vec::new(),
        Some(Value::Array(values)) => values
            .iter()
            .filter_map(Value::as_str)
            .map(String::from)
            .collect(),
        Some(Value::String(value)) => value.chars().map(|c| c.to_string()).collect(),
        Some(Value::Object(values)) => values.keys().cloned().collect(),
        Some(_) => return Err("catalog tools must be iterable".into()),
    };
    Ok(names.into_iter().map(|name| (name, json!(""))).collect())
}

#[derive(Serialize, Deserialize)]
pub struct LiveAdapter {
    pub host: String,
    pub repo_root: PathBuf,
    pub vocabulary: String,
    pub catalog: Map<String, Value>,
    pub protect: Vec<String>,
}
impl LiveAdapter {
    pub fn new(
        host: &str,
        repo_root: &Path,
        vocabulary: &str,
        catalog: Map<String, Value>,
        protect: Vec<String>,
    ) -> Self {
        Self {
            host: host.into(),
            repo_root: repo_root.into(),
            vocabulary: vocabulary.into(),
            catalog,
            protect,
        }
    }
    pub fn load_trajectory(&self, path: &Path) -> Vec<Call> {
        calls::load_trajectory(path)
    }
    pub fn tool_usage(&self, path: &Path) -> Result<BTreeMap<String, usize>, String> {
        Ok(if self.host == "claude" {
            claude::tool_usage(&paths::realpath(path)?, true)
        } else {
            calls::tool_usage(path)
        })
    }
    pub fn parse_catalog_with_source(
        &self,
        run_dir: &Path,
    ) -> Result<(Map<String, Value>, Value), String> {
        let transcript = paths::realpath(&run_dir.join("transcript.jsonl"))?;
        let mut cat = self.catalog.clone();
        let mut observed: BTreeSet<_> = self.tool_usage(&transcript)?.into_keys().collect();
        if self.host == "claude" {
            observed.extend(claude::deferred_tools(&transcript));
        } else if self.host == "copilot" && transcript.exists() {
            let bytes = fs::read(&transcript).map_err(|e| e.to_string())?;
            let text = String::from_utf8_lossy(&bytes)
                .replace("\r\n", "\n")
                .replace('\r', "\n");
            observed.extend(
                catalog::parse_copilot_names(&text)
                    .into_iter()
                    .map(|(name, _)| name),
            );
        }
        for name in observed {
            cat.entry(name).or_insert(json!(""));
        }
        let source = if cat.is_empty() {
            Value::Null
        } else {
            json!(if self.catalog.is_empty() {
                "observed"
            } else {
                "probe+observed"
            })
        };
        Ok((cat, source))
    }
    pub fn protected_tools(&self) -> ProtectedTools {
        let mut names: BTreeSet<_> = PROTECTED_TOOLS
            .iter()
            .chain(INTERACTIVE_PROTECTED)
            .map(|s| (*s).into())
            .collect();
        if self.host == "copilot" {
            names.extend(COPILOT_PROTECTED.iter().map(|s| (*s).into()));
        }
        ProtectedTools {
            names,
            patterns: self.protect.clone(),
        }
    }
    pub fn inapplicable_tools(&self) -> Vec<String> {
        Vec::new()
    }
    pub fn hardcoded_exclusions(&self, _used: &[String]) -> Vec<String> {
        Vec::new()
    }
    pub fn spiral_snapshot(&self, path: &Path) -> Value {
        state_from_trajectory(path).snapshot()
    }
    pub fn problem_statements(&self) -> Map<String, Value> {
        Map::new()
    }
    pub fn env_vocabulary(&self) -> Vocabulary {
        detect_vocabulary(&self.repo_root, &self.vocabulary)
    }
}
impl ToolingAdapter for LiveAdapter {
    fn protected_tools(&mut self) -> Result<ProtectedTools, String> {
        Ok(LiveAdapter::protected_tools(self))
    }
    fn tool_usage(&mut self, transcript: &Path) -> Result<BTreeSet<String>, String> {
        Ok(LiveAdapter::tool_usage(self, transcript)?
            .into_keys()
            .collect())
    }
    fn parse_catalog_with_source(
        &mut self,
        run_dir: &Path,
        _agent: &str,
    ) -> Result<(Map<String, Value>, Value), String> {
        LiveAdapter::parse_catalog_with_source(self, run_dir)
    }
    fn problem_statements(&mut self) -> Result<Map<String, Value>, String> {
        Ok(LiveAdapter::problem_statements(self))
    }
}
