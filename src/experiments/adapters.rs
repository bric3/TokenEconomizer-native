use crate::{
    loopguard::state::state_from_trajectory,
    microskills::observe::Vocabulary,
    toolprune::{
        protected::{ProtectedTools, fnmatchcase},
        stage::ToolingAdapter,
        tools::{INAPPLICABLE_TOOLS, PROTECTED_TOOLS},
    },
    trace::{Call, calls, catalog},
    value::truthy,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt, fs, io,
    path::{Component, Path, PathBuf},
    string::FromUtf8Error,
    sync::LazyLock,
};

pub const SWEBENCH_TASKS_ENV: &str = "TOKENECONOMIZER_SWEBENCH_TASKS";
pub const PROJECTS_ROOT_ENV: &str = "TOKENECONOMIZER_WEBBENCH_PROJECTS";
pub const CREDDATA_ROOT_ENV: &str = "TOKENECONOMIZER_CREDDATA_ROOT";
pub const REPOSVUL_ROOT_ENV: &str = "TOKENECONOMIZER_REPOSVUL_ROOT";
pub const NETWORK_TOOLS: &[&str] = &["WebSearch", "WebFetch"];
pub const CREDDATA_TASK_PROMPT: &str = include_str!("adapters/prompts/creddata-task.txt");
pub const CREDDATA_CLASSIFY_DOMAIN: &str = include_str!("adapters/prompts/creddata-classify.txt");
pub const REPOSVUL_TASK_PROMPT: &str = include_str!("adapters/prompts/reposvul-task.txt");
pub const REPOSVUL_CLASSIFY_DOMAIN: &str = include_str!("adapters/prompts/reposvul-classify.txt");

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Utf8(FromUtf8Error),
    Json(serde_json::Error),
    Invalid(&'static str, String),
}
impl Error {
    pub fn kind(&self) -> &str {
        match self {
            Self::Io(e) => match e.kind() {
                io::ErrorKind::NotFound => "FileNotFoundError",
                io::ErrorKind::PermissionDenied => "PermissionError",
                io::ErrorKind::IsADirectory => "IsADirectoryError",
                io::ErrorKind::NotADirectory => "NotADirectoryError",
                _ => "OSError",
            },
            Self::Utf8(_) => "UnicodeDecodeError",
            Self::Json(_) => "JSONDecodeError",
            Self::Invalid(kind, _) => kind,
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => e.fmt(f),
            Self::Utf8(e) => e.fmt(f),
            Self::Json(e) => e.fmt(f),
            Self::Invalid(_, message) => message.fmt(f),
        }
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;
fn invalid(kind: &'static str, message: impl Into<String>) -> Error {
    Error::Invalid(kind, message.into())
}
fn text_file(path: &Path) -> Result<String> {
    String::from_utf8(fs::read(path).map_err(Error::Io)?).map_err(Error::Utf8)
}
fn space(c: char) -> bool {
    c.is_whitespace() || matches!(c, '\u{1c}'..='\u{1f}')
}
fn statement(value: Option<&Value>) -> Result<String> {
    let Some(value) = value.filter(|v| truthy(v)) else {
        return Ok(String::new());
    };
    let text = value.as_str().ok_or_else(|| {
        invalid(
            "AttributeError",
            "problem statement has no attribute 'strip'",
        )
    })?;
    Ok(redact_gold_pointers(text.trim_matches(space)).0)
}
pub fn redact_gold_pointers(text: &str) -> (String, usize) {
    // Python's case-insensitive i also matches dotted/dotless I; its word class excludes marks.
    static GOLD: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)https?://(?:www\.)?g[iİı]thub\.com/[\p{L}\p{N}_.-]+/[\p{L}\p{N}_.-]+/(?:pull|pulls|[iİı]ssues|[iİı]ssue|comm[iİı]t|comm[iİı]ts|compare|blob|tree)/[^\s\x1c-\x1f]+").unwrap()
    });
    (
        GOLD.replace_all(text, "[link redacted]").into_owned(),
        GOLD.find_iter(text).count(),
    )
}
pub fn untool_exclusions(used: &BTreeSet<String>, extra: &[String]) -> Vec<String> {
    INAPPLICABLE_TOOLS
        .iter()
        .map(|s| (*s).to_owned())
        .chain(
            extra
                .iter()
                .filter(|t| !INAPPLICABLE_TOOLS.contains(&t.as_str()))
                .cloned(),
        )
        .filter(|t| !PROTECTED_TOOLS.contains(&t.as_str()) && !used.contains(t))
        .collect()
}

fn setting(path: Option<PathBuf>, env: &Map<String, Value>, key: &str) -> Option<PathBuf> {
    path.filter(|p| !p.as_os_str().is_empty())
        .or_else(|| env.get(key).and_then(Value::as_str).map(PathBuf::from))
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "name", rename_all = "lowercase")]
// shortcut: IDs follow the dataset string schemas; extend key types in the JSON parity audit.
pub enum Adapter {
    Swebench {
        tasks_path: Option<PathBuf>,
    },
    Webbench {
        project: String,
        projects_root: Option<PathBuf>,
    },
    Creddata {
        creddata_root: Option<PathBuf>,
        #[serde(rename = "_repo_ids")]
        repo_ids: Option<Vec<String>>,
    },
    Reposvul {
        reposvul_root: Option<PathBuf>,
        #[serde(rename = "_instance_ids")]
        instance_ids: Option<Vec<String>>,
    },
}
impl Adapter {
    pub fn swebench(path: Option<PathBuf>, env: &Map<String, Value>) -> Self {
        Self::Swebench {
            tasks_path: setting(path, env, SWEBENCH_TASKS_ENV),
        }
    }
    pub fn webbench(project: String, root: Option<PathBuf>, env: &Map<String, Value>) -> Self {
        Self::Webbench {
            project,
            projects_root: setting(root, env, PROJECTS_ROOT_ENV),
        }
    }
    pub fn creddata(
        root: Option<PathBuf>,
        ids: Option<Vec<String>>,
        env: &Map<String, Value>,
    ) -> Self {
        Self::Creddata {
            creddata_root: setting(root, env, CREDDATA_ROOT_ENV),
            repo_ids: ids,
        }
    }
    pub fn reposvul(
        root: Option<PathBuf>,
        ids: Option<Vec<String>>,
        env: &Map<String, Value>,
    ) -> Self {
        Self::Reposvul {
            reposvul_root: setting(root, env, REPOSVUL_ROOT_ENV),
            instance_ids: ids,
        }
    }
    pub fn name(&self) -> &str {
        match self {
            Self::Swebench { .. } => "swebench",
            Self::Webbench { .. } => "webbench",
            Self::Creddata { .. } => "creddata",
            Self::Reposvul { .. } => "reposvul",
        }
    }
    pub fn tasks_path(&self) -> Result<PathBuf> {
        let Self::Webbench {
            project,
            projects_root,
        } = self
        else {
            return Err(invalid(
                "AttributeError",
                "tasks_path is only a method on WebBenchAdapter",
            ));
        };
        let root = projects_root.as_ref().filter(|p| !p.as_os_str().is_empty()).ok_or_else(|| invalid("RuntimeError", format!("no Web-Bench projects root: set ${PROJECTS_ROOT_ENV} or pass projects_root=.")))?;
        Ok(root.join(project).join("tasks.jsonl"))
    }
    pub fn meta_dir(&self) -> Result<PathBuf> {
        let Self::Creddata { creddata_root, .. } = self else {
            return Err(invalid(
                "AttributeError",
                "meta_dir requires CredDataAdapter",
            ));
        };
        let root = creddata_root
            .as_ref()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or_else(|| {
                invalid(
                    "RuntimeError",
                    format!("no CredData root: set ${CREDDATA_ROOT_ENV} or pass creddata_root=."),
                )
            })?;
        Ok(root.join("meta"))
    }
    pub fn instances_dir(&self) -> Result<PathBuf> {
        let Self::Reposvul { reposvul_root, .. } = self else {
            return Err(invalid(
                "AttributeError",
                "instances_dir requires ReposVulAdapter",
            ));
        };
        let root = reposvul_root
            .as_ref()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or_else(|| {
                invalid(
                    "RuntimeError",
                    format!("no ReposVul root: set ${REPOSVUL_ROOT_ENV} or pass reposvul_root=."),
                )
            })?;
        Ok(root.join("instances"))
    }
    pub fn repo_ids(&self) -> Result<Vec<String>> {
        if let Self::Creddata {
            repo_ids: Some(ids),
            ..
        } = self
        {
            return Ok(ids.clone());
        }
        Ok(dataset_ids(&self.meta_dir()?, ".csv"))
    }
    pub fn instance_ids(&self) -> Result<Vec<String>> {
        if let Self::Reposvul {
            instance_ids: Some(ids),
            ..
        } = self
        {
            return Ok(ids.clone());
        }
        Ok(dataset_ids(&self.instances_dir()?, ".json"))
    }
    pub fn problem_statements(&self) -> Result<Map<String, Value>> {
        let mut out = Map::new();
        match self {
            Self::Swebench { tasks_path } => {
                let path = tasks_path.as_ref().filter(|p| !p.as_os_str().is_empty()).ok_or_else(|| invalid("RuntimeError", format!("no SWE-bench task spec configured: set ${SWEBENCH_TASKS_ENV} or pass tasks_path= to SwebenchAdapter (a JSON list of {{instance_id, problem_statement}}).")))?;
                let text = text_file(path)?;
                let data: Value = serde_json::from_str(&text).map_err(Error::Json)?;
                for item in data
                    .as_array()
                    .ok_or_else(|| invalid("TypeError", "task spec must be a list"))?
                {
                    let item = item
                        .as_object()
                        .ok_or_else(|| invalid("TypeError", "task record must be an object"))?;
                    let id = item
                        .get("instance_id")
                        .ok_or_else(|| invalid("KeyError", "instance_id"))?
                        .as_str()
                        .ok_or_else(|| invalid("TypeError", "instance_id must be a string"))?;
                    out.insert(
                        id.into(),
                        Value::String(statement(item.get("problem_statement"))?),
                    );
                }
            }
            Self::Webbench { .. } => {
                let text = text_file(&self.tasks_path()?)?;
                for line in text
                    .split(['\r', '\n'])
                    .map(|s| s.trim_matches(space))
                    .filter(|s| !s.is_empty())
                {
                    let item: Value = serde_json::from_str(line).map_err(Error::Json)?;
                    let item = item.as_object().ok_or_else(|| {
                        invalid("AttributeError", "task record has no attribute 'get'")
                    })?;
                    let Some(id) = item.get("id").filter(|v| truthy(v)) else {
                        continue;
                    };
                    let id = id
                        .as_str()
                        .ok_or_else(|| invalid("TypeError", "task id must be a string"))?;
                    out.insert(
                        id.into(),
                        Value::String(statement(item.get("description"))?),
                    );
                }
            }
            Self::Creddata { .. } => {
                for id in self.repo_ids()? {
                    out.insert(id, Value::String(CREDDATA_TASK_PROMPT.into()));
                }
            }
            Self::Reposvul { .. } => {
                for id in self.instance_ids()? {
                    out.insert(id, Value::String(REPOSVUL_TASK_PROMPT.into()));
                }
            }
        }
        Ok(out)
    }
    pub fn classify_domain(&self) -> Option<&'static str> {
        match self {
            Self::Creddata { .. } => Some(CREDDATA_CLASSIFY_DOMAIN),
            Self::Reposvul { .. } => Some(REPOSVUL_CLASSIFY_DOMAIN),
            _ => None,
        }
    }
    pub fn env_vocabulary(&self) -> Vocabulary {
        if matches!(self, Self::Webbench { .. }) {
            Vocabulary::node_web()
        } else {
            Vocabulary::default()
        }
    }
    pub fn load_trajectory(&self, path: &Path) -> Vec<Call> {
        calls::load_trajectory(path)
    }
    pub fn tool_usage(&self, path: &Path) -> BTreeMap<String, usize> {
        calls::tool_usage(path)
    }
    pub fn parse_catalog(&self, run_dir: &Path, agent: &str) -> Map<String, Value> {
        catalog::parse(run_dir, agent)
    }
    pub fn parse_catalog_with_source(
        &self,
        run_dir: &Path,
        agent: &str,
    ) -> (Map<String, Value>, Option<String>) {
        catalog::parse_with_source(run_dir, agent)
    }
    pub fn protected_tools(&self) -> ProtectedTools {
        ProtectedTools {
            names: PROTECTED_TOOLS.iter().map(|s| (*s).into()).collect(),
            patterns: Vec::new(),
        }
    }
    pub fn inapplicable_tools(&self) -> Vec<String> {
        INAPPLICABLE_TOOLS
            .iter()
            .chain(NETWORK_TOOLS)
            .map(|s| (*s).into())
            .collect()
    }
    pub fn hardcoded_exclusions(&self, used: &BTreeSet<String>) -> Vec<String> {
        untool_exclusions(
            used,
            &NETWORK_TOOLS
                .iter()
                .map(|s| (*s).into())
                .collect::<Vec<_>>(),
        )
    }
    pub fn spiral_snapshot(&self, path: &Path) -> Value {
        state_from_trajectory(path).snapshot()
    }
}

impl ToolingAdapter for Adapter {
    fn protected_tools(&mut self) -> std::result::Result<ProtectedTools, String> {
        Ok(Adapter::protected_tools(self))
    }
    fn tool_usage(&mut self, path: &Path) -> std::result::Result<BTreeSet<String>, String> {
        Ok(Adapter::tool_usage(self, path).into_keys().collect())
    }
    fn parse_catalog_with_source(
        &mut self,
        path: &Path,
        agent: &str,
    ) -> std::result::Result<(Map<String, Value>, Value), String> {
        let (catalog, source) = Adapter::parse_catalog_with_source(self, path, agent);
        Ok((catalog, source.map_or(Value::Null, Value::String)))
    }
    fn problem_statements(&mut self) -> std::result::Result<Map<String, Value>, String> {
        Adapter::problem_statements(self).map_err(|e| e.to_string())
    }
}

fn dataset_ids(root: &Path, suffix: &str) -> Vec<String> {
    let mut paths = vec![PathBuf::new()];
    for component in root.join(format!("*{suffix}")).components() {
        let Component::Normal(pattern) = component else {
            for path in &mut paths {
                path.push(component);
            }
            continue;
        };
        let pattern = pattern.to_string_lossy();
        if !pattern.contains(['*', '?', '[']) {
            for path in &mut paths {
                path.push(component);
            }
            continue;
        }
        paths = paths
            .into_iter()
            .flat_map(|path| {
                fs::read_dir(if path.as_os_str().is_empty() {
                    Path::new(".")
                } else {
                    &path
                })
                .into_iter()
                .flatten()
                .filter_map(std::result::Result::ok)
                .filter(|entry| {
                    let name = entry.file_name();
                    let name = name.to_string_lossy();
                    if name.starts_with('.') && !pattern.starts_with('.') {
                        return false;
                    }
                    #[cfg(windows)]
                    {
                        fnmatchcase(&name.to_lowercase(), &pattern.to_lowercase())
                    }
                    #[cfg(not(windows))]
                    {
                        fnmatchcase(&name, &pattern)
                    }
                })
                .map(|entry| path.join(entry.file_name()))
                .collect::<Vec<_>>()
            })
            .collect();
    }
    let mut ids: Vec<_> = paths
        .iter()
        .filter_map(|p| {
            let name = p.file_name()?.to_str()?;
            name.get(..name.len().checked_sub(suffix.len())?)
                .map(str::to_owned)
        })
        .collect();
    ids.sort();
    ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        config::ToolingConfig,
        toolprune::stage::{ToolingContext, ToolingStage},
    };
    use serde_json::json;

    #[test]
    fn native_tooling_uses_dataset_prompts_and_preserves_protected_tools() {
        let mut adapter = Adapter::creddata(None, Some(vec!["repo".into()]), &Map::new());
        let mut context = ToolingContext {
            adapter: Some(&mut adapter),
            ..ToolingContext::default()
        };
        let cfg = ToolingConfig {
            evidence: 1,
            judge: true,
            ..ToolingConfig::default()
        };
        let mut stage =
            ToolingStage::new(cfg, "codex", "agent", None, false, &mut context).unwrap();
        assert_eq!(stage.problem_by_iid["repo"], CREDDATA_TASK_PROMPT);
        let run_dir =
            std::env::temp_dir().join(format!("te-adapter-no-transcript-{}", std::process::id()));
        assert!(!run_dir.exists());
        let mut summary = json!({});
        stage
            .after_task(&json!({}), &run_dir, &mut summary, &mut context)
            .unwrap();
        let excluded = stage
            .build_tools(&json!({"instance_id":"repo"}), 1, &mut context)
            .unwrap()
            .unwrap();
        assert!(excluded.iter().any(|tool| tool == "spawn_agent"));
        assert!(
            !excluded
                .iter()
                .any(|tool| tool == "shell" || tool == "apply_patch")
        );
        assert!(context.messages.is_empty());
    }
}
