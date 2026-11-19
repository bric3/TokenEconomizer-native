mod detector;
mod digest;
mod prompt;
mod regex;

pub use detector::*;
pub use digest::*;
pub use prompt::*;

use crate::{
    lifecycle::{self, FactRecord, ForgetError},
    names::repo_filename,
    runner::Runner,
    store::write_json_atomic,
    value::{json_text, round_decimal, scalar_text, truthy},
};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

pub const PROPOSE_EVERY: i64 = 8;
pub const WINDOW: usize = 8;
pub const MAX_CANDS: usize = 4;
pub const MIN_CITE: usize = 2;
pub const MAX_EXTRACTORS: usize = 12;
pub const RAW_SAMPLE: usize = 3;
pub const ABSTAIN_GATE: f64 = 0.5;
pub const STATE_SUBDIR: &str = "manual_state";
pub const BLOB_CAP: usize = 20_000;

pub struct ProposeContext<'a> {
    pub runner: Option<&'a mut dyn Runner>,
    pub external: Option<&'a mut dyn FnMut() -> Result<Vec<String>, String>>,
    pub propose_every: i64,
    pub home: &'a str,
}
impl Default for ProposeContext<'_> {
    fn default() -> Self {
        Self {
            runner: None,
            external: None,
            propose_every: PROPOSE_EVERY,
            home: "",
        }
    }
}

pub fn forgotten_evidence(record: &Value) -> Vec<String> {
    let evidence = record
        .get("evidence")
        .filter(|v| !v.is_null())
        .unwrap_or(&record["record"]["spec"]["cite"]);
    evidence
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(String::from)
        .collect()
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Distiller {
    pub repo: String,
    pub generic: bool,
    pub now: i64,
    pub facts: IndexMap<String, Fact>,
    pub window: Vec<Value>,
    pub model: String,
    pub opt_tokens: f64,
    pub last_propose: i64,
    pub extractors: IndexMap<String, Extractor>,
    pub raw_window: Vec<Vec<Command>>,
    pub forgotten: Map<String, Value>,
}
impl Default for Distiller {
    fn default() -> Self {
        Self::new("", "agent", false)
    }
}
impl Distiller {
    pub fn new(repo: &str, model: &str, generic: bool) -> Self {
        Self {
            repo: repo.into(),
            generic,
            now: 0,
            facts: IndexMap::new(),
            window: Vec::new(),
            model: model.into(),
            opt_tokens: 0.0,
            last_propose: -1,
            extractors: IndexMap::new(),
            raw_window: Vec::new(),
            forgotten: Map::new(),
        }
    }
    pub fn featurize(&self, digest: &mut Value, commands: &[Command]) {
        let features: Map<String, Value> = self
            .extractors
            .iter()
            .map(|(name, extractor)| {
                (
                    name.clone(),
                    extractor.compute(commands).unwrap_or(json!(false)),
                )
            })
            .collect();
        digest["features"] = Value::Object(features);
    }
    pub fn sensed(&self, command: &str) -> bool {
        if self.generic {
            CHECK_RE.is_match(command)
        } else {
            PYTEST_RE.is_match(command) || REPRO_RE.is_match(command)
        }
    }
    pub fn coverage(&self, commands: &[Command]) -> Result<f64, String> {
        if commands.is_empty() {
            return Ok(1.0);
        }
        let mut seen = 0;
        for (command, _) in commands {
            let mut hit = self.sensed(command);
            if !hit {
                for extractor in self.extractors.values() {
                    if search(&extractor.rx, command)? {
                        hit = true;
                        break;
                    }
                }
            }
            seen += usize::from(hit);
        }
        Ok(seen as f64 / commands.len() as f64)
    }
    pub fn abstaining_excerpts(&self) -> Result<Vec<String>, String> {
        let mut out = Vec::new();
        for commands in &self.raw_window[self.raw_window.len().saturating_sub(WINDOW)..] {
            for (command, _) in commands {
                if self.sensed(command) {
                    continue;
                }
                let mut covered = false;
                for extractor in self.extractors.values() {
                    if search(&extractor.rx, command)? {
                        covered = true;
                        break;
                    }
                }
                if covered {
                    continue;
                }
                let text: String = command
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .chars()
                    .take(100)
                    .collect();
                if !text.is_empty() && !out.contains(&text) {
                    out.push(text);
                }
            }
        }
        out.truncate(RAW_SAMPLE * 4);
        Ok(out)
    }
    pub fn maybe_propose(&mut self, context: &mut ProposeContext<'_>) -> Result<(), String> {
        if context.runner.is_none() || self.window.is_empty() {
            return Ok(());
        }
        let coverage = self.raw_window[self.raw_window.len().saturating_sub(WINDOW)..]
            .iter()
            .map(|c| self.coverage(c))
            .collect::<Result<Vec<_>, _>>()?;
        let abstaining = !coverage.is_empty()
            && coverage.iter().sum::<f64>() / (coverage.len() as f64) < ABSTAIN_GATE;
        let excerpts = if abstaining {
            self.abstaining_excerpts()?
                .into_iter()
                .take(RAW_SAMPLE)
                .collect()
        } else {
            Vec::new()
        };
        let external = if let Some(external) = context.external.as_mut() {
            external()?
        } else {
            Vec::new()
        };
        let existing = Value::Object(
            self.facts
                .iter()
                .map(|(id, fact)| (id.clone(), fact.claim()))
                .collect(),
        );
        let prompt = build_proposer_prompt(
            &self.repo,
            &self.window,
            &existing,
            &self.extractors.keys().cloned().collect(),
            &excerpts,
            &external,
            self.generic,
        );
        let (raw, effective) = match context.runner.as_mut().unwrap().run(&prompt, &self.model) {
            Ok(reply) => reply,
            Err(_) => {
                self.last_propose = self.now;
                return Ok(());
            }
        };
        if let Some(effective) = effective {
            self.opt_tokens += effective;
        }
        let (extractors, facts) = parse_payload(&raw);
        for extractor in extractors {
            self.admit_extractor(&extractor)?;
        }
        let facts: Vec<_> = facts
            .into_iter()
            .filter(|f| !self.forgotten.contains_key(f["id"].as_str().unwrap_or("")))
            .take(MAX_CANDS)
            .collect();
        for mut spec in facts {
            self.admit(&mut spec)?;
        }
        self.last_propose = self.now;
        Ok(())
    }
    pub fn admit_extractor(&mut self, spec: &Value) -> Result<bool, String> {
        let name = spec["name"].as_str().ok_or("expected extractor name")?;
        if self.extractors.contains_key(name) || self.extractors.len() >= MAX_EXTRACTORS {
            return Ok(false);
        }
        let extractor = Extractor::new(spec)?;
        if extractor.rx.is_none() {
            return Ok(false);
        }
        self.extractors.insert(name.into(), extractor);
        let extractor = &self.extractors[name];
        // Replaying the saved blobs lets same-tick facts use the new extractor immediately.
        for digest in &mut self.window {
            let commands = reparse_cmds(digest);
            let value = extractor.compute(&commands)?;
            if digest.get("features").is_none() {
                digest["features"] = json!({});
            }
            digest["features"][name] = value;
        }
        Ok(true)
    }
    pub fn confirms(&self, spec: &Value) -> Result<BTreeSet<String>, String> {
        let mut out = BTreeSet::new();
        for digest in &self.window {
            if Detector::new(
                spec,
                Some(digest.get("features").cloned().unwrap_or(json!({}))),
            )?
            .verdict(digest)?
            .0
            .as_deref()
                == Some("confirm")
                && let Some(iid) = digest["iid"].as_str()
            {
                out.insert(iid.into());
            }
        }
        Ok(out)
    }
    pub fn evidence(&self, fact: &Fact) -> Result<Vec<String>, String> {
        Ok(self.confirms(&fact.spec["detector"])?.into_iter().collect())
    }
    pub fn restates_forgotten(&self, spec: &Value) -> Result<Option<String>, String> {
        let mine = self.confirms(&spec["detector"])?;
        if mine.len() < MIN_CITE {
            return Ok(None);
        }
        for (id, record) in &self.forgotten {
            let theirs: BTreeSet<_> = forgotten_evidence(record).into_iter().collect();
            if mine.intersection(&theirs).count() >= MIN_CITE && mine.is_subset(&theirs) {
                return Ok(Some(id.clone()));
            }
        }
        Ok(None)
    }
    pub fn ground(&self, spec: &Value) -> Result<usize, String> {
        let mut hits = 0;
        for iid in spec["cite"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            let Some(digest) = self.window.iter().rev().find(|d| d["iid"] == iid) else {
                continue;
            };
            if Detector::new(
                &spec["detector"],
                Some(digest.get("features").cloned().unwrap_or(json!({}))),
            )?
            .verdict(digest)?
            .0
            .as_deref()
                == Some("confirm")
            {
                hits += 1;
            }
        }
        Ok(hits)
    }
    pub fn build_fact(&self, spec: &Value) -> Result<Fact, String> {
        let mut fact = Fact::new(spec)?;
        for (index, digest) in self.window.iter().enumerate() {
            let files = digest["edited_files"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect::<BTreeSet<_>>();
            fact.observe(
                self.now - self.window.len() as i64 + index as i64,
                digest,
                files.into_iter().collect(),
            )?;
        }
        Ok(fact)
    }
    pub fn frozen_view(spec: &Value) -> Value {
        let mut detector = spec["detector"].as_object().cloned().unwrap_or_default();
        detector.remove("contradict");
        let mut detector = Value::Object(detector);
        detector.sort_all_objects();
        json!({"claim":spec["claim"],"injection":spec["injection"],"high_stakes":truthy(&spec["high_stakes"]),"detector":json_text(&detector)})
    }
    pub fn admit(&mut self, spec: &mut Value) -> Result<(bool, String), String> {
        let id = spec["id"].as_str().ok_or("expected fact id")?.to_owned();
        if self.forgotten.contains_key(&id) {
            return Ok((false, "forgotten".into()));
        }
        if !self.facts.contains_key(&id) {
            let hits = self.ground(spec)?;
            if hits < MIN_CITE {
                return Ok((false, format!("grounding-fail({hits}/{MIN_CITE})")));
            }
            if !self.forgotten.is_empty() && self.restates_forgotten(spec)?.is_some() {
                return Ok((false, "restates-forgotten".into()));
            }
            if spec.get("rev").is_none() {
                spec["rev"] = json!(1);
            }
            let fact = self.build_fact(spec)?;
            self.facts.insert(id, fact);
            return Ok((true, "admitted".into()));
        }
        let old = self.facts.get_mut(&id).unwrap();
        if Self::frozen_view(spec) != Self::frozen_view(&old.spec) {
            return Ok((false, "revise-not-contradict-only".into()));
        }
        let contra = &spec["detector"]["contradict"];
        if truthy(contra) && rx(contra)?.is_none() {
            return Ok((false, "revise-bad-regex".into()));
        }
        if *contra == old.spec["detector"]["contradict"] {
            return Ok((false, "revise-noop".into()));
        }
        // A revision keeps all earned lifecycle evidence and changes future retirement only.
        if old.spec.get("detector").is_none() {
            old.spec["detector"] = json!({});
        }
        old.spec["detector"]["contradict"] = contra.clone();
        let rev = old.spec.get("rev").and_then(Value::as_i64).unwrap_or(1) + 1;
        old.spec["rev"] = json!(rev);
        Ok((true, "revised-contradict".into()))
    }
    pub fn render(&self) -> String {
        let lines: Vec<_> = self
            .facts
            .values()
            .filter(|f| f.life.injectable())
            .map(|f| format!("- {}", scalar_text(&f.injection())))
            .collect();
        if lines.is_empty() {
            return String::new();
        }
        format!(
            "# repo operating manual ({}) {}\n{}\n",
            self.repo.rsplit('/').next().unwrap_or(""),
            if self.generic {
                "— learned from earlier sessions in this repository"
            } else {
                "— learned from prior tasks in this repo"
            },
            lines.join("\n")
        )
    }
    pub fn update(
        &mut self,
        path: &Path,
        iid: &str,
        resolved: Value,
        context: &mut ProposeContext<'_>,
    ) -> Result<(), String> {
        let calls = load_calls(path);
        let commands = bash_cmds(&calls);
        let mut digest = build_digest(&self.repo, path, iid, resolved, self.generic, context.home)?;
        self.featurize(&mut digest, &commands);
        self.raw_window.push(commands);
        if self.raw_window.len() > WINDOW {
            self.raw_window.drain(..self.raw_window.len() - WINDOW);
        }
        let files = digest["edited_files"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(String::from)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        for fact in self.facts.values_mut() {
            fact.observe(self.now, &digest, files.clone())?;
        }
        self.window.push(digest);
        if self.window.len() > WINDOW {
            self.window.drain(..self.window.len() - WINDOW);
        }
        self.now += 1;
        if self.now - self.last_propose >= context.propose_every {
            self.maybe_propose(context)?;
        }
        Ok(())
    }
    pub fn forget(&mut self, id: &str) -> Result<bool, ForgetError> {
        if self.forgotten.contains_key(id) {
            return Ok(false);
        }
        let fact = self
            .facts
            .get(id)
            .ok_or_else(|| ForgetError::UnknownId(id.into()))?;
        let evidence = self.evidence(fact).map_err(ForgetError::InvalidRecord)?;
        lifecycle::forget(
            &mut self.facts,
            &mut self.forgotten,
            id,
            Some(json!(evidence)),
        )
    }
    pub fn unforget(&mut self, id: &str) -> Result<Value, ForgetError> {
        lifecycle::unforget(&mut self.facts, &mut self.forgotten, id)
    }
    pub fn path(root: &Path, repo: &str) -> Result<PathBuf, String> {
        let directory = root.join(STATE_SUBDIR);
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        Ok(directory.join(repo_filename(repo) + ".json"))
    }
    pub fn save(&self, root: &Path) -> Result<(), String> {
        let tail = |text: &str, n: usize| {
            text.char_indices()
                .rev()
                .nth(n.saturating_sub(1))
                .map_or(text, |(index, _)| &text[index..])
                .to_owned()
        };
        let window: Vec<_> = self.window[self.window.len().saturating_sub(WINDOW)..]
            .iter()
            .map(|d| {
                let mut d = d.clone();
                d["_cmd_blob"] = json!(tail(d["_cmd_blob"].as_str().unwrap_or(""), BLOB_CAP));
                d["_out_blob"] = json!(tail(d["_out_blob"].as_str().unwrap_or(""), BLOB_CAP * 2));
                d
            })
            .collect();
        let raw: Vec<_> = self.raw_window[self.raw_window.len().saturating_sub(WINDOW)..]
            .iter()
            .map(|c| c.iter().map(|(c, _)| c).collect::<Vec<_>>())
            .collect();
        write_json_atomic(&Self::path(root,&self.repo)?,&json!({"repo":self.repo,"now":self.now,"opt_tokens":self.opt_tokens,"last_propose":self.last_propose,"extractors":self.extractors,"facts":self.facts,"forgotten":self.forgotten,"window":window,"raw_cmds":raw}),false).map_err(|e| e.to_string())
    }
    pub fn load(repo: &str, root: &Path, model: &str, generic: bool) -> Result<Self, String> {
        let path = Self::path(root, repo)?;
        let mut distiller = Self::new(repo, model, generic);
        if !path.exists() {
            return Ok(distiller);
        }
        let data: Value =
            serde_json::from_str(&fs::read_to_string(path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        distiller.now = data["now"].as_i64().unwrap_or(0);
        distiller.opt_tokens = data["opt_tokens"].as_f64().unwrap_or(0.0);
        distiller.last_propose = data["last_propose"].as_i64().unwrap_or(-1);
        for (id, spec) in data["extractors"].as_object().into_iter().flatten() {
            distiller
                .extractors
                .insert(id.clone(), Extractor::new(spec)?);
        }
        for (id, record) in data["facts"].as_object().into_iter().flatten() {
            distiller
                .facts
                .insert(id.clone(), Fact::from_record(record)?);
        }
        distiller.window = data["window"].as_array().cloned().unwrap_or_default();
        if distiller.window.len() > WINDOW {
            distiller.window.drain(..distiller.window.len() - WINDOW);
        }
        distiller.raw_window = data["raw_cmds"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|commands| {
                commands
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(|c| (c.into(), String::new()))
                    .collect()
            })
            .collect();
        distiller.forgotten = data["forgotten"].as_object().cloned().unwrap_or_default();
        lifecycle::drop_forgotten(&mut distiller.facts, &distiller.forgotten);
        Ok(distiller)
    }
    pub fn debug_row(&self) -> Value {
        let mut row:Map<String,Value>=self.facts.iter().map(|(id,f)| (id.clone(),json!({"state":f.life.state,"fire":format!("{}/{}",f.life.fire_num,f.life.fire_den),"rate":round_decimal(f.life.firing_rate(),2),"inject":f.life.injectable(),"claim":scalar_text(&f.claim()).chars().take(60).collect::<String>()}))).collect();
        row.insert(
            "_extractors".into(),
            json!(self.extractors.keys().cloned().collect::<BTreeSet<_>>()),
        );
        for (id, fact) in &self.facts {
            if let Some(record) = row.get_mut(id).filter(|v| v.is_object()) {
                record["rev"] = fact.spec.get("rev").cloned().unwrap_or(json!(1));
            }
        }
        Value::Object(row)
    }
}
