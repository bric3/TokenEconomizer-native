use super::regex::Pattern as Regex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Value, json};

use super::digest::Command;
use crate::{
    lifecycle::{FactRecord, Lifecycle},
    value::truthy,
};

pub fn rx(pattern: &Value) -> Result<Option<Regex>, String> {
    match pattern.as_str().filter(|p| !p.is_empty()) {
        Some(pattern) => Regex::new(pattern),
        None => Ok(None),
    }
}
pub fn search(pattern: &Option<Regex>, text: &str) -> Result<bool, String> {
    pattern
        .as_ref()
        .map(|r| r.is_match(text).map_err(|e| e.to_string()))
        .unwrap_or(Ok(false))
}

pub fn predicate(name: &str, digest: &Value) -> Option<bool> {
    let fallback =
        |primary: &str, other: &str| truthy(digest.get(primary).unwrap_or(&digest[other]));
    Some(match name {
        "repro_written" => truthy(&digest[name]),
        "importerror" => fallback("importerror", "missing_dependency"),
        "badarg" => fallback("badarg", "bad_usage"),
        "ran_tests" => truthy(&digest["test_invocations"]) || truthy(&digest["_ran_tests"]),
        "ran_checks" => truthy(&digest["check_commands"]) || truthy(&digest["test_invocations"]),
        "command_failed" => truthy(&digest["failed_commands"]),
        "missing_command" => truthy(&digest[name]),
        "missing_dependency" => fallback("missing_dependency", "importerror"),
        "bad_usage" => fallback("bad_usage", "badarg"),
        "edited" => truthy(&digest["edited_files"]),
        _ => return None,
    })
}

pub struct Detector {
    pub raw: Value,
    pub scope: Value,
    pub confirm: Option<Regex>,
    pub contra: Option<Regex>,
    pub fired: Value,
    pub features: Option<Value>,
}

impl Detector {
    pub fn new(spec: &Value, features: Option<Value>) -> Result<Self, String> {
        Ok(Self {
            raw: spec.clone(),
            scope: spec.get("scope").cloned().unwrap_or(json!("")),
            confirm: rx(&spec["confirm"])?,
            contra: rx(&spec["contradict"])?,
            fired: spec
                .get("fired")
                .filter(|v| truthy(v))
                .cloned()
                .unwrap_or(spec.get("scope").cloned().unwrap_or(Value::Null)),
            features,
        })
    }
    pub fn lookup(&self, name: &Value, digest: &Value) -> Result<bool, String> {
        let Some(name) = name.as_str() else {
            return Ok(false);
        };
        if let Some(value) = self.features.as_ref().and_then(|f| f.get(name)) {
            return Ok(truthy(value));
        }
        if let Some(value) = predicate(name, digest) {
            return Ok(value);
        }
        search(
            &rx(&json!(name))?,
            digest["_cmd_blob"].as_str().unwrap_or(""),
        )
    }
    pub fn exercised(&self, digest: &Value) -> Result<bool, String> {
        self.lookup(&self.scope, digest)
    }
    pub fn fired(&self, digest: &Value) -> Result<bool, String> {
        self.lookup(&self.fired, digest)
    }
    pub fn verdict(&self, digest: &Value) -> Result<(Option<String>, bool, bool), String> {
        if !self.exercised(digest)? {
            return Ok((None, false, false));
        }
        let blob = format!(
            "{}\n{}",
            digest["_cmd_blob"].as_str().unwrap_or(""),
            digest["_out_blob"].as_str().unwrap_or("")
        );
        if search(&self.contra, &blob)? {
            return Ok((Some("contradict".into()), true, true));
        }
        let verdict = if search(&self.confirm, &blob)? {
            Some("confirm".into())
        } else {
            None
        };
        Ok((verdict, self.fired(digest)?, true))
    }
    pub fn data(&self) -> Value {
        json!({"scope":self.scope,"confirm":self.confirm.as_ref().map(Regex::as_str),"contra":self.contra.as_ref().map(Regex::as_str),"fired":self.fired,"raw":self.raw,"features":self.features})
    }
}

pub struct Extractor {
    pub name: String,
    pub kind: String,
    pub rx: Option<Regex>,
    pub spec: Value,
}
impl Extractor {
    pub fn data(&self) -> Value {
        json!({"name":self.name,"kind":self.kind,"rx":self.rx.as_ref().map(Regex::as_str),"spec":self.spec})
    }
    pub fn new(spec: &Value) -> Result<Self, String> {
        Ok(Self {
            name: spec["name"]
                .as_str()
                .ok_or("expected extractor name")?
                .into(),
            kind: spec
                .get("kind")
                .and_then(Value::as_str)
                .unwrap_or("cmd_regex")
                .into(),
            rx: rx(&spec["pattern"])?,
            spec: spec.clone(),
        })
    }
    pub fn compute(&self, commands: &[Command]) -> Result<Value, String> {
        if self.rx.is_none() {
            return Ok(if self.kind == "cmd_count" {
                json!(0)
            } else {
                json!(false)
            });
        }
        let mut count = 0;
        for (command, out) in commands {
            let text = if self.kind == "out_regex" {
                out
            } else {
                command
            };
            if search(&self.rx, text)? {
                if self.kind != "cmd_count" {
                    return Ok(json!(true));
                }
                count += 1;
            }
        }
        Ok(if self.kind == "cmd_count" {
            json!(count)
        } else {
            json!(false)
        })
    }
}
impl Serialize for Extractor {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.spec.serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for Extractor {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Self::new(&Value::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Fact {
    pub spec: Value,
    pub life: Lifecycle,
}
impl Fact {
    pub fn data(&self) -> Result<Value, String> {
        Ok(
            json!({"spec":self.spec,"life":self.life,"id":self.id(),"claim":self.claim(),"injection":self.injection(),"high":truthy(&self.spec["high_stakes"]),"detector":Detector::new(&self.spec["detector"],None)?.data()}),
        )
    }
    pub fn new(spec: &Value) -> Result<Self, String> {
        let id = spec["id"].as_str().ok_or("expected fact id")?;
        Detector::new(&spec["detector"], None)?;
        Ok(Self {
            spec: spec.clone(),
            life: Lifecycle::new(id.into(), truthy(&spec["high_stakes"]), true),
        })
    }
    pub fn id(&self) -> &str {
        self.spec["id"].as_str().unwrap_or("")
    }
    pub fn claim(&self) -> Value {
        self.spec.get("claim").unwrap_or(&self.spec["id"]).clone()
    }
    pub fn injection(&self) -> Value {
        self.spec
            .get("injection")
            .cloned()
            .unwrap_or_else(|| self.claim())
    }
    pub fn observe(
        &mut self,
        now: i64,
        digest: &Value,
        files: Vec<String>,
    ) -> Result<(Option<String>, bool, bool), String> {
        let verdict = Detector::new(
            &self.spec["detector"],
            Some(digest.get("features").cloned().unwrap_or(json!({}))),
        )?
        .verdict(digest)?;
        self.life.update(
            now,
            verdict.0.as_deref().unwrap_or(""),
            verdict.1,
            verdict.2,
            files,
        );
        Ok(verdict)
    }
}
impl FactRecord for Fact {
    fn claim(&self) -> Value {
        self.claim()
    }
    fn from_record(record: &Value) -> Result<Self, String> {
        let mut fact = Self::new(&record["spec"])?;
        fact.life = Lifecycle::from_record(&record["life"])?;
        Ok(fact)
    }
}
