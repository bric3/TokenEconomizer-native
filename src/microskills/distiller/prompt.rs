use crate::value::{json_indented, scalar_text, truthy};
use serde_json::{Map, Value};
use std::collections::BTreeSet;

pub const PROPOSER_SYSTEM: &str = include_str!("prompts/paper.txt");
pub const PROPOSER_SYSTEM_GENERIC: &str = include_str!("prompts/generic.txt");

pub fn proposer_view(digest: &Value) -> Value {
    Value::Object(
        digest
            .as_object()
            .into_iter()
            .flatten()
            .filter(|(key, _)| !key.starts_with('_'))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
    )
}
pub fn build_proposer_prompt(
    repo: &str,
    window: &[Value],
    existing: &Value,
    features: &BTreeSet<String>,
    excerpts: &[String],
    external: &[String],
    generic: bool,
) -> String {
    let existing: Map<String, Value> = if let Some(map) = existing.as_object() {
        map.clone()
    } else {
        existing
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|id| (id.into(), Value::String(String::new())))
            .collect()
    };
    let mut ids: Vec<_> = existing.keys().collect();
    ids.sort();
    let known = if generic {
        let facts = ids
            .iter()
            .map(|id| {
                if truthy(&existing[*id]) {
                    format!("  - {id}: {}", scalar_text(&existing[*id]))
                } else {
                    format!("  - {id}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "\n\nExisting facts, as `id: claim` (do NOT restate or paraphrase one under a NEW id; re-emit an id with ONLY detector.contradict changed to revise its contradiction condition, else use a NEW id for a fact not listed here):\n{}",
            if facts.is_empty() { "(none)" } else { &facts }
        )
    } else {
        format!(
            "\n\nExisting fact ids (re-emit an id with ONLY detector.contradict changed to revise its contradiction condition, else use a NEW id):\n{}",
            if ids.is_empty() {
                "(none)".into()
            } else {
                ids.into_iter().cloned().collect::<Vec<_>>().join(", ")
            }
        )
    };
    let feature_names = if features.is_empty() {
        "(none)".into()
    } else {
        features.iter().cloned().collect::<Vec<_>>().join(", ")
    };
    let view = Value::Array(window.iter().map(proposer_view).collect());
    let mut prompt = format!(
        "{}\n\nRepo: {repo}{known}\n\nExisting feature names:\n{feature_names}\n\nTask digests (chronological):\n{}",
        if generic {
            PROPOSER_SYSTEM_GENERIC
        } else {
            PROPOSER_SYSTEM
        },
        json_indented(&view, false, 1)
    );
    if !excerpts.is_empty() {
        prompt.push_str(&format!("\n\nRAW command excerpts from recent tasks the current extractors did NOT cover (author extractors to reach these if they hide a durable fact):\n{}",excerpts.iter().map(|c| format!("  $ {c}")).collect::<Vec<_>>().join("\n")));
    }
    if !external.is_empty() {
        prompt.push_str(&format!("\n\nFacts ALREADY injected before every task by a separate layer (do NOT restate or paraphrase them; author only facts beyond these):\n{}",external.iter().map(|c| format!("  - {c}")).collect::<Vec<_>>().join("\n")));
    }
    prompt.push_str("\n\nReturn the JSON object {extractors, facts}:");
    prompt
}
pub fn detector_fields_ok(detector: &Value) -> bool {
    ["scope", "fired", "confirm", "contradict"]
        .iter()
        .all(|key| detector[*key].is_null() || detector[*key].is_string())
}
pub fn parse_payload(raw: &str) -> (Vec<Value>, Vec<Value>) {
    let (Some(start), Some(end)) = (raw.find('{'), raw.rfind('}')) else {
        return (Vec::new(), Vec::new());
    };
    if start > end {
        return (Vec::new(), Vec::new());
    }
    let Ok(object) = serde_json::from_str::<Value>(&raw[start..=end]) else {
        return (Vec::new(), Vec::new());
    };
    let nonempty_string = |v: &Value| v.as_str().is_some_and(|s| !s.is_empty());
    let extractors = object["extractors"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|e| e.is_object() && nonempty_string(&e["name"]) && nonempty_string(&e["pattern"]))
        .cloned()
        .collect();
    let facts = object["facts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|f| {
            f.is_object()
                && nonempty_string(&f["id"])
                && f["detector"].is_object()
                && truthy(&f["detector"])
                && detector_fields_ok(&f["detector"])
                && (f["cite"].is_null() || f["cite"].is_array())
        })
        .cloned()
        .collect();
    (extractors, facts)
}
