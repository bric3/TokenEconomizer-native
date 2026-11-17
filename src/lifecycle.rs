use std::collections::{BTreeSet, HashSet};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

pub const K_PROMOTE: usize = 3;
pub const GAMMA: f64 = 0.8;
pub const LAMBDA: f64 = 5.0;
pub const SCORE_FLOOR: f64 = 0.0;
pub const FIRE_GATE: f64 = 0.40;
pub const FIRE_MIN_N: u64 = 4;
pub const NO_RECORD: &str = "no record";

pub fn edited_files<'a>(
    calls: impl IntoIterator<Item = (&'a str, &'a Value)>,
    python_only: bool,
) -> BTreeSet<String> {
    let mut files = BTreeSet::new();
    for (tool, args) in calls {
        let paths = if tool == "apply_patch" {
            args["paths"]
                .as_array()
                .map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>())
                .unwrap_or_default()
        } else if ["edit", "multiedit", "write", "create", "str_replace_editor"].contains(&tool) {
            vec![
                args["file_path"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .or_else(|| args["path"].as_str())
                    .unwrap_or(""),
            ]
        } else {
            continue;
        };
        for path in paths {
            if (python_only && path.ends_with(".py"))
                || (!python_only && (tool == "apply_patch" || !path.is_empty()))
            {
                let name = if cfg!(windows) {
                    path.rsplit(['/', '\\']).next()
                } else {
                    path.rsplit('/').next()
                }
                .unwrap_or("");
                files.insert(name.into());
            }
        }
    }
    files
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lifecycle {
    pub key: String,
    pub high: bool,
    #[serde(default = "default_drift")]
    pub drift: bool,
    pub state: String,
    pub confirms: Vec<(i64, Vec<String>)>,
    pub contradicts: Vec<i64>,
    pub fire_num: u64,
    pub fire_den: u64,
    pub promoted_at: Option<i64>,
    pub retired_at: Option<i64>,
}

fn default_drift() -> bool {
    true
}

impl Lifecycle {
    pub fn from_record(data: &Value) -> Result<Self, String> {
        let key = data["key"].as_str().ok_or("expected lifecycle key")?;
        let high = data["high"]
            .as_bool()
            .ok_or("expected lifecycle high flag")?;
        let mut record = serde_json::to_value(Self::new(key.into(), high, true)).unwrap();
        record
            .as_object_mut()
            .unwrap()
            .extend(data.as_object().ok_or("expected lifecycle record")?.clone());
        serde_json::from_value(record).map_err(|e| e.to_string())
    }

    pub fn new(key: String, high: bool, drift: bool) -> Self {
        Self {
            key,
            high,
            drift,
            state: "CANDIDATE".into(),
            confirms: Vec::new(),
            contradicts: Vec::new(),
            fire_num: 0,
            fire_den: 0,
            promoted_at: None,
            retired_at: None,
        }
    }

    pub fn drifted_confirms(&self) -> usize {
        if !self.drift {
            return self.confirms.len();
        }
        let mut kept = Vec::<HashSet<&String>>::new();
        for (_, files) in &self.confirms {
            let files = files.iter().collect::<HashSet<_>>();
            if files.is_empty() || kept.iter().all(|prior| prior.is_disjoint(&files)) {
                kept.push(files);
            }
        }
        kept.len()
    }

    pub fn score(&self, now: i64) -> f64 {
        self.confirms
            .iter()
            .map(|(i, _)| GAMMA.powf(now as f64 - *i as f64))
            .sum::<f64>()
            - LAMBDA
                * self
                    .contradicts
                    .iter()
                    .map(|i| GAMMA.powf(now as f64 - *i as f64))
                    .sum::<f64>()
    }

    pub fn firing_rate(&self) -> f64 {
        if self.fire_den == 0 {
            0.0
        } else {
            self.fire_num as f64 / self.fire_den as f64
        }
    }

    pub fn injectable(&self) -> bool {
        self.state == "ACTIVE" && self.fire_den >= FIRE_MIN_N && self.firing_rate() >= FIRE_GATE
    }

    pub fn update(
        &mut self,
        now: i64,
        verdict: &str,
        fired: bool,
        exercised: bool,
        mut files: Vec<String>,
    ) {
        if exercised {
            self.fire_den += 1;
            if fired {
                self.fire_num += 1;
            }
        }
        match verdict {
            "confirm" => {
                files.sort();
                self.confirms.push((now, files));
            }
            "contradict" => self.contradicts.push(now),
            _ => (),
        }
        if self.state == "CANDIDATE" {
            if self.drifted_confirms() >= K_PROMOTE {
                self.state = "ACTIVE".into();
                self.promoted_at = Some(now);
            }
        } else if self.state == "ACTIVE"
            && ((self.high && verdict == "contradict") || self.score(now) < SCORE_FLOOR)
        {
            self.state = "RETIRED".into();
            self.retired_at = Some(now);
        }
    }
}

pub trait FactRecord: Serialize + Sized {
    fn claim(&self) -> Value;
    fn from_record(record: &Value) -> Result<Self, String>;
}

#[derive(Debug)]
pub enum ForgetError {
    UnknownId(String),
    InvalidRecord(String),
}

impl std::fmt::Display for ForgetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownId(id) => write!(f, "{id}"),
            Self::InvalidRecord(error) => write!(f, "{error}"),
        }
    }
}
impl std::error::Error for ForgetError {}

pub fn known_ids<F>(
    facts: &IndexMap<String, F>,
    forgotten: &Map<String, Value>,
) -> BTreeSet<String> {
    facts.keys().chain(forgotten.keys()).cloned().collect()
}

pub fn forget<F: FactRecord>(
    facts: &mut IndexMap<String, F>,
    forgotten: &mut Map<String, Value>,
    id: &str,
    evidence: Option<Value>,
) -> Result<bool, ForgetError> {
    if forgotten.contains_key(id) {
        return Ok(false);
    }
    let fact = facts
        .get(id)
        .ok_or_else(|| ForgetError::UnknownId(id.into()))?;
    let mut entry = json!({"claim": fact.claim(), "record": serde_json::to_value(fact).map_err(|e| ForgetError::InvalidRecord(e.to_string()))?});
    if let Some(evidence) = evidence {
        entry["evidence"] = evidence;
    }
    facts.shift_remove(id);
    forgotten.insert(id.into(), entry);
    Ok(true)
}

pub fn unforget<F: FactRecord>(
    facts: &mut IndexMap<String, F>,
    forgotten: &mut Map<String, Value>,
    id: &str,
) -> Result<Value, ForgetError> {
    let Some(entry) = forgotten.remove(id) else {
        return if facts.contains_key(id) {
            Ok(json!(false))
        } else {
            Err(ForgetError::UnknownId(id.into()))
        };
    };
    let record = &entry["record"];
    if !crate::value::truthy(record) {
        return Ok(json!(NO_RECORD));
    }
    facts.insert(
        id.into(),
        F::from_record(record).map_err(ForgetError::InvalidRecord)?,
    );
    Ok(json!(true))
}

pub fn drop_forgotten<F>(facts: &mut IndexMap<String, F>, forgotten: &Map<String, Value>) {
    for id in forgotten.keys() {
        facts.shift_remove(id);
    }
}
