use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use super::{
    distiller::{self, Distiller, ProposeContext},
    envfacts::{self, EnvFactsDistiller},
    observe::Vocabulary,
};
use crate::{
    config::InjectionConfig,
    lifecycle::{self, ForgetError},
    names::repo_filename,
    runner::Runner,
    value::truthy,
};

/// The environment layer renders and learns before the paid manual.
#[derive(Default, Serialize, Deserialize)]
pub struct FactLayers {
    pub env: Option<EnvFactsDistiller>,
    pub dist: Option<Distiller>,
}

impl FactLayers {
    pub fn opt_tokens(&self) -> f64 {
        self.dist.as_ref().map_or_else(
            || self.env.as_ref().map_or(0.0, |e| e.opt_tokens),
            |d| d.opt_tokens,
        )
    }

    pub fn now(&self) -> i64 {
        self.dist
            .as_ref()
            .map_or_else(|| self.env.as_ref().map_or(0, |e| e.now), |d| d.now)
    }

    pub fn render(&mut self) -> String {
        let a = self
            .env
            .as_mut()
            .map_or_else(String::new, EnvFactsDistiller::render);
        let b = self
            .dist
            .as_ref()
            .map_or_else(String::new, Distiller::render);
        format!(
            "{a}{}{b}",
            if !a.is_empty() && !b.is_empty() {
                "\n"
            } else {
                ""
            }
        )
    }

    pub fn injected_ids(&self) -> Vec<String> {
        self.env
            .as_ref()
            .map_or_else(Vec::new, EnvFactsDistiller::injected_ids)
    }

    pub fn seed_injected(&mut self, ids: &[String]) {
        if let Some(env) = self.env.as_mut() {
            env.seed_injected(ids);
        }
    }

    pub fn update(
        &mut self,
        path: &Path,
        iid: &str,
        resolved: Value,
        edited: Option<BTreeSet<String>>,
        context: &mut ProposeContext<'_>,
        environment_claims: bool,
    ) -> Result<(), String> {
        if let Some(env) = self.env.as_mut() {
            env.update(path, iid, resolved.clone(), edited)?;
        }
        if let Some(dist) = self.dist.as_mut() {
            if let Some(env) = self.env.as_ref().filter(|_| environment_claims) {
                // The proposer sees claims earned by this task's environment fold.
                let mut external = || Ok(env.injectable_claims());
                let mut composed = ProposeContext {
                    runner: context.runner.as_mut().map(|r| &mut **r as &mut dyn Runner),
                    external: Some(&mut external),
                    propose_every: context.propose_every,
                    home: context.home,
                };
                dist.update(path, iid, resolved, &mut composed)?;
            } else {
                dist.update(path, iid, resolved, context)?;
            }
        }
        Ok(())
    }

    pub fn save(&self, root: &Path) -> Result<(), String> {
        if let Some(env) = &self.env {
            env.save(root)?;
        }
        if let Some(dist) = &self.dist {
            dist.save(root)?;
        }
        Ok(())
    }

    pub fn debug_row(&self) -> Value {
        let mut row = self
            .dist
            .as_ref()
            .map_or_else(Map::new, |d| d.debug_row().as_object().unwrap().clone());
        if let Some(env) = &self.env {
            row.extend(env.debug_row().as_object().unwrap().clone());
        }
        if self.env.is_some() && self.dist.is_some() {
            row.insert("_profile".into(), json!("envfacts+distiller"));
        }
        Value::Object(row)
    }

    fn known_ids(&self) -> BTreeSet<String> {
        let mut ids = BTreeSet::new();
        if let Some(env) = &self.env {
            ids.extend(lifecycle::known_ids(&env.facts, &env.forgotten));
        }
        if let Some(dist) = &self.dist {
            ids.extend(lifecycle::known_ids(&dist.facts, &dist.forgotten));
        }
        ids
    }

    fn forgotten(&self) -> &Map<String, Value> {
        if let Some(env) = &self.env {
            &env.forgotten
        } else {
            &self.dist.as_ref().unwrap().forgotten
        }
    }

    fn change_forgotten(&mut self, id: &str, undo: bool) -> Result<Value, ForgetError> {
        if let Some(env) = &mut self.env {
            if undo {
                lifecycle::unforget(&mut env.facts, &mut env.forgotten, id)
            } else {
                lifecycle::forget(&mut env.facts, &mut env.forgotten, id, None).map(|b| json!(b))
            }
        } else {
            let dist = self.dist.as_mut().unwrap();
            if undo {
                dist.unforget(id)
            } else {
                dist.forget(id).map(|b| json!(b))
            }
        }
    }

    fn claim(&self, id: &str) -> Value {
        self.env
            .as_ref()
            .and_then(|e| e.facts.get(id))
            .map(|f| json!(f.claim()))
            .or_else(|| {
                self.dist
                    .as_ref()
                    .and_then(|d| d.facts.get(id))
                    .map(|f| f.claim())
            })
            .unwrap_or_else(|| {
                self.forgotten()
                    .get(id)
                    .and_then(|record| record.get("claim"))
                    .cloned()
                    .unwrap_or(json!(""))
            })
    }
}

#[derive(Serialize, Deserialize)]
pub struct PrefixStage {
    pub results_root: PathBuf,
    pub reason: String,
    pub distiller: FactLayers,
}

impl PrefixStage {
    pub const NAME: &str = "prefix";

    pub fn new(
        cfg: &InjectionConfig,
        repo: &str,
        root: &Path,
        fresh: bool,
        model: &str,
        vocab: Vocabulary,
        context: &mut ProposeContext<'_>,
    ) -> Result<Self, String> {
        if cfg.propose_every > 0 {
            context.propose_every = cfg.propose_every;
        }
        let profile = if cfg.profile.is_empty() {
            "distiller"
        } else {
            &cfg.profile
        };
        let env = if profile.contains("envfacts") {
            Some(if fresh {
                EnvFactsDistiller::new(
                    repo,
                    vocab.clone(),
                    Some(cfg.max_facts),
                    Some(cfg.max_tokens),
                )
            } else {
                EnvFactsDistiller::load(
                    repo,
                    root,
                    vocab.clone(),
                    Some(cfg.max_facts),
                    Some(cfg.max_tokens),
                )?
            })
        } else {
            None
        };
        let dist = if profile != "envfacts" {
            Some(if fresh {
                Distiller::new(repo, model, vocab.generic)
            } else {
                Distiller::load(repo, root, model, vocab.generic)?
            })
        } else {
            None
        };
        let reason = match (env.is_some(), dist.is_some()) {
            (true, true) => "envfacts+manual-distill",
            (true, false) => "envfacts",
            _ => "manual-distill",
        }
        .into();
        Ok(Self {
            results_root: root.into(),
            reason,
            distiller: FactLayers { env, dist },
        })
    }

    pub fn render(&mut self) -> String {
        self.distiller.render()
    }

    pub fn injected_ids(&self) -> Vec<String> {
        self.distiller.injected_ids()
    }

    pub fn after_task(
        &mut self,
        task: &Value,
        run_dir: &Path,
        srow: &mut Value,
        context: &mut ProposeContext<'_>,
    ) -> Result<(f64, String), String> {
        if let Some(ids) = task.get("injected_ids").filter(|v| !v.is_null()) {
            let ids: Vec<String> =
                serde_json::from_value(ids.clone()).map_err(|e| e.to_string())?;
            self.distiller.seed_injected(&ids);
        }
        let before = self.distiller.opt_tokens();
        self.distiller.update(
            &run_dir.join("transcript.jsonl"),
            task["instance_id"].as_str().ok_or("expected instance_id")?,
            srow["resolved"].clone(),
            None,
            context,
            true,
        )?;
        self.distiller.save(&self.results_root)?;
        let delta = self.distiller.opt_tokens() - before;
        srow["manual_facts"] = self.distiller.debug_row();
        Ok((delta, self.reason.clone()))
    }
}

pub fn saved_layers(
    repo: &str,
    root: &Path,
    vocab: &Vocabulary,
) -> Result<Vec<FactLayers>, String> {
    let name = repo_filename(repo) + ".json";
    let mut layers = Vec::new();
    if root.join(envfacts::STATE_SUBDIR).join(&name).exists() {
        layers.push(FactLayers {
            env: Some(EnvFactsDistiller::load(
                repo,
                root,
                vocab.clone(),
                None,
                None,
            )?),
            dist: None,
        });
    }
    if root.join(distiller::STATE_SUBDIR).join(name).exists() {
        layers.push(FactLayers {
            env: None,
            dist: Some(Distiller::load(repo, root, "agent", vocab.generic)?),
        });
    }
    Ok(layers)
}

#[derive(Debug)]
pub enum SavedFactError {
    UnknownIds(Vec<String>),
    State(String),
}
impl From<String> for SavedFactError {
    fn from(error: String) -> Self {
        Self::State(error)
    }
}
impl From<ForgetError> for SavedFactError {
    fn from(error: ForgetError) -> Self {
        Self::State(error.to_string())
    }
}

pub fn forget_facts(
    repo: &str,
    root: &Path,
    ids: &[String],
    vocab: &Vocabulary,
    undo: bool,
) -> Result<Vec<(String, Value, Value)>, SavedFactError> {
    let mut layers = saved_layers(repo, root, vocab)?;
    let mut owner = BTreeMap::new();
    for (index, layer) in layers.iter().enumerate() {
        for id in layer.known_ids() {
            owner.entry(id).or_insert(index);
        }
    }
    let unknown: Vec<_> = ids
        .iter()
        .filter(|id| !owner.contains_key(*id))
        .cloned()
        .collect();
    if !unknown.is_empty() {
        return Err(SavedFactError::UnknownIds(unknown));
    }

    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    let mut changed = Vec::new();
    for id in ids {
        if !seen.insert(id) {
            continue;
        }
        let index = owner[id];
        let layer = &mut layers[index];
        let did = layer.change_forgotten(id, undo)?;
        if truthy(&did) && !changed.contains(&index) {
            changed.push(index);
        }
        out.push((id.clone(), did, layer.claim(id)));
    }
    for index in changed {
        layers[index].save(root)?;
    }
    Ok(out)
}

pub fn forgotten_facts(
    repo: &str,
    root: &Path,
    vocab: &Vocabulary,
) -> Result<Map<String, Value>, String> {
    let mut out = Map::new();
    for layer in saved_layers(repo, root, vocab)? {
        for (id, record) in layer.forgotten() {
            out.entry(id.clone())
                .or_insert_with(|| record.get("claim").cloned().unwrap_or(json!("")));
        }
    }
    Ok(out)
}
