use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::value::{json_pretty, number, python_repr, scalar_text, truthy};

pub const MICROSKILLS: &[&str] = &["off", "envfacts", "envfacts+distiller", "distiller"];
pub const LOCAL_MODELS: &[&str] = &["off", "fm"];
pub const LOCAL_ROLES: &[&str] = &["judge", "nudge", "proposer"];
pub const TOOLPRUNE: &[&str] = &["off", "propose", "auto"];
pub const LOOPGUARD: &[&str] = &["off", "nudge", "stop"];
pub const REPOMAP: &[&str] = &["off", "on", "start"];
pub const MODES: &[&str] = &["auto", "interactive", "headless"];
pub const AGGR: &[&str] = &["conservative", "default", "aggressive"];
pub const ALIASES: &[(&str, &str)] = &[
    ("inject", "microskills"),
    ("inject_max_facts", "microskills_max_facts"),
    ("inject_max_tokens", "microskills_max_tokens"),
    ("propose_every", "microskills_propose_every"),
    ("untool", "toolprune"),
    ("untool_evidence", "toolprune_evidence"),
    ("spiral", "loopguard"),
    ("spiral_aggr", "loopguard_aggr"),
    ("spiral_nudges", "loopguard_nudges"),
    ("spiral_thresholds", "loopguard_thresholds"),
];
pub const OBSOLETE_PAID: &str = "the loop guard always asks its judge (`loopguard=off` turns it off), and micro-skills' proposer runs whenever micro-skills is on (`microskills=envfacts` keeps only the facts that need no model call)";

pub struct ThresholdSpec {
    pub name: &'static str,
    pub integer: bool,
    pub min: f64,
    pub max: Option<f64>,
    pub description: &'static str,
}
pub const LOOPGUARD_KEYS: &[ThresholdSpec] = &[
    ThresholdSpec {
        name: "min_turns",
        integer: true,
        min: 1.0,
        max: None,
        description: "tool calls in a turn before it can count as stuck",
    },
    ThresholdSpec {
        name: "edit_churn",
        integer: true,
        min: 1.0,
        max: None,
        description: "edits to one file that count as churn",
    },
    ThresholdSpec {
        name: "distinct_edit_files",
        integer: true,
        min: 1.0,
        max: None,
        description: "files edited that count as scattered",
    },
    ThresholdSpec {
        name: "error_outputs",
        integer: true,
        min: 1.0,
        max: None,
        description: "failing tool results that count as sustained errors",
    },
    ThresholdSpec {
        name: "stagnation",
        integer: false,
        min: 0.0,
        max: Some(1.0),
        description: "share of failures that repeat an earlier one",
    },
    ThresholdSpec {
        name: "envelope_pct",
        integer: false,
        min: 1.0,
        max: Some(100.0),
        description: "percentile of this repository's ended-green turns",
    },
    ThresholdSpec {
        name: "envelope_margin",
        integer: false,
        min: 0.1,
        max: None,
        description: "multiplier on that percentile: the envelope",
    },
    ThresholdSpec {
        name: "envelope_min_samples",
        integer: true,
        min: 1.0,
        max: None,
        description: "ended-green turns needed before the envelope is used",
    },
    ThresholdSpec {
        name: "backstop_mult",
        integer: false,
        min: 0.1,
        max: None,
        description: "stuck past this multiple of the envelope: redirect without the judge",
    },
    ThresholdSpec {
        name: "judge_confidence",
        integer: false,
        min: 0.0,
        max: Some(1.0),
        description: "the judge's confidence needed to redirect",
    },
    ThresholdSpec {
        name: "gate_min_turns",
        integer: true,
        min: 1.0,
        max: None,
        description: "tool calls before the judge is consulted",
    },
    ThresholdSpec {
        name: "gate_error_outputs",
        integer: true,
        min: 1.0,
        max: None,
        description: "failing results that wake the judge without a repeat",
    },
];

pub fn threshold_name(key: &str) -> Option<&str> {
    key.strip_prefix("loopguard.")
        .or_else(|| key.strip_prefix("spiral."))
}
pub fn canonical(key: &str) -> String {
    if let Some(name) = threshold_name(key) {
        format!("loopguard.{name}")
    } else {
        ALIASES
            .iter()
            .find(|(old, _)| *old == key)
            .map_or(key, |(_, new)| *new)
            .into()
    }
}
fn choices(key: &str) -> Option<&'static [&'static str]> {
    Some(match key {
        "mode" => MODES,
        "microskills" => MICROSKILLS,
        "toolprune" => TOOLPRUNE,
        "loopguard" => LOOPGUARD,
        "repomap" => REPOMAP,
        "loopguard_aggr" => AGGR,
        "local_model" => LOCAL_MODELS,
        "vocabulary" => &["auto", "generic", "python-git", "node-web"],
        "claude_paid_effort" => &["", "low", "medium", "high", "xhigh", "max"],
        "codex_paid_effort" => &[
            "", "none", "minimal", "low", "medium", "high", "xhigh", "max",
        ],
        _ => return None,
    })
}
fn tuple(values: &[&str]) -> String {
    format!(
        "({})",
        values
            .iter()
            .map(|v| python_repr(&json!(v)))
            .collect::<Vec<_>>()
            .join(", ")
    )
}
fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "NoneType",
        Value::Array(_) => "list",
        Value::Object(_) => "dict",
        Value::Bool(_) => "bool",
        Value::String(_) => "str",
        Value::Number(n) if n.is_f64() => "float",
        _ => "int",
    }
}

fn numeric_text(value: &str) -> Option<String> {
    // shortcut: numeric coercion uses ASCII digits and i64, extend both in the parity audit.
    let chars: Vec<_> = value.trim().chars().collect();
    for (index, character) in chars.iter().enumerate() {
        if *character == '_'
            && (index == 0
                || index + 1 == chars.len()
                || !chars[index - 1].is_ascii_digit()
                || !chars[index + 1].is_ascii_digit())
        {
            return None;
        }
    }
    Some(chars.into_iter().filter(|c| *c != '_').collect())
}

/// Floating input includes non-finite numbers which JSON cannot represent.
pub enum SettingValue {
    Json(Value),
    Float(f64),
}
impl From<Value> for SettingValue {
    fn from(value: Value) -> Self {
        Self::Json(value)
    }
}
impl From<f64> for SettingValue {
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}
impl SettingValue {
    fn json(&self) -> Option<&Value> {
        match self {
            Self::Json(v) => Some(v),
            _ => None,
        }
    }
    fn is_bool(&self) -> bool {
        self.json().is_some_and(Value::is_boolean)
    }
    fn text(&self) -> String {
        match self {
            Self::Json(value) => scalar_text(value),
            Self::Float(n) if n.is_nan() => "nan".into(),
            Self::Float(n) if n.is_infinite() => {
                if n.is_sign_negative() { "-inf" } else { "inf" }.into()
            }
            Self::Float(n) => format!("{n:?}"),
        }
    }
    fn repr(&self) -> String {
        match self {
            Self::Json(v) => python_repr(v),
            _ => self.text(),
        }
    }
    fn float(&self) -> Result<f64, String> {
        match self {
            Self::Float(n) => Ok(*n),
            Self::Json(Value::String(s)) => numeric_text(s)
                .and_then(|s| s.parse::<f64>().ok())
                .ok_or_else(|| {
                    format!(
                        "could not convert string to float: {}",
                        python_repr(&json!(s))
                    )
                }),
            Self::Json(v) => number(v).ok_or_else(|| {
                format!(
                    "float() argument must be a string or a real number, not '{}'",
                    type_name(v)
                )
            }),
        }
    }
    fn is_float(&self) -> bool {
        matches!(self, Self::Float(_))
            || self
                .json()
                .is_some_and(|v| v.as_number().is_some_and(|n| n.is_f64()))
    }
    fn integer(&self) -> Result<i64, String> {
        if let Some(Value::String(s)) = self.json() {
            numeric_text(s)
                .and_then(|s| s.parse::<i64>().ok())
                .ok_or_else(|| {
                    format!(
                        "invalid literal for int() with base 10: {}",
                        python_repr(&json!(s))
                    )
                })
        } else {
            self.float().map(|n| n as i64)
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LiveConfig {
    #[serde(flatten)]
    pub values: Map<String, Value>,
    #[serde(default)]
    pub warnings: Vec<String>,
}
impl Default for LiveConfig {
    fn default() -> Self {
        let mut values = json!({"enabled":true,"mode":"auto","microskills":"off","microskills_max_facts":4,"microskills_max_tokens":150,"microskills_propose_every":3,
            "toolprune":"auto","toolprune_evidence":10,"toolprune_min_sessions":2,"toolprune_sessions":5,"toolprune_quiet_minutes":60,"toolprune_protect":[],
            "loopguard":"nudge","loopguard_aggr":"default","loopguard_nudges":1,"loopguard_thresholds":{},"repomap":"off","repomap_tokens":2048,"repomap_python":"","repomap_timeout":8}).as_object().unwrap().clone();
        values.extend(json!({
            "paid_model":"","claude_paid_model":"","codex_paid_model":"","copilot_paid_model":"","pi_paid_model":"","opencode_paid_model":"","cursor_paid_model":"","grok_paid_model":"","hermes_paid_model":"","droid_paid_model":"",
            "claude_paid_effort":"","codex_paid_effort":"","local_model":"off","local_roles":["judge"],"local_judge_confidence":0.9,"pi_extensions":[],"pi_cli":"","hermes_cli":"","droid_sidecall":false,
            "history_import":false,"history_max_sessions":200,"history_days":365,"history_dirs":[],"shadow":false,"vocabulary":"auto"}).as_object().unwrap().clone());
        Self {
            values,
            warnings: Vec::new(),
        }
    }
}

impl LiveConfig {
    pub fn as_dict(&self) -> Value {
        Value::Object(self.values.clone())
    }

    pub fn set(&mut self, key: &str, value: impl Into<SettingValue>, source: &str) -> bool {
        let mut value = value.into();
        if let Some(name) = threshold_name(key) {
            return self.set_threshold(name, value, source);
        }
        let key = canonical(key);
        if key == "paid" {
            return true;
        }
        if key == "warnings" || !Self::default().values.contains_key(&key) {
            self.warnings.push(format!(
                "{source}: unknown key {}",
                python_repr(&json!(key))
            ));
            return false;
        }
        if key == "loopguard_thresholds" {
            let Some(values) = value.json().and_then(Value::as_object) else {
                self.warnings
                    .push(format!("{source}: loopguard_thresholds must be an object"));
                return false;
            };
            let mut valid = true;
            for (name, value) in values {
                valid &= self.set_threshold(name, value.clone(), source);
            }
            return valid;
        }
        let on = match key.as_str() {
            "microskills" => Some("envfacts+distiller"),
            "toolprune" => Some("auto"),
            "loopguard" => Some("nudge"),
            "repomap" => Some("on"),
            _ => None,
        };
        if let Some(on) = on
            && value.json().is_some_and(|v| {
                v.is_string() || v.is_boolean() || v.as_number().is_some_and(|n| !n.is_f64())
            })
        {
            let word = value.text().trim().to_lowercase();
            if ["on", "true", "yes", "1", "enable", "enabled"].contains(&word.as_str()) {
                value = SettingValue::Json(json!(on));
            } else if ["off", "false", "no", "0", "disable", "disabled"].contains(&word.as_str()) {
                value = SettingValue::Json(json!("off"));
            } else if key == "loopguard" && word == "guard" {
                value = SettingValue::Json(json!("stop"));
            }
        }
        let current = self.values.get(&key).unwrap_or(&Value::Null);
        let coerce = || -> Result<Value, String> {
            if current.is_array() {
                let items: Vec<String> =
                    if let Some(values) = value.json().and_then(Value::as_array) {
                        values.iter().map(scalar_text).collect()
                    } else {
                        value.text().split(',').map(String::from).collect()
                    };
                return Ok(json!(
                    items
                        .iter()
                        .map(|s| s.trim())
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                ));
            }
            if current.as_number().is_some_and(|n| n.is_f64()) {
                if value.is_bool() {
                    return Err("not a number".into());
                }
                let n = value.float()?;
                if !(0.0..=1.0).contains(&n) {
                    return Err("must be between 0 and 1".into());
                }
                return Ok(json!(n));
            }
            if current.is_boolean() {
                return Ok(json!(if value.is_bool() {
                    value.json().unwrap().as_bool().unwrap()
                } else {
                    ["1", "true", "yes", "on"]
                        .contains(&value.text().trim().to_lowercase().as_str())
                }));
            }
            if current.is_number() {
                if value.is_bool() {
                    return Err("not a number".into());
                }
                if value.is_float()
                    && (!value.float()?.is_finite() || value.float()?.fract() != 0.0)
                {
                    return Err("must be a whole number".into());
                }
                let n = value.integer()?;
                let min = if key == "toolprune_quiet_minutes" {
                    0
                } else {
                    1
                };
                if n < min {
                    return Err(format!("must be >= {min}"));
                }
                return Ok(json!(n));
            }
            Ok(json!(value.text().trim()))
        };
        let converted = match coerce() {
            Ok(value) => value,
            Err(error) => {
                self.warnings.push(format!(
                    "{source}: {key}={} rejected ({error})",
                    value.repr()
                ));
                return false;
            }
        };
        if key == "local_roles" {
            let bad: Vec<_> = converted
                .as_array()
                .unwrap()
                .iter()
                .filter(|v| !v.as_str().is_some_and(|s| LOCAL_ROLES.contains(&s)))
                .cloned()
                .collect();
            if !bad.is_empty() {
                self.warnings.push(format!(
                    "{source}: local_roles {} not in {}",
                    python_repr(&json!(bad)),
                    tuple(LOCAL_ROLES)
                ));
                return false;
            }
        }
        if let Some(choices) = choices(&key)
            && !converted.as_str().is_some_and(|s| choices.contains(&s))
        {
            self.warnings.push(format!(
                "{source}: {key}={} not one of {}",
                python_repr(&converted),
                tuple(choices)
            ));
            return false;
        }
        self.values.insert(key, converted);
        true
    }

    pub fn set_threshold(
        &mut self,
        name: &str,
        value: impl Into<SettingValue>,
        source: &str,
    ) -> bool {
        let value = value.into();
        let Some(spec) = LOOPGUARD_KEYS.iter().find(|s| s.name == name) else {
            self.warnings.push(format!(
                "{source}: unknown loopguard threshold {} (one of {})",
                python_repr(&json!(name)),
                LOOPGUARD_KEYS
                    .iter()
                    .map(|s| s.name)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            return false;
        };
        let coerce = || -> Result<f64, String> {
            if value.is_bool() {
                return Err("not a number".into());
            }
            let n = value.float()?;
            if !n.is_finite() {
                return Err("must be finite".into());
            }
            if spec.integer && n.fract() != 0.0 {
                return Err("must be a whole number".into());
            }
            Ok(n)
        };
        let n = match coerce() {
            Ok(n) => n,
            Err(error) => {
                self.warnings.push(format!(
                    "{source}: loopguard.{name}={} rejected ({error})",
                    value.repr()
                ));
                return false;
            }
        };
        let converted = if spec.integer {
            json!(n as i64)
        } else {
            json!(n)
        };
        if n < spec.min || spec.max.is_some_and(|max| n > max) {
            let range = spec.max.map_or_else(
                || {
                    format!(
                        ">= {}",
                        if spec.integer {
                            (spec.min as i64).to_string()
                        } else {
                            format!("{:?}", spec.min)
                        }
                    )
                },
                |max| format!("between {:?} and {max:?}", spec.min),
            );
            self.warnings.push(format!(
                "{source}: loopguard.{name}={} rejected (must be {range})",
                python_repr(&converted)
            ));
            return false;
        }
        self.values["loopguard_thresholds"]
            .as_object_mut()
            .unwrap()
            .insert(name.into(), converted);
        true
    }
}

pub fn environment() -> Vec<(String, String)> {
    let mut vars: Vec<_> = [
        ("TE_MODE", "mode"),
        ("TE_SHADOW", "shadow"),
        ("TE_PAID_MODEL", "paid_model"),
        ("TE_VOCABULARY", "vocabulary"),
    ]
    .into_iter()
    .map(|(a, b)| (a.into(), b.into()))
    .collect();
    for agent in [
        "claude", "codex", "copilot", "pi", "opencode", "cursor", "grok", "hermes", "droid",
    ] {
        vars.push((
            format!("TE_{}_PAID_MODEL", agent.to_uppercase()),
            format!("{agent}_paid_model"),
        ));
    }
    vars.extend(
        [
            ("TE_CLAUDE_PAID_EFFORT", "claude_paid_effort"),
            ("TE_CODEX_PAID_EFFORT", "codex_paid_effort"),
            ("TE_LOCAL_MODEL", "local_model"),
            ("TE_LOCAL_ROLES", "local_roles"),
            ("TE_LOCAL_JUDGE_CONFIDENCE", "local_judge_confidence"),
            ("TE_INJECT", "microskills"),
            ("TE_INJECT_MAX_FACTS", "microskills_max_facts"),
            ("TE_INJECT_MAX_TOKENS", "microskills_max_tokens"),
            ("TE_PROPOSE_EVERY", "microskills_propose_every"),
            ("TE_UNTOOL", "toolprune"),
            ("TE_UNTOOL_EVIDENCE", "toolprune_evidence"),
            ("TE_SPIRAL", "loopguard"),
            ("TE_SPIRAL_AGGR", "loopguard_aggr"),
            ("TE_SPIRAL_NUDGES", "loopguard_nudges"),
            ("TE_MICROSKILLS", "microskills"),
            ("TE_MICROSKILLS_MAX_FACTS", "microskills_max_facts"),
            ("TE_MICROSKILLS_MAX_TOKENS", "microskills_max_tokens"),
            ("TE_MICROSKILLS_PROPOSE_EVERY", "microskills_propose_every"),
            ("TE_TOOLPRUNE", "toolprune"),
            ("TE_TOOLPRUNE_EVIDENCE", "toolprune_evidence"),
            ("TE_TOOLPRUNE_QUIET_MINUTES", "toolprune_quiet_minutes"),
            ("TE_TOOLPRUNE_MIN_SESSIONS", "toolprune_min_sessions"),
            ("TE_TOOLPRUNE_SESSIONS", "toolprune_sessions"),
            ("TE_TOOLPRUNE_PROTECT", "toolprune_protect"),
            ("TE_LOOPGUARD", "loopguard"),
            ("TE_LOOPGUARD_AGGR", "loopguard_aggr"),
            ("TE_LOOPGUARD_NUDGES", "loopguard_nudges"),
            ("TE_REPOMAP", "repomap"),
            ("TE_REPOMAP_TOKENS", "repomap_tokens"),
            ("TE_REPOMAP_PYTHON", "repomap_python"),
            ("TE_REPOMAP_TIMEOUT", "repomap_timeout"),
            ("TE_PI_EXTENSIONS", "pi_extensions"),
            ("TE_PI_CLI", "pi_cli"),
            ("TE_HERMES_CLI", "hermes_cli"),
            ("TE_HISTORY_IMPORT", "history_import"),
            ("TE_HISTORY_MAX_SESSIONS", "history_max_sessions"),
            ("TE_HISTORY_DAYS", "history_days"),
            ("TE_HISTORY_DIRS", "history_dirs"),
        ]
        .into_iter()
        .map(|(a, b)| (a.into(), b.into())),
    );
    vars
}

pub fn read(path: &Path) -> Map<String, Value> {
    fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default()
}
pub fn old_names_first(items: &Map<String, Value>) -> Vec<(String, Value)> {
    let mut items: Vec<_> = items.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    items.sort_by_key(|(k, _)| canonical(k) == *k);
    items
}

pub fn load_config(root: &Path, repo: Option<&Path>, env: &Map<String, Value>) -> LiveConfig {
    let mut cfg = LiveConfig::default();
    for (label, path) in [
        ("global", Some(root.join("config.json"))),
        ("repo", repo.map(|p| p.join("config.json"))),
    ] {
        if let Some(path) = path {
            for (key, value) in old_names_first(&read(&path)) {
                cfg.set(&key, value, &format!("{label} {}", path.display()));
            }
        }
    }
    for (var, key) in environment() {
        if let Some(value) = env.get(&var).filter(|v| !v.is_null() && **v != json!("")) {
            cfg.set(&key, value.clone(), &var);
        }
    }
    for var in ["TE_SPIRAL_THRESHOLDS", "TE_LOOPGUARD_THRESHOLDS"] {
        if let Some(value) = env.get(var).filter(|v| truthy(v)) {
            match value
                .as_str()
                .and_then(|s| serde_json::from_str::<Value>(s).ok())
            {
                Some(value) => {
                    cfg.set("loopguard_thresholds", value, var);
                }
                None => cfg.warnings.push(format!("{var}: not valid JSON")),
            }
        }
    }
    if env.get("TE_DISABLE").is_some_and(|v| {
        ["1", "true", "yes", "on"].contains(&scalar_text(v).trim().to_lowercase().as_str())
    }) {
        cfg.values.insert("enabled".into(), json!(false));
    }
    if cfg.values["loopguard"] == "stop" {
        cfg.warnings.push("loopguard=stop acts as nudge: the plugin no longer stops a turn; a turn still stuck after the redirect gets a notice saying how to stop it (set loopguard=nudge to silence this)".into());
    }
    cfg
}

pub fn drop_old_names(current: &mut Map<String, Value>, key: &str) {
    for (old, new) in ALIASES {
        if *new == key {
            current.remove(*old);
        }
    }
}
fn thresholds(current: &mut Map<String, Value>) -> Result<&mut Map<String, Value>, String> {
    current
        .entry("loopguard_thresholds")
        .or_insert(json!({}))
        .as_object_mut()
        .ok_or_else(|| "loopguard_thresholds must be an object".into())
}
pub fn write_config(
    path: &Path,
    updates: &Map<String, Value>,
) -> Result<Map<String, Value>, String> {
    let gone: Vec<_> = updates
        .keys()
        .filter(|k| canonical(k) == "paid")
        .map(|k| format!("`{k}` no longer exists: {OBSOLETE_PAID}"))
        .collect();
    if !gone.is_empty() {
        return Err(gone.join("; "));
    }
    let mut probe = LiveConfig::default();
    let mut valid = true;
    for (key, value) in updates {
        valid &= probe.set(key, value.clone(), "te config");
    }
    if !valid {
        return Err(probe.warnings.join("; "));
    }
    let mut current = read(path);
    if let Some(Value::Object(mut old)) = current.remove("spiral_thresholds") {
        if let Some(new) = current.get("loopguard_thresholds").filter(|v| truthy(v)) {
            old.extend(
                new.as_object()
                    .ok_or("loopguard_thresholds must be an object")?
                    .clone(),
            );
        }
        current.insert("loopguard_thresholds".into(), json!(old));
    }
    for key in updates.keys() {
        let canonical = canonical(key);
        if let Some(name) = threshold_name(key) {
            thresholds(&mut current)?.insert(
                name.into(),
                probe.values["loopguard_thresholds"][name].clone(),
            );
        } else if canonical == "loopguard_thresholds" {
            thresholds(&mut current)?.extend(
                probe.values["loopguard_thresholds"]
                    .as_object()
                    .unwrap()
                    .clone(),
            );
        } else {
            drop_old_names(&mut current, &canonical);
            current.insert(canonical.clone(), probe.values[&canonical].clone());
        }
    }
    write(path, &json!(current))?;
    Ok(current)
}

pub fn unset_config(path: &Path, keys: &[String]) -> Result<Map<String, Value>, String> {
    let defaults = LiveConfig::default();
    let bad: Vec<_> = keys
        .iter()
        .filter(|key| {
            !defaults.values.contains_key(&canonical(key))
                && !threshold_name(key)
                    .is_some_and(|name| LOOPGUARD_KEYS.iter().any(|s| s.name == name))
        })
        .cloned()
        .collect();
    if !bad.is_empty() {
        return Err(format!("unknown key(s): {}", bad.join(", ")));
    }
    let mut current = read(path);
    for key in keys {
        if let Some(name) = threshold_name(key) {
            for table in ["loopguard_thresholds", "spiral_thresholds"] {
                if let Some(value) = current.get_mut(table).filter(|v| truthy(v)) {
                    value
                        .as_object_mut()
                        .ok_or("thresholds must be an object")?
                        .remove(name);
                }
                if !current.get(table).is_some_and(truthy) {
                    current.remove(table);
                }
            }
        } else {
            let key = canonical(key);
            current.remove(&key);
            drop_old_names(&mut current, &key);
        }
    }
    write(path, &json!(current))?;
    Ok(current)
}

pub fn write(path: &Path, obj: &Value) -> Result<(), String> {
    fs::create_dir_all(path.parent().ok_or("missing config directory")?)
        .map_err(|e| e.to_string())?;
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    fs::write(&temporary, json_pretty(obj, true))
        .and_then(|_| fs::rename(temporary, path))
        .map_err(|e| e.to_string())
}
