use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::value::{integer, truthy};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct InjectionConfig {
    pub enabled: bool,
    pub propose_every: i64,
    pub profile: String,
    pub max_facts: i64,
    pub max_tokens: i64,
}

impl Default for InjectionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            propose_every: 5,
            profile: "distiller".into(),
            max_facts: 0,
            max_tokens: 0,
        }
    }
}

impl InjectionConfig {
    pub const PROFILES: [&str; 3] = ["distiller", "envfacts", "envfacts+distiller"];

    pub fn validate(&self) -> Result<(), String> {
        if Self::PROFILES.contains(&self.profile.as_str()) {
            Ok(())
        } else {
            Err(format!(
                "injection profile {:?} is not one of {:?}",
                self.profile,
                Self::PROFILES
            ))
        }
    }

    pub fn paid(&self) -> bool {
        self.enabled && self.profile.contains("distiller")
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ToolingConfig {
    pub evidence: i64,
    pub evidence_reaudit: i64,
    pub judge: bool,
    pub codex_untool: bool,
}

impl ToolingConfig {
    pub fn enabled(&self) -> bool {
        self.evidence != 0 || self.codex_untool
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct SpiralConfig {
    pub guard: bool,
    pub judge: bool,
    pub nudge: bool,
    pub nudge_llm: bool,
    pub aggr: String,
    pub frozen_envelope: Option<f64>,
    pub codex_spiral: bool,
    pub codex_spiral_profile: String,
}

impl Default for SpiralConfig {
    fn default() -> Self {
        Self {
            guard: false,
            judge: false,
            nudge: false,
            nudge_llm: false,
            aggr: "default".into(),
            frozen_envelope: None,
            codex_spiral: false,
            codex_spiral_profile: "post-green".into(),
        }
    }
}

impl SpiralConfig {
    pub const CODEX_SPIRAL_PROFILES: [&str; 2] = ["post-green", "defensive"];

    pub fn validate(&self) -> Result<(), String> {
        if Self::CODEX_SPIRAL_PROFILES.contains(&self.codex_spiral_profile.as_str()) {
            Ok(())
        } else {
            Err(format!(
                "codex_spiral_profile must be one of {:?}, got {:?}",
                Self::CODEX_SPIRAL_PROFILES,
                self.codex_spiral_profile
            ))
        }
    }

    pub fn enabled(&self) -> bool {
        self.guard || self.codex_spiral
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct EconomizerConfig {
    pub injection: InjectionConfig,
    pub tooling: ToolingConfig,
    pub spiral: SpiralConfig,
}

impl EconomizerConfig {
    pub fn needs_optimizer(&self) -> bool {
        self.injection.paid()
            || self.tooling.judge
            || (self.spiral.guard && (self.spiral.judge || self.spiral.nudge_llm))
    }

    pub fn from_args(args: &Value) -> Result<Self, String> {
        let flag = |key: &str| truthy(&args[key]);
        let count = |key: &str| {
            integer(&args[key])
                .map(|n| n as i64)
                .ok_or_else(|| format!("invalid integer for {key}"))
        };
        let injection = InjectionConfig {
            enabled: flag("inject"),
            propose_every: if flag("propose_every") {
                count("propose_every")?
            } else {
                5
            },
            profile: args["inject_profile"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or("distiller")
                .into(),
            ..InjectionConfig::default()
        };
        let tooling = ToolingConfig {
            evidence: count("untool_evidence")?,
            evidence_reaudit: count("untool_evidence_reaudit")?,
            judge: flag("untool_judge"),
            codex_untool: flag("codex_untool"),
        };
        let spiral = SpiralConfig {
            guard: flag("spiral_guard"),
            judge: flag("spiral_judge"),
            nudge: flag("spiral_nudge"),
            nudge_llm: flag("spiral_nudge_llm"),
            aggr: args["spiral_aggr"].as_str().unwrap_or("default").into(),
            frozen_envelope: args["spiral_frozen_envelope"].as_f64(),
            codex_spiral: flag("codex_spiral"),
            codex_spiral_profile: args["codex_spiral_profile"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or("post-green")
                .into(),
        };
        injection.validate()?;
        spiral.validate()?;
        Ok(Self {
            injection,
            tooling,
            spiral,
        })
    }
}
