use super::*;
use serde::{Deserialize, Serialize};

// The SDK stores opaque slots; only Rust interprets their contents during recovery.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Slot {
    Config,
    Current,
    Manual(String),
    Count(String),
    Turn(String),
    Open(String),
    Starting(String),
    Pending(String),
    Unsettled(String),
    Records(String),
    Record(String, u64),
    Message(u64),
    Step(String),
    Name(String),
}

pub(super) fn id(value: &Id) -> Value {
    let mut out = json!({});
    if let Some(value) = value {
        out["id"] = value.clone();
    }
    out
}
fn read_id(value: &Value) -> Result<Id> {
    Ok(value
        .as_object()
        .ok_or("invalid saved session id")?
        .get("id")
        .cloned())
}
fn number(value: &Value) -> Result<u64> {
    value
        .as_u64()
        .filter(|v| *v <= 9_007_199_254_740_991)
        .ok_or_else(|| "invalid saved message reference".into())
}

impl State {
    pub(super) fn changed(&mut self, slot: Slot, value: Option<Value>) {
        let key = serde_json::to_string(&slot).expect("slot keys are JSON values");
        self.changes.push(match value {
            Some(value) => json!([key, value]),
            None => json!([key]),
        });
    }

    fn restore(version: u8, saved: Value, bootstrap: &Value) -> Result<Self> {
        let mut state = Self::new(version)?;
        let entries: Vec<(String, Value)> =
            serde_json::from_value(saved).map_err(|e| e.to_string())?;
        let mut seen = HashSet::new();
        let mut slots = Vec::new();
        for (key, value) in entries {
            if !seen.insert(key.clone()) {
                return Err("duplicate saved slot".into());
            }
            let slot: Slot = serde_json::from_str(&key).map_err(|e| e.to_string())?;
            if serde_json::to_string(&slot).map_err(|e| e.to_string())? != key {
                return Err("noncanonical saved slot".into());
            }
            if matches!(
                slot,
                Slot::Starting(_) | Slot::Unsettled(_) | Slot::Records(_) | Slot::Record(_, _)
            ) && value != true
            {
                return Err("invalid saved marker".into());
            }
            slots.push((slot, value));
        }
        let mut messages = HashMap::new();
        for (slot, value) in &slots {
            if let Slot::Message(id) = slot {
                if !value["info"].is_object()
                    || !value["parts"]
                        .as_array()
                        .is_some_and(|parts| parts.iter().all(Value::is_object))
                    || number(&json!(id)).is_err()
                {
                    return Err("invalid saved message".into());
                }
                state.next_message = state.next_message.max(id + 1);
                messages.insert(
                    *id,
                    Arc::new(MessageData {
                        id: *id,
                        value: Mutex::new(value.clone()),
                    }),
                );
            }
        }
        let mut records = Vec::new();
        let mut starting = Vec::new();
        let mut pending = Vec::new();
        for (slot, value) in slots {
            match slot {
                Slot::Config => {
                    if value["version"].as_u64() != Some(version.into()) {
                        return Err("saved SDK version does not match".into());
                    }
                    state.directory = read_id(&value["directory"])?;
                    state.initialised = true;
                }
                Slot::Current => state.current = value,
                Slot::Manual(key) => {
                    state.manuals.insert(key, value);
                }
                Slot::Count(key) => {
                    let n = value
                        .as_f64()
                        .filter(|n| n.is_finite() && *n >= 0.0 && n.fract() == 0.0)
                        .ok_or("invalid saved prompt count")?;
                    state.prompts.insert(key, n);
                }
                Slot::Turn(key) => {
                    state
                        .turns
                        .insert(key, value.as_str().ok_or("invalid saved turn")?.into());
                }
                Slot::Open(key) => {
                    let sid = read_id(&value)?;
                    if super::key(sid.as_ref()) != key {
                        return Err("saved session key does not match".into());
                    }
                    state.open.insert(key, sid);
                }
                Slot::Starting(key) => starting.push(key),
                Slot::Pending(key) => pending.push((key, value)),
                Slot::Unsettled(key) => {
                    state.unsettled.insert(key);
                }
                Slot::Records(key) => {
                    state.records.insert(key, Vec::new());
                }
                Slot::Record(key, id) => records.push((key, id)),
                Slot::Message(_) => (),
                Slot::Step(key) => {
                    state.steps.insert(
                        key,
                        messages
                            .get(&number(&value)?)
                            .ok_or("missing saved step")?
                            .clone(),
                    );
                }
                Slot::Name(key) => {
                    state.names.insert(key, value);
                }
            }
        }
        if !state.initialised {
            if !seen.is_empty() {
                return Err("saved SDK configuration is missing".into());
            }
            state.prepare("init", bootstrap)?;
        }
        for (key, id) in records {
            state
                .records
                .get_mut(&key)
                .ok_or("missing saved session records")?
                .push(messages.get(&id).ok_or("missing saved message")?.clone());
        }
        // Failed hooks are not replayed. Finish their pending in-memory prompt updates.
        for key in starting {
            state.manuals.insert(key.clone(), json!(""));
            state.changed(Slot::Manual(key.clone()), Some(json!("")));
            state.changed(Slot::Starting(key), None);
        }
        for (key, value) in pending {
            let sid = read_id(&value["id"])?;
            if super::key(sid.as_ref()) != key {
                return Err("saved prompt key does not match".into());
            }
            state.prompt(&sid, &value["body"])?;
            state.changed(Slot::Pending(key), None);
        }
        Ok(state)
    }
}

impl Bridge {
    pub fn restore(&self, version: u8, saved: Value, bootstrap: &Value) -> Result<()> {
        let restored = State::restore(version, saved, bootstrap)?;
        let mut state = self.state()?;
        if state.initialised || state.version != version {
            return Err("restore is only valid before initialisation".into());
        }
        *state = restored;
        Ok(())
    }
}
