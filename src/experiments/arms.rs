use crate::config::{EconomizerConfig, InjectionConfig, SpiralConfig, ToolingConfig};
use indexmap::IndexMap;
use serde::Serialize;
use std::collections::BTreeMap;

pub const ARM_ORDER: [&str; 27] = [
    "baseline",
    "baseline-medium",
    "baseline-low",
    "inject-only",
    "untool-only",
    "spiral-only",
    "spiral-only-web",
    "trimech",
    "trimech-web",
    "codex-untool",
    "codex-spiral",
    "codex-spiral-postgreen",
    "codex-spiral-defensive",
    "codex-combo",
    "codex-inject",
    "codex-inject-untool",
    "codex-inject-envfacts",
    "codex-inject-distiller",
    "codex-trimech",
    "codex-trimech-rtk",
    "codex-trimech-repomap",
    "rtk",
    "serena",
    "repomap",
    "trimech-rtk",
    "trimech-rtk-web",
    "trimech-repomap",
];

#[derive(Serialize)]
pub struct Arm {
    pub mode: &'static str,
    pub config: EconomizerConfig,
    pub env: BTreeMap<&'static str, &'static str>,
    pub scaffold: Option<&'static str>,
}

pub fn get(name: &str) -> Option<Arm> {
    if !ARM_ORDER.contains(&name) {
        return None;
    }
    let injection = match name {
        "inject-only" | "trimech" | "trimech-web" | "trimech-rtk" | "trimech-rtk-web"
        | "trimech-repomap" => InjectionConfig {
            enabled: true,
            propose_every: 5,
            ..InjectionConfig::default()
        },
        "codex-inject-envfacts" => InjectionConfig {
            enabled: true,
            profile: "envfacts".into(),
            ..InjectionConfig::default()
        },
        "codex-inject"
        | "codex-inject-untool"
        | "codex-inject-distiller"
        | "codex-trimech"
        | "codex-trimech-rtk"
        | "codex-trimech-repomap" => InjectionConfig {
            enabled: true,
            profile: "envfacts+distiller".into(),
            propose_every: 10,
            ..InjectionConfig::default()
        },
        _ => InjectionConfig::default(),
    };
    let tooling = match name {
        "untool-only" | "trimech" | "trimech-web" | "trimech-rtk" | "trimech-rtk-web"
        | "trimech-repomap" => ToolingConfig {
            evidence: 3,
            judge: true,
            ..ToolingConfig::default()
        },
        "codex-untool"
        | "codex-combo"
        | "codex-inject-untool"
        | "codex-trimech"
        | "codex-trimech-rtk"
        | "codex-trimech-repomap" => ToolingConfig {
            codex_untool: true,
            ..ToolingConfig::default()
        },
        _ => ToolingConfig::default(),
    };
    let spiral = match name {
        "spiral-only" | "trimech" | "trimech-rtk" | "trimech-repomap" => SpiralConfig {
            guard: true,
            judge: true,
            nudge: true,
            ..SpiralConfig::default()
        },
        "spiral-only-web" | "trimech-web" | "trimech-rtk-web" => SpiralConfig {
            guard: true,
            judge: true,
            nudge: true,
            nudge_llm: true,
            frozen_envelope: Some(30.0),
            ..SpiralConfig::default()
        },
        "codex-spiral-postgreen" => SpiralConfig {
            codex_spiral: true,
            ..SpiralConfig::default()
        },
        "codex-spiral"
        | "codex-spiral-defensive"
        | "codex-combo"
        | "codex-trimech"
        | "codex-trimech-rtk"
        | "codex-trimech-repomap" => SpiralConfig {
            codex_spiral: true,
            codex_spiral_profile: "defensive".into(),
            ..SpiralConfig::default()
        },
        _ => SpiralConfig::default(),
    };
    let scaffold = match name {
        "rtk" | "trimech-rtk" | "trimech-rtk-web" | "codex-trimech-rtk" => Some("rtk"),
        "repomap" | "trimech-repomap" | "codex-trimech-repomap" => Some("repomap"),
        "serena" => Some("serena"),
        _ => None,
    };
    let mut env = BTreeMap::new();
    match name {
        "baseline-low" => {
            env.insert("TE_EFFORT_LEVEL", "low");
        }
        "baseline-medium" => {
            env.insert("TE_EFFORT_LEVEL", "medium");
        }
        _ => {}
    }
    Some(Arm {
        mode: if injection.enabled {
            "manual4"
        } else {
            "standalone"
        },
        config: EconomizerConfig {
            injection,
            tooling,
            spiral,
        },
        env,
        scaffold,
    })
}

pub fn registry() -> IndexMap<&'static str, Arm> {
    ARM_ORDER
        .into_iter()
        .map(|name| (name, get(name).expect("registered arm")))
        .collect()
}
