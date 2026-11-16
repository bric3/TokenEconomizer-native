use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::{
    names::repo_filename,
    store::Store,
    trace::jsonl::TextReader,
    value::{integer, json_pretty, json_text, scalar_text, truthy},
};

pub const EVIDENCE_CANDIDATES: [&str; 8] = [
    "collaboration",
    "web",
    "goals",
    "interactive_guidance",
    "skills",
    "request_user_input",
    "image_media",
    "deferred_wait",
];
pub const CORE_CAPABILITIES: [&str; 1] = ["deferred_wait"];
pub const CANDIDATE_CAPABILITIES: [&str; 7] = [
    "collaboration",
    "web",
    "goals",
    "interactive_guidance",
    "skills",
    "request_user_input",
    "image_media",
];
pub const CONFIG_CAPABILITIES: [&str; 6] = [
    "collaboration",
    "web",
    "goals",
    "skills",
    "image_media",
    "interactive_guidance",
];
pub const MIN_LOADED_EXPOSURES: [i64; 7] = [1, 1, 1, 1, 2, 2, 3];
pub const MAX_DEMOTIONS_PER_TASK: usize = 2;
pub const RECENT_WINDOW: usize = 3;
pub const FIRST_PROBE_AT: i64 = 24;
pub const INITIAL_PROBE_GAP: i64 = 32;
pub const PROBE_PRIORITY: [&str; 7] = [
    "image_media",
    "skills",
    "request_user_input",
    "web",
    "collaboration",
    "goals",
    "interactive_guidance",
];
pub const PROFILE_VERSION: &str = "codex-recent-experience-untool-indirect-use-core";
pub const RETAINED_CAPABILITIES: [&str; 4] =
    ["apply_patch", "exec_command", "planning", "write_stdin"];
pub const AUDIT_CAPABILITIES: [&str; 8] = [
    "collaboration",
    "deferred_wait",
    "goals",
    "image_media",
    "interactive_guidance",
    "request_user_input",
    "skills",
    "web",
];

fn capability(name: &str) -> Option<&'static str> {
    Some(match name {
        "spawn_agent" | "send_input" | "send_message" | "followup_task" | "resume_agent"
        | "wait_agent" | "close_agent" | "interrupt_agent" | "list_agents" => "collaboration",
        "create_goal" | "get_goal" | "update_goal" => "goals",
        "view_image" | "image" | "audio" | "generated_image" | "image_generation" => "image_media",
        "update_plan" | "todo_list" => "planning",
        "web" | "web_search" | "web_fetch" | "web__run" => "web",
        "request_user_input" => "request_user_input",
        _ => return None,
    })
}

fn text(value: &Value) -> String {
    if truthy(value) {
        scalar_text(value)
    } else {
        String::new()
    }
}
fn sorted_json(value: &Value) -> String {
    let mut value = value.clone();
    value.sort_all_objects();
    json_text(&value)
}
fn names(value: &Value) -> BTreeSet<String> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(String::from)
        .collect()
}
fn count(value: &Value) -> Result<i64, String> {
    integer(value)
        .map(|n| n as i64)
        .ok_or_else(|| format!("invalid integer: {value}"))
}
fn number(state: &Value, key: &str, default: i64) -> Result<i64, String> {
    state.get(key).map_or(Ok(default), count)
}
fn tail(values: &mut Vec<Value>, cap: usize) {
    if values.len() > cap {
        values.drain(..values.len() - cap);
    }
}

pub fn item_capabilities(item: &Value) -> BTreeSet<String> {
    static NESTED: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\btools\.([A-Za-z_][A-Za-z0-9_]*)\s*\(").unwrap());
    let mut found = BTreeSet::new();
    if !item.is_object() {
        return found;
    }
    for key in ["type", "name", "tool_name"] {
        if let Some(capability) = capability(&text(&item[key])) {
            found.insert(capability.into());
        }
    }
    let kind = text(&item["type"]);
    if kind == "collab_tool_call" {
        found.insert("collaboration".into());
    }
    if ["custom_tool_call", "function_call", "tool_call"].contains(&kind.as_str())
        || item["name"] == "exec"
    {
        for key in ["input", "arguments"] {
            let raw = &item[key];
            let raw = if raw.is_object() {
                sorted_json(raw)
            } else if let Some(s) = raw.as_str() {
                s.into()
            } else {
                continue;
            };
            for matched in NESTED.captures_iter(&raw) {
                if let Some(capability) = capability(&matched[1]) {
                    found.insert(capability.into());
                }
            }
        }
    }
    found
}

pub fn task_hints(problem: &Value) -> Vec<String> {
    static IMAGE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)\b(?:screenshot|png|jpe?g|gif|svg|audio|video)\b|\b(?:image|figure)\s+(?:file|output|render|display)\b").unwrap()
    });
    static SKILL: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(concat!(r"(?i)\b(?:OpenAI|ChatGPT|Codex|Responses API)\b|\b(?:generate|create|edit|transform)\s+(?:an?\s+)?",
        r"(?:image|photo|illustration|texture|sprite|mockup)\b|\b(?:create|update|install|scaffold)\s+(?:an?\s+)?",
        r"(?:Codex\s+)?(?:skill|plugin)\b|\bSKILL\.md\b|\.codex-plugin\b")).unwrap()
    });
    let text = text(problem);
    let mut hints = Vec::new();
    if IMAGE.is_match(&text) {
        hints.push("image_media".into());
    }
    if SKILL.is_match(&text) {
        hints.push("skills".into());
    }
    hints
}

pub fn wire_text(item: &Value) -> String {
    ["input", "arguments", "command", "text"]
        .iter()
        .filter_map(|key| {
            let value = &item[key];
            if value.is_object() {
                Some(sorted_json(value))
            } else {
                value.as_str().map(String::from)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn audit_capabilities(item: &Value) -> BTreeSet<String> {
    static SKILL_USE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)(?:\bSKILL\.md\b|[/\\]skills[/\\]|\bskills\.read\s*\()").unwrap()
    });
    let mut found = item_capabilities(item);
    if !item.is_object() {
        return found;
    }
    if item["name"] == "wait"
        || (!truthy(&item["name"]) && item["tool_name"] == "wait")
        || item["type"] == "wait"
    {
        found.insert("deferred_wait".into());
    }
    if SKILL_USE.is_match(&wire_text(item)) {
        found.insert("skills".into());
    }
    found
}

pub fn inspect_usage(path: &Path) -> Value {
    let zero = || {
        AUDIT_CAPABILITIES
            .iter()
            .map(|c| (c.to_string(), BTreeSet::<String>::new()))
            .collect::<BTreeMap<_, _>>()
    };
    let (mut attempted, mut completed, mut failed) = (zero(), zero(), zero());
    let mut tools = BTreeMap::<String, i64>::new();
    let mut seen_tools = BTreeSet::new();
    let mut reconnects = 0;
    if let Ok(mut reader) = TextReader::open(path) {
        let mut line_number = 0;
        while let Ok(line) = reader.readline(usize::MAX) {
            if line.is_empty() {
                break;
            }
            line_number += 1;
            let Ok(event) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if !event.is_object() {
                continue;
            }
            let phase = text(&event["type"]);
            let message = if truthy(&event["message"]) {
                &event["message"]
            } else {
                &event["error"]
            };
            if phase == "error" && text(message).to_lowercase().contains("reconnecting") {
                reconnects += 1;
            }
            if !["item.started", "item.completed"].contains(&phase.as_str()) {
                continue;
            }
            let item = &event["item"];
            let represented: Vec<_> = audit_capabilities(item)
                .into_iter()
                .filter(|c| AUDIT_CAPABILITIES.contains(&c.as_str()))
                .collect();
            if represented.is_empty() {
                continue;
            }
            let id = ["id", "call_id"]
                .iter()
                .map(|key| &item[key])
                .find(|v| truthy(v))
                .map_or_else(|| line_number.to_string(), scalar_text);
            let label = ["name", "tool_name", "tool", "type"]
                .iter()
                .map(|key| &item[key])
                .find(|v| truthy(v))
                .map_or_else(|| "unknown".into(), scalar_text);
            if seen_tools.insert(id.clone()) {
                *tools.entry(label).or_default() += 1;
            }
            let status = text(&item["status"]).to_lowercase();
            for cap in represented {
                attempted.get_mut(&cap).unwrap().insert(id.clone());
                if phase == "item.completed" {
                    let target = if ["failed", "error", "cancelled"].contains(&status.as_str()) {
                        &mut failed
                    } else {
                        &mut completed
                    };
                    target.get_mut(&cap).unwrap().insert(id.clone());
                }
            }
        }
    }
    let totals = |m: BTreeMap<String, BTreeSet<String>>| -> Value {
        Value::Object(
            AUDIT_CAPABILITIES
                .iter()
                .map(|c| (c.to_string(), json!(m[*c].len())))
                .collect(),
        )
    };
    json!({"attempted":totals(attempted),"completed":totals(completed),"failed":totals(failed),"tools":tools,"reconnects":reconnects})
}

pub fn relay_capabilities(logical: &[String]) -> Vec<String> {
    logical
        .iter()
        .filter(|c| {
            [
                "web",
                "skills",
                "request_user_input",
                "image_media",
                "deferred_wait",
            ]
            .contains(&c.as_str())
        })
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub fn contract_fingerprint() -> String {
    let min: Map<_, _> = CANDIDATE_CAPABILITIES
        .iter()
        .zip(MIN_LOADED_EXPOSURES)
        .map(|(c, n)| (c.to_string(), json!(n)))
        .collect();
    let relay: Map<_, _> = EVIDENCE_CANDIDATES
        .iter()
        .map(|c| (c.to_string(), json!(relay_capabilities(&[c.to_string()]))))
        .collect();
    let config: BTreeSet<_> = CONFIG_CAPABILITIES.into_iter().collect();
    let contract = json!({"profile":PROFILE_VERSION,"evidence_candidates":EVIDENCE_CANDIDATES,"candidates":CANDIDATE_CAPABILITIES,"core_capabilities":CORE_CAPABILITIES,
        "min_loaded_exposures":min,"max_demotions_per_task":MAX_DEMOTIONS_PER_TASK,"recent_window":RECENT_WINDOW,"config_capabilities":config,"relay_capabilities":relay,
        "first_probe_at":FIRST_PROBE_AT,"initial_probe_gap":INITIAL_PROBE_GAP,"probe_priority":PROBE_PRIORITY});
    format!("{:x}", Sha256::digest(sorted_json(&contract).as_bytes()))
}

#[derive(Serialize, Deserialize)]
pub struct CodexUntoolProfile {
    pub repo: String,
    pub path: PathBuf,
    #[serde(rename = "_pending")]
    pub pending: Option<Value>,
    pub state: Value,
}

impl CodexUntoolProfile {
    pub fn new(repo: &str, root: &Path, fresh: bool) -> Result<Self, String> {
        let path = Store::new(root)
            .map_err(|e| e.to_string())?
            .base
            .join("interface_state")
            .join(repo_filename(repo))
            .join("codex_untool.json");
        let mut profile = Self {
            repo: repo.into(),
            path,
            pending: None,
            state: Self::new_state(repo),
        };
        if !fresh {
            profile.load()?;
        }
        Ok(profile)
    }

    pub fn new_state(repo: &str) -> Value {
        let zero: Map<_, _> = EVIDENCE_CANDIDATES
            .iter()
            .map(|c| (c.to_string(), json!(0)))
            .collect();
        let recent: Map<_, _> = EVIDENCE_CANDIDATES
            .iter()
            .map(|c| (c.to_string(), json!([])))
            .collect();
        json!({"version":PROFILE_VERSION,"contract_fingerprint":contract_fingerprint(),"repo":repo,"tasks_completed":0,"current_pruned":[],"ever_completed":[],"core_capabilities":CORE_CAPABILITIES,
            "attempted_total":zero,"completed_total":zero,"failed_total":zero,"loaded_exposures":zero,"incomplete_attempt_exposures":zero,"zero_attempt_exposures":zero,
            "recent_loaded_outcomes":recent,"next_probe_at":FIRST_PROBE_AT,"probe_gap":INITIAL_PROBE_GAP,"probe_cursor":0,"transitions":[],"history":[]})
    }

    pub fn load(&mut self) -> Result<(), String> {
        let Ok(bytes) = fs::read(&self.path) else {
            return Ok(());
        };
        let Ok(loaded) = serde_json::from_slice::<Value>(&bytes) else {
            return Ok(());
        };
        let Some(loaded) = loaded.as_object() else {
            return Err("expected saved Codex state object".into());
        };
        if loaded.get("version") != Some(&json!(PROFILE_VERSION))
            || loaded.get("contract_fingerprint") != Some(&json!(contract_fingerprint()))
            || loaded.get("repo") != Some(&json!(self.repo))
        {
            return Ok(());
        }
        let mut state = Self::new_state(&self.repo);
        state.as_object_mut().unwrap().extend(loaded.clone());
        for field in [
            "attempted_total",
            "completed_total",
            "failed_total",
            "loaded_exposures",
            "incomplete_attempt_exposures",
            "zero_attempt_exposures",
        ] {
            let mut merged: Map<_, _> = EVIDENCE_CANDIDATES
                .iter()
                .map(|c| (c.to_string(), json!(0)))
                .collect();
            for (c, n) in state[field].as_object().into_iter().flatten() {
                if EVIDENCE_CANDIDATES.contains(&c.as_str()) {
                    merged.insert(c.clone(), json!(count(n)?));
                }
            }
            state[field] = Value::Object(merged);
        }
        let mut recent: Map<_, _> = EVIDENCE_CANDIDATES
            .iter()
            .map(|c| (c.to_string(), json!([])))
            .collect();
        for (cap, outcomes) in state["recent_loaded_outcomes"]
            .as_object()
            .into_iter()
            .flatten()
        {
            if let Some(slot) = recent.get_mut(cap) {
                let mut valid: Vec<_> = outcomes
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|v| {
                        [
                            json!("completed"),
                            json!("incomplete_attempt"),
                            json!("unused"),
                        ]
                        .contains(v)
                    })
                    .cloned()
                    .collect();
                tail(&mut valid, RECENT_WINDOW);
                *slot = json!(valid);
            }
        }
        state["recent_loaded_outcomes"] = Value::Object(recent);
        state["current_pruned"] = json!(
            names(&state["current_pruned"])
                .into_iter()
                .filter(|c| CANDIDATE_CAPABILITIES.contains(&c.as_str()))
                .collect::<Vec<_>>()
        );
        state["ever_completed"] = json!(
            names(&state["ever_completed"])
                .into_iter()
                .filter(|c| EVIDENCE_CANDIDATES.contains(&c.as_str()))
                .collect::<Vec<_>>()
        );
        state["core_capabilities"] = json!(CORE_CAPABILITIES);
        self.state = state;
        Ok(())
    }

    pub fn save(&self) -> Result<(), String> {
        fs::create_dir_all(self.path.parent().ok_or("missing state directory")?)
            .map_err(|e| e.to_string())?;
        let mut temporary = self.path.as_os_str().to_owned();
        temporary.push(".tmp");
        let temporary = PathBuf::from(temporary);
        fs::write(&temporary, json_pretty(&self.state, true))
            .and_then(|_| fs::rename(temporary, &self.path))
            .map_err(|e| e.to_string())
    }

    pub fn choose_probe(
        &self,
        applied: &BTreeSet<String>,
        hints: &BTreeSet<String>,
    ) -> Result<Option<String>, String> {
        if number(&self.state, "tasks_completed", 0)?
            < number(&self.state, "next_probe_at", FIRST_PROBE_AT)?
        {
            return Ok(None);
        }
        let start = number(&self.state, "probe_cursor", 0)?;
        for offset in 0..PROBE_PRIORITY.len() {
            let cap = PROBE_PRIORITY
                [(start + offset as i64).rem_euclid(PROBE_PRIORITY.len() as i64) as usize];
            if applied.contains(cap) && !hints.contains(cap) {
                return Ok(Some(cap.into()));
            }
        }
        Ok(None)
    }

    pub fn eligible_demotions(
        &self,
        pruned: &BTreeSet<String>,
        protected: &BTreeSet<String>,
    ) -> Vec<String> {
        CANDIDATE_CAPABILITIES
            .iter()
            .zip(MIN_LOADED_EXPOSURES)
            .filter(|(c, needed)| {
                if pruned.contains(**c) || protected.contains(**c) {
                    return false;
                }
                let outcomes = self.state["recent_loaded_outcomes"][*c]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                outcomes.len() >= *needed as usize
                    && !outcomes[outcomes.len() - *needed as usize..].contains(&json!("completed"))
            })
            .take(MAX_DEMOTIONS_PER_TASK)
            .map(|(c, _)| c.to_string())
            .collect()
    }

    pub fn begin_task(&mut self, task_id: &Value, problem: &Value) -> Result<Value, String> {
        let completed = number(&self.state, "tasks_completed", 0)?;
        let hints: BTreeSet<_> = task_hints(problem).into_iter().collect();
        let mut logical = names(&self.state["current_pruned"])
            .difference(&hints)
            .cloned()
            .collect::<BTreeSet<_>>();
        let probe = self.choose_probe(&logical, &hints)?;
        if let Some(probe) = &probe {
            logical.remove(probe);
        }
        if logical
            .iter()
            .any(|c| CORE_CAPABILITIES.contains(&c.as_str()))
        {
            return Err("attempted to prune an indirect-use core package".into());
        }
        let reason = if completed == 0 {
            "observe-complete-contract".into()
        } else if let Some(probe) = &probe {
            format!("single-package-probe:{probe}")
        } else if !hints.is_empty() {
            format!(
                "task-hint:{}",
                hints.iter().cloned().collect::<Vec<_>>().join(",")
            )
        } else {
            "recent-experience-indirect-use-core-mask".into()
        };
        let retained: BTreeSet<_> = RETAINED_CAPABILITIES
            .into_iter()
            .chain(CORE_CAPABILITIES)
            .collect();
        let config: Vec<_> = logical
            .iter()
            .filter(|c| CONFIG_CAPABILITIES.contains(&c.as_str()))
            .cloned()
            .collect();
        let pending = json!({"task_id":scalar_text(task_id),"profile":PROFILE_VERSION,"audit":completed==0,"reason":reason,"logical_capabilities":logical,"config_capabilities":config,
            "relay_capabilities":relay_capabilities(&logical.into_iter().collect::<Vec<_>>()),"task_hints":hints,"probe_capability":probe,"probe_reason":if probe.is_some() {json!("evidence-aging")}else{Value::Null},
            "tasks_completed_before":completed,"core_capabilities":CORE_CAPABILITIES,"retained_capabilities":retained});
        self.pending = Some(pending.clone());
        Ok(pending)
    }

    pub fn transition(
        &mut self,
        kind: &str,
        capabilities: &[String],
        reason: &str,
        task_id: Value,
    ) -> Result<(), String> {
        if capabilities.is_empty() {
            return Ok(());
        }
        let mut transitions = self.state["transitions"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let mut capabilities = capabilities.to_vec();
        capabilities.sort();
        transitions.push(json!({"after_tasks":number(&self.state,"tasks_completed",0)?,"task_id":task_id,"kind":kind,"capabilities":capabilities,"reason":reason}));
        tail(&mut transitions, 128);
        self.state["transitions"] = json!(transitions);
        Ok(())
    }

    pub fn observe(&mut self, usage: &Value, resolved: Value) -> Result<Value, String> {
        let pending = self
            .pending
            .clone()
            .ok_or("observe() called without begin_task()")?;
        let logical = names(&pending["logical_capabilities"]);
        let loaded: BTreeSet<_> = EVIDENCE_CANDIDATES
            .iter()
            .filter(|c| !logical.contains(**c))
            .map(|c| c.to_string())
            .collect();
        let mut current = names(&self.state["current_pruned"]);
        let mut protected = names(&self.state["ever_completed"]);
        let mut completed_now = Vec::new();
        let mut counts = Map::new();
        for kind in ["attempted", "completed", "failed"] {
            counts.insert(
                kind.into(),
                Value::Object(
                    EVIDENCE_CANDIDATES
                        .iter()
                        .map(|c| Ok((c.to_string(), json!(count(&usage[kind][*c])?))))
                        .collect::<Result<_, String>>()?,
                ),
            );
        }
        for cap in EVIDENCE_CANDIDATES {
            let attempted = count(&counts["attempted"][cap])?;
            let completed = count(&counts["completed"][cap])?;
            for (kind, field) in [
                ("attempted", "attempted_total"),
                ("completed", "completed_total"),
                ("failed", "failed_total"),
            ] {
                self.state[field][cap] =
                    json!(count(&self.state[field][cap])? + count(&counts[kind][cap])?);
            }
            if completed > 0 {
                protected.insert(cap.into());
                current.remove(cap);
                completed_now.push(cap.into());
            }
            if !loaded.contains(cap) {
                continue;
            }
            self.state["loaded_exposures"][cap] =
                json!(count(&self.state["loaded_exposures"][cap])? + 1);
            let outcome = if completed > 0 {
                "completed"
            } else if attempted > 0 {
                "incomplete_attempt"
            } else {
                "unused"
            };
            let field = match outcome {
                "incomplete_attempt" => Some("incomplete_attempt_exposures"),
                "unused" => Some("zero_attempt_exposures"),
                _ => None,
            };
            if let Some(field) = field {
                self.state[field][cap] = json!(count(&self.state[field][cap])? + 1);
            }
            let mut outcomes = self.state["recent_loaded_outcomes"][cap]
                .as_array()
                .cloned()
                .unwrap_or_default();
            outcomes.push(json!(outcome));
            tail(&mut outcomes, RECENT_WINDOW);
            self.state["recent_loaded_outcomes"][cap] = json!(outcomes);
        }
        let probe = pending["probe_capability"].as_str();
        if let Some(probe) = probe {
            let gap = number(&self.state, "probe_gap", INITIAL_PROBE_GAP)?;
            let gap = if gap == 0 {
                INITIAL_PROBE_GAP
            } else {
                gap.max(1)
            };
            self.state["next_probe_at"] =
                json!(number(&self.state, "next_probe_at", FIRST_PROBE_AT)? + gap);
            self.state["probe_gap"] = json!(gap * 2);
            self.state["probe_cursor"] = json!(
                (PROBE_PRIORITY
                    .iter()
                    .position(|c| *c == probe)
                    .ok_or("invalid probe capability")?
                    + 1)
                    % PROBE_PRIORITY.len()
            );
            let (kind, reason) = if !protected.contains(probe) {
                current.insert(probe.into());
                ("probe-reprune", "targeted-probe-produced-no-completed-use")
            } else {
                ("probe-protect", "targeted-probe-produced-completed-use")
            };
            self.transition(kind, &[probe.into()], reason, pending["task_id"].clone())?;
        }
        let newly = self.eligible_demotions(&current, &protected);
        current.extend(newly.iter().cloned());
        current.retain(|c| !protected.contains(c) && CANDIDATE_CAPABILITIES.contains(&c.as_str()));
        self.state["tasks_completed"] = json!(number(&self.state, "tasks_completed", 0)? + 1);
        self.state["current_pruned"] = json!(current);
        self.state["ever_completed"] = json!(protected);
        self.transition(
            "gradual-demotion",
            &newly,
            "recent-loaded-exposures-with-zero-completed-use",
            pending["task_id"].clone(),
        )?;
        self.transition(
            "completed-protect",
            &completed_now,
            "completed-use-permanently-protected",
            pending["task_id"].clone(),
        )?;
        let record = json!({"task_id":pending["task_id"],"audit":truthy(&pending["audit"]),"reason":pending["reason"],"logical_capabilities":logical,"config_capabilities":pending["config_capabilities"],
            "relay_capabilities":pending["relay_capabilities"],"task_hints":pending["task_hints"],"probe_capability":pending["probe_capability"],"probe_reason":pending["probe_reason"],"resolved":resolved,
            "loaded_capabilities":loaded,"attempted":counts["attempted"],"completed":counts["completed"],"failed":counts["failed"],"newly_pruned":newly,"mask_after":self.state["current_pruned"]});
        let mut history = self.state["history"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        history.push(record.clone());
        tail(&mut history, 256);
        self.state["history"] = json!(history);
        self.save()?;
        self.pending = None;
        Ok(record)
    }
}
